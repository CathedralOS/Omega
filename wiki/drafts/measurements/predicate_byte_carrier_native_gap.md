# Indexed stores into domain-carrying byte carriers

A sample that writes a byte at an index into a carrier declared
`[u8; N] in <Domain>` used to check and then fail native realization with

```
terminal-artifact production failed:
  Lowering(Unsupported("byte replacement requires bounded bytes and exact branch-free operands"))
```

`[u8; N] in Utf8` lowers to `PredicateByteSequence { carrier: BoundedOwned, .. }`.
Stage 05's indexed-store emission, the Terminal verifier's field lookup and the
Omega unit validator each admitted only the plain `ByteSequence` carrier. Widening
the matches was not the repair: an arbitrary byte written into a `Utf8` buffer
can break the declaration that later reads and calls rely on.

## What a qualified store proves

[Structural access](../../spec/terminal-psi/structural_access.md) owns the rule.
Terminal Psi keeps a field's declared predicates true after every write, so an
indexed store owes more than its bound:

- **The stored byte's class.** The verifier rebuilds the store's goal from the
  destination declaration: `index < length`, then `value <= 127` for
  `ascii_only` or `valid_utf8`, then `1 <= value` for `no_nul`. The producer's
  ordinary certificate search proves it. The checker's own preservation facts
  are not carried as authority.
- **The carrier's prior contents, for UTF-8.** An ASCII byte written over part
  of a multi-byte scalar breaks UTF-8. Unless the field also declares
  `ascii_only`, every path into the store must reach a whole replacement of the
  same field whose source replays to `ascii_only` (literal octets, qualified
  formals, concatenation inputs), with only same-field stores or disjoint
  operations after it. The machine entry supplies only the declaration.
- **The Omega consumer** requires the accepted obligation to be exactly that
  goal and replays the carrier rule on its current graph.

`terminal_semantics::indexed_byte_store_goal` builds the goal once for both
consumers.

## The ten customers after the repair

Measured on macOS arm64 with this repair on main `e4dab11af5`, using
`tools/sample_native_gate.py --jobs 5 --timeout 2400`.

| Sample | Result |
| --- | --- |
| `basics/print_number` | Runs; prints `150`, exit 0. |
| `text/hex_dump` | Runs; prints `4f 6d 65 00`, exit 0. |
| `games/dice_histogram` | Store goal unproved at the proof-search capacity (4096). |
| `text/caesar_cipher` | Store goal `OperationProofUnavailable`. |
| `text/format_number` | Store goal `OperationProofUnavailable`. |
| `arithmetic/prime_sieve` | Store goal `OperationProofUnavailable`. |
| `basics/multiplication_table` | Store goal `OperationProofUnavailable`. |
| `basics/print_squares` | Store goal `OperationProofUnavailable`. |
| `simulation/calendar` | Store goal `OperationProofUnavailable`. |
| `algorithms/maze_flood` | Store goal unproved at the proof-search capacity (4096) after a 1895 s compile. |

For each of the eight proof refusals, `OMEGA_DUMP_OBLIGATIONS=1` names the
indexed store's own obligation, `index < length AND value <= 127`, and no
reconstructed axiom bounds the stored byte.

## What remains

The stored byte's ASCII leg needs a premise Terminal reconstruction does not
yet supply. Three shapes cover all eight:

- **A narrowing call's result.** `print_number` guards each digit with
  `transition self.ch < 128`, and that branch fact proves the leg. The others
  store the result of `narrow_i32_to_u8_trapping` (`dice_histogram`,
  `format_number`, `prime_sieve`, `multiplication_table`, `calendar`) or
  `narrow_u32_to_u8_wrapping` (`print_squares`). The checker bounds that
  result from the callee body; the Terminal call publishes no guarantee
  relating result to argument, so after the call the value is an unbounded
  `u8`. Where the byte crosses a state join into a field (`tc` is either a
  narrowed digit or the constant 32), the bound must also survive that join.
- **A guard on a field that reaches the store through a join.** `maze_flood`
  guards `self.tc < 128 && self.oc < 128` on both arrivals at `num_write`, one
  after `tc` is narrowed and one after `tc = 32`. Each branch fact names that
  arrival's own read of the field; nothing carries `tc < 128` across the
  two-arrival join as a field fact the store's later read can use.
  `print_number`'s store block has one arrival, so its guard reaches it.
- **A byte read from a literal-established field.** `caesar_cipher` stores
  `text[i] + 3`. The checker bounds `text[i]` by the octets of the `"HELLO"`
  literal that established `text`; a Terminal field-byte read carries no
  bound from the literal that established its field.

Neither is repaired by recognizing callee names, weakening the field's domain,
or trusting the checker's facts. Each needs its premise reconstructed where the
verifier can recheck it.

The corpus fixture `pass/text/runtime_number_to_decimal_exit` is the same
narrowing shape: it moves from the stage-05 refusal to an unproved store goal.

## Running a sample natively, for the next reader

`omega run` needs a completed package review, which is per-checkout and not
committed (`omega.lock` and `build/` are ignored):

```
omega update --project <sample dir> --target macos_arm64      # writes the review
sed -i '' 's/^\(decision row [0-9a-f]*\) pending$/\1 accept/' \
    <sample dir>/build/package-manager/review-macos_arm64.txt
omega update --project <sample dir> --resume                  # --resume takes no --target
omega run <sample dir>/main.omg < /dev/null
```

`tools/sample_native_gate.py --filter <names>` performs those steps and compares
each sample's documented oracle.
