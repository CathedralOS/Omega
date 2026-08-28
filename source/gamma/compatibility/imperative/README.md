# Parked imperative Gamma compatibility compiler

This directory contains the older compiler-first language with variables
`a`–`j`, mutation, `if`/`while`, and decimal I/O. It is retained only for
historical compatibility and optional differential investigation.

It is not the Gamma rung. Canonical Gamma is defined by
[`../../LANGUAGE.md`](../../LANGUAGE.md), evaluated by `../../interp.beta`, and
checked by `../../typeck.beta`. Nothing in the default lattice invokes this
directory.

The legacy Windows build remains directly runnable:

```sh
sh source/gamma/compatibility/imperative/rebuild.sh
sh source/gamma/compatibility/imperative/build.sh
```

Do not add canonical Gamma or Delta publication dependencies here.
