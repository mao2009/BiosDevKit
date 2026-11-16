//! Architecture-neutral import of *candidate* BIOS API metadata.
//! Imported source assertions are never evidence of verified ABI compatibility.
use crate::contracts;
use serde_json::{json, Value};
use std::collections::HashSet;

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

pub trait SourceAdapter {
    fn extract(&self, source: &Value) -> Result<Vec<Candidate>, String>;
}

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
        && value.bytes().all(|b| {
            b.is_ascii_alphanumeric() || matches!(b, b'_' | b'-' | b'.' | b':')
        })
}
fn is_lower_hex(value: &str, len: usize) -> bool {
    value.len() == len
        && value.bytes().all(|b| b.is_ascii_hexdigit() && !b.is_ascii_uppercase())
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
    // The adapter sees untrusted source data. It must produce a complete IR.
    // Its error may contain secrets; discard it at the public boundary.
    let candidates = adapter
        .extract(source)
        .map_err(|_| "source adapter rejected input".to_string())?;
    if candidates.is_empty() {
        return Err("no candidate APIs".into());
    }
    let mut api_ids = HashSet::new();
    let mut apis = Vec::with_capacity(candidates.len());
    for candidate in candidates {
        if !is_token(&candidate.id)
            || !is_token(&candidate.name)
            || candidate.entry_point.is_empty()
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
        fn extract(&self, source: &Value) -> Result<Vec<Candidate>, String> {
            let first = source["slots"].as_array().ok_or("bad slots")?
                .first().ok_or("empty")?;
            let mut item = candidate("TEST_8BIT", "0xE000");
            item.id = first["key"].as_str().ok_or("missing key")?.to_string();
            item.outputs = vec![json!({"name":"A","meaning":"candidate output","width_bits":8})];
            Ok(vec![item])
        }
    }
    struct MultiCpu;
    impl SourceAdapter for MultiCpu {
        fn extract(&self, source: &Value) -> Result<Vec<Candidate>, String> {
            let symbol = source["exports"]["entry"].as_str().ok_or("missing symbol")?;
            let mut item = candidate("TEST_MULTICPU", &format!("symbol:{symbol}"));
            item.inputs = vec![json!({"name":"r0","meaning":"candidate argument","width_bits":32})];
            item.source_claim = SourceClaim::ClaimedVerified;
            Ok(vec![item])
        }
    }
    #[test]
    fn two_architectures_share_a_single_import_path_without_promotion() {
        for (doc, platform, arch) in [
            (import(&EightBit, &json!({"slots":[{"key":"api0"}]}),
                    &pin(), "synthetic-a", "8bit").unwrap(), "synthetic-a", "8bit"),
            (import(&MultiCpu, &json!({"exports":{"entry":"test"}}),
                    &pin(), "synthetic-b", "multicpu").unwrap(), "synthetic-b", "multicpu"),
        ] {
            assert_eq!(doc.imported_candidates, 1);
            assert_eq!(doc.verified_abis, 0);
            assert_eq!(doc.document["platform"], platform);
            assert_eq!(doc.document["architecture"], arch);
            assert_eq!(doc.document["apis"][0]["platform_extension"]["biosdevkit_import"]["verification"], "UNVERIFIED");
            assert_eq!(doc.document["apis"][0]["platform_extension"]["biosdevkit_import"]["execution_permitted"], false);
            assert_eq!(doc.document["apis"][0]["platform_extension"]["biosdevkit_import"]["release_eligible"], false);
        }
        let multi = import(&MultiCpu, &json!({"exports":{"entry":"test"}}),
                           &pin(), "synthetic-b", "multicpu").unwrap();
        assert_eq!(multi.document["apis"][0]["platform_extension"]["biosdevkit_import"]["source_claim"],
                   "CLAIMED_VERIFIED");
    }
    struct Fixed(Vec<Candidate>);
    impl SourceAdapter for Fixed {
        fn extract(&self, _: &Value) -> Result<Vec<Candidate>, String> {
            Ok(self.0.clone())
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
        assert!(import(&EightBit, &json!({"slots":[{"key":"api0"}]}),
                       &bad_pin, "x", "y").is_err());
        bad_pin = pin();
        bad_pin.sha256 = "x".repeat(64);
        assert!(import(&EightBit, &json!({"slots":[{"key":"api0"}]}),
                       &bad_pin, "x", "y").is_err());
    }
    #[test]
    fn rejects_duplicates_malformed_fields_and_missing_source() {
        let a = candidate("TEST", "0x00");
        assert!(import(&Fixed(vec![a.clone(), a.clone()]), &json!({}),
                       &pin(), "x", "y").is_err());
        let mut bad = a.clone();
        bad.unknown_fields = vec!["same".into(), "same".into()];
        assert!(import(&Fixed(vec![bad]), &json!({}), &pin(), "x", "y").is_err());
        let mut bad = a;
        bad.outputs = vec![json!({"name":"A","meaning":"test","width_bits":0})];
        assert!(import(&Fixed(vec![bad]), &json!({}), &pin(), "x", "y").is_err());
        assert!(import(&EightBit, &json!({"slots":[]}), &pin(), "x", "y").is_err());
        assert!(import(&EightBit, &json!({"slots":[{"key":"api0"}]}),
                       &pin(), "", "y").is_err());
    }
}