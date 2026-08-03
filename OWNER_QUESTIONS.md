# Owner Questions

Only unresolved owner-level language or architecture decisions belong here.
Settled decisions live in the language guide and design briefs; implementation
and deliberately deferred research live in `TASKS.md`. Questions are numbered
consecutively; pruning or adding one requires updating every repository
reference in the same change.

Last pruned: 2026-08-03.

## Q1 — What is the source surface for proposition families and aliases?

The law-bearing-relation ruling requires nominal proposition families over
representative values, typed proof-static index telescopes, and transparent
proposition aliases backed by carrierless selected-conformance evidence. It
does not settle how any of those declarations are written. The live grammar
has executable `bool` machines and transparent domain aliases, but neither is
a proposition family: running a decider is not evidence, and a domain
classifies one carrier rather than relating independently indexed values.
Which source model is canonical?

- add a dedicated proposition-family declaration with explicit static-index
  and representative-value binders, plus a distinct transparent proposition-
  alias declaration whose right side names carrierless evidence; or
- define proposition families entirely as a normalized projection of an
  ordinary proof trait/conformance, with a separate alias form naming that
  projection and no new nominal declaration kind?

The choice fixes proposition symbol identity, generic/index binder syntax,
how proposition application appears in `requires`/`ensures` and proof bodies,
how aliases bind hidden evidence terms, and which declaration selected
`Reflexive`/`Symmetric`/`Transitive` conformances name. Do not migrate `%` from
its executable-`bool` pilot or add a parser-only `Prop` spelling until this
surface is settled end to end.

## Q2 — How does a named whole-trait conformance bind its requirement satisfiers?

Omega promises that `Type satisfies Trait as Name` selects one coherent,
complete requirement surface and that one type may provide several such
surfaces. The live representation gives the standalone whole-trait edge a
stable `Type::Name` symbol, but validation currently chooses attached states by
requirement name without consulting `Name`. Separately, a machine may spell
`satisfies Trait::requirement as Name`; the guide's one-requirement proxy
example treats that spelling as enough to cast to `dyn Type::Name`, even though
no standalone whole-trait edge is declared. Which binding model is canonical?

- make the standalone whole-trait declaration own an explicit, complete
  requirement-to-satisfier map (including defaults and laws), with machine
  aliases serving only as references used by that map; or
- define a conformance as the coherent group of attached satisfiers carrying
  the same `as Name`, with a precise rule for whether a standalone declaration
  is required and whether unique unaliased/default satisfiers may fill missing
  members.

The choice fixes completeness checking, default-member inclusion, overload
selection, whether two named conformances may deliberately share a satisfier,
third-party conformance coherence, and the exact per-requirement adapter rows
stored in a local dynamic table. Checked selection may retain the stable edge
identity, but Psi and Omega must not emit requirement adapters or a table by
guessing satisfiers from matching state names until this association is
settled.

## Q3 — What is the complete-contract surface for abnormal non-return?

The settled model puts deliberate nuclear abort, explicit trapping arithmetic,
and other non-returning control outcomes on a failure/control axis independent
from service reach, suspension, blocking, and ordinary termination. It does not
settle the source spelling or the normalized row propagated through callable
contracts. Which surface is canonical?

- add dedicated declaration and terminal-statement spellings for nuclear abort,
  with trapping arithmetic contributing the same normalized control axis; or
- add a general declared control-outcome row whose closed vocabulary includes
  nuclear abort and trap, with statements and selected operations naming one
  outcome from that row?

The choice fixes contract identity and entailment, call-site propagation,
whether trap-capable arithmetic is invocation-refined or categorically
published, ordinary-edge versus no-cleanup terminator representation, terminal
Psi vocabulary, and build-time admission diagnostics. The parser currently
accepts contextual `trap` by lowering it to an ordinary terminal transition;
that erases the required semantic distinction and must not be treated as a
settled spelling or control-outcome fact.
