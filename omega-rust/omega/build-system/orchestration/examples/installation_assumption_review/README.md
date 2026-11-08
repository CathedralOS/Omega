# Review conditions without checking the project again

The example can save a checked ordinary project's Psi artifact and original
native inputs, then finish native compilation after its environment conditions
have been reviewed. Preparing performs the normal accepted-package, provider
selection and trust checks. It does not accept package changes or assume a
physical environment.

Build the example with an optimized compiler in the checkout's private target:

```sh
CARGO_PROFILE_DEV_OPT_LEVEL=1 mbx build --locked -p orchestration --example installation_assumption_review
```

Use an ordinary project whose package review is already complete:

```sh
installation_assumption_review prepare PROJECT/main.omg uefi_x86_64 NEW_PREPARATION
```

This saves `terminal.psi`, `native-inputs.bin` and `catalog.txt` from the same
checked production. It round-trips the pair through the ordinary recovery API
before reporting success. Source activity counts and both hashes are printed.
The catalog names the actual entry and installation operation; those identifiers
are not permission to assume an environment. Nonempty independent-component
compositions currently refuse persistence rather than lose their inputs.

Review the exact program, selected target and external launch description.
Only the caller responsible for that environment may record its assumptions:

```sh
installation_assumption_review accept-bootstrap NEW_PREPARATION/terminal.psi uefi_x86_64 ENTRY_ID INSTALLATION_ID environment.md CALLER bootstrap.bin
installation_assumption_review accept-interrupt-delivery NEW_PREPARATION/terminal.psi uefi_x86_64 ENTRY_ID INSTALLATION_ID environment.md CALLER delivery.bin
installation_assumption_review show bootstrap.bin
installation_assumption_review show delivery.bin
```

These records describe caller assumptions, not observed or proved physical
truth. A changed program must be reviewed with its own actual identifiers.

Finish in a separate invocation, supplying only the saved program and explicitly
reviewed records. A program needing no installation assumptions can omit them:

```sh
installation_assumption_review finish NEW_PREPARATION NEW_NATIVE_DIRECTORY bootstrap.bin delivery.bin
```

Finishing does not read project source or an accepted lock. It recovers the
original native proposal and accepted package policy, checks the supplied
conditions, completes required scalar evaluation and runs ordinary native
admission, emission and final-image validation. It publishes direct images only.
Any failed gate prevents successful output. Printed source activity must remain
zero for this separate invocation. Saving inputs does not establish backing,
receiver authority, loader behavior or runtime execution; review and execution
of the resulting image remain separate steps.

The existing `inspect` command remains available for an in-memory inspection
that does not save native inputs. Package update/review remains an independent
required operation when project inputs change; this workflow avoids repeating
the subsequent inspection's source work for native completion.
