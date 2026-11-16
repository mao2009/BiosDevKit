//! Portable trace-adapter boundary. No CPU, emulator or bus assumptions.
//! Only typed, allowlisted data reaches Trace Event v1 JSONL.
use crate::traces;
use serde_json::{json, Map, Value};
use std::collections::{BTreeMap, HashSet};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EventKind {
    BiosCall,
    BiosReturn,
    Interrupt,
    MemoryWrite,
    DeviceEvent,
}
impl EventKind {
    fn name(self) -> &'static str {
        match self {
            Self::BiosCall => "bios_call",
            Self::BiosReturn => "bios_return",
            Self::Interrupt => "interrupt",
            Self::MemoryWrite => "memory_write",
            Self::DeviceEvent => "device_event",
        }
    }
}

/// Values are numbers, not raw source strings (paths, media names or logs).
/// The adapter decides how physical registers and addresses are observed.
#[derive(Debug, Clone)]
pub struct Observation {
    pub stream_id: String,
    pub cpu: String,
    pub sequence: u64,
    pub event: EventKind,
    pub registers: BTreeMap<String, u64>,
    pub address: Option<u64>,
    pub numeric_details: BTreeMap<String, u64>,
}

pub trait TraceSourceAdapter {
    fn extract(&self, source: &Value) -> Result<Vec<Observation>, String>;
}

/// All names here must be curated *public* identifiers, not source filenames.
#[derive(Debug, Clone)]
pub struct TracePolicy {
    pub run_id: String,
    pub streams: Vec<String>,
    pub cpus: Vec<String>,
    pub registers: Vec<String>,
    pub numeric_details: Vec<String>,
    pub max_events: usize,
}

fn token(s: &str) -> bool {
    !s.is_empty()
        && s.len() <= 64
        && s.bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'_' | b'-' | b'.'))
        // Reject digest-looking values, which can identify private media.
        && !(matches!(s.len(), 40 | 64) && s.bytes().all(|b| b.is_ascii_hexdigit()))
}
fn unique_tokens(entries: &[String]) -> bool {
    let mut seen = HashSet::new();
    entries.iter().all(|s| token(s) && seen.insert(s))
}
fn hex(v: u64) -> String {
    format!("0x{v:X}")
}

/// Convert one source dialect into deterministic Trace Event v1 JSONL.
///
/// Ordering is retained, never sorted or inferred across processors.
/// The existing v1 validator enforces per-(run_id,stream_id) monotonicity.
pub fn normalize<A: TraceSourceAdapter>(
    adapter: &A,
    source: &Value,
    policy: &TracePolicy,
) -> Result<String, String> {
    if !token(&policy.run_id)
        || policy.streams.is_empty()
        || policy.cpus.is_empty()
        || !unique_tokens(&policy.streams)
        || !unique_tokens(&policy.cpus)
        || !unique_tokens(&policy.registers)
        || !unique_tokens(&policy.numeric_details)
        || policy.max_events == 0
        || policy.max_events > 100_000
    {
        return Err("invalid trace policy".into());
    }
    // Adapter parse errors can include raw private content. Never surface them.
    let observations = adapter
        .extract(source)
        .map_err(|_| "trace adapter rejected input".to_string())?;
    if observations.is_empty() || observations.len() > policy.max_events {
        return Err("invalid trace event count".into());
    }

    let mut lines = Vec::with_capacity(observations.len());
    for obs in observations {
        if !policy.streams.contains(&obs.stream_id)
            || !policy.cpus.contains(&obs.cpu)
            || obs.registers.keys().any(|name| !policy.registers.contains(name))
            || obs
                .numeric_details
                .keys()
                .any(|name| !policy.numeric_details.contains(name))
        {
            return Err("trace contains unregistered identity or field".into());
        }
        let registers: Map<String, Value> = obs
            .registers
            .iter()
            .map(|(name, value)| (name.clone(), json!(hex(*value))))
            .collect();
        let mut event = json!({
            "schema_version": 1,
            "run_id": policy.run_id,
            "stream_id": obs.stream_id,
            "cpu": obs.cpu,
            "sequence": obs.sequence,
            "event": obs.event.name(),
            "registers": registers,
        });
        if let Some(addr) = obs.address {
            event["address"] = json!(hex(addr));
        }
        if !obs.numeric_details.is_empty() {
            let details: Map<String, Value> = obs
                .numeric_details
                .iter()
                .map(|(name, value)| (name.clone(), json!(*value)))
                .collect();
            event["details"] = Value::Object(details);
        }
        lines.push(event.to_string());
    }
    let jsonl = format!("{}\n", lines.join("\n"));
    // Never forward validator diagnostics containing source-derived IDs.
    traces::validate_jsonl(&jsonl)
        .map_err(|_| "normalized trace failed validation".to_string())?;
    Ok(jsonl)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn policy() -> TracePolicy {
        TracePolicy {
            run_id: "synthetic-run".into(),
            streams: vec!["lane-a".into(), "lane-b".into()],
            cpus: vec!["cpu0".into(), "cpu1".into()],
            registers: vec!["a".into(), "r0".into()],
            numeric_details: vec!["bytes".into(), "device_code".into()],
            max_events: 10,
        }
    }
    fn observation() -> Observation {
        Observation {
            stream_id: "lane-a".into(),
            cpu: "cpu0".into(),
            sequence: 0,
            event: EventKind::BiosCall,
            registers: BTreeMap::from([("a".into(), 42)]),
            address: Some(4096),
            numeric_details: BTreeMap::new(),
        }
    }
    // One CPU: a compact instruction-hook dialect.
    struct SingleCpu;
    impl TraceSourceAdapter for SingleCpu {
        fn extract(&self, data: &Value) -> Result<Vec<Observation>, String> {
            let tick = data["tick"].as_u64().ok_or("private-path /game.cue")?;
            let mut x = observation();
            x.sequence = tick;
            Ok(vec![x])
        }
    }
    // Two CPUs: a distinct, device-centric event dialect.
    struct MultiCpu;
    impl TraceSourceAdapter for MultiCpu {
        fn extract(&self, data: &Value) -> Result<Vec<Observation>, String> {
            let code = data["device"]["code"].as_u64().ok_or("bad device event")?;
            Ok(vec![
                Observation {
                    stream_id: "lane-b".into(),
                    cpu: "cpu1".into(),
                    sequence: 0,
                    event: EventKind::DeviceEvent,
                    registers: BTreeMap::new(),
                    address: None,
                    numeric_details: BTreeMap::from([("device_code".into(), code)]),
                },
                Observation {
                    stream_id: "lane-a".into(),
                    cpu: "cpu0".into(),
                    sequence: 7,
                    event: EventKind::Interrupt,
                    registers: BTreeMap::from([("r0".into(), 0x1234)]),
                    address: None,
                    numeric_details: BTreeMap::new(),
                },
            ])
        }
    }
    struct Fixed(Vec<Observation>);
    impl TraceSourceAdapter for Fixed {
        fn extract(&self, _: &Value) -> Result<Vec<Observation>, String> {
            Ok(self.0.clone())
        }
    }

    #[test]
    fn two_source_dialects_share_normalization_and_validation() {
        let one = normalize(&SingleCpu, &json!({"tick":0}), &policy()).unwrap();
        let two = normalize(&MultiCpu, &json!({"device":{"code":7}}), &policy()).unwrap();
        assert_eq!(traces::validate_jsonl(&one), Ok(1));
        assert_eq!(traces::validate_jsonl(&two), Ok(2));
        let first: Value = serde_json::from_str(one.lines().next().unwrap()).unwrap();
        assert_eq!(first["registers"]["a"], "0x2A");
        assert_eq!(first["address"], "0x1000");
        let second: Value = serde_json::from_str(two.lines().next().unwrap()).unwrap();
        assert_eq!(second["details"]["device_code"], 7);
        assert_eq!(second["cpu"], "cpu1");
        assert!(!one.contains("cue"));
    }
    #[test]
    fn rejects_misordered_duplicates_and_unknown_fields() {
        let mut a = observation();
        a.sequence = 9;
        let mut b = observation();
        b.sequence = 8;
        assert!(normalize(&Fixed(vec![a.clone(), b.clone()]), &json!({}), &policy()).is_err());
        b.sequence = 9;
        assert!(normalize(&Fixed(vec![a.clone(), b.clone()]), &json!({}), &policy()).is_err());
        b.sequence = 10;
        b.stream_id = "unregistered".into();
        assert!(normalize(&Fixed(vec![a.clone(), b]), &json!({}), &policy()).is_err());
        a.registers.insert("private/path.rom".into(), 1);
        assert!(normalize(&Fixed(vec![a]), &json!({}), &policy()).is_err());
    }
    #[test]
    fn fails_closed_for_private_adapter_errors_and_bad_policy() {
        let err = normalize(&SingleCpu, &json!({"bad":"/private/game.cue"}), &policy())
            .unwrap_err();
        assert_eq!(err, "trace adapter rejected input");
        assert!(!err.contains("game"));
        let mut bad = policy();
        bad.run_id = "f".repeat(64);
        assert!(normalize(&SingleCpu, &json!({"tick":0}), &bad).is_err());
        let mut bad = policy();
        bad.streams = vec!["lane-a".into(), "lane-a".into()];
        assert!(normalize(&SingleCpu, &json!({"tick":0}), &bad).is_err());
        let mut bad = policy();
        bad.max_events = 0;
        assert!(normalize(&SingleCpu, &json!({"tick":0}), &bad).is_err());
        assert!(normalize(&Fixed(vec![]), &json!({}), &policy()).is_err());
    }
}
