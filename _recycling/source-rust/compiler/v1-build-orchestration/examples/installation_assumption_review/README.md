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

This saves `terminal.psi`, `native-inputs.bin` and `catalog.txt` from one
checked compilation. Before reporting success it loads the first two back
through the ordinary recovery API. It prints source activity counts and both
hashes. The catalog names the program's entry and installation operation;
those identifiers do not permit assuming an environment. Projects containing separately
compiled components are rejected because this helper does not yet save those
components.

Review the exact program, selected target and external launch description. Only the caller responsible for that environment may record its
assumptions:

```sh
installation_assumption_review accept-bootstrap NEW_PREPARATION/terminal.psi uefi_x86_64 ENTRY_ID INSTALLATION_ID environment.md CALLER bootstrap.bin
installation_assumption_review accept-interrupt-delivery NEW_PREPARATION/terminal.psi uefi_x86_64 ENTRY_ID INSTALLATION_ID environment.md CALLER delivery.bin
installation_assumption_review show bootstrap.bin
installation_assumption_review show delivery.bin
```

These records state what the caller assumes, not what was observed or proved
about the hardware. A changed program must be reviewed with its own identifiers.

Finish in a separate run after explicitly accepting the compiler that produced
the saved program. `--trust-producer` records that decision by the receiving
host; saved hashes and assumption records do not prove who produced it. Pass
the saved program and reviewed records; a program needing no installation
assumptions can omit them:

```sh
installation_assumption_review finish --trust-producer NEW_PREPARATION NEW_NATIVE_DIRECTORY bootstrap.bin delivery.bin
```

Producer trust applies only to the artifact this run loads and is not written
into the saved inputs. Finishing reads no project source and no accepted lock.
It recovers the original native proposal and accepted package policy, checks
the supplied conditions, computes the remaining layout and calling-convention
values, and runs ordinary native admission, emission and final-image
validation. It publishes direct images only. Any failed check prevents output.
The printed source activity must stay zero for this run. Saving inputs does
not establish backing storage, receiver authority, loader behavior or runtime
execution; reviewing and running the image are separate steps.

The `inspect` command still inspects in memory without saving native inputs.
When project inputs change, package update/review is still required separately;
this workflow only avoids repeating the later inspection's source work to
finish native compilation.
