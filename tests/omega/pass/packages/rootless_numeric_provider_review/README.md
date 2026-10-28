# Rootless numeric provider review

This package deliberately has no ProgramEntry. Its unused ordinary named
implementation supplies `NegationSlot` with a checked body that calls the
public core negate requirement. It grants no compiler intrinsic. Its ordinary
public classifier uses a different numeric operation.

The sibling [consumer](../rootless_numeric_provider_consumer/main.omg) checks
both finite and NaN inputs and retains exit 0 in Terminal Psi and native execution.
The [invalid control](../../../fail/packages/rootless_numeric_provider_missing_satisfaction/main.omg)
keeps the same implementation body and signature but supplies `DifferentSlot`.
Selecting it for `NegationSlot` must reject for the missing exact satisfaction.

Full review is a separate acceptance from corpus fixture consent:

```sh
omega update --project tests/omega/pass/packages/rootless_numeric_provider_consumer --target macos_arm64
# Accept the local package's pending review decisions, then resume.
omega update --resume --project tests/omega/pass/packages/rootless_numeric_provider_consumer
omega audit packages --project tests/omega/pass/packages/rootless_numeric_provider_consumer --target macos_arm64 --details
omega run --both tests/omega/pass/packages/rootless_numeric_provider_consumer/main.omg
```

The fresh review must include `UnusedNumericProvider::negate` even though the
consumer never calls it. This pins declaration review, not a new executable IR
or permission to select a compiler intrinsic.
