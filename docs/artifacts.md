# Artifact manifest v1

Manifests record hashes and metadata, never proprietary BIOS images. The synthetic fixture represents an **empty byte sequence**, whose SHA-256 digest is public and deterministic.

A future verifier must stream file bytes, calculate SHA-256 and size, and compare to the manifest. It must reject duplicate logical names, unsafe paths, mismatch, missing artifacts, and symlink escape when resolving manifest entries. `logical_name` is a label, not a filesystem path.

`origin` is self-reported metadata and does **not** establish copyright ownership or provenance. Hash equality does not prove legality or full platform compatibility. No verifier CLI is implemented in this PR.
