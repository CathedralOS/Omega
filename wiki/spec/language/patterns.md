# Patterns and dispatch

A dispatch evaluates its subject once, tests alternatives in authored order,
and executes only the selected arm. Matching contributes facts valid for that
subject and program point. Failed alternatives do not execute their bodies.
An expression `match` produces compatible arm values; a `transition` selects an
outgoing control edge. Both use the same pattern vocabulary.

Decomposing a `some` package uses one stable subject/version opening for all
dependent fields. Patterns cannot guess its concrete witness, equate it with
another package's witness, or let dependent bindings escape their valid scope.
This is ordinary decomposition under the [package opening rules](some_packages.md#opening-by-stable-subject),
not a runtime type test.

## Pattern subjects and bindings

Scalars use value patterns. Records use structural or domain patterns.
Case-bearing subjects use case patterns, domain patterns, and payload bindings.
Tuple dispatch combines component patterns; `_` waives a component without
creating a value or fact about it. Payload bindings require a known case shape.

```omega
transition header {
    Header { ok: 0, version } -> accept(version)
    Header { ok as _, version as _ } -> reject()
}
```

`field` binds the field, `field as name` renames it, `field as _` waives it, and
`field: value` is ordinary projected equality. Without `..`, a structural pattern
mentions every field; adding a field invalidates incomplete patterns. `..` opts
out of this drift check in arm position. Extraction uses the saved subject and
retains ordinary ownership and borrow obligations.

## Data and case decomposition

Named decision: `record-or-sum-data`, settled 2026-11-10. It replaces
`separate-data-and-case-decomposition` (2026-10-29) and `case-projection-type`
(2026-11-09).

A record decomposes into its declared fields. A sum is matched by case. A
record that [holds a sum](data_and_literals.md#records-sums-and-nested-declarations)
decomposes into its fields, and the sum is matched through its field like any
other subject:

```omega
data Command {
    command_id: u64;

    op: data Operation {
        case GetUser(user_id: u64);
        case CountUsers;
    };
}

machine targets_one_user(op: &Command::Operation) -> bool {
    match op {
        Command::Operation::GetUser { user_id as _ } -> true
        Command::Operation::CountUsers -> false
    }
}

machine inspect(command: &Command) -> bool {
    targets_one_user(&command.op)
}
```

Named and positional data patterns express the same record decomposition. A
positional pattern binds fields by position; binding names need not match
declaration names. These positions are semantic declaration order, not stable
schema numbers, physical field offsets or layout-policy order. For `Command`
the positional pattern is `(_, operation)`. Named patterns account for every
field once; positional patterns have the corresponding exact arity. An
explicit `..` may waive the remaining components. `_` and `field as _` ignore
bindings, not ownership, cleanup or validity obligations.

A case pattern destructures only that case's payload, and case names keep
their owner-qualified paths. Patterns are one level deep: a record pattern
binds or tests its fields and does not contain a case pattern. A held sum is
matched as its own subject, alone or as a tuple component:

```omega
match command.op {
    Command::Operation::GetUser { user_id } -> lookup(user_id)
    Command::Operation::CountUsers -> count()
}

match (command.command_id, command.op) {
    (0, _) -> reject()
    (_, Command::Operation::GetUser { user_id }) -> lookup(user_id)
    (_, Command::Operation::CountUsers) -> count()
}
```

Tuple dispatch preserves each subject's ordinary ownership and borrowing:
inspecting a non-copy field does not move it out of a borrowed parent. The
parent's established predicates combine with the case fact about `command.op`
as they do in a direct match.

A pattern has no `case` selector, a record has no
case-only component, and no type names one: `case as name` and `Data::case`
reject.

### A sum held in a field

A field that holds a sum is an ordinary projected place, and the rules for
facts, loans and invariants apply to it without a special route.

- **The case fact belongs to the place.** Matching or testing `command.op`
  establishes a fact about that exact field place. Outer bindings and
  still-valid facts remain available inside nested matches, and mutation
  invalidates facts under ordinary footprint rules.
- **The holder's predicates combine with it.** Matching `command.op` of an
  established `Command` combines the selected case with `Command`'s
  default- and named-domain predicates, including case-dependent type
  equalities. It drops no constraint on the other fields and creates no
  validity or authority evidence.
- **A helper sees the sum's own contract.** A machine receiving
  `&Command::Operation` is checked independently. It has the sum's default
  domain and each payload field's declared type under the selected case. Facts
  the caller holds about the complete record or its other fields do not cross
  the call; a helper that needs one states it as its own requirement, and the
  caller proves it.
- **Lending the field keeps the holder's obligations.** Through
  `&mut command.op` a helper may mutate the active payload or replace the
  case. The lender owes the complete record's predicates again at handback
  under [contract-based repair](dependent_values.md#contract-based-repair).
- **Ownership is ordinary.** Borrowed decomposition retains the subject's
  lifetime, access and loans, and the other fields stay disjoint sibling
  places. Owned decomposition follows ordinary move, affine cleanup and
  linear-custody rules, and a sum moved out of its field is an ordinary owned
  value.

Matching an established value combines its [default-domain predicates](data_and_literals.md#case-constraints)
and any established named-domain predicates with the selected case fact, all
substituted onto the exact subject and payload bindings. Case-conditional
consequences follow from that combination, not case-local clauses or a special
GADT proof route. Type equalities use ordinary generic equality reasoning;
they may justify operations and results in the arm without a cast. They do not
change the selected type arguments. At joins, only facts justified on every
surviving predecessor remain. Payload mutation and case replacement retain
ordinary subject invalidation and loan rules. Each dispatch evaluates its own
subject once and proves its own coverage.

## Irrefutable data bindings

Named decision: `irrefutable-data-bindings`, settled 2026-10-29.

Inside a machine body, `let Pattern = expression;` uses the same named or
positional data-decomposition patterns as dispatch. The subject's checked type
and live facts must prove the pattern irrefutable: every admitted value reaches
that binding. A pattern that can fail rejects; a binding introduces no hidden
branch, trap, fallback or runtime assertion. Use ordinary match arms when
alternatives remain possible.

For an existing `command: &Command`, the outer match above may instead be written:

```omega
let Command { command_id as _, op as operation } = command;

match operation {
    Command::Operation::GetUser { user_id as _ } -> true
    Command::Operation::CountUsers -> false
}
```

The positional binding is `let (_, operation) = command;`. Named selectors,
arbitrary positional binding names, exact component coverage, explicit rest and
ignore forms have the same meaning as in a match pattern. Evaluate the initializer
once and retain its exact subject snapshot; projections do not reevaluate it.
Bindings enter the following lexical scope only after formation, irrefutability,
access and custody obligations are discharged. They retain the same static
arguments, domain facts and mutation-invalidation rules as dispatch bindings.

Owned decomposition follows ordinary move/copy disposition and residual cleanup;
it adds no implicit cloning or relaxed multiplicity. A complete consuming
pattern on a subject with a selected nominal cleanup ends that wrapper without
invoking the cleanup, and is accepted only in the carrier's owning package or
its selected cleanup machine under
[owned cleanup consumers](ownership.md#consumers-and-cleanup). A field-value binding from
borrowed storage still needs ordinary copy eligibility. Non-copy fields require
an otherwise-authorized explicit borrow or waiver, not an implicit field move.
A binding of a borrowed field retains the source's borrow and backing lifetime
rather than copying its carrier. No reference-pattern syntax, new binding mode or access
authority follows from destructuring. Ignored components retain their ordinary
drop, retirement and restoration obligations.

The match and transition forms remain valid, including an irrefutable
one-arm outer match followed by an inner case match. Prefer a local binding when
only decomposition is intended. There is no separate `match expression as Pattern`
header form. Ordinary scalar local declarations are unchanged.
`RECORD-OR-SUM-DATA` in [TASKS.md](../../../TASKS.md) owns implementation and
source/tooling acceptance of this binding spelling.

## Coverage and domain tests

Every dispatch must cover the admitted subject. Missing runtime coverage is a
compile error, not an implicit trap or fallthrough. A wildcard closes coverage.
Coverage is checked separately for each authored dispatch. A later dispatch,
ordinary result, or explicit crash cannot cover alternatives missing from an
earlier dispatch, even when lowering places their control edges next to it.
Finite case coverage includes ordinary and historical-lineage sums, pure
case-union domains, and prior facts excluding impossible cases. Boolean coverage
includes true/false and complete Boolean tuple alternatives; complementary
equality/inequality guards over the same unchanged subjects cover their domain.
Other value/comparison ladders and predicate-domain guards require a wildcard
unless their coverage is otherwise established by the specified judgment.

Default- or named-domain constraints may exclude a case when selecting it
contradicts the subject's established facts, including exact type equalities.
Coverage consumes that evidence; it does not require unrestricted theorem search
over case predicates. If impossibility is not established, the case still needs
coverage or an explicit proof discharging the dead arm. Failure to construct or
prove a case is not evidence that it cannot occur.

An intentional Unit completion is explicit, for example `_ -> {}`. A result-
producing context still requires a compatible result. A wildcard does not supply
missing guarantees, discharge linear ownership, or satisfy a migration
completeness theorem merely by matching unknown input.

Executable domain tests require pure, finite runtime-checkable predicates and
any routed provenance required by [domain membership](domains.md#refinement-and-executable-membership).
A proof-visible proposition alone does not imply an executable test. Overlapping
domain alternatives use first-match order; unordered union proofs do not.
