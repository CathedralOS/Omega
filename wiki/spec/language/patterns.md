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

Named decision: `separate-data-and-case-decomposition`, settled 2026-10-29.

A record decomposes into its declared fields. A mixed value decomposes into
its common fields in authored declaration order, followed by one case-only
component. All inline cases contribute to that one final component, not one
component per case. Inline case declarations form a consecutive tail after the
common fields under [data declarations](data_and_literals.md).

Named and positional data patterns express the same decomposition. A named
pattern uses ordinary field bindings and `case as name` for the anonymous case
component. A positional pattern binds components by position; binding names
need not match declaration names. These positions are semantic declaration
order, not stable schema numbers, physical field offsets or layout-policy order.

```omega
data Command {
    command_id: u64;
    case GetUser(user_id: u64);
    case CountUsers;
}

machine targets_one_user(command: &Command) -> bool {
    match command {
        Command { command_id as _, case as operation } ->
            match operation {
                Command::GetUser { user_id as _ } -> true
                Command::CountUsers -> false
            }
    }
}
```

The equivalent positional outer pattern is `(_, operation)`. For a record
without cases, the positional sequence contains only its fields. Named
patterns account for every common field and the case component once; positional
patterns have the corresponding exact arity. An explicit `..` may waive the
remaining components. `_`, `field as _` and `case as _` ignore bindings, not
ownership, cleanup or validity obligations.

`case` is a reserved pattern selector, not a stored field or a general field
access. It introduces no schema member, stable number, storage slot, additional
discriminator or independently constructed enum. Ordinary field renaming remains
`field as binding`; `field: value` remains a value test, not a rename.

The inner case pattern destructures only that case's payload. Common-field
names in a case pattern are retired: obtain them in the outer data pattern.
A pure sum is already case-only and supports direct case matching without a
redundant outer pattern. Case names retain their original carrier-qualified
paths, as `Command::GetUser` above; projection does not invent a new case owner.

A case view retains the original subject, static arguments and exact storage
identity. Outer bindings and still-valid facts remain available inside nested
matches; selecting a case combines its fact with the original value's established
default- and named-domain predicates, including case-dependent type equalities.
It does not drop common-field constraints or create validity/authority evidence.
Mutation invalidates facts under ordinary footprint rules.

Borrowed decomposition retains the subject's lifetime, access and loans. Owned
decomposition follows ordinary move, affine cleanup and linear-custody rules;
the common fields and active payload cannot be duplicated or disposed of twice.
The view cannot escape its backing lifetime or widen access. Each dispatch
evaluates its own subject once and proves its own coverage; projecting the case
component never reevaluates the expression that produced the outer value.

This changes patterns, not case-literal construction or physical layout.

### Case projection type

Named decision: `case-projection-type`, settled 2026-11-09. Implementation is
tracked by `CASE-TYPE`; the [design record](../../drafts/designs/case_projection_type.md)
lists the questions left open.

`Command::case` is a type expression naming the case component of the mixed
data `Command`, and `Command<T>::case` names it for one application of generic
data. It is the type of the component a `case as name` pattern already binds,
so a machine can accept that component without receiving, or decomposing
again, the complete value:

```omega
machine targets_one_user(operation: &Command::case) -> bool {
    match operation {
        Command::GetUser { user_id as _ } -> true
        Command::CountUsers -> false
    }
}

machine inspect(command: &Command) -> bool {
    let Command { command_id as _, case as operation } = command;
    targets_one_user(operation)
}
```

Decomposing `&Command` binds `operation` at `&Command::case`, and decomposing
`&mut Command` binds it at `&mut Command::case`. The type's identity is the
data declaration, its static arguments and this projection. Two declarations
with identical cases have distinct projection types. `case` is reserved, so the
path cannot collide with a case or member name.

Naming the projection adds nothing to the component. It introduces no case
owner, tag, storage slot, allocation, copy or independently selected layout,
and cases keep their original paths such as `Command::GetUser`. A projection
type is written only as the referent of a reference. It has no literal, zero
default or cast, and a value of it arises only by decomposition. A by-value
`Command::case` annotation rejects; owned decomposition keeps its existing
rules inside the machine that performs it. `Data::case` also rejects for data with no
common fields, which is already case-only, and for a record with no cases.

A reference to the projection is the view described above. It retains the
original storage identity, lifetime, access mode and loans, cannot escape its
backing lifetime or widen access, and reaches only the case component: no
operation recovers the enclosing value or its common fields from it. The
common fields remain disjoint sibling places under ordinary projected-place
rules.

A machine receiving the projection is checked independently against its own
contract. The type guarantees each payload field's declared type under the
selected case. A data `requires` clause belongs to the complete value and is
not implied by the projection type. Facts the caller holds about the complete
value or its common fields do not cross the call; a helper that needs one
states it as its own requirement over the projection, and the caller proves it.

Through `&mut Command::case` a helper may mutate the active payload or replace
the case under ordinary rules. The lender keeps every obligation of the
complete value under [contract-based repair](dependent_values.md#contract-based-repair):
its default-domain and named-domain predicates must be provable again at
handback from the helper's guarantees.

Matching an established value combines its [default-domain predicates](data_and_literals.md#case-constraints)
and any established named-domain predicates with the selected case fact, all
substituted onto the exact subject and payload bindings. Case-conditional
consequences follow from that combination, not case-local clauses or a special
GADT proof route. Type equalities use ordinary generic equality reasoning;
they may justify operations and results in the arm without a cast. They do not
change the selected type arguments. At joins, only facts justified on every
surviving predecessor remain. Payload mutation and case replacement retain
ordinary subject invalidation and loan rules.

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
let Command { command_id as _, case as operation } = command;

match operation {
    Command::GetUser { user_id as _ } -> true
    Command::CountUsers -> false
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
A case-view binding retains the source's borrow and backing lifetime rather than
copying its carrier. No reference-pattern syntax, new binding mode or access
authority follows from destructuring. Ignored components retain their ordinary
drop, retirement and restoration obligations.

The original match and transition forms remain valid, including an irrefutable
one-arm outer match followed by an inner case match. Prefer a local binding when
only decomposition is intended. There is no separate `match expression as Pattern`
header form. Ordinary scalar local declarations are unchanged.
`DATA-DECOMPOSITION` in [TASKS.md](../../../TASKS.md) owns implementation and
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
