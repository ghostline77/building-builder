# Building builder

Work from this repository root. Building layouts, kit rules, composition and UI
belong here. Native decoding/writing belongs in sibling `ghostline-red`; shared
geometry, materials and GPU rendering belong in sibling `renderer`.

Game files are read-only. Generated previews, exports and local projects go in
ignored `work/`. Keep authored examples and measured library data separate.
Never claim native serialization proves runtime collision or navigation.

Use Codescope to inspect complete functions. Format only this package's sources,
not sibling workspaces. Check with `cargo test --locked` and
`cargo clippy --all-targets --locked -- -D warnings` after implementation.
Inspect representative GPU previews and native export readbacks.
