# Reconstruct a callback address without changing its neighbors

A C registrar reconstructs 33 Omega handler addresses from three fragments each
and invokes every one. The last handler has a distinct callback requirement and
runtime result. Every call checks selector, IST, attributes and reserved-word
canaries in all 33 slots, plus the outer fields. The Omega caller also checks
the result and representative neighbors. Expected native exit is 0.

Two inline groups hold 16 and 17 gates. Every gate keeps its original split
pointer layout; the root table has one ordinary runtime argument. This crosses
the previous 32-callback limit without changing the native parameter limit.
The negative twin `fail/layouts/private_callback_capacity_wrong_contract`
permutes two unlike binders, retaining a unique binder for every destination,
and rejects the exact callback-contract mismatch.

This fixture retains the Darwin ARM64 calling plan. It exercises ordinary
callbacks, not interrupt arrival or return. The current Terminal foreign-call
adapter refuses plans that materialize callbacks, so this fixture does not
claim native/Terminal agreement. The checked-provider companion
`build/terminal_private_callback_calling` supplies both-engine coverage.
Cathedral's hardware timer acceptance remains separate.

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
