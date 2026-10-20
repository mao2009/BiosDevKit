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

#[derive(Debug, PartialEq)]
pub struct Difference {
    pub kind: DifferenceKind,
    pub run_id: String,
    pub stream_id: String,
    pub cpu: String,
    pub index: usize,
    pub expected: Option<Value>,
    pub actual: Option<Value>,
}

type Lane = (String, String, String);

fn lanes(input: &str) -> Result<BTreeMap<Lane, Vec<Value>>, String> {
    validate_jsonl(input)?;
    let mut output: BTreeMap<Lane, Vec<Value>> = BTreeMap::new();
    for line in input.lines() {
        let v: Value = serde_json::from_str(line).map_err(|e| e.to_string())?;
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

/// Report the first deterministic lane/index mismatch. No cross-lane causal inference.
pub fn first_difference(expected: &str, actual: &str) -> Result<Option<Difference>, String> {
    let left = lanes(expected)?;
    let right = lanes(actual)?;
    let keys: BTreeSet<_> = left.keys().chain(right.keys()).cloned().collect();
    for (run, stream, cpu) in keys {
        let key = (run.clone(), stream.clone(), cpu.clone());
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
            // Exact single insert/delete alignment takes precedence over reorder.
            // Compare the entire remaining suffix, including repeated events,
            // so equal payloads elsewhere do not create a false Reordered result.
            let same = |lhs: &[Value], rhs: &[Value]| {
                lhs.len() == rhs.len()
                    && lhs
                        .iter()
                        .zip(rhs)
                        .all(|(l, r)| normalized(l.clone()) == normalized(r.clone()))
            };
            let kind = if b.len() > index && same(&a[index..], &b[index + 1..]) {
                DifferenceKind::Extra
            } else if a.len() > index && same(&a[index + 1..], &b[index..]) {
                DifferenceKind::Missing
            } else if index + 1 < a.len()
                && index + 1 < b.len()
                && normalized(a[index].clone()) == normalized(b[index + 1].clone())
                && normalized(a[index + 1].clone()) == normalized(b[index].clone())
                && same(&a[index + 2..], &b[index + 2..])
            {
                DifferenceKind::Reordered
            } else {
                DifferenceKind::Changed
            };
            return Ok(Some(Difference {
                kind,
                run_id: run,
                stream_id: stream,
                cpu,
                index,
                expected: x,
                actual: y,
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
}
