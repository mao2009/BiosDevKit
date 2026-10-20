//! Validates JSONL trace events and per-producer sequence monotonicity.
use std::collections::HashMap;
pub fn validate_jsonl(input: &str) -> Result<usize, String> {
    let mut last: HashMap<(String, String), u64> = HashMap::new();
    let mut count = 0usize;
    for (line_index, line) in input.lines().enumerate() {
        let line_number = line_index + 1;
        if line.trim().is_empty() {
            return Err(format!("line {line_number}: blank event"));
        }
        let v: serde_json::Value =
            serde_json::from_str(line).map_err(|e| format!("line {line_number}: {e}"))?;
        let obj = v
            .as_object()
            .ok_or_else(|| format!("line {line_number}: event must be object"))?;
        for field in obj.keys() {
            if ![
                "schema_version",
                "sequence",
                "cpu",
                "event",
                "registers",
                "address",
                "details",
                "run_id",
                "stream_id",
            ]
            .contains(&field.as_str())
            {
                return Err(format!("line {line_number}: unknown field {field}"));
            }
        }
        if v.get("address").is_some_and(|x| !x.is_string())
            || v.get("details").is_some_and(|x| !x.is_object())
        {
            return Err(format!("line {line_number}: invalid address or details"));
        }
        let get = |key: &str| -> Result<&str, String> {
            v.get(key)
                .and_then(|x| x.as_str())
                .filter(|s| !s.is_empty())
                .ok_or_else(|| format!("line {line_number}: missing {key}"))
        };
        if v.get("schema_version").and_then(|x| x.as_u64()) != Some(1) {
            return Err(format!("line {line_number}: unsupported version"));
        }
        let key = (get("run_id")?.to_string(), get("stream_id")?.to_string());
        get("cpu")?;
        if !matches!(
            get("event")?,
            "bios_call" | "bios_return" | "interrupt" | "memory_write" | "device_event"
        ) {
            return Err(format!("line {line_number}: invalid event"));
        }
        if !v.get("registers").is_some_and(|r| {
            r.as_object()
                .is_some_and(|o| o.values().all(|v| v.is_string()))
        }) {
            return Err(format!("line {line_number}: invalid registers"));
        }
        let seq = v
            .get("sequence")
            .and_then(|s| s.as_u64())
            .ok_or_else(|| format!("line {line_number}: invalid sequence"))?;
        if last.get(&key).is_some_and(|prior| seq <= *prior) {
            return Err(format!(
                "line {line_number}: duplicate or out of order stream {}",
                key.1
            ));
        }
        last.insert(key, seq);
        count += 1;
    }
    if count == 0 {
        return Err("empty trace".into());
    }
    Ok(count)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn validates_example_and_rejects_empty() {
        let fixture = include_str!("../examples/trace.synthetic.jsonl");
        assert_eq!(validate_jsonl(fixture), Ok(2));
        assert!(validate_jsonl("").is_err());
        assert!(validate_jsonl("\n").is_err());
    }
    #[test]
    fn rejects_duplicate_and_bad_json() {
        let first = include_str!("../examples/trace.synthetic.jsonl")
            .lines()
            .next()
            .unwrap();
        assert!(validate_jsonl(&format!("{first}\n{first}\n")).is_err());
        assert!(validate_jsonl("{broken}").is_err());
    }
}

#[cfg(test)]
mod schema_conformance_tests {
    use super::*;
    #[test]
    fn rejects_extra_and_wrong_optional_fields() {
        let first = include_str!("../examples/trace.synthetic.jsonl")
            .lines()
            .next()
            .unwrap();
        for (key, bad) in [
            ("unexpected", serde_json::json!(true)),
            ("address", serde_json::json!(12)),
            ("details", serde_json::json!([])),
        ] {
            let mut doc: serde_json::Value = serde_json::from_str(first).unwrap();
            doc[key] = bad;
            assert!(validate_jsonl(&format!("{doc}")).is_err(), "accepted {key}");
        }
    }
    #[test]
    fn distinct_interleaved_streams_with_gaps_are_valid() {
        let first: serde_json::Value = serde_json::from_str(
            include_str!("../examples/trace.synthetic.jsonl")
                .lines()
                .next()
                .unwrap(),
        )
        .unwrap();
        let mut other = first.clone();
        other["stream_id"] = serde_json::json!("other");
        let mut third = first.clone();
        third["sequence"] = serde_json::json!(12);
        let input = format!("{first}\n{other}\n{third}\n");
        assert_eq!(validate_jsonl(&input), Ok(3));
        let reverse = format!("{third}\n{first}\n");
        assert!(validate_jsonl(&reverse).is_err());
    }
}
