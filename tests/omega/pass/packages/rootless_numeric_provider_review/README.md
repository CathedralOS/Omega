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

Under decision `target-realized-float-arithmetic`
([numeric values](../../../../../wiki/spec/language/numeric_values.md#target-realization-and-control-state)),
each target's declared realization table supplies `F32::negate`, and neither
this library nor the consumer may select a provider for it. Both build programs
therefore make no float selections. The unused external member stays a
reviewed declaration of a package that still has no ProgramEntry; it is never
an execution candidate. The history below records the earlier selection-based
form.

The original consumer at source `69ec9f80599b` first failed package setup because
the old fixture copier omitted its sibling. With the sibling present, the same
compiler correctly refused the competing `NativeFloats` and
`UnusedNumericProvider` defaults. The first explicit-root-choice replay at input source `ae987fcc85a9`
then refused its unexposed numeric subject before execution. The migrated consumer imports `float_operations` explicitly and names the visible
`F32::negate` slot and library package root in its four root rows. It subsequently
produced native/Terminal exit 0 on all four hosts at the producers recorded in
the consumer README; the earlier refusals remain historical observations.

The [integrated Linux run](https://github.com/CathedralOS/Omega/actions/runs/42026360839)
at `6b1c338126fa` again completes normal package review and executes the consumer
with native/Terminal exit 0. Its generated review and accepted lock include the
unused private external `UnusedNumericProvider::native_negate`, its exact
`F32::negate` coordinate and signature, and its compiler-intrinsic binding. Reading
those two accepted review rows does not establish an invocation of the unused
member. No additional `audit packages` or `inspect-terminal` command ran in that
scope.
