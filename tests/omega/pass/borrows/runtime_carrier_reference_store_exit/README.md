# Store through a reference-bearing local

The local carrier holds scalar metadata before a mutable reference. A helper
borrows the carrier and updates only its metadata; a later store through the
reference must reach the original backing, not overwrite the pointer slot.
An unrelated shared argument and sibling field must retain their values.
Success exits 70; an incorrect observation exits 71.

From the repository root, after ordinary package review:

```sh
omega run --both tests/omega/pass/borrows/runtime_carrier_reference_store_exit/main.omg
```

The compiler corpus supplies its own test admission and checks native and
checked-interpreter behavior without substituting for public package review:

```sh
python3 tools/corpus_gate.py --both --filter runtime_carrier_reference_store_exit
```

On macOS ARM64 the corpus executes both engines with exit 70. The ordinary
CLI currently exits 200 because fresh package-review decisions remain pending;
this fixture's execution evidence does not approve those decisions.
