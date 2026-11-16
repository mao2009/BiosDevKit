# Portable trace adapter interface (#32)

The generic `trace_ingest` adapter boundary converts platform-specific
observations into valid, deterministic **Trace Event v1 JSONL**. It is **not**
a Geargrafx, Mega-CD or Saturn implementation. Concrete emulator bindings
belong in separately reviewed adapter issues.

The core requires an explicit, public `TracePolicy` for run, CPU and stream
IDs, register keys, and numeric detail fields. Input adapters produce typed
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
