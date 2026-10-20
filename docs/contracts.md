# API contracts v1

`schemas/api-contract-v1.schema.json` describes machine-readable API call contracts. The core fields are platform-neutral; emulator-specific register/ABI conventions must live in `platform_extension` and adapter-specific documents.

`entry_point` is an opaque symbolic string, **not** a universal CPU address. Consumers must resolve it via a platform adapter. No emulator execution or semantic validator is implemented in this PR. Schema validation alone cannot prove that the BIOS implementation behaves correctly.

Rules for future validators: reject duplicate `(platform, api.id)` entries; reject unknown schema versions; avoid implicit register defaults; store test evidence separately from contract claims.

The fixture under `examples/` is synthetic, not a hardware compatibility claim.
