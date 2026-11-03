# Reconstruct a callback address without changing its neighbors

A C registrar reconstructs one Omega handler address from three descriptor
fragments, calls it with 102, and requires the result 51. Both the registrar
and the Omega caller check the selector, IST, attributes and reserved-word
canaries. The caller traps if registration reports failure or a neighbor changes.
The expected native exit is 0.

This fixture retains the Darwin ARM64 calling plan. It exercises an ordinary
callback, not interrupt arrival or return. The current Terminal foreign-call adapter refuses plans that materialize
callbacks, so this fixture does not claim native/Terminal agreement. Actual x86 interrupt acceptance belongs to
Cathedral's QEMU timer test; this fixture makes no claim about it.

From the repository root on Darwin ARM64, use an optimized compiler built from
the candidate being checked. Prepare the C library explicitly; the corpus does
not automatically run `fixture/prepare.py`:

```sh
fixture=tests/omega/pass/layouts/private_callback_descriptor_fragments_exit
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
