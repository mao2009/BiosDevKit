# Differential trace comparator (Issue #5)

`first_difference` accepts two **independently validated** Trace Event v1 JSONL
inputs, and revalidates each through the existing Rust validator. It compares
normalized event records per `(run_id, stream_id, cpu)` lane, in that lane's
input order. `sequence` and `run_id` are ignored for payload equivalence;
the run identifier is nevertheless used to pair lanes. Captures with different
run ID sets now fail with a clear error; callers must explicitly normalize
run IDs before comparing different runs.

Classification: exact single-event insertion/deletion alignment across the
remaining suffix = extra/missing (also with duplicated payloads); adjacent
pair swap with identical remaining suffix = reordered; otherwise changed.
This conservative first-difference comparator does not implement a general
edit script for multiple simultaneous insertions or deletions.
The first mismatch is deterministic by sorted lane key, then input index.
This is a conservative heuristic, not full edit-distance alignment and not
evidence of causality across CPU streams. Shared counters across CPUs do not
imply happens-before. Distinct event payloads or unsupported versions fail
closed via the trace validator. Golden hardware comparison is out of scope.

Future follow-up: configurable identity-normalization adapter and robust
insertion/deletion alignment; cross-platform timing and SH-2 dual-CPU
causality require separate proof.

Equal-length differences are classified `Changed` only when a single payload
at the first differing index changes and the rest of the lane is identical.
Composite or ambiguous equal-length edits return an error instead of guessing.
