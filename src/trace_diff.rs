//! Differential comparisons of two independently validated Trace Event v1 JSONL inputs.
//! Order is meaningful only within each (run_id, stream_id, cpu) lane.
use crate::traces::validate_jsonl;
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};

#[derive(Debug, PartialEq, Eq)]
pub enum DifferenceKind {
    Missing,
    Extra,
    Reordered,
    Changed,
}

/// Publication-safe mismatch summary. No untrusted run/stream/CPU identifiers,
/// addresses, registers, nested details or raw event objects are returned.
/// lane_index is the position in the sorted internal lane set; callers that
/// need identifiers must resolve it in a separately authenticated private map.
#[derive(Debug, PartialEq)]
pub struct Difference {
    pub kind: DifferenceKind,
    pub lane_index: usize,
    pub index: usize,
    pub expected_kind: Option<&'static str>,
    pub actual_kind: Option<&'static str>,
}

type Lane = (String, String, String);

fn lanes(input: &str) -> Result<BTreeMap<Lane, Vec<Value>>, String> {
    validate_jsonl(input).map_err(|_| "invalid trace input".to_string())?;
    let mut output: BTreeMap<Lane, Vec<Value>> = BTreeMap::new();
    for line in input.lines() {
        let v: Value = serde_json::from_str(line).map_err(|_| "invalid trace input".to_string())?;
        let key = (
            v["run_id"].as_str().unwrap().to_owned(),
            v["stream_id"].as_str().unwrap().to_owned(),
            v["cpu"].as_str().unwrap().to_owned(),
        );
        output.entry(key).or_default().push(v);
    }
    Ok(output)
}

fn normalized(mut v: Value) -> Value {
    if let Some(obj) = v.as_object_mut() {
        obj.remove("sequence");
        obj.remove("run_id");
    }
    v
}

/// Return only a member of the fixed Trace Event v1 event-kind vocabulary.
fn public_event_kind(value: Option<&Value>) -> Option<&'static str> {
    match value?.get("event")?.as_str()? {
        "bios_call" => Some("bios_call"),
        "bios_return" => Some("bios_return"),
        "interrupt" => Some("interrupt"),
        "memory_write" => Some("memory_write"),
        "device_event" => Some("device_event"),
        _ => None,
    }
}

/// Report the first deterministic *sanitized* lane/index mismatch.
/// No source-controlled identifiers or raw event data cross this boundary.
/// No cross-lane causal inference.
pub fn first_difference(expected: &str, actual: &str) -> Result<Option<Difference>, String> {
    let left = lanes(expected)?;
    let right = lanes(actual)?;
    let left_runs: BTreeSet<_> = left.keys().map(|key| key.0.clone()).collect();
    let right_runs: BTreeSet<_> = right.keys().map(|key| key.0.clone()).collect();
    if left_runs != right_runs {
        return Err("run_id sets differ: explicit capture pairing is required".into());
    }
    let left_keys: BTreeSet<_> = left.keys().cloned().collect();
    let right_keys: BTreeSet<_> = right.keys().cloned().collect();
    if left_keys != right_keys {
        return Err("trace lane sets differ: explicit stream/cpu pairing is required".into());
    }
    let keys = left_keys;
    for (lane_index, key) in keys.into_iter().enumerate() {
        let a = left.get(&key).map(Vec::as_slice).unwrap_or(&[]);
        let b = right.get(&key).map(Vec::as_slice).unwrap_or(&[]);
        for index in 0..a.len().max(b.len()) {
            let x = a.get(index).cloned();
            let y = b.get(index).cloned();
            if x.as_ref().map(|v| normalized(v.clone()))
                == y.as_ref().map(|v| normalized(v.clone()))
            {
                continue;
            }
            // Compare normalized suffixes as ordered subsequences. This handles
            // multiple insertions/deletions without mistaking them for reorder.
            let na: Vec<_> = a[index..].iter().cloned().map(normalized).collect();
            let nb: Vec<_> = b[index..].iter().cloned().map(normalized).collect();
            let subsequence = |short: &[Value], long: &[Value]| {
                let mut matched = 0usize;
                for value in long {
                    if matched < short.len() && short[matched] == *value {
                        matched += 1;
                    }
                }
                matched == short.len()
            };
            let kind = if na.len() < nb.len() && subsequence(&na, &nb) {
                DifferenceKind::Extra
            } else if nb.len() < na.len() && subsequence(&nb, &na) {
                DifferenceKind::Missing
            } else if na.len() >= 2
                && nb.len() == na.len()
                && na[0] == nb[1]
                && na[1] == nb[0]
                && na[2..] == nb[2..]
            {
                DifferenceKind::Reordered
            } else if na.len() == nb.len() && na[1..] == nb[1..] {
                DifferenceKind::Changed
            } else {
                return Err("ambiguous trace alignment".into());
            };
            return Ok(Some(Difference {
                kind,
                lane_index,
                index,
                expected_kind: public_event_kind(x.as_ref()),
                actual_kind: public_event_kind(y.as_ref()),
            }));
        }
    }
    Ok(None)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn event(run: &str, stream: &str, cpu: &str, seq: u64, address: &str) -> String {
        serde_json::json!({
            "schema_version": 1,
            "run_id": run,
            "stream_id": stream,
            "cpu": cpu,
            "sequence": seq,
            "event": "bios_call",
            "registers": {},
            "address": address
        })
        .to_string()
    }

    #[test]
    fn match_and_changed() {
        let a = event("r", "s", "huc6280", 1, "0x100");
        let b = event("r", "s", "huc6280", 10, "0x100");
        assert_eq!(first_difference(&a, &b).unwrap(), None);
        let c = event("r", "s", "huc6280", 10, "0x200");
        assert_eq!(
            first_difference(&a, &c).unwrap().unwrap().kind,
            DifferenceKind::Changed
        );
    }

    #[test]
    fn missing_extra_and_reorder() {
        let a = format!(
            "{}\n{}",
            event("r", "s", "cpu", 1, "A"),
            event("r", "s", "cpu", 2, "B")
        );
        let b = event("r", "s", "cpu", 1, "A");
        assert_eq!(
            first_difference(&a, &b).unwrap().unwrap().kind,
            DifferenceKind::Missing
        );
        assert_eq!(
            first_difference(&b, &a).unwrap().unwrap().kind,
            DifferenceKind::Extra
        );
        let reordered = format!(
            "{}\n{}",
            event("r", "s", "cpu", 1, "B"),
            event("r", "s", "cpu", 2, "A")
        );
        assert_eq!(
            first_difference(&a, &reordered).unwrap().unwrap().kind,
            DifferenceKind::Reordered
        );
    }

    #[test]
    fn insertion_deletion_alignment_with_repeats() {
        let make = |addresses: &[&str]| {
            addresses
                .iter()
                .enumerate()
                .map(|(i, addr)| event("r", "s", "cpu", i as u64 + 1, addr))
                .collect::<Vec<_>>()
                .join("\n")
        };
        let baseline = make(&["A", "B", "C", "D"]);
        assert_eq!(
            first_difference(&baseline, &make(&["A", "X", "B", "C", "D"]))
                .unwrap()
                .unwrap()
                .kind,
            DifferenceKind::Extra
        );
        assert_eq!(
            first_difference(&baseline, &make(&["A", "C", "D"]))
                .unwrap()
                .unwrap()
                .kind,
            DifferenceKind::Missing
        );
        assert_eq!(
            first_difference(
                &make(&["A", "B", "B", "C"]),
                &make(&["A", "B", "B", "B", "C"])
            )
            .unwrap()
            .unwrap()
            .kind,
            DifferenceKind::Extra
        );
        assert_eq!(
            first_difference(&baseline, &make(&["A", "C", "B", "D"]))
                .unwrap()
                .unwrap()
                .kind,
            DifferenceKind::Reordered
        );
    }

    #[test]
    fn multiple_insertions_and_deletions() {
        let make = |addresses: &[&str]| {
            addresses
                .iter()
                .enumerate()
                .map(|(i, addr)| event("r", "s", "cpu", i as u64 + 1, addr))
                .collect::<Vec<_>>()
                .join("\n")
        };
        assert_eq!(
            first_difference(&make(&["A", "B", "C"]), &make(&["A", "X", "Y", "B", "C"]))
                .unwrap()
                .unwrap()
                .kind,
            DifferenceKind::Extra
        );
        assert_eq!(
            first_difference(&make(&["A", "B", "C", "D"]), &make(&["A", "D"]))
                .unwrap()
                .unwrap()
                .kind,
            DifferenceKind::Missing
        );
        assert!(first_difference(&make(&["A", "B", "C"]), &make(&["A", "X", "Y", "C"])).is_err());
    }

    #[test]
    fn rejects_composite_same_length_edits_and_run_mismatch() {
        let make = |run: &str, values: &[&str]| {
            values
                .iter()
                .enumerate()
                .map(|(i, addr)| event(run, "s", "cpu", i as u64 + 1, addr))
                .collect::<Vec<_>>()
                .join("\n")
        };
        assert!(first_difference(
            &make("r", &["A", "B", "C", "D"]),
            &make("r", &["A", "X", "B", "D"])
        )
        .is_err());
        assert!(first_difference(
            &make("r", &["A", "B", "C", "D"]),
            &make("r", &["A", "X", "C", "Y"])
        )
        .is_err());
        assert!(first_difference(&make("r1", &["A"]), &make("r2", &["A"])).is_err());
    }

    #[test]
    fn rejects_unpaired_stream_or_cpu_lane() {
        let a = event("r", "s", "huc6280", 1, "A");
        let wrong_stream = event("r", "other", "huc6280", 1, "A");
        let wrong_cpu = event("r", "s", "m68000", 1, "A");
        assert!(first_difference(&a, &wrong_stream).is_err());
        assert!(first_difference(&a, &wrong_cpu).is_err());
    }

    #[test]
    fn separate_lanes_and_bad_input() {
        let a = format!(
            "{}\n{}",
            event("r", "s", "cpu", 1, "A"),
            event("r", "other", "cpu", 1, "B")
        );
        let b = format!(
            "{}\n{}",
            event("r", "other", "cpu", 7, "B"),
            event("r", "s", "cpu", 9, "A")
        );
        assert_eq!(first_difference(&a, &b).unwrap(), None);
        assert!(first_difference("broken", &b).is_err());
    }
    #[test]
    fn never_exposes_private_paths_hashes_or_nested_payloads() {
        let secret_path = "/private/customer/game.cue";
        let private_hash = "a".repeat(64);
        let secret_run = "private-run/game.cue";
        let mut first: Value = serde_json::from_str(&event(
            secret_run,
            "private-stream/game.cue",
            "private-cpu",
            1,
            secret_path,
        ))
        .unwrap();
        first["details"] = serde_json::json!({
            "nested":{"file":secret_path,"sha256":private_hash},
            "list":[{"value":secret_path}]
        });
        first["registers"]["X"] = serde_json::json!(secret_path);
        let mut second = first.clone();
        second["address"] = serde_json::json!("/private/customer/other.cue");
        let difference = first_difference(&first.to_string(), &second.to_string())
            .unwrap()
            .unwrap();
        assert_eq!(difference.kind, DifferenceKind::Changed);
        assert_eq!(difference.lane_index, 0);
        assert_eq!(difference.expected_kind, Some("bios_call"));
        assert_eq!(difference.actual_kind, Some("bios_call"));
        let displayed = format!("{difference:?}");
        for private in [
            secret_path,
            private_hash.as_str(),
            secret_run,
            "private-stream",
            "private-cpu",
            "other.cue",
        ] {
            assert!(!displayed.contains(private), "private data leaked");
        }

        // A malformed (duplicate sequence) trace must not forward validator
        // errors that interpolate user-provided run and stream identifiers.
        let invalid = format!("{}\n{}\n", first, first);
        let error = first_difference(&invalid, &second.to_string()).unwrap_err();
        assert_eq!(error, "invalid trace input");
        assert!(!error.contains(secret_run));

        // A multi-event ambiguous alignment must likewise remain fixed text.
        let make = |addresses: &[&str]| {
            addresses
                .iter()
                .enumerate()
                .map(|(idx, addr)| {
                    event(
                        secret_run,
                        "private-stream/game.cue",
                        "private-cpu",
                        idx as u64 + 1,
                        addr,
                    )
                })
                .collect::<Vec<_>>()
                .join("\n")
        };
        let ambiguous = first_difference(
            &make(&["before", secret_path, "after"]),
            &make(&["before", "replacement", "new", "after"]),
        )
        .unwrap_err();
        assert_eq!(ambiguous, "ambiguous trace alignment");
    }

    #[test]
    fn missing_and_extra_events_only_report_fixed_kind_and_lane_index() {
        let first = event(
            "secret/run/path",
            "secret/stream",
            "private/cpu",
            1,
            "sensitive",
        );
        let second = format!(
            "{}\n{}",
            first,
            event(
                "secret/run/path",
                "secret/stream",
                "private/cpu",
                2,
                "sensitive2"
            )
        );
        let extra = first_difference(&first, &second).unwrap().unwrap();
        assert_eq!(extra.kind, DifferenceKind::Extra);
        assert_eq!(extra.lane_index, 0);
        assert_eq!(extra.expected_kind, None);
        assert_eq!(extra.actual_kind, Some("bios_call"));
        assert!(!format!("{extra:?}").contains("secret"));
    }
}
