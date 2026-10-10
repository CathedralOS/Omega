# dependent_subtract

The relational subtraction rule: `main.count - main.j` under
`requires main.j <= main.count` proves non-negative in EXACT u32 — the
dependent atom gives `count - j >= 0` (capacity-minus-used). A
`data Main where count <= 11` premise keeps the fact standing.

Compile-checked only: no standard-library dependency and
no `Main::exit` — run a check target, not native.
