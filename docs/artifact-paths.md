# File resolution security boundary (Issue #15)

Logical artifact names are **opaque** IDs and are resolved only through an
explicit caller-provided mapping to root-relative paths. Paths with absolute,
parent traversal, dot components, symlinks, nonexistent entries, and nonregular
files fail closed. Hashing is a separate function in `artifacts::verify`.

This implementation assumes the root and its children are on a **trusted,
non-adversarially-mutating filesystem** during resolution. Canonical checks
and metadata checks are **not** atomic with opening. Concurrent attacker rename
or symlink swaps are NOT guaranteed safe: consumers requiring adversarial race
resistance MUST NOT call this resolver on untrusted writable directories.
That scenario remains BLOCKED until a platform-specific handle-relative
`openat2`/equivalent API is introduced and verified separately.

The resolver makes no firmware compatibility, provenance, or license claims.
