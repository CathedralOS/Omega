A returned reference selects between independently reborrowed encoded-byte and
ordinary-i64 parents. Its write must preserve the actual endpoint and codec;
after the child ends, both original parameters become readable again. The i64
starts at 257 so writing 127 must clear its high byte. Neighboring byte 85 stays
unchanged.

Run `omega run --both main.omg` or
`python3 tools/corpus_gate.py --both --filter layouts/runtime_selected_reference_parent_restoration_exit`.
Select the independent checked-tree oracle with `--checked-interpreter` and
`--golden tests/omega/corpus_both_macos_arm64.txt`.
