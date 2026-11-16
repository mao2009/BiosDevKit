# Architecture-neutral contract import (#30)

The `contract_import` module is a **candidate metadata converter**. Adapters
extract an intermediate `Candidate` from their own source dialects. The core
validates it and writes API Contract v1, with importer metadata isolated in the
existing `platform_extension.biosdevkit_import` object.

The input `SourcePin` contains a caller-asserted immutable revision and public
source file SHA-256. Import does **not** download sources, verify the SHA against
a file, authenticate a rights decision, or observe an emulator. Upstream audit
and rights approval must be completed outside this function before using a
real source. In particular, a syntactically valid pin is not proof of origin.

Every imported API is a **candidate**, even if the upstream adapter reports
`CLAIMED_VERIFIED`. The original claim is retained as `source_claim`,
distinct from importer `verification=UNVERIFIED`. Requested execution or
release eligibility is rejected, not silently accepted or upgraded.

The importer retains contract fields (`id`, `name`, `entry_point`,
`inputs`, `outputs`, `side_effects`) and tracks unsupported source fields
under `unknown_source_fields`. Adapters must surface unsupported material
there, not fabricate values. CPU-specific meanings and syntaxes are opaque to
the core. The two unit-test source dialects demonstrate 8-bit registers and
32-bit multicore registers through the same path.

## Scope boundaries

- This is **not** a PC Engine, Mega-CD or Saturn adapter. Real-source
  integration belongs in a follow-up Issue after #19 and #20.
- No genuine BIOS ABI compatibility, proprietary binary processing, or
  reference BIOS execution is established by this mapping.
- Version 1 does not establish externally authenticated contract status;
  consumers must never elevate imported candidate metadata into approval.
- Adapter errors are reduced to fixed text at the core boundary, since raw
  source parsing errors could contain private information.
