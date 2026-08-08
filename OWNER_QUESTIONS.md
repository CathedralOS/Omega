# Owner Questions

Only unresolved owner-level language or architecture decisions belong here.
Settled decisions live in the language guide and design briefs; implementation
and deliberately deferred research live in `TASKS.md`. Questions are numbered
consecutively; pruning or adding one requires updating every repository
reference in the same change.

Last pruned: 2026-08-07.

## Q1 — Generic telescope on a subjectless conformance

A carrier-owned conformance inherits its static telescope from the subject
type. A carrierless evidence conformance has no such subject, but evidence
interfaces such as `ConvergenceEvidence<Left, Right>` need generic machine or
type binders whose names are in scope in the trait application and block
members. The settled subjectless block model does not yet give those binders a
source position.

Choose the declaration shape for that telescope. Plausible forms include:

```omega
satisfies<machine Left, machine Right>
    ConvergenceEvidence<Left, Right> as TogetherEvidence
{
    ...
}

satisfies ConvergenceEvidence<Left, Right>
    as TogetherEvidence<machine Left, machine Right>
{
    ...
}
```

A dedicated `conformance` declaration could also carry the name and telescope,
but would spend a keyword that the settled carrier-owned form does not need.

The choice must preserve the already-settled semantics: the name is
package-scoped; the binder telescope, instantiated trait application, and
closed normalized row map enter semantic identity; no binder is inferred as a
carrier; proposition binders use their ordinary authored `where proposition`
signatures; and concrete subjectless conformances remain expressible without a
vacuous telescope.
