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

/// Emit observations incrementally; the sink may reject the very next event.
/// Adapters must not pre-buffer an unbounded Vec before calling the sink.
pub trait TraceSourceAdapter {
    fn visit(
        &self,
        source: &Value,
        max_events: usize,
        emit: &mut dyn FnMut(Observation) -> Result<(), String>,
    ) -> Result<(), String>;
}

/// All names here must be curated *public* identifiers, not source filenames.
#[derive(Debug, Clone)]
pub struct TracePolicy {
    pub run_id: String,
    /// Every producer stream has exactly one CPU owner. A CPU may own
    /// multiple streams. Independent per-CPU counters require distinct streams.
    pub stream_cpu: BTreeMap<String, String>,
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
const MAX_OUTPUT_BYTES: usize = 32 * 1024 * 1024;
const MAX_POLICY_FIELDS: usize = 128;

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
        || policy.stream_cpu.is_empty()
        || policy.stream_cpu.len() > MAX_POLICY_FIELDS
        || policy
            .stream_cpu
            .iter()
            .any(|(stream, cpu)| !token(stream) || !token(cpu))
        || policy.registers.len() > MAX_POLICY_FIELDS
        || policy.numeric_details.len() > MAX_POLICY_FIELDS
        || !unique_tokens(&policy.registers)
        || !unique_tokens(&policy.numeric_details)
        || policy.max_events == 0
        || policy.max_events > 100_000
    {
        return Err("invalid trace policy".into());
    }
    // Build lookup sets once, rather than scanning policy Vecs for every
    // register/detail on every event. Their size is already capped at 128.
    let valid_registers: HashSet<&str> = policy.registers.iter().map(String::as_str).collect();
    let valid_details: HashSet<&str> = policy.numeric_details.iter().map(String::as_str).collect();

    // Stream output incrementally and stop the adapter at the event budget.
    // The input Value and adapter's internal allocations are separate concerns.
    let mut jsonl = String::new();
    let mut count = 0usize;
    let mut validate_and_store = |obs: Observation| -> Result<(), String> {
        if count >= policy.max_events {
            return Err("event limit exceeded".into());
        }
        if policy.stream_cpu.get(&obs.stream_id) != Some(&obs.cpu)
            || obs
                .registers
                .keys()
                .any(|name| !valid_registers.contains(name.as_str()))
            || obs
                .numeric_details
                .keys()
                .any(|name| !valid_details.contains(name.as_str()))
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
        let line = event.to_string();
        if jsonl.len().saturating_add(line.len()).saturating_add(1) > MAX_OUTPUT_BYTES {
            return Err("normalized trace output limit exceeded".into());
        }
        jsonl.push_str(&line);
        jsonl.push('\n');
        count += 1;
        Ok(())
    };
    // A buggy adapter might ignore an error from the sink and return Ok.
    // Never allow that to turn a rejected trace into a truncated success.
    let mut sink_rejected = false;
    let mut emit = |obs: Observation| -> Result<(), String> {
        if sink_rejected {
            return Err("trace sink already rejected input".into());
        }
        let result = validate_and_store(obs);
        if result.is_err() {
            sink_rejected = true;
        }
        result
    };
    let visit_result = adapter.visit(source, policy.max_events, &mut emit);
    if sink_rejected || visit_result.is_err() {
        return Err("trace adapter rejected input".into());
    }
    if count == 0 {
        return Err("invalid trace event count".into());
    }
    // Never forward validator diagnostics containing source-derived IDs.
    traces::validate_jsonl(&jsonl).map_err(|_| "normalized trace failed validation".to_string())?;
    Ok(jsonl)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn policy() -> TracePolicy {
        TracePolicy {
            run_id: "synthetic-run".into(),
            stream_cpu: BTreeMap::from([
                ("lane-a".into(), "cpu0".into()),
                ("lane-b".into(), "cpu1".into()),
            ]),
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
        fn visit(
            &self,
            data: &Value,
            _max: usize,
            emit: &mut dyn FnMut(Observation) -> Result<(), String>,
        ) -> Result<(), String> {
            let tick = data["tick"].as_u64().ok_or("private-path /game.cue")?;
            let mut x = observation();
            x.sequence = tick;
            emit(x)
        }
    }
    // Two CPUs: a distinct, device-centric event dialect.
    struct MultiCpu;
    impl TraceSourceAdapter for MultiCpu {
        fn visit(
            &self,
            data: &Value,
            _max: usize,
            emit: &mut dyn FnMut(Observation) -> Result<(), String>,
        ) -> Result<(), String> {
            let code = data["device"]["code"].as_u64().ok_or("bad device event")?;
            emit(Observation {
                stream_id: "lane-b".into(),
                cpu: "cpu1".into(),
                sequence: 0,
                event: EventKind::DeviceEvent,
                registers: BTreeMap::new(),
                address: None,
                numeric_details: BTreeMap::from([("device_code".into(), code)]),
            })?;
            emit(Observation {
                stream_id: "lane-a".into(),
                cpu: "cpu0".into(),
                sequence: 7,
                event: EventKind::Interrupt,
                registers: BTreeMap::from([("r0".into(), 0x1234)]),
                address: None,
                numeric_details: BTreeMap::new(),
            })
        }
    }
    struct Fixed(Vec<Observation>);
    impl TraceSourceAdapter for Fixed {
        fn visit(
            &self,
            _: &Value,
            _max: usize,
            emit: &mut dyn FnMut(Observation) -> Result<(), String>,
        ) -> Result<(), String> {
            for item in &self.0 {
                emit(item.clone())?;
            }
            Ok(())
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
        let err =
            normalize(&SingleCpu, &json!({"bad":"/private/game.cue"}), &policy()).unwrap_err();
        assert_eq!(err, "trace adapter rejected input");
        assert!(!err.contains("game"));
        let mut bad = policy();
        bad.run_id = "f".repeat(64);
        assert!(normalize(&SingleCpu, &json!({"tick":0}), &bad).is_err());
        let mut bad = policy();
        bad.stream_cpu.clear();
        assert!(normalize(&SingleCpu, &json!({"tick":0}), &bad).is_err());
        let mut bad = policy();
        bad.max_events = 0;
        assert!(normalize(&SingleCpu, &json!({"tick":0}), &bad).is_err());
        assert!(normalize(&Fixed(vec![]), &json!({}), &policy()).is_err());
    }
    #[test]
    fn independently_numbered_cpu_lanes_are_valid_but_shared_stream_is_not() {
        let first = observation();
        let mut other = observation();
        other.cpu = "cpu1".into();
        other.stream_id = "lane-b".into();
        // Independent sequence zero for both CPU streams is valid.
        let jsonl = normalize(
            &Fixed(vec![first.clone(), other.clone()]),
            &json!({}),
            &policy(),
        )
        .unwrap();
        assert_eq!(traces::validate_jsonl(&jsonl), Ok(2));
        other.stream_id = "lane-a".into();
        other.sequence = 1; // still invalid: the shared stream has a different CPU owner
        assert!(normalize(&Fixed(vec![first, other]), &json!({}), &policy()).is_err());
    }

    struct Flood(std::cell::Cell<usize>);
    impl TraceSourceAdapter for Flood {
        fn visit(
            &self,
            _: &Value,
            _max: usize,
            emit: &mut dyn FnMut(Observation) -> Result<(), String>,
        ) -> Result<(), String> {
            for n in 0..100 {
                self.0.set(n + 1);
                let mut item = observation();
                item.sequence = n as u64;
                emit(item)?;
            }
            Ok(())
        }
    }
    #[test]
    fn fails_during_emission_not_after_buffering_all_events() {
        let mut p = policy();
        p.max_events = 2;
        let flood = Flood(std::cell::Cell::new(0));
        assert!(normalize(&flood, &json!({}), &p).is_err());
        assert_eq!(flood.0.get(), 3);
    }
    #[test]
    fn allowlist_lookup_accepts_large_policy_and_rejects_unknown_fields() {
        let mut p = policy();
        p.registers = (0..MAX_POLICY_FIELDS)
            .map(|index| format!("reg{index}"))
            .collect();
        p.numeric_details = (0..MAX_POLICY_FIELDS)
            .map(|index| format!("detail{index}"))
            .collect();
        let mut item = observation();
        item.registers.clear();
        item.registers.insert("reg127".into(), 0x42);
        item.numeric_details.insert("detail127".into(), 7);
        assert_eq!(
            traces::validate_jsonl(&normalize(&Fixed(vec![item.clone()]), &json!({}), &p).unwrap()),
            Ok(1)
        );
        item.registers.insert("unknown_register".into(), 0);
        assert!(normalize(&Fixed(vec![item.clone()]), &json!({}), &p).is_err());
        item.registers.remove("unknown_register");
        item.numeric_details.insert("unknown_detail".into(), 0);
        assert!(normalize(&Fixed(vec![item]), &json!({}), &p).is_err());
    }
    struct IgnoreRejection;
    impl TraceSourceAdapter for IgnoreRejection {
        fn visit(
            &self,
            _: &Value,
            _max: usize,
            emit: &mut dyn FnMut(Observation) -> Result<(), String>,
        ) -> Result<(), String> {
            let _ = emit(observation());
            let mut wrong = observation();
            wrong.cpu = "not_registered".into();
            wrong.sequence = 1;
            let _ = emit(wrong); // ignored by a buggy adapter
            Ok(())
        }
    }
    #[test]
    fn ignored_sink_rejection_does_not_return_partially_valid_trace() {
        assert!(normalize(&IgnoreRejection, &json!({}), &policy()).is_err());
    }

    struct IgnoreOverflow;
    impl TraceSourceAdapter for IgnoreOverflow {
        fn visit(
            &self,
            _: &Value,
            _max: usize,
            emit: &mut dyn FnMut(Observation) -> Result<(), String>,
        ) -> Result<(), String> {
            for sequence in 0..3 {
                let mut item = observation();
                item.sequence = sequence;
                let _ = emit(item);
            }
            Ok(())
        }
    }
    #[test]
    fn ignored_event_limit_is_never_a_successful_truncated_trace() {
        let mut p = policy();
        p.max_events = 2;
        assert!(normalize(&IgnoreOverflow, &json!({}), &p).is_err());
    }

}
