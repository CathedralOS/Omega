# Product compiler entrypoints

[`build.omg`](build.omg) and [`main.omg`](main.omg) are the hosted product build
and machine entrypoints. The first versioned checkpoint exposes the real Psi
source-to-token phase; later checkpoints extend that same closure through
terminal Psi and Omega artifact emission. The target-neutral implementation
lives under `source/psi/`; target realization and these product entrypoints live
under `source/omega/`.

The current Rust development CLI lives with its migration/reference producer at
`source/on-ramp/rust/apps/omega-cli/`.
