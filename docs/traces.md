# Trace event model v1

Trace events use **one JSON object per line** (JSONL), with a monotonically increasing `sequence` in the producer stream. `cpu` is an opaque identifier (for example, `master-sh2`, `slave-sh2`, `m68k`, or `huc6280`) and no core-specific register list is hardwired into the shared schema.

The order of events must be defined by the capture adapter; inter-CPU causality cannot be inferred solely from sequence. A future comparator must distinguish missing events, reordered events and differences within normalized register sets. Only captured/defined fields should be compared; avoid inventing or normalizing unknown CPU state.

The fixture is intentionally synthetic. This PR specifies a format, not a functioning differential executor.

Each event now carries nonempty `run_id` and `stream_id`. `tests/trace-schema.py` rejects non-increasing sequence values within each `(run_id, stream_id)` pair; missing sequence values are permitted because captures may be partial. The check cannot prove completeness or causal ordering across independent CPU streams. A future platform adapter must define stable CPU IDs, interrupt origins, address spaces and ordering/correlation semantics.
