# Test Evidence v2 — declared execution provenance

This is an **explicitly versioned** companion to Test Evidence v1. The v1 schema,
fixtures and existing readers remain unchanged. Consumers MUST dispatch by
`schema_version`; they MUST NOT silently coerce a v1 record into v2.

The required `run_provenance` object contains runner version, a 40-hex immutable
source commit declaration, OS/architecture with optional string metadata, RFC3339
start/end timestamps, and distinct artifact/trace URI-like reference strings.
All fields are **untrusted declarations** until independently checked against
a pinned checkout, runner attestation and content-addressed artifact hashes.
An ISO timestamp is not trusted time; a syntactically valid SHA is not proof of
source identity. No external links are fetched or authenticated here.

Status semantics remain fail-closed: only explicit PASS counts as PASS; FAIL,
BLOCKED and SKIP require a non-whitespace reason. The synthetic v2 example
is BLOCKED, not evidence of hardware or BIOS compatibility.

Migration: v1 producers continue producing v1. To emit v2, the producer must
supply all required provenance fields and explicitly set `schema_version=2`.
If fields are unknown, do not invent them or upgrade; keep v1 or BLOCKED.
Cross-field ordering of timestamps and true artifact provenance require a
future semantic validator/attestation adapter, not JSON Schema alone.

Check: `python3 tests/evidence-provenance-v2.py`.
