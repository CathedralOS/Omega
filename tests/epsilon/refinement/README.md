# Epsilon evaluator refinement gate

`sh tests/epsilon/refinement/run.sh` performs the direct checked-evaluator
refinement that `bootstrap/4_epsilon/LANGUAGE.md` section 12 requires: the
canonical evaluator edge (the packed `epsilon_compiler.delta.sources` closure
plus the canonical entry, reconstructed through the bound Delta compiler and
executed on the bound Gamma tape, all identities pinned in
[`tools/bootstrap/epsilon/evaluator_env.sh`](../../../tools/bootstrap/epsilon/evaluator_env.sh))
is checked against an **independent reconstruction** of `CheckEpsilon` and
`RunEpsilon` under mutations of the source, the sealed stdin, the resource
profile, and the observation.

## The independent model

[`model.py`](model.py) is a test-owned reconstruction of the Epsilon v1
contract — lexer, parser, closed-reason checking with exact coordinates, and
the `RunEpsilon` observation — written from
[`bootstrap/4_epsilon/LANGUAGE.md`](../../../bootstrap/4_epsilon/LANGUAGE.md)
rather than from the evaluator's Delta sources. The evaluator's published
observation must equal the model's derived bytes on every input; neither side
is the oracle, so a disagreement is a witnessed conformance gap whichever side
the contract contradicts.

The contract grammar leaves one disambiguation unwritten: whether `-> return`
carries an expression when another transition arm follows. The model
reproduces the evaluator's pattern-arrow probe for that read
(`return_is_bare`); every other judgment is contract-derived. Constructs
outside the modeled fragment raise `ModelExcluded` rather than inventing a
judgment; see the `model.py` docstring for the declared fragment.

## Mutations

[`corpus.py`](corpus.py) holds the base programs; [`gate.py`](gate.py) applies
mutations and re-derives every expectation through the model:

- **Source mutations** — operators, literals, names, case bindings, arms, and
  call targets changed one spelling at a time across machines, states,
  records, sums, arrays, views, Console effects, recursion, and every trap
  kind. Each mutated program's observation is re-derived by the model and
  must match the evaluator's byte-for-byte.
- **Rejection mutations** — lexical defects (`InvalidSourceByte`,
  `InvalidToken`, `InvalidCharacterLiteral`, `UnterminatedString`,
  `InvalidEscape`, `IntegerLiteralOutOfRange`), parse defects
  (`UnexpectedToken`, `UnexpectedEnd`), checking defects (`DuplicateName`,
  `MissingEntry`, `InvalidEntry`, `UnknownName`, `UnknownType`,
  `ArityMismatch`, `TypeMismatch`, `InvalidArrayLength`, `EscapingView`,
  `InvalidTerminal`, `DuplicatePattern`), block-exit relations (a bare
  `return` in a value machine, a `return` value in a resultless machine, an
  absent terminal in a value machine, a resultless machine call used as a
  continuation in a value machine, and an executable construct after a
  `never` statement), and type-formation placements (`u8` outside stored
  data, `never` outside a return type, `Console` as a declared type, a
  zero-length array, a view field) must publish the same closed reason and
  coordinate.
- **Stdin mutations** — alternate sealed inputs (including EOF, NUL, and
  0xFF) re-derive `read_byte` results and trap prefixes through the model.
- **Profile mutations** — every unassigned EREQ profile refuses with the
  EEOUT `unknown_profile` frame; no Epsilon observation is published.
- **Observation mutations** — every byte position of the expected
  observation, plus truncation and extension, is checked to discriminate:
  the published bytes equal the model's and nothing else.

## Running

```sh
sh tests/epsilon/refinement/run.sh
```

The gate needs macOS arm64 or Windows x64 (the bound Gamma tape runs
natively), `python3`, and a few minutes: the canonical evaluator receipt is
reconstructed once per run (~4 minutes) and each of the ~120 request
executions is a full evaluator invocation. The gate prints one summary line
on success and stops at the first divergence with both sides' bytes.

The status-252 pair-arena boundary is intentionally not exercised here; it is
witnessed directly by `tests/epsilon/pair-boundary/`. Resource refusals
(outer `Incomplete` statuses 250/252/253/254) remain covered by
`tests/epsilon/evaluator-entry/` and `tests/epsilon/pair-boundary/`; this
gate refines the admitted observation paths and the request-profile
refusals.

## Bounded scope

The evaluator collects every candidate rejection and publishes the one at
the minimum source coordinate; the model raises the first defect it
reaches, except in the one place where declaration-order traversal provably
diverges from minimum offset (type formation, which the model collects
program-wide before raising). Corpus mutations therefore produce at most
one defect each — the gate compares single-defect observations. Constructs
outside `model.py`'s declared fragment (for example forward local
references, which the evaluator resolves through pending bindings as
`UseBeforeInitialization`) raise `ModelExcluded` instead of a guessed
judgment and are not exercised by the corpus.
