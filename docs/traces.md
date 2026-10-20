# Trace event model v1

Trace events use **one JSON object per line** (JSONL), with a monotonically increasing `sequence` in the producer stream. `cpu` is an opaque identifier (for example, `master-sh2`, `slave-sh2`, `m68k`, or `huc6280`) and no core-specific register list is hardwired into the shared schema.

The order of events must be defined by the capture adapter; inter-CPU causality cannot be inferred solely from sequence. A future comparator must distinguish missing events, reordered events and differences within normalized register sets. Only captured/defined fields should be compared; avoid inventing or normalizing unknown CPU state.

The fixture is intentionally synthetic. This PR specifies a format, not a functioning differential executor.
