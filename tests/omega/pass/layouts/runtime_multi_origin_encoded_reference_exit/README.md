A helper selects either a signed-byte-backed reference or an ordinary i64
reference carried by an owned input, then writes through the returned reference.
Both choices must preserve the selected codec, the other source, and neighboring
byte 85.
The ordinary integer starts at 257: writing 127 must clear its high byte,
which distinguishes the full-width codec from an erroneous signed-byte store.

Run `omega run --both main.omg` or
`python3 tools/corpus_gate.py --both --filter layouts/runtime_multi_origin_encoded_reference_exit`.
The checked-tree oracle is selected explicitly with `--checked-interpreter`
and `--golden tests/omega/corpus_both_macos_arm64.txt`.
