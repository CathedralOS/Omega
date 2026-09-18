# Gamma product comparison experiment

Exploratory evidence for `GAMMA-PRODUCT-COMPARISON` in `TASKS_BOOTSTRAP.md`.
It is not accepted Gamma syntax, not a change to the selected evaluator, and
not a Delta compiler change. The customer is the seven-field argument-checking
continuation in
`bootstrap/3_delta/implementation/checking/types/calls.gamma`
(`typing_arguments` builds it; `typing_resume_argument` decodes it).

Run from the repository root:

```sh
sh tests/gamma/product-comparison-experiment/run.sh
sh tests/gamma/product-comparison-experiment/run.sh --reference
```

The first form runs the selected Beta evaluator and needs a host Alpha seed
(macOS arm64 or Windows x64). In that mode it also compiles each
`nominal*.delta` fixture through the canonical Delta compiler, which is the
customer's own static nominal-typing implementation, and executes each
published receipt. The second form runs the selected evaluator tape under
the untrusted `tests/alpha/reference/alpha_ref.py` and prints Alpha instruction
counts and pair allocations; it exists so a host without a seed can still
reproduce the numbers below, and its output labels itself diagnostic. The
candidate-D leg is selected-mode only: the Delta compiler is far too large
for the instruction-counting reference interpreter.

## Candidates

| Fixture | Candidate | Build | Decode | Observation |
| --- | --- | --- | --- | --- |
| `pairs.gamma` | A: nested pairs, copied from `calls.gamma` | six `pair` | five spine `second` bindings, seven field projections | status 0, byte 28 |
| `pairs_short.gamma` | A with a six-field producer | five `pair` | same consumer | authored trap, status 2 |
| `checked.gamma` | B: identity and arity words ahead of the same spine | eight `pair` | two `eq` checks, then A's decode | status 0, byte 28 |
| `checked_wrong_tag.gamma` | B given another constructor's identity | hand-built | identity check fails | status 0, byte 201 |
| `checked_wrong_arity.gamma` | B given a producer declaring six fields | hand-built | arity check fails | status 0, byte 202 |
| `forged.gamma` | B given six fields under a forged seven-field header | hand-built | both checks pass | authored trap, status 2 |
| `boundary.gamma` | C: the customer's `(pair kind payload)` frame, kind checked at the boundary | seven `pair` | one `eq`, then A's decode | status 0, byte 28 |
| `boundary_wrong_kind.gamma` | C given another kind's frame | hand-built | kind check fails | status 0, byte 203 |
| `boundary_short_payload.gamma` | C given six fields under the right kind | hand-built | kind check passes | authored trap, status 2 |
| `nominal_single.delta` | D: static nominal typing, one field | `(data Box (Box Int))` | one `match` arm | compile 0, run byte 28 |
| `nominal.delta` | D: the seven-field payload | `(data ArgumentFrame (ArgumentFrame Int*7))` | one `match` arm | compile 0, run byte 28 |
| `nominal_order.delta` | D with an order-sensitive fold | same declaration | one `match` arm | compile 0, run byte 4 |
| `nominal_swapped.delta` | D with two same-typed fields swapped | same declaration | same consumer | compile 0, run byte 6 |
| `nominal_distinct.delta` | D with a nominal wrapper per field | seven `(data F (F Int))` plus the frame | one `match` per field read | compile 0, run byte 4 |
| `nominal_short.delta` | D given six construction arguments | declared constructor | arity judgment | Reject code 16 at byte 515 |
| `nominal_wrong_field.delta` | D given `Bytes` in an `Int` position | declared constructor | argument judgment | Reject code 15 at byte 598 |
| `nominal_owner.delta` | D given a same-shaped other type | `(OtherFrame Int*7)` | owner judgment | Reject code 15 at byte 740 |
| `nominal_distinct_swapped.delta` | D's distinct-field variant, swapped | declared constructor | argument judgment | Reject code 15 at byte 1,224 |

Candidate B is the smallest source-level form of a "dynamically checked named
product": the constructor is a named function, its identity is a nullary
constant, and the consumer checks identity and arity before decoding.

Candidate C is the "Delta-side status at the frame boundary" named below: the
payload keeps A's spine, and the checked word is the continuation kind the
customer's frame already carries — the real dispatcher maps an unknown kind to
`InternalFailure` code 1 (`retained_frontend_contradiction`). The customer's
actual frame is `(pair kind (pair payload previous))`; C collapses the
previous-link pair because one frame is emulated, so its real cost there is
eight pairs, not seven. C therefore measures a mechanism the selected checker
already owns, not a new layout; its fixtures make that boundary's cost and
residual hole observable on the same payload as A and B.

Candidate D is witnessed through the selected canonical Delta compiler under
DCREQ profile 1, not by a new language feature: the customer's own
implementation already contains the static nominal judgments the third
candidate needs (`checking/declarations/data.gamma` for declarations,
`checking/types/calls.gamma` for construction arity and argument types,
`checking/types/matches*.gamma` for destructuring). The fixtures therefore
measure what those judgments observe on the same payload: an accepted program
yields a Gamma receipt whose constructor lowers to `(pair tag spine)` — the
same representation C already carries — and whose `match` lowers to unchecked
`first`/`second` projections, while a disagreement is a compile-time Reject
carrying a code and the exact source coordinate, not a runtime trap or a
chosen scalar. They do not demonstrate nominal products in Gamma itself; that
cost is counted, not built, below.

## Measured under `--reference` at the selected tape

| Fixture | Source bytes | Definitions | Alpha steps | Pairs |
| --- | ---: | ---: | ---: | ---: |
| `pairs.gamma` | 1,064 | 3 | 101,859 | 6 |
| `pairs_short.gamma` | 1,041 | 3 | 96,049 | 5 |
| `checked.gamma` | 1,560 | 6 | 147,419 | 8 |
| `checked_wrong_tag.gamma` | 595 | 5 | 51,722 | 4 |
| `checked_wrong_arity.gamma` | 591 | 5 | 55,275 | 4 |
| `forged.gamma` | 1,232 | 5 | 111,655 | 7 |
| `boundary.gamma` | 1,477 | 5 | 131,359 | 7 |
| `boundary_wrong_kind.gamma` | 994 | 4 | 77,057 | 7 |
| `boundary_short_payload.gamma` | 1,029 | 4 | 93,307 | 6 |

Steps count every Alpha instruction from tape start to halt, so they include
the evaluator's census and validation of the whole source, which grow with
source size; they are a whole-run cost, not an isolated construction or
projection cost. Pairs are 40-byte records, so the seven-field payload costs
240 bytes as A, 320 bytes as B, and 280 bytes as C.

## Measured under the canonical Delta compiler

Candidate-D rows run `delta_fixtures.tsv`: each `.delta` source is admitted
through the canonical compiler (DCREQ profile 1) under the selected evaluator;
compile status and output pin exactly (SHA-256 for receipts, all 40 bytes for
DCOUT failure frames), and an accepted receipt then executes under the same
evaluator with its published bytes pinned. Observed on macOS arm64:

| Fixture | Source bytes | Compile | Receipt bytes | Receipt run |
| --- | ---: | --- | ---: | --- |
| `nominal_single.delta` | 380 | accept | 3,235 | status 0, byte 28 |
| `nominal.delta` | 1,006 | accept | 5,091 | status 0, byte 28 |
| `nominal_order.delta` | 692 | accept | 4,849 | status 0, byte 4 |
| `nominal_distinct.delta` | 1,308 | accept | 5,603 | status 0, byte 4 |
| `nominal_swapped.delta` | 844 | accept | 4,849 | status 0, byte 6 |
| `nominal_short.delta` | 659 | Reject 16 at 515 | — | — |
| `nominal_wrong_field.delta` | 631 | Reject 15 at 598 | — | — |
| `nominal_owner.delta` | 770 | Reject 15 at 740 | — | — |
| `nominal_distinct_swapped.delta` | 1,298 | Reject 15 at 1,224 | — | — |

Receipt bytes include the emitted `$application` marker, the bound byte-rope
and ConformanceBytesV1 support members, and the lowered definitions, so they
are whole-program sizes, not per-payload costs. The payload's runtime shape is
visible in the receipt: `build_frame` lowers to `(pair 0 (pair node (pair
actual_count ...)))`, a tag word plus the same right-nested six-pair spine as
A — seven pairs, exactly C's emulated shape — and `consume_frame` lowers to
`first`/`second` spine projections that perform no runtime shape check at all.
Rejects are `compiler_source_rejection` frames: tag 1, space 1, the code, and
the offending construct's source byte offset in `coordinate`.

## What the fixtures establish

- Construction order and scope are identical under A, B, C, and D: fields
  evaluate left to right and decode positionally. D's `match` binds its seven
  fields in declaration order inside one arm, like the grouped `let`. Tail
  behavior is unchanged under every candidate: the consumer's final expression
  stays the tail call.
- Failure mapping differs by *when* the disagreement is decided. A traps at
  the first bad projection with no identity, arity, or field position in the
  observation. B and C map a wrong header word to a scalar the program chooses
  before decoding. D maps it to a compile-time Reject carrying a code and the
  exact source coordinate — `nominal_short.delta` answers arity code 16 at
  byte 515 where `pairs_short.gamma` traps anonymously, and no runnable
  artifact exists at all.
- Forged identities defeat source-level B (`forged.gamma` passes both checks
  and traps exactly like A) and cannot exist under D: identity is the
  declaration, not a word. `nominal_owner.delta` hands `consume_frame` an
  `OtherFrame` whose seven-Int layout is byte-for-byte the same shape and the
  owner judgment rejects it at compile time. Under the lowered representation
  the tag is again an ordinary pair word — D's guarantee is that no checked
  source can produce the forgery, which is why it never needs B's
  evaluator-minted runtime identity.
- Manual layout obligations removed: none under A, B, or C; under D the
  declaration checks arity, per-field type, and nominal owner between producer
  and consumer, so the whole spine-shape agreement stops being manual. That is
  the only candidate that removes the obligation the board item names.
- The residual hole narrows but does not close. A, B, and C all miss wrong
  shape under a right header; D misses only order among same-typed positions —
  `nominal_swapped.delta` compiles and publishes 6 where
  `nominal_order.delta` publishes 4, silently. The only nominal closure is a
  distinct declared type per position: `nominal_distinct_swapped.delta`
  rejects the same swap with code 15, at the price of seven wrapper
  declarations and one `match` per field read. Separately, D's checks live at
  compile time: a payload malformed at runtime would still trap like A, which
  does not arise for this customer because producer and consumer are inside
  one checked program.
- Cost where the checks already exist: the customer's implementation of these
  judgments is `checking/declarations/data.gamma` (4,205 bytes),
  `checking/types/calls.gamma` (4,731 bytes), `checking/types/matches*.gamma`
  (6,273 bytes across three members), and `checking/types/bindings.gamma`
  (3,530 bytes), inside a 147,840-byte packed compiler. D removes the manual
  obligation using machinery the customer already pays for in Delta; the
  question the acceptance asks is what the same machinery would cost inside
  Gamma.

## What an evaluator-owned checked product would cost

Counted against the selected Beta evaluator, not built:

- Representation: a variable-length record (magic, identity, arity, then
  value/tag word pairs) beside the fixed 40-byte pair record. `validate_pair`
  rejects references by `mod 0x28` alignment against a single heap base; a
  second record size needs either a second heap region with its own base,
  limit, and exhaustion status, or a fixed maximum arity padded to one record
  size. Either choice changes the capacity arithmetic pinned by
  `tests/gamma/heap-boundary/`.
- Beta implementation: new syntax forms for construction and projection or
  destructuring, evaluator-minted identities (a census-time constructor table
  or per-declaration allocation), and validation of identity and arity at every
  projection. This is new Beta in the trusted evaluator; the self-augmentation
  route cannot mint unforgeable identities from source.
- Contract and tests: `LANGUAGE.md` grammar and value sections,
  `EVALUATOR_PROFILE.md` status table if identity or arity failures get their
  own status, the evaluator-development behavior gate, and heap-boundary
  exact/adjacent fixtures.
- Proof obligations: the Beta encoding theory and derivation gates under
  `tests/gamma/` currently know pairs only; every new record kind adds
  formation, layout, and comparison rules.

## What static nominal typing inside Gamma would cost

Counted against the same evaluator, not built. The D fixtures witness the
semantics through the customer's own implementation; this counts the analogous
machinery in `bootstrap/2_gamma/gamma_evaluator.beta`:

- Grammar and census: `program := function+` gains `data` declarations, plus
  constructor application and `match` expression forms. The evaluator's census
  currently builds one function table (r88/r94); a nominal candidate needs a
  second global table for types and constructors with ordered field-type
  spines, including duplicate and unknown-name rejections before bodies are
  validated.
- Validation: `validate_and_classify_program` gains the declaration and call
  judgments — construction arity and argument types, match scrutinee owner,
  binder scoping, and exhaustiveness. The customer's Delta implementation of
  exactly these judgments is the member cost listed above; in Beta they are
  new trusted code, and they replace runtime checks rather than adding to
  them.
- Representation: none beyond pairs. The D receipts prove nominal values
  encode as `(pair tag spine)` on the existing 40-byte heap records — no new
  record kind, region, or capacity arithmetic, which is precisely where the
  evaluator-minted B design pays. A forged nominal word is impossible because
  validation only admits constructor forms at nominal positions; no runtime
  identity is needed.
- Resources: the same pair heap and the same capacity obligations; the lowered
  consumer performs zero shape checks, so runtime cost equals A's decode plus
  one tag word.
- Contract and tests: `LANGUAGE.md` gains the declaration, construction, and
  match sections and the boundary note that nominal values are checked rather
  than tagged at runtime; `EVALUATOR_PROFILE.md` needs no new status because
  disagreement rejects before evaluation; the evaluator-development gate gains
  the judgment and refusal cases mirrored by the `.delta` fixtures here.
- Proof obligations: the derivation gates under `tests/gamma/` know pairs
  only, so each new source form adds formation, layout, comparison, and
  substitution clauses to the Beta encoding theory and the derivation
  checker's theory corpus — for a feature whose runtime representation is
  unchanged.

The C fixtures settled the open question's cheaper half: the Delta-side status
at the frame boundary is already purchased — the customer's continuation frame
carries a checked kind word whose unknown-kind outcome is `InternalFailure`
code 1 — and adding it to an unmarked payload costs one pair and one `eq`
(131,359 steps for C versus 147,419 for B on the same decode). The D fixtures
now cover the other half's semantics: static nominal typing is the only
candidate that removes the producer/consumer layout agreement, it does so at
compile time with code-and-coordinate rejections instead of runtime traps or
chosen scalars, and its forgery and arity holes close entirely — while the
positional meaning of same-typed fields still survives every candidate unless
each field buys its own declared type, and a runtime-malformed payload would
still trap like A. The evidence continues to retain A's plain pairs with the
kind-boundary status the checker already has: the customer's actual
defect-localization history remains unmeasured, and no measurement here has
yet justified paying Gamma the census, judgment, contract, test, and proof
costs counted above — nor is the customer's payload order-sensitive in a way
the cheaper boundary check misses.
