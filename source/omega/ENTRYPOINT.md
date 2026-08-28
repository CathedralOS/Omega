# Product compiler entrypoints

[`build.omg`](build.omg) and [`main.omg`](main.omg) are the hosted product build
and machine entrypoints. The current live slice exposes the real Psi
source-to-token phase followed by fail-closed parsing of ordinary
`use path::member;` roots and basic `[pub] data` declarations with optional
`[copy]`, bare named field types, and ordered payload-free cases. Later
implementation extends that same ordinary source closure through richer data
forms, the remaining declarations, checking, terminal Psi, and Omega artifact
emission. The target-neutral implementation lives in the sibling
`source/psi/` package;
target realization and these product entrypoints live in the rest of
`source/omega/`.

The current Rust development command is the product package rooted at
`source/omega-rust/omega/`.
