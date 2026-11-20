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
each invalid `@` spelling. `--profile envelope` checks all eight EEREQ header
identity bytes and all 32 closure digest bytes, plus the first reported failure
when one request has multiple faults. `historical-at` compares the original
inputs with their original expectations, including the three Console examples
that the grammar refuses. [Portable cases](../../conformance/README.md)

`--receipt` skips Delta compilation and prints the reused file. For comparisons,
record which route constructed that file. Reuse still checks its bound
identity, rebuilds the lower stages and executes the same Gamma interpreter.
`--seconds` sets a wall-time limit for Delta compilation and each subsequent
request. The optional `--omega` step uses the same bootstrap compiler, sealed
OCREQ bytes, tape publication check and Alpha program exit check as the
compiled-host Epsilon evaluator. It checks the bootstrap compiler and entry
identities before invoking the Gamma interpreter.

`python3 tests/bootstrap/independent/successor_test.py --compile-epsilon`
stores its sealed lexer inputs and observations under
`build/independent-successors/compiled-epsilon/`. Use the retained interpreter
and receipt to replay the same input. For example,

```sh
peer=build/independent-successors/compiled-epsilon
target/debug/xtask gamma-invoke --evaluator /tmp/epsilon-cold/reference \
  --source /tmp/epsilon-cold/evaluator.gamma --timeout 90 \
  --input "$peer/lexical-edge-byte-255.request" --output /tmp/byte-255.observation
cmp /tmp/byte-255.observation "$peer/lexical-edge-byte-255.observation"
```

## Original-chain observations

On Linux x86-64 the chain constructed the canonical 726,601-byte Gamma receipt
in 566.729 seconds of wall time. Its SHA-256 was
`1d3a700d970acda04ee4699c19bf03a3654eb481a46755399d0daf759fe030af`.
The native-hosted Delta compiler produced identical bytes. With this receipt,
the original interpreter passed 5/5 language observations and 2/2 syntax
controls. The historical profile passed 2/5. All three mismatches returned
`02 02 a7 00 00 00`, reporting an invalid token at source byte 167.

The same receipt ran the portable 180-byte `program.omg` request with the
839,299-byte bootstrap compiler from `e488ba404bf`. The enclosing EEREQ was
858,941 bytes (`fd63c0a10ee565f0e7bfedcda9dc8f7e96fd204b9dd418c7d29d2f568afdc906`).
That invocation reached its 300-second wall limit before publishing a tape.

The lexer comparisons used the independent suite's Epsilon sources. Each
invocation had one low-priority process on a shared host. The four lexical
predicates test whitespace, identifier start, identifier continuation and
hexadecimal value. The six-byte sample used the same 2,256-byte source with
the six listed bytes as stdin. Its 29-byte observation matched the native
evaluator and the expected predicate results, SHA-256
`a2ec682ebf61392faa6d4f16ee3074ab07a3b8baa55e7bfcc462af877547c9d0`.

| Sealed input | EEREQ SHA-256 prefix | Interpreted result |
| --- | --- | --- |
| `lexical-edge-byte-255.request`, four predicates on `ff` | `13ba5ccb091e0fbb` | Epsilon exited 42 and returned four zeros in 44.960s (90s limit) |
| Four predicates on `20 30 5f 61 46 ff` | `3459e763cf06bef9` | Epsilon exited 42 and returned all 24 predicate values in 10.810s (90s limit) |
| `lexical-reference.request`, four predicates on all 256 bytes | `2219cfb472d7c5f0` | Reached the 240s limit without an observation |
| `scan-string-evaluator.request`, 17 direct string-scanner cases | `ac5d5121f795627e` | Reached the 240s limit without an observation |
| `scan-public-evaluator.request`, 23 public lexer cases | `2367558ac614f14f` | Reached the 480s limit without an observation |
