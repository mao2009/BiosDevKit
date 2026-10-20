# PCE/MCD source extraction audit — primary-source inventory

This document records a **limited primary-source inventory**. It does not authorize source migration or claim clean-clone regression success.

| Repository | Reviewed pinned main commit | Inspected source paths | License | Existing tests | Migration gate |
|---|---|---|---|---|---|
| [oss-pce-cd-bios](https://github.com/mao2009/oss-pce-cd-bios) | `d95ae8c30eddd433602361b619894cb3d5ad2f1b` | [spec/api_contracts.json](https://github.com/mao2009/oss-pce-cd-bios/blob/d95ae8c30eddd433602361b619894cb3d5ad2f1b/spec/api_contracts.json), [tests/host/test_api_contracts.py](https://github.com/mao2009/oss-pce-cd-bios/blob/d95ae8c30eddd433602361b619894cb3d5ad2f1b/tests/host/test_api_contracts.py) | [MIT LICENSE](https://github.com/mao2009/oss-pce-cd-bios/blob/d95ae8c30eddd433602361b619894cb3d5ad2f1b/LICENSE), copyright 2026 mao2009 | `tests/host/test_api_contracts.py` synthetic negative tests; Geargrafx real-core test referenced at `tests/integration/geargrafx_contracts.py` | BLOCKED: caller/dependency paths and clean-clone integration not verified |
| [oss-mcd-bios](https://github.com/mao2009/oss-mcd-bios) | `94ce0c23c6b53f2bacac0d8b9051d5f1abea33f5` | [tools/evidence/evidence.py](https://github.com/mao2009/oss-mcd-bios/blob/94ce0c23c6b53f2bacac0d8b9051d5f1abea33f5/tools/evidence/evidence.py), [tools/evidence/result.schema.json](https://github.com/mao2009/oss-mcd-bios/blob/94ce0c23c6b53f2bacac0d8b9051d5f1abea33f5/tools/evidence/result.schema.json) | [MIT LICENSE](https://github.com/mao2009/oss-mcd-bios/blob/94ce0c23c6b53f2bacac0d8b9051d5f1abea33f5/LICENSE), copyright 2026 oss-mcd-bios contributors | `tests/host/test_evidence.py` exists, execution unverified | BLOCKED: caller/dependency graph, existing test results, migration compatibility unverified |

## Confirmed design distinctions

- PCE `spec/api_contracts.json` is a large project-specific contract inventory, with a `target`, `inventory`, `source_inventory` and source provenance entries. It **does not** directly conform to BiosDevKit contract v1; a deliberate export/adapter mapping is required.
- PCE `tests/host/test_api_contracts.py` explicitly identifies synthetic negative tests. It imports `tools/diagnostics` and `tests/integration/geargrafx_contracts.py` helpers. These imports are evidence of current coupling, **not** proof the logic can be extracted unchanged.
- MCD `tools/evidence/evidence.py` uses only Python standard-library modules in its visible imports and implements collection, validation, aggregation and matrix generation; it defines PASS/FAIL/SKIP/BLOCKED with fail-closed semantics. This makes the data rules an extraction **candidate**, not automatically the Python implementation.
- MCD `tools/evidence/result.schema.json` requires checkpoint, ROM SHA, ROM commit, emulator, environment, observed evidence, log excerpt and reason. BiosDevKit Test Evidence v1 has materially different fields; adapters/versioned migration rules are needed.

## Prioritized next evidence gates

1. **Evidence mapping:** enumerate MCD `tools/evidence/evidence.py` callers and `tests/host/test_evidence.py` assertions; produce a field-by-field MCD ↔ BiosDevKit v1 mapping and semantic equivalence tests.
2. **API mapping:** inspect PCE `spec/api_contracts.json` consumers and `tools` validation call sites; specify lossless handling of PCE-specific ABI and provenance before conversion.
3. **Artifact verification:** inspect PCE artifact guards and MCD ROM validation utilities, their test dependencies and license context before extracting hashing logic.
4. **Before any migration:** record clean-clone baseline results for both repositories, compare generated evidence outputs and retain platform-specific tests.

**Status:** Partial source inspection performed; **actual extraction is BLOCKED**. No clean-clone tests or file moves performed. Issue #19 remains open until every gate is backed by reproducible evidence.
