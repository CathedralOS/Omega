# Interpreted Epsilon conformance

Run the portable Epsilon cases through the Beta-written Gamma evaluator on the
audited Alpha VM. Give each run a new directory. The test rebuilds Beta from
the admitted tape, then builds Gamma with that Beta and compares both tapes
byte for byte. Gamma interprets the Delta compiler to produce the Epsilon
evaluator in Gamma source, called its receipt. The test verifies the receipt
identity and interprets it for every request.

From the repository root, with the repository's xtask built for packing source
manifests, run

```sh
python3 tests/bootstrap/interpreted/epsilon/portable.py /tmp/epsilon-cold
python3 tests/bootstrap/interpreted/epsilon/portable.py /tmp/epsilon-invalid \
  --receipt /tmp/epsilon-cold/evaluator.gamma --profile syntax-controls
python3 tests/bootstrap/interpreted/epsilon/portable.py /tmp/omega-interpreted \
  --receipt /tmp/epsilon-cold/evaluator.gamma --seconds 300 --omega
```

The default `language` profile checks the language's accepted Console spelling,
exit, stdin, trap and framing behavior. `syntax-controls` checks refusal of
each invalid `@` spelling. `historical-at` compares the original inputs with
their original expectations, including the three Console examples that the
grammar refuses. [Portable cases](../../conformance/README.md)

`--receipt` skips Delta compilation and prints the reused file. For comparisons,
record which route constructed that file. Reuse still checks its bound
identity, rebuilds the lower stages and executes the same Gamma interpreter.
`--seconds` sets a wall-time limit for Delta compilation and each subsequent
request. The optional `--omega` step uses the same bootstrap compiler, sealed
OCREQ bytes, tape publication check and Alpha program exit check as the
compiled-host Epsilon evaluator. It checks the bootstrap compiler and entry
identities before invoking the Gamma interpreter. A timeout means the run supplied no result for that step.

For another sealed EEREQ file, use the retained interpreter and verified receipt
with `xtask gamma-invoke --evaluator /tmp/epsilon-cold/reference --source
/tmp/epsilon-cold/evaluator.gamma --input request.bin --output observation.bin`.
