# Cross-platform evidence import (#31)

This is an architecture-neutral, **allowlist-only** bridge from heterogeneous
runner outputs into Test Evidence v1. A platform adapter extracts typed
`SourceRecord` values; the core maps only registered test IDs, platform IDs,
pinned source revisions, runner versions and explicitly permitted **numeric**
metrics. Free-form source metadata, filenames, paths, ROM/CD identifiers and
human-readable error messages are discarded rather than sanitized by guessing.

The trusted catalog is registered by the caller. Exact revisions and runner
versions are checked against the records, but **no cryptographic attestation
or independent execution occurs here**. Output observations explicitly say
`identity_trust=DECLARED_NOT_ATTESTED` and identify the declared execution
scope (synthetic/host/emulator/hardware). No combination of catalog and
runner-reported PASS proves genuine BIOS compatibility.

Test Evidence v1 cannot encode `NOT_RUN` without conflating it with `SKIP`.
The importer therefore returns an explicit typed `NotRun` variant instead of
fabricating a v1 PASS/SKIP record. The caller must handle this separately;
a future schema version could serialize it unambiguously.

Only fixed reason codes are exported. The source adapter's error strings are
replaced at the importer boundary, preventing raw private content from entering
diagnostics. The allowlist approach intentionally cannot export arbitrary
memory dumps or textual observations. Any richer observation type requires
a separately reviewed, explicit safe encoding.

This module is independent of PC Engine, Mega-CD, Saturn and their emulators.
Machine-specific adapters are later work after the source audit (#19).
