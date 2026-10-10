# utf8_field_states

A byte-sequence domain field operates across states: `label` fills from a
string literal `"abc"` in `fill`, takes an indexed byte write
`main.label[0] = 66` in `stamp`, and reads back `main.label[0] as i32`
in `done`.

Compile-checked only: no standard-library dependency and
no `Main::exit` — run a check target, not native.
