Run through the corpus package-preparation and execution route:

```sh
python3 tools/corpus_gate.py --both --filter layouts/runtime_sparse_fragmented_bits_exit
```

Exit 70 requires reordered fragments in distant, odd-sized containers to
reconstruct the same scalar, clear its former high bits, and preserve both
neighboring fields. The 302-byte envelope must not become a scalar width.
