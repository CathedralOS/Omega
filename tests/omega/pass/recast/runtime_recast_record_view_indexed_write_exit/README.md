# Indexed writes through a record recast

After ordinary package review, run:

```text
omega run --both tests/omega/pass/recast/runtime_recast_record_view_indexed_write_exit/main.omg
```

Both engines must return 70. An exclusive `Src`-to-`Dst` record view retains
its original backing while writing a scalar field and a fixed-array element.
A second view writes through a runtime selector. The caller checks both element
fields and the neighboring tag through the original `Src` type.

The Build entry binds Windows x86-64, Linux x86-64, macOS ARM64, and Linux
ARM64. The authored source and exit behavior are the same on each host.
