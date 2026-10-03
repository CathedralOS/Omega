# Shared-array polynomial recurrence

`main.omg` is byte-identical to `samples/cli/algorithms/horner_eval/main.omg`.
Keep the caller-owned coefficients, indexed initial state argument, wrapping
multiply-add recurrence and console interaction.

```sh
python3 tools/corpus_gate.py --both --filter collections/runtime_shared_array_polynomial_exit
```

Both native execution and the checked interpreter must calculate 57, select
the success branch, exit 70 and match all three stdout lines. The fixture uses
repository admission; the sample README documents its separate public package
review. Base `e56af86474b` already executes this program.
