This control forwards `FixedBuffer<N>` while explicitly selecting `3` for
the selector's const parameter. The real caller instantiates `N` as `2` and
`3`; inference must not overwrite the explicit selection to admit both.

From the repository root, package preparation must reject this program:

```sh
mbx run -p omega -- update --project tests/omega/fail/generics/forwarded_const_argument_conflicts_with_explicit_selection --target macos_arm64
```
