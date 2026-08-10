# Owner Questions

Only unresolved owner-level language or architecture decisions belong here.
Settled decisions live in the language guide and design briefs; implementation
and deliberately deferred research live in `TASKS.md`. Questions are numbered
consecutively; pruning or adding one requires updating every repository
reference in the same change.

Last pruned: 2026-08-08.

## Q1 — Fixed-operator surface-binding syntax

Named `operator` declarations are the semantic identities behind fixed surface
tokens such as `+`, `[]`, and `[..]`. The language guide previously wrote this
association with a `spelling` clause, but that keyword and clause shape were
never approved and are not part of the settled language.

Choose the source form that binds a fixed operator token to a named declaration.
The decision must settle where the binding appears relative to the signature and
contract, how punctuation-shaped tokens such as `[]` and `[..]` are named, and
whether one declaration may bind more than one fixed token. It must preserve the
settled semantics: the named path remains canonical, resolution is static and
operand-directed, the public signature and proof contract remain visible, and a
`boundary operator` differs only in how its implementation is supplied.
