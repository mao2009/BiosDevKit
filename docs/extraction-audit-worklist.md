# PCE/MCD source extraction audit — evidence collection worklist

Status: **BLOCKED: primary-source line-level audit not yet performed**. This file is not authorization to migrate any source file. The planning matrix in `docs/extraction-audit.md` is not evidence of reusable code.

| Target | Pinned commit | Exact source path | License evidence | Callers/dependencies | Baseline test | Gate |
|---|---|---|---|---|---|---|
| oss-pce-cd-bios | NOT VERIFIED | NOT VERIFIED | NOT VERIFIED | NOT VERIFIED | NOT VERIFIED | BLOCKED |
| oss-mcd-bios | NOT VERIFIED | NOT VERIFIED | NOT VERIFIED | NOT VERIFIED | NOT VERIFIED | BLOCKED |

Source code movement requires exact reviewed paths/commits from **both** projects, license compatibility, tests in clean clones and a separately reviewed migration PR. Platform-specific emulator adapters must remain in their repositories.

This is a traceable worklist only; do not close #19 as completed until both repository rows are evidence-backed.
