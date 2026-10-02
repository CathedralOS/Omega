# Mathematical guarantee across a ranked loop

From the repository root, run:

```sh
cargo run -p omega -- run --both tests/omega/pass/proofs/runtime_inductive_climbing_sum_exit/main.omg
```

Prepare and accept this fixture's package dependencies with the normal package
review workflow before running. Both native execution and the checked interpreter
must exit **70**: `climb(4, 0, 0)` returns `4`. Exit `71` means the returned
accumulator was wrong.

The limit comes from a runtime field. Its read precedes forwarding the exclusive
subject into `climb`, following left-to-right argument evaluation. The recursive
machine retains its bounded-distance measure, all three entry requirements and
its mathematical result guarantee. Subtraction outside `embed` is mathematical
integer subtraction, including when the index already exceeds the limit.

The stage 05 `ranked_value_guarantees` regression serializes the resulting
Terminal module and proofs, verifies them independently and executes that
artifact. Changing only the accumulator forwarded on the backedge must reject
the guarantee evidence while preserving the identical termination question.
