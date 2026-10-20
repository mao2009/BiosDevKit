# Differential trace comparator (Issue #5)

`first_difference` accepts two **independently validated** Trace Event v1 JSONL
inputs, and revalidates each through the existing Rust validator. It compares
normalized event records per `(run_id, stream_id, cpu)` lane, in that lane's
input order. `sequence` and `run_id` are ignored for payload equivalence;
the run identifier is nevertheless used to pair lanes, so callers must first
normalize run IDs across captures or comparison yields missing/extra lanes.

Classification: absent expected lane/index = extra, absent actual = missing;
cross-position matching in the same lane = reordered, otherwise changed.
The first mismatch is deterministic by sorted lane key, then input index.
This is a conservative heuristic, not full edit-distance alignment and not
evidence of causality across CPU streams. Shared counters across CPUs do not
imply happens-before. Distinct event payloads or unsupported versions fail
closed via the trace validator. Golden hardware comparison is out of scope.

Future follow-up: configurable identity-normalization adapter and robust
insertion/deletion alignment; cross-platform timing and SH-2 dual-CPU
causality require separate proof.
