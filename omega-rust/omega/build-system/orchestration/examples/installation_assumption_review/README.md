# Review conditions without checking the project again

Checking a large project can take minutes. This example saves the result so
native compilation can continue after the caller reviews the boot conditions,
without checking source again.

Preparation still runs the normal package checks and requires the caller's
acceptance decisions for dependency and permission changes. It does not accept
those changes or assume a physical environment. The saved result contains the
checked program (the Psi artifact) and its original native inputs.

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
are not permission to assume an environment. Projects containing separately
compiled components are rejected because this helper does not yet save those
components.

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
conditions, evaluates the remaining layout and calling-convention values, and
runs ordinary native admission, emission and final-image validation. It
publishes direct images only.
Any failed gate prevents successful output. Printed source activity must remain
zero for this separate invocation. Saving inputs does not establish backing,
receiver authority, loader behavior or runtime execution; review and execution
of the resulting image remain separate steps.

The existing `inspect` command remains available for an in-memory inspection
that does not save native inputs. Package update/review remains an independent
required operation when project inputs change; this workflow avoids repeating
the subsequent inspection's source work for native completion.
