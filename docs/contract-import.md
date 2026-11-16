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

The importer retains approved contract fields (`id`, `name`,
`entry_point`, `inputs`, `outputs`, `side_effects`) and tracks
approved unsupported source field *names* under `unknown_source_fields`.
Adapters must surface unsupported material there, not fabricate values.

**Source data is not an authorization to publish.** The independent caller
must provide a `SourcePin.publication: PublicationPolicy` of specifically
approved public API IDs, names, entry points, register names/descriptions,
side effects, and unknown-field names. Every source-derived value must
match an approved entry; nested register structures and oversized/non-numeric
register widths are rejected rather than passed through. The core additionally
rejects path-like entry points and slash-containing descriptions, media file
extensions, and 40/64-character opaque IDs. Unapproved candidates cause
the *entire* import to fail, even if an adapter ignores the sink error.
No partial contract is returned.

Only a separately reviewed public catalog is a trusted publication authority:
the importer cannot independently confirm the catalog author, identify a
secret embedded in an otherwise public-looking label, or authenticate
license/ABI evidence. Real source adapters must keep their catalogs separate
from untrusted source data. CPU-specific meanings and syntaxes remain opaque
to the core. Unit tests cover 8-bit and 32-bit synthetic dialects and
negative paths, 64-character hashes, nested values and ignored sink errors.

## Scope boundaries

- This is **not** a PC Engine, Mega-CD or Saturn adapter. Real-source
  integration belongs in a follow-up Issue after #19 and #20.
- No genuine BIOS ABI compatibility, proprietary binary processing, or
  reference BIOS execution is established by this mapping.
- Version 1 does not establish externally authenticated contract status;
  consumers must never elevate imported candidate metadata into approval.
- Adapter errors are reduced to fixed text at the core boundary, since raw
  source parsing errors could contain private information.

## Resource budget and adapter contract

`SourceAdapter::visit` emits candidates one at a time through a fallible sink.
The core stops on the first rejected emission, including the **4,096th + 1**
candidate, and checks per-candidate register, side-effect and unknown-field
counts. Adapters receive the budget and **must stop on sink errors**; they must
not construct an unbounded `Vec<Candidate>` before emitting. The limiter
constrains generated output, not a malicious adapter implementation.

**Input parsing is a separate trust boundary:** this API accepts an already
materialized `serde_json::Value`. It cannot cap memory already allocated
by a caller reading a large file, nor arbitrary transient allocations inside
an adapter or a single nested register value. Public source readers must
apply byte, nesting and field-size limits **before** decoding JSON. These
limits are not yet provided by this module; do not expose it directly to
unbounded input sources.
