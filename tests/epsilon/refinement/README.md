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

## D closure members

The gate's second leg refines the model over the **exact D closure member
sources** — the same eight programs
[`tests/epsilon/d-composition/`](../d-composition/README.md) checks and
executes through the canonical edge. [`d_closure.py`](d_closure.py) assembles
the six whole-member customers from the live, digest-pinned members under
`bootstrap/5_omega/` plus the pinned customer mains under
`tests/epsilon/interpreted-omega-experiment/customers/`, then the two
whole-closure customers from the bound 558,065-byte packed closure and the
d-composition entry sources (`main.epsilon` reading the sealed
[`program.omg`](../d-composition/program.omg) stdin, and
`check_only_main.epsilon`). Member and packed identities restate the bound
records, so a changed member refuses before any evaluator invocation.

Each customer derives its observation through `model.observation` and the
canonical edge must publish it byte-for-byte — including the
complete-closure customer, where the model itself interprets D's own
`OmegaScalarCompiler::compile` of the sealed Omega source and the edge's
emitted Alpha tape must equal the model's bytes. Each customer then applies
its bounded member mutations: one spelling change per D member across the
set — an `InvalidArrayLength` formation reject plus an invalid-byte reject
in `representations`, trap discriminations in `lexical_classification`
(a whitespace arm and a digit bound, each turning the customer's exit into
an `Assertion` trap), and invalid-byte lexical rejects at packed offsets
inside `alpha_tape`, `request_and_utf8` (twice), `lexer`, `parser`,
`scalar_compilation`, and `outcome`.
The model re-derives every mutated observation; the edge must match, and a
mutation that leaves the observation unchanged fails the gate rather than
counting as coverage. A member or mutation whose constructs sit outside the
model's declared fragment is recorded as `ModelExcluded` in the per-customer
and summary lines — never assigned a judgment to force agreement.

## Running

```sh
sh tests/epsilon/refinement/run.sh
```

The gate needs macOS arm64 or Windows x64 (the bound Gamma tape runs
natively) and `python3`. The canonical evaluator receipt is reconstructed
once per run (~5 minutes), the ~120 synthetic-corpus request executions take
a few minutes, and the D-member leg is the slow part: the edge's per-customer
times recorded for d-composition are roughly 60, 550, 670, 510, 140, 450,
3,700, and 3,300 seconds respectively on a loaded host, with each lexical
member mutation priced at one lex of the packed prefix and each trap or
formation mutation at one full customer invocation. This is an explicit slow
gate in the same shape as d-composition, not a routine smoke.

Selecting customers runs only the named D members after the corpus:

```sh
sh tests/epsilon/refinement/run.sh 'Omega D lexer' 'Omega D numeric-base sums'
```

`--skip-members` runs only the synthetic corpus; `--skip-corpus` skips it
(useful for iterating on the D leg). `OMEGA_REFINE_D_SECONDS` overrides the
14,400-second per-invocation watchdog for the D leg.

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
