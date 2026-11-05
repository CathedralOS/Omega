# Rootless numeric provider review

This package deliberately has no ProgramEntry. Its unused external negate
implementation satisfies the exact numeric requirement; its ordinary public
classifier uses a different numeric operation.

The sibling [consumer](../rootless_numeric_provider_consumer/main.omg) checks
both finite and NaN inputs and has an exit-zero oracle for both engines.
The [invalid control](../../../fail/packages/rootless_numeric_provider_missing_satisfaction/main.omg)
keeps a well-formed, same-signature external implementation but changes its
satisfaction edge to square root. Selecting it for negate must reject.

Full review is a separate acceptance from corpus fixture consent:

```sh
omega update --project tests/omega/pass/packages/rootless_numeric_provider_consumer --target macos_arm64
# Accept the local package's pending review decisions, then resume.
omega update --resume --project tests/omega/pass/packages/rootless_numeric_provider_consumer
omega audit packages --project tests/omega/pass/packages/rootless_numeric_provider_consumer --target macos_arm64 --details
omega run --both tests/omega/pass/packages/rootless_numeric_provider_consumer/main.omg
```

The fresh review must include `UnusedNumericProvider::native_negate` even though the
consumer never calls it. Execution still requires the exact selected contract and installed provider
authority.

The library exports a negate choice even though its classifier never calls
negate. Core now also supplies a default for that slot. Different providers
in the same default tier conflict, so the consumer must make an explicit root
choice under [provider selection](../../../../../wiki/spec/build/provider_selection.md).
Its four target rows choose the library’s exact public provider. Neither default
is discarded, and the library still has no ProgramEntry.

The original consumer at source `69ec9f80599b` first failed package setup because
the old fixture copier omitted its sibling. With the sibling present, the same
compiler correctly refused the competing `NativeFloats` and
`UnusedNumericProvider` defaults. The explicit-root-choice migration has not yet
been executed; it does not relabel either earlier refusal as success.
