# Named constant array extents

The program stores and reads bytes through arrays sized by a computed constant,
a qualified imported constant, a type-scoped constant, and a shadowing
const-generic parameter. It exits 70 only when their lengths are 8, 3, 4, and 2
and each stored byte is preserved.
The observer constant is declared before the extent constant and reaches it only
through a local's declared `Storage` type. Evaluation must settle that dependency
and retain its exact declaration after folding the extent into a number.

From the repository root, with the built compiler on `PATH`:

```sh
omega update --project tests/omega/pass/modules/runtime_named_constant_array_extent_exit --target macos_arm64
omega run tests/omega/pass/modules/runtime_named_constant_array_extent_exit/main.omg
```

Use the matching host target for package preparation on another platform. The
update may require explicit project capability review; resolve its pending
decisions and resume it before `run`:

```sh
omega update --resume --project tests/omega/pass/modules/runtime_named_constant_array_extent_exit
```

The current fresh-project CLI frontier is that acceptance step, not a compiler
rejection. The focused native regression command is:

```sh
python3 tools/corpus_gate.py --native --filter modules/runtime_named_constant_array_extent_exit
```

The sibling `fail/modules/array_extent_*` fixtures check private and ambiguous
selection, runtime and type-parameter shadowing, invalid values, and dependency
cycles. They must reject the actual extent or selection, not merely fail later
because their compile-only entry has no native root.

`runtime_named_extent_generic_copy_exit` separately observes an evaluated generic
identity invocation whose array type argument has a named extent. Its exit 70
checks that generic replacement retains the constant dependency after folding.
It does not claim runtime lowering of arbitrary array-return/guard compositions.
