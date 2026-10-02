Run from the repository root on a matching hosted target:

```sh
omega run --both tests/omega/pass/traits/nominal_generic_reference_forwarding_exit/main.omg
```

The checked interpreter and native product both exit 1 after the caller observes
the increment made through its forwarded mutable reference.

On a fresh checkout, prepare package acceptance with `omega update --project
tests/omega/pass/traits/nominal_generic_reference_forwarding_exit --target <host-target>`,
complete the generated review, and resume it with `omega update --project
tests/omega/pass/traits/nominal_generic_reference_forwarding_exit --resume`. The repository test harness
uses its reviewed fixture policy for the same source.
