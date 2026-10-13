# 0007: Binding receiver call shorthand

Status: draft proposal, not accepted syntax or an implementation prerequisite.
The current language requires the qualified static form described by
[expressions](../spec/language/expressions.md#calls-places-and-temporaries).
Examples using receiver syntax below are proposals only.

Affected owners: [expressions](../spec/language/expressions.md),
[service bindings](../spec/build/component_publication.md#bindings-and-era-entry),
[boundary calls](../spec/terminal-psi/boundary_calls.md), and guide chapters 1,
14, and 19.

## Problem

A call through an established `Binding<R>` currently repeats the requirement
name and writes the binding as an explicit leading call-authority argument:

```omega
Console::write_line(&context.console, "Hello");
```

This is uniform with Omega's static machine calls and keeps dot access as data
projection. It is longer than a receiver-style spelling when the binding's type
already identifies `R`.

## Proposed shorthand

Permit a place of exact type `Binding<R>` to select a requirement operation with
receiver syntax:

```omega
context.console.write_line("Hello");
```

The proposed form would resolve `write_line` only within exact requirement `R`,
borrow the binding as call authority, and pass every declared operation argument
in authored order. It would not add receiver syntax to other data, traits,
qualified machines, or arbitrary wrappers.

This proposal changes source syntax. Existing compiler acceptance and existing
source examples do not ratify it; they are migration evidence only. Until an
explicit owner decision accepts this proposal, conforming source uses the
qualified static form and a receiver-spelled call must reject.

## Alternatives

- **Keep the current explicit form.** `R::operation(&binding, arguments)` names
  both the requirement and authority. This is the accepted language today.
- **Ambient binding selection.** `R::operation(arguments)` would search an entry
  context for one matching binding. Reject: call meaning would depend on
  surrounding storage and ambiguity rules.
- **Make the binding an ordinary requirement parameter.** This would recursively
  expose `Binding<R>` in every operation signature and change requirement
  identity. It is not shorthand and is outside this proposal.

## Open questions

- Whether shortening this one authority-bearing call form is worth reintroducing
  a receiver exception after receiver syntax was removed from ordinary machines.
- Whether chained projections and temporary binding values would be permitted or
  only stable places.
- Whether diagnostics and formatting could keep the implicit requirement
  selection visible enough for authority review.

## Ratification

Acceptance requires an explicit owner decision. The accepting change must move
the exact rule into `expressions.md`, update the guide, add positive and negative
source fixtures, and migrate the resolver without granting receiver syntax to
ordinary data. This proposal alone authorizes none of that work.
