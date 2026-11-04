# Register an inherited handler with its chosen calling convention

Cathedral needs to reuse a shared handler interface while choosing how the
processor enters its timer handler. The compiler previously lost that choice
and refused registration. This fixture checks that the registered handler uses
the chosen registers for its arguments, even when another interface declares a
different choice.

The registrar names `TimerCallback::call`, inherited from `Callback`, and
`TimerHandler` supplies the implementation. The selected policy reads the input
from x0; the unused sibling reads x1 and promises to leave vector registers
untouched. Using the sibling's plan would read the wrong input.

The C registrar reconstructs the handler address from three descriptor fragments,
calls it with 102, and requires 51. Both C and Omega check the selector, IST,
attributes and reserved-word canaries. The native oracle remains exit 0.

The rejection twin at
`tests/omega/fail/layouts/private_callback_inherited_policy_missing` changes only
the binder's requirement to `Callback::call`. That parent has no calling policy.
The selected handler, private slot, registrar and sibling must not donate one.

This fixture is limited to ordinary Darwin ARM64 callback execution. The current
Terminal foreign-call adapter refuses callback-materializing plans, so no
native/Terminal agreement is claimed. Actual x86 interrupt acceptance remains
with Cathedral's QEMU timer test.

From the repository root on Darwin ARM64, use an optimized compiler built from
the candidate being checked. Prepare the C library explicitly; the corpus does
not automatically run `fixture/prepare.py`:

```sh
fixture=tests/omega/pass/layouts/private_callback_inherited_policy_exit
provider_output="$PWD/build/descriptor-callback-provider"
omega_cli="$PWD/target/debug/omega"
python3 tools/build_slot.py -- python3 "$fixture/fixture/prepare.py" --output "$provider_output"
export DYLD_LIBRARY_PATH="$provider_output${DYLD_LIBRARY_PATH:+:$DYLD_LIBRARY_PATH}"
python3 tools/build_slot.py -- "$omega_cli" update --project "$fixture" --target macos_arm64
```

If update requests review, inspect
`$fixture/build/package-manager/review-macos_arm64.txt` and accept only the
fixture's reviewed declarations and foreign registration binding. Resume the
ordinary package workflow after saving those decisions:

```sh
python3 tools/build_slot.py -- "$omega_cli" update --project "$fixture" --resume
```

Once update succeeds, execute the native oracle:

```sh
python3 tools/build_slot.py -- "$omega_cli" run "$fixture/main.omg"
```

Keep the compiler identity, preparation receipt and actual native output with
the observation. A successful library build alone does not exercise the callback.
