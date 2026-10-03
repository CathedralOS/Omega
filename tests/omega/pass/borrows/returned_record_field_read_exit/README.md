# Runtime-selected returned reference

From the repository root, run:

```sh
cargo run -p omega -- run --both tests/omega/pass/borrows/returned_record_field_read_exit/main.omg
```

Exit **70** means each call reads the nested field through the address actually
returned by `pick`: 43 from the second holder, then 17 from the first. Padding
makes the field offset nonzero. Exit 71 exposes a wrong address or offset.
The producer's possible-origin set is lifetime evidence, not a static choice of
which address to load. Reading retains the carrier's mutable declaration.

`python3 tools/corpus_gate.py --both --filter borrows/returned_record_field_read_exit`
checks the native oracle and interpreter agreement. The ordinary product command
also requires project/package review; the fixture does not bypass it. When using
a Cargo target directory shared with an older checkout, set `OMEGA_LIBRARY_ROOT`
to this checkout's absolute `source/library` directory, as documented in
[runtime library discovery](../../../../../tools/release/README.md#runtime-library-discovery).
