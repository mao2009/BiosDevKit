# Differential trace comparator (Issue #5)

`first_difference` accepts two **independently validated** Trace Event v1 JSONL
inputs, and revalidates each through the existing Rust validator. It compares
normalized event records per `(run_id, stream_id, cpu)` lane, in that lane's
input order. `sequence` and `run_id` are ignored for payload equivalence;
the run identifier is nevertheless used to pair lanes. Captures with different
run ID sets or `(run_id, stream_id, cpu)` lane sets now fail with a clear
error; callers must explicitly normalize and pair captures before comparing.

Classification: ordered-subsequence alignment of normalized remaining events
recognizes one or multiple pure insertions/deletions as extra/missing, including
duplicated payloads. Adjacent pair swaps with identical remaining suffix are
reordered; a single event substitution with unchanged suffix is changed.
Mixed or ambiguous edits return an explicit error rather than guessing.
This comparator reports the first difference, not a complete edit script.
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
