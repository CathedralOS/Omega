A returned signed-byte-backed reference is captured in an owned record and
forwarded from its reference field to a writing helper. Both writes retain the
original byte endpoint and physical codec: the intermediate logical read is
-128, the final byte is 127, and neighboring byte 85 is unchanged.

Run `omega run --both main.omg`, or
`python3 tools/corpus_gate.py --both --filter layouts/runtime_stored_returned_encoded_reference_exit`.
The independent checked-tree source oracle uses `--checked-interpreter
--golden tests/omega/corpus_both_macos_arm64.txt` on that corpus command.
