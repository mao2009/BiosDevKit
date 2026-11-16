//! Architecture-neutral, fail-closed import of runner-reported evidence.
//! Untrusted free-form source text is never copied into public reports.
use serde_json::{json, Value};
use std::collections::{HashMap, HashSet};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Outcome {
    Pass,
    Fail,
    Blocked,
    Skip,
    NotRun,
}

#[derive(Debug, Clone, Copy)]
pub enum Reason {
    Mismatch,
    Timeout,
    InputUnavailable,
    Disabled,
    InvalidInput,
}

impl Reason {
    fn code(self) -> &'static str {
        match self {
            Self::Mismatch => "MISMATCH",
            Self::Timeout => "TIMEOUT",
            Self::InputUnavailable => "INPUT_UNAVAILABLE",
            Self::Disabled => "DISABLED",
            Self::InvalidInput => "INVALID_INPUT",
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub enum RunnerScope {
    Synthetic,
    Host,
    Emulator,
    Hardware,
}

impl RunnerScope {
    fn code(self) -> &'static str {
        match self {
            Self::Synthetic => "SYNTHETIC",
            Self::Host => "HOST",
            Self::Emulator => "EMULATOR",
            Self::Hardware => "HARDWARE",
        }
    }
}

/// A curated local catalog entry. The caller must independently verify this
/// registration; importing does not authenticate any test execution.
#[derive(Debug, Clone)]
pub struct RegisteredTest {
    pub source_key: String,
    pub public_test_id: String,
    pub platform: String,
    pub source_revision: String,
    pub runner_version: String,
    pub scope: RunnerScope,
    pub permitted_metrics: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct SourceRecord {
    pub source_key: String,
    pub claimed_source_revision: String,
    pub claimed_runner_version: String,
    pub outcome: Outcome,
    pub reason: Option<Reason>,
    /// Numerical observations only. No raw strings, paths or media hashes.
    pub metrics: Vec<(String, u64)>,
}

/// Produce records one at a time and stop when the sink rejects a record.
/// Implementations must avoid unbounded intermediate buffers.
pub trait EvidenceAdapter {
    fn visit(
        &self,
        input: &Value,
        max_records: usize,
        emit: &mut dyn FnMut(SourceRecord) -> Result<(), String>,
    ) -> Result<(), String>;
}
pub const MAX_RECORDS: usize = 10_000;

#[derive(Debug, Clone)]
pub enum ImportedRecord {
    /// Valid Test Evidence v1 (which does not have a NOT_RUN outcome).
    EvidenceV1(Value),
    /// Explicit separate state: cannot be turned into PASS or SKIP by v1.
    NotRun {
        test_id: String,
        platform: String,
        reason_code: String,
    },
}

fn token(s: &str) -> bool {
    !s.is_empty()
        && s.len() <= 128
        && s.bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'_' | b'.' | b'-' | b':'))
}
fn revision(s: &str) -> bool {
    s.len() == 40
        && s.bytes()
            .all(|b| b.is_ascii_hexdigit() && !b.is_ascii_uppercase())
}

/// Import from an untrusted source using an explicit public catalog.
/// Reject rather than echo arbitrary data, including adapter error messages.
pub fn import<A: EvidenceAdapter>(
    adapter: &A,
    input: &Value,
    catalog: &[RegisteredTest],
) -> Result<Vec<ImportedRecord>, String> {
    if catalog.is_empty() || catalog.len() > MAX_RECORDS {
        return Err("invalid evidence catalog".into());
    }
    let mut trusted = HashMap::new();
    for item in catalog {
        if !token(&item.source_key)
            || !token(&item.public_test_id)
            || !token(&item.platform)
            || !revision(&item.source_revision)
            || !token(&item.runner_version)
            || trusted.insert(item.source_key.as_str(), item).is_some()
        {
            return Err("invalid or duplicate evidence registration".into());
        }
        let mut names = HashSet::new();
        if item
            .permitted_metrics
            .iter()
            .any(|key| !token(key) || !names.insert(key))
        {
            return Err("invalid metric registration".into());
        }
    }
    // A successful record requires one registered key. Stop the adapter as
    // soon as it attempts to emit more items than the catalog can authorize.
    let limit = catalog.len().min(MAX_RECORDS);
    let mut seen = HashSet::new();
    let mut output = Vec::new();
    let mut validate_and_store = |record: SourceRecord| -> Result<(), String> {
        if output.len() >= limit {
            return Err("evidence record limit exceeded".into());
        }
        let item = trusted
            .get(record.source_key.as_str())
            .ok_or("unregistered evidence source")?;
        if !seen.insert(record.source_key.clone())
            || record.claimed_source_revision != item.source_revision
            || record.claimed_runner_version != item.runner_version
            || (record.outcome != Outcome::Pass && record.reason.is_none())
            || (record.outcome == Outcome::Pass && record.reason.is_some())
        {
            return Err("duplicate, contradictory, or mismatched evidence".into());
        }
        let mut metrics = HashSet::new();
        if record.metrics.len() > item.permitted_metrics.len()
            || record.metrics.iter().any(|(key, _)| {
                !item.permitted_metrics.contains(key) || !metrics.insert(key.clone())
            })
        {
            return Err("unregistered or duplicate evidence metric".into());
        }
        if record.outcome == Outcome::NotRun {
            output.push(ImportedRecord::NotRun {
                test_id: item.public_test_id.clone(),
                platform: item.platform.clone(),
                reason_code: record
                    .reason
                    .expect("checked non-PASS reason")
                    .code()
                    .into(),
            });
            return Ok(());
        }
        let status = match record.outcome {
            Outcome::Pass => "PASS",
            Outcome::Fail => "FAIL",
            Outcome::Blocked => "BLOCKED",
            Outcome::Skip => "SKIP",
            Outcome::NotRun => unreachable!(),
        };
        let mut observations = vec![
            json!({"key":"execution_scope","value":item.scope.code()}),
            json!({"key":"identity_trust","value":"DECLARED_NOT_ATTESTED"}),
        ];
        // Stable ordering independent of adapter/HashMap insertion order.
        let mut sorted = record.metrics;
        sorted.sort_by(|a, b| a.0.cmp(&b.0));
        for (key, value) in sorted {
            observations.push(json!({"key":key,"value":value.to_string()}));
        }
        let mut result = json!({
            "schema_version": 1,
            "test_id": item.public_test_id,
            "platform": item.platform,
            "result": status,
            "source_revision": item.source_revision,
            "observations": observations
        });
        if let Some(reason) = record.reason {
            result["reason"] = json!(reason.code());
        }
        output.push(ImportedRecord::EvidenceV1(result));
        Ok(())
    };
    // The adapter is not allowed to suppress a sink failure and then report
    // success with a partial result. Latch the first error at this boundary.
    let mut sink_rejected = false;
    let mut emit = |record: SourceRecord| -> Result<(), String> {
        if sink_rejected {
            return Err("evidence sink already rejected input".into());
        }
        let result = validate_and_store(record);
        if result.is_err() {
            sink_rejected = true;
        }
        result
    };
    let visit_result = adapter.visit(input, limit, &mut emit);
    if sink_rejected || visit_result.is_err() {
        return Err("evidence adapter rejected input".into());
    }
    if output.is_empty() {
        return Err("invalid number of evidence records".into());
    }
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn registered(key: &str, scope: RunnerScope) -> RegisteredTest {
        RegisteredTest {
            source_key: key.into(),
            public_test_id: format!("public.{key}"),
            platform: "synthetic-target".into(),
            source_revision: "a".repeat(40),
            runner_version: "runner-v1".into(),
            scope,
            permitted_metrics: vec!["checks".into(), "cycles".into()],
        }
    }
    fn record(key: &str, outcome: Outcome) -> SourceRecord {
        SourceRecord {
            source_key: key.into(),
            claimed_source_revision: "a".repeat(40),
            claimed_runner_version: "runner-v1".into(),
            outcome,
            reason: if outcome == Outcome::Pass {
                None
            } else {
                Some(Reason::Disabled)
            },
            metrics: vec![("checks".into(), 3)],
        }
    }
    struct SingleCpu;
    impl EvidenceAdapter for SingleCpu {
        fn visit(
            &self,
            v: &Value,
            _max: usize,
            emit: &mut dyn FnMut(SourceRecord) -> Result<(), String>,
        ) -> Result<(), String> {
            let status = v["status"].as_str().ok_or("private bad status")?;
            let outcome = match status {
                "ok" => Outcome::Pass,
                "disabled" => Outcome::NotRun,
                _ => return Err("private sensitive filename /secret/game.cue".into()),
            };
            emit(record("single", outcome))
        }
    }
    struct MultiCpu;
    impl EvidenceAdapter for MultiCpu {
        fn visit(
            &self,
            v: &Value,
            _max: usize,
            emit: &mut dyn FnMut(SourceRecord) -> Result<(), String>,
        ) -> Result<(), String> {
            let count = v["summary"]["checks"].as_u64().ok_or("missing checks")?;
            let mut x = record("multi", Outcome::Fail);
            x.reason = Some(Reason::Mismatch);
            x.metrics = vec![("cycles".into(), count)];
            emit(x)
        }
    }
    struct Fixed(Vec<SourceRecord>);
    impl EvidenceAdapter for Fixed {
        fn visit(
            &self,
            _: &Value,
            _max: usize,
            emit: &mut dyn FnMut(SourceRecord) -> Result<(), String>,
        ) -> Result<(), String> {
            for item in &self.0 {
                emit(item.clone())?;
            }
            Ok(())
        }
    }

    #[test]
    fn two_dialects_use_one_core_without_raw_text() {
        let a = import(
            &SingleCpu,
            &json!({"status":"ok"}),
            &[registered("single", RunnerScope::Host)],
        )
        .unwrap();
        let b = import(
            &MultiCpu,
            &json!({"summary":{"checks":7}}),
            &[registered("multi", RunnerScope::Emulator)],
        )
        .unwrap();
        match (&a[0], &b[0]) {
            (ImportedRecord::EvidenceV1(x), ImportedRecord::EvidenceV1(y)) => {
                assert_eq!(x["result"], "PASS");
                assert_eq!(y["result"], "FAIL");
                assert_eq!(y["reason"], "MISMATCH");
                assert_eq!(y["observations"][0]["value"], "EMULATOR");
                assert_eq!(x["observations"][1]["value"], "DECLARED_NOT_ATTESTED");
                assert_eq!(y["observations"][2]["value"], "7");
                assert!(crate::evidence::aggregate(&[
                    crate::evidence::Record {
                        status: crate::evidence::Status::Pass,
                        reason: None
                    },
                    crate::evidence::Record {
                        status: crate::evidence::Status::Fail,
                        reason: Some("MISMATCH")
                    },
                ])
                .is_ok());
            }
            _ => panic!("wrong output type"),
        }
    }

    #[test]
    fn not_run_is_explicit_and_never_mapped_to_v1_skip_or_pass() {
        let out = import(
            &SingleCpu,
            &json!({"status":"disabled"}),
            &[registered("single", RunnerScope::Synthetic)],
        )
        .unwrap();
        assert!(matches!(&out[0], ImportedRecord::NotRun { reason_code, .. }
                          if reason_code == "DISABLED"));
    }
    #[test]
    fn fails_closed_on_private_error_provenance_and_duplicate_input() {
        let catalog = [registered("single", RunnerScope::Hardware)];
        let err = import(&SingleCpu, &json!({"status":"/secret/game.cue"}), &catalog).unwrap_err();
        assert!(!err.contains("secret"));
        assert!(!err.contains("cue"));
        let mut wrong = record("single", Outcome::Pass);
        wrong.claimed_source_revision = "b".repeat(40);
        assert!(import(&Fixed(vec![wrong]), &json!({}), &catalog).is_err());
        let same = record("single", Outcome::Pass);
        assert!(import(&Fixed(vec![same.clone(), same]), &json!({}), &catalog).is_err());
        let mut leak = record("single", Outcome::Pass);
        leak.metrics.push(("/private/bios.rom".into(), 42));
        assert!(import(&Fixed(vec![leak]), &json!({}), &catalog).is_err());
        let mut bad = record("single", Outcome::Blocked);
        bad.reason = None;
        assert!(import(&Fixed(vec![bad]), &json!({}), &catalog).is_err());
    }
    #[test]
    fn rejects_unregistered_and_invalid_catalog_entries() {
        assert!(import(
            &Fixed(vec![record("other", Outcome::Pass)]),
            &json!({}),
            &[registered("single", RunnerScope::Host)]
        )
        .is_err());
        let mut bad = registered("single", RunnerScope::Host);
        bad.source_revision = "not-a-commit".into();
        assert!(import(&SingleCpu, &json!({"status":"ok"}), &[bad]).is_err());
        assert!(import(
            &Fixed(vec![]),
            &json!({}),
            &[registered("single", RunnerScope::Host)]
        )
        .is_err());
    }
    struct Flood(std::cell::Cell<usize>);
    impl EvidenceAdapter for Flood {
        fn visit(
            &self,
            _: &Value,
            _max: usize,
            emit: &mut dyn FnMut(SourceRecord) -> Result<(), String>,
        ) -> Result<(), String> {
            for n in 0..100 {
                self.0.set(n + 1);
                emit(record(&format!("id{n}"), Outcome::Pass))?;
            }
            Ok(())
        }
    }
    #[test]
    fn early_backpressure_stops_unbounded_source_record_emission() {
        let adapter = Flood(std::cell::Cell::new(0));
        let registrations = [
            registered("id0", RunnerScope::Synthetic),
            registered("id1", RunnerScope::Synthetic),
        ];
        assert!(import(&adapter, &json!({}), &registrations).is_err());
        assert_eq!(adapter.0.get(), 3);
    }
    struct IgnoreRejection;
    impl EvidenceAdapter for IgnoreRejection {
        fn visit(
            &self,
            _: &Value,
            _max: usize,
            emit: &mut dyn FnMut(SourceRecord) -> Result<(), String>,
        ) -> Result<(), String> {
            // A buggy plugin ignores the rejection of an unregistered record.
            let _ = emit(record("not_registered", Outcome::Pass));
            let _ = emit(record("single", Outcome::Pass));
            Ok(())
        }
    }
    #[test]
    fn ignored_sink_error_cannot_turn_partial_evidence_into_success() {
        let catalog = [registered("single", RunnerScope::Synthetic)];
        assert!(import(&IgnoreRejection, &json!({}), &catalog).is_err());
    }

}
