# BiosDevKit

Open-source BIOS development and compatibility testing toolkit.

BiosDevKit aims to provide architecture-neutral tooling for BIOS contract validation, firmware artifact provenance, test evidence, and differential trace comparison.

## Targets

- PC Engine CD-ROM² / Super CD-ROM² (planned adapter)
- Sega Mega-CD (planned adapter)
- Sega Saturn (planned adapter)

This project does **not** bundle proprietary BIOS images, commercial game ROMs, or copyrighted firmware data. A test passing does not imply system-wide compatibility.

## Roadmap

1. Establish Rust CLI and continuous integration.
2. Define machine-readable, versioned BIOS API contracts.
3. Add firmware artifact hashes and test evidence reports.
4. Add reference-vs-implementation trace comparisons.
5. Build per-platform adapters without placing platform specifics in the core.

## Contributing

See upcoming CONTRIBUTING.md. Prefer one focused issue and one draft PR per change; test and review before merging.

## Status

Early bootstrap. No BIOS compatibility claims are made.
