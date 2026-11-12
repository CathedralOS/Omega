# Named result policy and computed float operands

The named requirement declares a Saturating result. Its caller multiplies a
computed operand by an explicitly qualified operand. The program exits 70 when
the result is 8, and 71 otherwise.
The same value policy must survive each composition; being called `F32` grants
the ordinary named requirement no implicit policy adapter.

From the repository root, with the built compiler on `PATH`:

```sh
omega update --project tests/omega/pass/float/runtime_named_policy_result_exit --target macos_arm64
omega run tests/omega/pass/float/runtime_named_policy_result_exit/main.omg
```

Use the matching host target on another platform. Package preparation may
require explicit project capability review; resolve any pending decisions and
resume the update before running. The native regression route is:

```sh
python3 tools/corpus_gate.py --native --filter float/runtime_named_policy_result_exit
```

The negative controls `named_call_implicit_policy_erasure` and
`named_call_computed_policy_erasure` check the other side of this distinction:
an ordinary named requirement declaring bare float parameters cannot silently
erase Saturating from either a stored or computed argument.
