# Shared fixed-array cycle

`main.omg` is byte-identical to `samples/cli/collections/array_max/main.omg`.
Keep the real loop, array values, console output and EOF/Enter interaction;
do not replace them with a helper-only check.

```sh
python3 tools/corpus_gate.py --both --filter collections/runtime_shared_fixed_array_cycle_exit
```

Native and checked-interpreter execution must both return 42 and match all
three stdout lines. The shared array remains backed by the caller's local
storage throughout the loop. Ordinary sample preparation and its separate
package-review requirement are documented in the sample README.
