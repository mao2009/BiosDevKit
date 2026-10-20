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
