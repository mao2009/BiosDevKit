# Platform ABI adapters — synthetic contract examples

The shared API contract treats `entry_point` as an opaque symbol; do not
infer an executable CPU address. An ABI adapter must explicitly map each
logical register to a CPU register and bit width, identify input/output direction,
declare flags preserved/clobbered and both stack and return-state semantics.
Missing mapping entries or unknown fields are schema errors.

Examples are **synthetic**, intentionally not extracted from System Card,
Mega-CD ROM, or any emulator's behavior. PCE (`huc6280`) and MCD (`m68000`)
are distinct platform/architecture pairs. Example register and flag names are
illustrative **only**, not claims about a particular BIOS API. Even a
schema-valid adapter does not establish that a callee preserves flags,
restores stack or returns correctly. A verified contract needs executed
reference-ROM ABI evidence under a platform-specific harness.

For future Sega Saturn adapters, separately version the mapping to represent
master/slave SH-2 CPU identity, 68k sound processor, interrupt/stack context
and cross-CPU ordering. Never infer dual-CPU causal order from a single
synthetic sequence or silently coerce an SH-2 mapping into HuC6280/m68k.

Validate fixtures: `python3 tests/abi-adapter-schema.py`. This PR
does not connect emulators or expose a hardware execution API.
