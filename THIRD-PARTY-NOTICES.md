# Third-party notices

The XML reader/writer uses **xml-rs 0.8.29**, licensed under MIT.
The dependency version and checksum are pinned in Cargo.lock.
The upstream license is reproduced in `licenses/xml-rs-MIT.txt` and must accompany redistributions of this dependency.

Rocket MSI itself is licensed under MIT; see `LICENSE`. Third-party components retain their respective licenses and notices.

The optional Tauri GUI has additional dependencies. See `builder/THIRD-PARTY-NOTICES.md`, `builder/Cargo.lock`, and `builder/third-party-dependencies.json`.

Windows executable icon resources are compiled at build time with **tauri-winres 0.3.6** (MIT OR Apache-2.0) and its dependencies. These are also included in the portable distribution's dependency inventory and license notices produced by `scripts/build-builder.ps1`.
