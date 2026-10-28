# Rootless numeric provider review

This package deliberately has no ProgramEntry. Its unused external negate
implementation satisfies the exact numeric requirement; its ordinary public
classifier uses a different numeric operation.

The sibling [consumer](../rootless_numeric_provider_consumer/main.omg) checks
both finite and NaN inputs and exits 0 in Terminal Psi and native execution.
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

The fresh review must include `UnusedNumericProvider::negate` even though the
consumer never calls it. This pins declaration review, not a new executable IR
or permission to execute an external provider.
