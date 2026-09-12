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
(macOS arm64 or Windows x64). The second runs the selected evaluator tape under
the untrusted `tests/alpha/reference/alpha_ref.py` and prints Alpha instruction
counts and pair allocations; it exists so a host without a seed can still
reproduce the numbers below, and its output labels itself diagnostic.

## Candidates

| Fixture | Candidate | Build | Decode | Observation |
| --- | --- | --- | --- | --- |
| `pairs.gamma` | A: nested pairs, copied from `calls.gamma` | six `pair` | five spine `second` bindings, seven field projections | status 0, byte 28 |
| `pairs_short.gamma` | A with a six-field producer | five `pair` | same consumer | authored trap, status 2 |
| `checked.gamma` | B: identity and arity words ahead of the same spine | eight `pair` | two `eq` checks, then A's decode | status 0, byte 28 |
| `checked_wrong_tag.gamma` | B given another constructor's identity | hand-built | identity check fails | status 0, byte 201 |
| `checked_wrong_arity.gamma` | B given a producer declaring six fields | hand-built | arity check fails | status 0, byte 202 |
| `forged.gamma` | B given six fields under a forged seven-field header | hand-built | both checks pass | authored trap, status 2 |

Candidate B is the smallest source-level form of a "dynamically checked named
product": the constructor is a named function, its identity is a nullary
constant, and the consumer checks identity and arity before decoding. Static
nominal typing is a third candidate that needs declaration and call judgments
the Gamma checker does not have; it is not represented by a fixture because no
source in the selected language can express it, and it must not be read as
implied by B.

## Measured under `--reference` at the selected tape

| Fixture | Source bytes | Definitions | Alpha steps | Pairs |
| --- | ---: | ---: | ---: | ---: |
| `pairs.gamma` | 1,064 | 3 | 101,859 | 6 |
| `pairs_short.gamma` | 1,041 | 3 | 96,049 | 5 |
| `checked.gamma` | 1,560 | 6 | 147,419 | 8 |
| `checked_wrong_tag.gamma` | 595 | 5 | 51,722 | 4 |
| `checked_wrong_arity.gamma` | 591 | 5 | 55,275 | 4 |
| `forged.gamma` | 1,232 | 5 | 111,655 | 7 |

Steps count every Alpha instruction from tape start to halt, so they include
the evaluator's census and validation of the whole source, which grow with
source size; they are a whole-run cost, not an isolated construction or
projection cost. Pairs are 40-byte records, so the seven-field payload costs
240 bytes as A and 320 bytes as B.

## What the fixtures establish

- Construction order and scope are identical: both candidates evaluate fields
  left to right through ordinary `pair`, and both decode through one grouped
  `let`. B adds no binding forms. Tail behavior is unchanged: the consumer's
  final expression remains the tail call in either candidate.
- Failure mapping differs. A can only trap: a shorter producer reaches the
  authored trap with no identity, arity, or field position in the observation.
  B maps a wrong identity or arity to a scalar the program chooses before any
  projection, which is the only defect-localization gain B offers.
- Forged identities defeat source-level B. The identity and arity words are
  ordinary integers, so `forged.gamma` passes both checks and then traps
  exactly like A. Source-level checks document the layout convention; they do
  not enforce it. Enforcement requires an evaluator-minted identity that source
  cannot construct, which is exactly how the selected evaluator already
  protects pairs (private heap range, record alignment, `PAIR` magic, and per
  field scalar/pair tags in `validate_pair`).
- Manual layout obligations removed by source-level B: none. The producer and
  consumer still agree on the right-nested spine by convention; B adds two more
  words to agree on.

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

Adoption is not decided here. The remaining open comparison is whether the
customer's actual defect history justifies that evaluator cost, or whether the
one gain B demonstrates (a chosen failure status instead of an anonymous trap)
is better bought by a Delta-side status at the frame boundary without any new
Gamma value kind.
