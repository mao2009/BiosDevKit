# PCE CD / Mega-CD extraction audit (initial inventory)

## Scope and verification limits

This document defines a **candidate inventory**, not a completed line-by-line source audit. Before moving code, record exact source paths, license status, runtime dependencies, current tests and API consumers from both repositories. Do not equate a similar feature name with reusable implementation.

## Candidate matrix

| Candidate | Shared core | Platform adapter | Migration gate |
|---|---|---|---|
| API contract format | Schema/versioning | Register names, ABI, entry-point mapping | Existing PCE contract tests retain semantics |
| Evidence reporting | Result states, metadata, revision | Hardware/emulator evidence producer | No PASS inferred from SKIP/BLOCKED |
| Artifact verification | Streaming hashing, reproducible manifests | Firmware layout, build policies | No proprietary artifacts committed |
| Trace comparison | JSONL parser, event differ | HuC6280 vs M68K/Z80 vs SH-2 mapping | Real traces tested and provenance recorded |
| Test orchestration | Structured execution result | Emulator launch and fixtures | Run against clean clone per target |
| CD media checks | Generic track descriptors only | Console CD controllers and boot formats | Real-disc validation stays platform-owned |

## Boundaries

- `oss-pce-cd-bios`: HuC6280, MPR, PCE BIOS calls, System Card ROM layout, Geargrafx integration.
- `oss-mcd-bios`: 68000/Z80, Mega-CD BIOS ABI and CD/gate-array integration.
- Future Saturn adapter: dual SH-2 and hardware event ordering; do not assume single-CPU execution.
- `BiosDevKit`: architecture-neutral schemas, evidence ingestion, artifact hash policy, comparator engines.

## Migration checklist

1. Capture exact file/source path, current revision and license for each proposed extraction.
2. Identify command-line inputs, output formats and external emulator contracts.
3. Add synthetic unit tests in BiosDevKit; preserve current project-specific integration tests.
4. Integrate via versioned release or pinned revision, not unreviewed copies.
5. Run clean-clone tests before replacing original code; maintain compatibility claims only with measured evidence.

**Status:** Planning-only. No code has been moved from either BIOS repository.
