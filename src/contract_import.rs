//! Architecture-neutral import of *candidate* BIOS API metadata.
//! Imported source assertions are never evidence of verified ABI compatibility.
use crate::contracts;
use serde_json::{json, Value};
use std::collections::HashSet;

/// A caller-curated **public** vocabulary, independent from the untrusted
/// SourceAdapter. Every exported source-derived string must be explicitly
/// registered here. A catalog is an authorization decision, not a proof of ABI.
#[derive(Debug, Clone)]
pub struct PublicationPolicy {
    pub api_ids: HashSet<String>,
    pub api_names: HashSet<String>,
    pub entry_points: HashSet<String>,
    pub register_names: HashSet<String>,
    pub register_meanings: HashSet<String>,
    pub side_effects: HashSet<String>,
    pub unknown_source_fields: HashSet<String>,
}

#[derive(Debug, Clone)]
pub struct SourcePin {
    /// Opaque, public source identifier (not a filesystem path).
    pub source_id: String,
    /// Exact 40-character commit identifier, asserted by the caller.
    pub revision: String,
    /// SHA-256 of the pinned public input, asserted by the caller.
    pub sha256: String,
    /// An externally reviewed rights decision; not authenticated by this crate.
    pub rights_approved: bool,
    /// Only fields matching this independent public policy may be serialized.
    pub publication: PublicationPolicy,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SourceClaim {
    Unverified,
    Blocked,
    ClaimedVerified,
}

impl SourceClaim {
    fn as_str(self) -> &'static str {
        match self {
            Self::Unverified => "UNVERIFIED",
            Self::Blocked => "BLOCKED",
            Self::ClaimedVerified => "CLAIMED_VERIFIED",
        }
    }
}

/// An adapter translates a particular source dialect to this candidate-only IR.
/// It must not infer values for missing fields or treat source assertions as proof.
#[derive(Debug, Clone)]
pub struct Candidate {
    pub id: String,
    pub name: String,
    pub entry_point: String,
    pub inputs: Vec<Value>,
    pub outputs: Vec<Value>,
    pub side_effects: Vec<String>,
    pub source_claim: SourceClaim,
    pub source_execution_permitted: bool,
    pub source_release_eligible: bool,
    /// Unsupported source fields must be explicitly accounted for.
    pub unknown_fields: Vec<String>,
}

/// Emit candidates one by one, stopping as soon as the sink returns an error.
/// Adapters must not build an unbounded internal buffer before invoking the sink.
pub trait SourceAdapter {
    fn visit(
        &self,
        source: &Value,
        max_candidates: usize,
        emit: &mut dyn FnMut(Candidate) -> Result<(), String>,
    ) -> Result<(), String>;
}
pub const MAX_CANDIDATES: usize = 4096;
const MAX_REGISTERS: usize = 64;
const MAX_FIELDS: usize = 64;
const MAX_ENTRY_BYTES: usize = 256;

#[derive(Debug)]
pub struct ImportedContract {
    pub document: Value,
    pub imported_candidates: usize,
    /// Every output contract is unverified, regardless of upstream assertions.
    pub verified_abis: usize,
}

fn is_token(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 128
        && value
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'_' | b'-' | b'.' | b':'))
}
/// Structured public identifiers cannot contain paths, media file extensions
/// or hash-looking SHA-1/SHA-256 values. Membership in the publication
/// allowlist is additionally mandatory.
fn public_identifier(s: &str) -> bool {
    is_token(s)
        && s.len() <= 64
        && !matches!(s.len(), 40 | 64) // conservative: don't publish digest IDs
        && ![".cue", ".bin", ".rom", ".iso", ".chd", ".pce"]
            .iter()
            .any(|suffix| s.to_ascii_lowercase().ends_with(suffix))
}
fn public_entry_point(s: &str) -> bool {
    if let Some(symbol) = s.strip_prefix("symbol:") {
        return public_identifier(symbol);
    }
    s.strip_prefix("0x").is_some_and(|digits| {
        !digits.is_empty() && digits.len() <= 16 && digits.bytes().all(|b| b.is_ascii_hexdigit())
    })
}
fn public_description(s: &str) -> bool {
    !s.trim().is_empty()
        && s.len() <= 128
        && s.bytes().all(|b| {
            b.is_ascii_alphanumeric() || matches!(b, b' ' | b'_' | b'-' | b',' | b'(' | b')')
        })
}
fn approved_registers(registers: &[Value], publication: &PublicationPolicy) -> bool {
    registers.iter().all(|entry| {
        let Some(obj) = entry.as_object() else {
            return false;
        };
        if !(2..=3).contains(&obj.len())
            || obj
                .keys()
                .any(|key| !matches!(key.as_str(), "name" | "meaning" | "width_bits"))
        {
            return false;
        }
        let Some(name) = obj.get("name").and_then(Value::as_str) else {
            return false;
        };
        let Some(meaning) = obj.get("meaning").and_then(Value::as_str) else {
            return false;
        };
        if !public_identifier(name)
            || !public_description(meaning)
            || !publication.register_names.contains(name)
            || !publication.register_meanings.contains(meaning)
        {
            return false;
        }
        obj.get("width_bits").is_none_or(|width| {
            width
                .as_u64()
                .is_some_and(|bits| (1..=1024).contains(&bits))
        })
    })
}
fn approved_candidate(candidate: &Candidate, publication: &PublicationPolicy) -> bool {
    public_identifier(&candidate.id)
        && public_identifier(&candidate.name)
        && public_entry_point(&candidate.entry_point)
        && publication.api_ids.contains(&candidate.id)
        && publication.api_names.contains(&candidate.name)
        && publication.entry_points.contains(&candidate.entry_point)
        && approved_registers(&candidate.inputs, publication)
        && approved_registers(&candidate.outputs, publication)
        && candidate.side_effects.iter().all(|side_effect| {
            public_description(side_effect) && publication.side_effects.contains(side_effect)
        })
        && candidate
            .unknown_fields
            .iter()
            .all(|name| public_identifier(name) && publication.unknown_source_fields.contains(name))
}
fn is_lower_hex(value: &str, len: usize) -> bool {
    value.len() == len
        && value
            .bytes()
            .all(|b| b.is_ascii_hexdigit() && !b.is_ascii_uppercase())
}

/// Fail closed on invalid metadata; do not echo untrusted source data in errors.
/// The pin is a *declaration*: external tooling must independently authenticate
/// its revision, file hash, and license review before calling this function.
pub fn import<A: SourceAdapter>(
    adapter: &A,
    source: &Value,
    pin: &SourcePin,
    platform: &str,
    architecture: &str,
) -> Result<ImportedContract, String> {
    if !is_token(&pin.source_id)
        || !is_lower_hex(&pin.revision, 40)
        || !is_lower_hex(&pin.sha256, 64)
        || !pin.rights_approved
    {
        return Err("invalid or unapproved source provenance".into());
    }
    if !is_token(platform) || !is_token(architecture) {
        return Err("invalid platform or architecture".into());
    }
    // Backpressure reaches the adapter during extraction, not after a Vec
    // has already been created. This is a cap on output, not a source-file cap.
    let mut api_ids = HashSet::new();
    let mut apis = Vec::new();
    let mut validate_and_store = |candidate: Candidate| -> Result<(), String> {
        if apis.len() >= MAX_CANDIDATES {
            return Err("candidate limit exceeded".into());
        }
        if candidate.entry_point.len() > MAX_ENTRY_BYTES
            || candidate.inputs.len() > MAX_REGISTERS
            || candidate.outputs.len() > MAX_REGISTERS
            || candidate.side_effects.len() > MAX_FIELDS
            || candidate.unknown_fields.len() > MAX_FIELDS
            || candidate
                .side_effects
                .iter()
                .any(|v| v.len() > MAX_ENTRY_BYTES)
        {
            return Err("candidate resource limit exceeded".into());
        }
        if !approved_candidate(&candidate, &pin.publication)
            || candidate.source_execution_permitted
            || candidate.source_release_eligible
            || !api_ids.insert(candidate.id.clone())
        {
            return Err("invalid, duplicate, or unsafe candidate".into());
        }
        let mut fields = HashSet::new();
        if candidate
            .unknown_fields
            .iter()
            .any(|field| !is_token(field) || !fields.insert(field.clone()))
        {
            return Err("invalid unknown-field accounting".into());
        }

        let record = json!({
            "id": candidate.id,
            "name": candidate.name,
            "entry_point": candidate.entry_point,
            "inputs": candidate.inputs,
            "outputs": candidate.outputs,
            "side_effects": candidate.side_effects,
            "platform_extension": {
                "biosdevkit_import": {
                    "source_id": pin.source_id,
                    "source_revision": pin.revision,
                    "source_sha256": pin.sha256,
                    "source_claim": candidate.source_claim.as_str(),
                    "verification": "UNVERIFIED",
                    "execution_permitted": false,
                    "release_eligible": false,
                    "unknown_source_fields": candidate.unknown_fields
                }
            }
        });
        apis.push(record);
        Ok(())
    };
    // Never accept a partial document if an adapter ignores a rejected
    // candidate, including an unauthorized free-form string.
    let mut sink_rejected = false;
    let mut emit = |candidate: Candidate| -> Result<(), String> {
        if sink_rejected {
            return Err("candidate sink already rejected input".into());
        }
        let result = validate_and_store(candidate);
        if result.is_err() {
            sink_rejected = true;
        }
        result
    };
    let result = adapter.visit(source, MAX_CANDIDATES, &mut emit);
    if sink_rejected || result.is_err() {
        return Err("source adapter rejected input".into());
    }
    if apis.is_empty() {
        return Err("no candidate APIs".into());
    }
    let document = json!({
        "schema_version": 1,
        "platform": platform,
        "architecture": architecture,
        "apis": apis
    });
    contracts::validate(&document).map_err(|_| "imported API contract failed validation")?;
    Ok(ImportedContract {
        imported_candidates: document["apis"].as_array().map_or(0, Vec::len),
        document,
        verified_abis: 0,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pin() -> SourcePin {
        SourcePin {
            source_id: "synthetic-source".into(),
            revision: "a".repeat(40),
            sha256: "b".repeat(64),
            rights_approved: true,
            publication: PublicationPolicy {
                api_ids: (0..=MAX_CANDIDATES)
                    .map(|index| format!("api{index}"))
                    .collect(),
                api_names: ["TEST", "TEST_8BIT", "TEST_MULTICPU"]
                    .into_iter()
                    .map(str::to_string)
                    .collect(),
                entry_points: ["0x00", "0xE000", "symbol:test", "symbol:synthetic"]
                    .into_iter()
                    .map(str::to_string)
                    .collect(),
                register_names: ["A", "r0"].into_iter().map(str::to_string).collect(),
                register_meanings: ["candidate output", "candidate argument", "test"]
                    .into_iter()
                    .map(str::to_string)
                    .collect(),
                side_effects: ["changes status flags"]
                    .into_iter()
                    .map(str::to_string)
                    .collect(),
                unknown_source_fields: ["timing", "same"].into_iter().map(str::to_string).collect(),
            },
        }
    }
    fn candidate(name: &str, entry: &str) -> Candidate {
        Candidate {
            id: "api0".into(),
            name: name.into(),
            entry_point: entry.into(),
            inputs: vec![],
            outputs: vec![],
            side_effects: vec![],
            source_claim: SourceClaim::Unverified,
            source_execution_permitted: false,
            source_release_eligible: false,
            unknown_fields: vec!["timing".into()],
        }
    }
    // The two dialects have distinct event shapes, field conventions and CPUs.
    struct EightBit;
    impl SourceAdapter for EightBit {
        fn visit(
            &self,
            source: &Value,
            _max: usize,
            emit: &mut dyn FnMut(Candidate) -> Result<(), String>,
        ) -> Result<(), String> {
            let first = source["slots"]
                .as_array()
                .ok_or("bad slots")?
                .first()
                .ok_or("empty")?;
            let mut item = candidate("TEST_8BIT", "0xE000");
            item.id = first["key"].as_str().ok_or("missing key")?.to_string();
            item.outputs = vec![json!({"name":"A","meaning":"candidate output","width_bits":8})];
            emit(item)
        }
    }
    struct MultiCpu;
    impl SourceAdapter for MultiCpu {
        fn visit(
            &self,
            source: &Value,
            _max: usize,
            emit: &mut dyn FnMut(Candidate) -> Result<(), String>,
        ) -> Result<(), String> {
            let symbol = source["exports"]["entry"]
                .as_str()
                .ok_or("missing symbol")?;
            let mut item = candidate("TEST_MULTICPU", &format!("symbol:{symbol}"));
            item.inputs = vec![json!({"name":"r0","meaning":"candidate argument","width_bits":32})];
            item.source_claim = SourceClaim::ClaimedVerified;
            emit(item)
        }
    }
    #[test]
    fn two_architectures_share_a_single_import_path_without_promotion() {
        for (doc, platform, arch) in [
            (
                import(
                    &EightBit,
                    &json!({"slots":[{"key":"api0"}]}),
                    &pin(),
                    "synthetic-a",
                    "8bit",
                )
                .unwrap(),
                "synthetic-a",
                "8bit",
            ),
            (
                import(
                    &MultiCpu,
                    &json!({"exports":{"entry":"test"}}),
                    &pin(),
                    "synthetic-b",
                    "multicpu",
                )
                .unwrap(),
                "synthetic-b",
                "multicpu",
            ),
        ] {
            assert_eq!(doc.imported_candidates, 1);
            assert_eq!(doc.verified_abis, 0);
            assert_eq!(doc.document["platform"], platform);
            assert_eq!(doc.document["architecture"], arch);
            assert_eq!(
                doc.document["apis"][0]["platform_extension"]["biosdevkit_import"]["verification"],
                "UNVERIFIED"
            );
            assert_eq!(
                doc.document["apis"][0]["platform_extension"]["biosdevkit_import"]
                    ["execution_permitted"],
                false
            );
            assert_eq!(
                doc.document["apis"][0]["platform_extension"]["biosdevkit_import"]
                    ["release_eligible"],
                false
            );
        }
        let multi = import(
            &MultiCpu,
            &json!({"exports":{"entry":"test"}}),
            &pin(),
            "synthetic-b",
            "multicpu",
        )
        .unwrap();
        assert_eq!(
            multi.document["apis"][0]["platform_extension"]["biosdevkit_import"]["source_claim"],
            "CLAIMED_VERIFIED"
        );
    }
    struct Fixed(Vec<Candidate>);
    impl SourceAdapter for Fixed {
        fn visit(
            &self,
            _: &Value,
            _max: usize,
            emit: &mut dyn FnMut(Candidate) -> Result<(), String>,
        ) -> Result<(), String> {
            for item in &self.0 {
                emit(item.clone())?;
            }
            Ok(())
        }
    }
    #[test]
    fn rejects_claimed_permissions_and_bad_provenance() {
        let mut unsafe_item = candidate("TEST", "0x00");
        unsafe_item.source_release_eligible = true;
        assert!(import(&Fixed(vec![unsafe_item]), &json!({}), &pin(), "x", "y").is_err());
        let mut unsafe_item = candidate("TEST", "0x00");
        unsafe_item.source_execution_permitted = true;
        assert!(import(&Fixed(vec![unsafe_item]), &json!({}), &pin(), "x", "y").is_err());
        let mut bad_pin = pin();
        bad_pin.rights_approved = false;
        assert!(import(
            &EightBit,
            &json!({"slots":[{"key":"api0"}]}),
            &bad_pin,
            "x",
            "y"
        )
        .is_err());
        bad_pin = pin();
        bad_pin.sha256 = "x".repeat(64);
        assert!(import(
            &EightBit,
            &json!({"slots":[{"key":"api0"}]}),
            &bad_pin,
            "x",
            "y"
        )
        .is_err());
    }
    #[test]
    fn rejects_duplicates_malformed_fields_and_missing_source() {
        let a = candidate("TEST", "0x00");
        assert!(import(
            &Fixed(vec![a.clone(), a.clone()]),
            &json!({}),
            &pin(),
            "x",
            "y"
        )
        .is_err());
        let mut bad = a.clone();
        bad.unknown_fields = vec!["same".into(), "same".into()];
        assert!(import(&Fixed(vec![bad]), &json!({}), &pin(), "x", "y").is_err());
        let mut bad = a;
        bad.outputs = vec![json!({"name":"A","meaning":"test","width_bits":0})];
        assert!(import(&Fixed(vec![bad]), &json!({}), &pin(), "x", "y").is_err());
        assert!(import(&EightBit, &json!({"slots":[]}), &pin(), "x", "y").is_err());
        assert!(import(
            &EightBit,
            &json!({"slots":[{"key":"api0"}]}),
            &pin(),
            "",
            "y"
        )
        .is_err());
    }
    struct Flood(std::cell::Cell<usize>);
    impl SourceAdapter for Flood {
        fn visit(
            &self,
            _: &Value,
            _max: usize,
            emit: &mut dyn FnMut(Candidate) -> Result<(), String>,
        ) -> Result<(), String> {
            for _ in 0..MAX_CANDIDATES + 100 {
                let n = self.0.get();
                self.0.set(n + 1);
                let mut item = candidate("TEST", "symbol:synthetic");
                item.id = format!("api{n}");
                emit(item)?;
            }
            Ok(())
        }
    }
    #[test]
    fn rejects_during_production_without_buffering_all_candidates() {
        let adapter = Flood(std::cell::Cell::new(0));
        assert!(import(&adapter, &json!({}), &pin(), "synthetic", "synthetic").is_err());
        assert_eq!(adapter.0.get(), MAX_CANDIDATES + 1);
    }
    #[test]
    fn rejects_oversized_single_candidate_before_output() {
        let mut bad = candidate("TEST", "symbol:synthetic");
        bad.side_effects = vec!["x".repeat(MAX_ENTRY_BYTES + 1)];
        assert!(import(
            &Fixed(vec![bad]),
            &json!({}),
            &pin(),
            "synthetic",
            "synthetic"
        )
        .is_err());
    }
    #[test]
    fn rejects_private_paths_hashes_and_nested_register_data_even_if_adapter_emits_them() {
        let private_path = "/private/customer/game.cue";
        let private_hash = "e".repeat(64);
        let baseline = candidate("TEST", "0x00");

        let mut bad = baseline.clone();
        bad.entry_point = private_path.into();
        assert!(import(
            &Fixed(vec![bad]),
            &json!({}),
            &pin(),
            "synthetic",
            "synthetic"
        )
        .is_err());

        let mut bad = baseline.clone();
        bad.name = "customer_game.rom".into();
        assert!(import(
            &Fixed(vec![bad]),
            &json!({}),
            &pin(),
            "synthetic",
            "synthetic"
        )
        .is_err());

        let mut bad = baseline.clone();
        bad.inputs = vec![json!({
            "name": "A", "meaning": "test", "width_bits": 8,
            "nested": {"local": private_path, "media_hash": private_hash}
        })];
        assert!(import(
            &Fixed(vec![bad]),
            &json!({}),
            &pin(),
            "synthetic",
            "synthetic"
        )
        .is_err());

        let mut bad = baseline.clone();
        bad.outputs = vec![json!({"name":"A","meaning":private_path,"width_bits":8})];
        assert!(import(
            &Fixed(vec![bad]),
            &json!({}),
            &pin(),
            "synthetic",
            "synthetic"
        )
        .is_err());

        let mut bad = baseline.clone();
        bad.side_effects.push(private_path.into());
        assert!(import(
            &Fixed(vec![bad]),
            &json!({}),
            &pin(),
            "synthetic",
            "synthetic"
        )
        .is_err());

        let mut bad = baseline.clone();
        bad.unknown_fields.push("private_media.rom".into());
        assert!(import(
            &Fixed(vec![bad]),
            &json!({}),
            &pin(),
            "synthetic",
            "synthetic"
        )
        .is_err());

        // Even adding an unsafe entry to the caller catalog does not bypass
        // the path/filename/description gate.
        let mut policy = pin();
        policy.publication.side_effects.insert(private_path.into());
        let mut bad = baseline.clone();
        bad.side_effects.push(private_path.into());
        assert!(import(
            &Fixed(vec![bad]),
            &json!({}),
            &policy,
            "synthetic",
            "synthetic"
        )
        .is_err());

        let mut missing = pin();
        missing.publication.api_ids.clear();
        assert!(import(
            &Fixed(vec![baseline]),
            &json!({}),
            &missing,
            "synthetic",
            "synthetic"
        )
        .is_err());
    }

    #[test]
    fn approved_public_descriptions_are_kept_exactly() {
        let mut candidate = candidate("TEST", "0x00");
        candidate
            .inputs
            .push(json!({"name":"A","meaning":"test","width_bits":8}));
        candidate.side_effects.push("changes status flags".into());
        let result = import(
            &Fixed(vec![candidate]),
            &json!({}),
            &pin(),
            "synthetic",
            "synthetic",
        )
        .unwrap();
        assert_eq!(result.document["apis"][0]["inputs"][0]["meaning"], "test");
        assert_eq!(
            result.document["apis"][0]["side_effects"][0],
            "changes status flags"
        );
        assert_eq!(
            result.document["apis"][0]["platform_extension"]["biosdevkit_import"]["verification"],
            "UNVERIFIED"
        );
    }

    struct IgnoreRejection;
    impl SourceAdapter for IgnoreRejection {
        fn visit(
            &self,
            _: &Value,
            _max: usize,
            emit: &mut dyn FnMut(Candidate) -> Result<(), String>,
        ) -> Result<(), String> {
            let _ = emit(candidate("TEST", "/private/media/game.cue"));
            let _ = emit(candidate("TEST", "0x00"));
            Ok(())
        }
    }
    #[test]
    fn discarded_rejection_cannot_create_partial_published_contract() {
        assert!(import(
            &IgnoreRejection,
            &json!({}),
            &pin(),
            "synthetic",
            "synthetic"
        )
        .is_err());
    }
}
