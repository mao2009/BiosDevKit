# Portable trace adapter interface (#32)

The generic `trace_ingest` adapter boundary converts platform-specific
observations into valid, deterministic **Trace Event v1 JSONL**. It is **not**
a Geargrafx, Mega-CD or Saturn implementation. Concrete emulator bindings
belong in separately reviewed adapter issues.

The core requires an explicit, public `TracePolicy` for run, CPU and stream
IDs, register keys, and numeric detail fields. The CPU/stream binding is a
bounded map; register/detail allowlists are indexed once into hash sets before
event processing (at most 128 names of each type), avoiding per-event scans.
Input adapters produce typed
`Observation` values with integer registers/addresses, avoiding free-form
logs, filenames, ROM/media hashes and raw byte strings. The normalized output
retains only allowlisted fields. Adapter errors and downstream validator
errors are replaced with fixed messages to prevent source-data disclosure.

The common core formats numeric values as canonical uppercase hexadecimal
for registers and addresses; it deliberately does not assign CPU bit widths
or guess MPR/bank/stack semantics. Device detail numbers remain JSON integers.
CPU-specific representation/width handling belongs in the adapter.

Events retain source order; sequence monotonicity is validated by the
existing `traces::validate_jsonl` on each `(run_id, stream_id)` stream. Gaps
and interleaved streams are valid. **Cross-CPU happens-before relations are
not inferred** from this representation. Missing/unobserved events must not
be fabricated by an adapter.

Two synthetic, structurally different dialects (single CPU instruction hook
and dual CPU device stream) test the same core. No proprietary BIOS data,
commercial media, actual emulator execution or ABI-verification claim is
involved. The explicit public policy is still caller-supplied, not a
cryptographic attestation of source identity.

## Stream ownership and sequence semantics

**One stream is owned by exactly one CPU/producer**, declared in
`TracePolicy.stream_cpu`. Multiple streams may belong to the same CPU;
a CPU's independent sequence counter must never be multiplexed into another
CPU's stream. The Trace Event v1 validator tests monotonic sequence within
`(run_id, stream_id)` only, **not per CPU**. Two CPUs can legitimately both
start at sequence 0 if they use **different streams**. The adapter rejects a
CPU/stream mismatch even when its sequence number would otherwise increase.
This rule is local to this adapter; it does not silently change Trace Event
v1 or other producers. Cross-CPU ordering still cannot be inferred.

## Resource budget

`TraceSourceAdapter::visit` emits one event at a time through a fallible
sink, respecting `max_events`. The core rejects the first excess event,
limits JSONL output to 32 MiB and checks registered stream/CPU identifiers.
Adapters must stop on sink errors and must not build unbounded intermediate
`Vec<Observation>` collections.

The source argument is an already-materialized JSON `Value`. The normalizer
cannot bound the upstream parse or arbitrary internal allocations in an
adapter. External log readers need independent byte/depth limits before JSON
decoding; the typed adapter boundary is **not** a complete sandbox for
untrusted parsers.
