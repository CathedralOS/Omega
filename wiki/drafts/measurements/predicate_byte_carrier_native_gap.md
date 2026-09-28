# A domain-carrying byte carrier checks but cannot be realized

`--check` is the sample gate's measure, and for one whole family it overstates
readiness: a sample that writes a byte at an index into a carrier declared
`[u8; N] in <Domain>` passes checking and then fails native realization with

```
terminal-artifact production failed:
  Lowering(Unsupported("byte replacement requires bounded bytes and exact branch-free operands"))
```

**The message is misleading.** The carrier is bounded and the operands are
branch-free. Neither is why it refuses.

## Measured at `5f75c49529b`

`05_checked-trees-to-lowered-psi/src/emission/structural_byte_sequence_index_store.rs`
refuses unless five clauses hold. Instrumenting all five and running
`samples/cli/games/dice_histogram` prints:

```
field_ok=false  index_ok=true  value_ok=true
value_type=Integer(Fixed, Unsigned, 8)
index_sc=false  value_sc=false
field_type=PredicateByteSequence { carrier: BoundedOwned { capacity: 24 },
                                   predicates: ByteSequencePredicateSet(1) }
```

Exactly one clause fails. The match admits
`StructuralFieldType::ByteSequence(BoundedOwned | BorrowedView)` and does not
mention `PredicateByteSequence`. A carrier with a domain — `[u8; 24] in Utf8`
— lowers to `PredicateByteSequence` wrapping the *same* `BoundedOwned`
carrier, so it never matches.

## Sibling sites carry the arm this one lacks

This reads as an oversight rather than a deliberate refusal:

- `expression_preparation/bindings/structural_fields/mod.rs:584-593` is the
  identical `ByteSequence(BoundedOwned | BorrowedView)` match **plus** a
  `PredicateByteSequence { carrier: BoundedOwned | BorrowedView, .. }` arm.
- `unit/attached_unit/ordinary_machine/projected_moves.rs:236-248` admits both
  the same way.

Not verified here: whether lowering must consult a retained domain fact
before emitting such a store. The domain obligation is discharged in checking,
and the two carriers have the same byte layout, but confirm that before
widening the match.

## How much rides on it

Three samples confirmed by running them, each from a different group and two
of them untouched by this session: `games/dice_histogram`,
`text/caesar_cipher`, `basics/print_number`. All three check and all three
fail identically.

A grep over the 119 checking samples for a sized `[u8; N]` domain plus an
indexed byte write returns **21**. That is an upper estimate, not a
measurement — only the three above were run:

`binary_search_viz`, `maze_flood`, `sort_visualizer`, `prime_sieve`,
`multiplication_table`, `print_number`, `print_squares`, `dice_histogram`,
`calculator`, `direction_command`, `bouncing_console`, `ripple_field`,
`tick_marquee`, `calendar`, `cellular_automaton`, `game_of_life_glider`,
`langtons_ant`, `random_walk`, `caesar_cipher`, `format_number`, `hex_dump`.

## Why this matters for SAMPLE-CORPUS

The item's acceptance is "checked semantics, native publication for its
authored targets, and documented exit/output", and it warns that "a
successful refresh is only the build half". This is that half made concrete:
the gate's 119/148 is a checking number, and a sixth of it may not survive
realization. Treat the gate as necessary, not sufficient, and do not read a
rising check count as rising native coverage.

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

The file holding this refusal is claimed by **LOCAL-DATA-CLUSTER-REPAIR**; the
finding above was sent to that ticket and no edit was made here.
