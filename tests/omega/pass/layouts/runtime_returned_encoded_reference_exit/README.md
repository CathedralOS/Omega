A helper reborrows and returns its signed-byte-backed `&mut i64` parameter. A caller forwards
the returned reference to a writing helper. The two invocations update byte 0 from
0 to -128 and then 127 without changing neighboring byte 1 (85). A logical read
between the writes must decode the stored minimum as -128, not raw byte 128.

Run Native and Terminal with `omega run --both main.omg`, or the corpus gate:
`python3 tools/corpus_gate.py --both --filter layouts/runtime_returned_encoded_reference_exit`.
The independent checked-tree oracle uses the same fixture with
`--checked-interpreter --golden tests/omega/corpus_both_macos_arm64.txt` on the
corpus command; its engine fact differs from the Terminal golden.
