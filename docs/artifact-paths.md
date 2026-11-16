# File resolution security boundary (Issue #15)

Logical artifact names are opaque IDs and must be explicitly mapped to root-relative
normal path components. Traversal, absolute paths, missing objects, symlinks and
nonregular files are rejected.

On **Linux x86_64/aarch64**, the resolver uses `openat2` to acquire an open
root directory handle with `RESOLVE_NO_SYMLINKS`, then opens the mapped file
relative to that handle with `RESOLVE_BENEATH | RESOLVE_NO_SYMLINKS`.
The returned open `File` handle must be passed directly to the hash verifier;
never reconstruct the path and reopen it. Kernel or filesystem rejection,
including unavailable `openat2`, fails closed.

On **Windows, macOS and other platforms**, this version returns an explicit
unsupported error: safe handle-relative traversal is not yet implemented.
This deliberately trades availability for safety and **does not claim
cross-platform resolution support**. Add a separately reviewed safe Windows
reparse-point/junction traversal and other platform implementations before
removing that gate.

The opened root directory is the trust anchor. When a writable ancestor
changes during open, the selected directory can change, but subsequent lookup
cannot escape the acquired root handle. Hard-linked files inside the root are
not distinguishable as imported external content; the root's ownership,
content provenance and authorized writers remain a separate policy obligation.

Hashing remains the responsibility of `artifacts::verify`. This resolver makes
no firmware-compatibility, provenance-authentication or license claims.
