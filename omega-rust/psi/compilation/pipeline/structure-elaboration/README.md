# Structure-only elaboration

This crate turns frozen typed trees into an unsealed Terminal Psi module and
fills the [check tables](../../representations/terminal-psi/check-tables/README.md)
that describe it, for route one's scalar subset. It is the second piece of
[check on the lowered form](../../../../../wiki/drafts/designs/check_on_lowered_form.md):
it proves nothing, reads no checker fact, and mints every obligation the
lowered form implies without discharging any. It is not wired into the
default compile and replaces nothing yet.

Start at [lib.rs](src/lib.rs) (`elaborate`). [read.rs](src/read.rs) resolves a
machine's typed trees into the closed scalar body of [body.rs](src/body.rs);
[lower.rs](src/lower.rs) turns that body into blocks, values, operations,
edges and invocations.

## The subset

Machines with one state and a complete body whose formals, locals and result
are builtin integers (`i8` to `u64`) or `bool`, using: scalar locals and
assignment to them; integer and Boolean literals; builtin `Exact` integer
arithmetic, bitwise operations, shifts and comparisons; `!`, `~`, `&&` and
`||`; direct calls of other such machines with `requires` and `ensures`
clauses over their formals; guarded transition arms; and returns. Everything
else is refused with `RefusalReason::Unsupported` and a detail naming the
construct (`expression match`, `type reference`, `transition crash`, ...): a
reference, record, slice, loop, recursion through a cycle, generic, callable
value, suspension, assembly, crash, cast, float, domain or service reach is
never skipped, approximated or passed through. A refusal anywhere in the
closure refuses the whole closure.

## How identity is fixed

- **Types** are scalar only through `SymbolTable::builtin_type_atom` on the
  resolved symbol of a `Named` type: the compiler-installed root slot. The
  typed trees' `PrimitiveType::from_name` classifies by spelling and is not
  used. A `Named` type with an invalid symbol is refused as a spelling; a
  declaration spelled `u32` is a different symbol and is refused as nominal.
  Integer literals take their type from the landing stage 04 recorded.
- **Names** are slots only when the path has one member and a valid symbol
  equal to a formal's or local's symbol. A name without a symbol is refused.
- **Callees** are `MachineId`s reached from a `SymbolKind::Machine` symbol.
- No table here is keyed by text; text appears only in `Refusal::detail`.

## Evaluation order and control

From [the evaluation schedule](../../../../../wiki/spec/language/expressions.md#evaluation-schedule):
strict binary operands left then right, call arguments left to right, each
operation emitted after its operands, so block order is evaluation order.
`a > b` lowers to `IntegerLessThan(b, a)` over already evaluated operands;
`a != b` is `BooleanNot(IntegerEqual(a, b))`.

`left && right` ends the current block in a `Conditional` on the left value.
The true edge enters a block that evaluates `right` and jumps to a join with
it; the false edge goes straight to the join carrying the left value, which
is the result there. The join's one Boolean block parameter is the result.
`||` swaps the two edges.

Stage 01 desugars `transition subject { arms }` into a `DispatchSubject`
local, evaluated once, followed by one `Transition` statement per arm
(`01_tokens-to-syntax-trees/src/bodies_and_transitions.rs`,
`parse_transition_block_handles`). Each guarded arm ends the current block in
a `Conditional`: true enters the arm, which completes the machine; false
continues with the next statement. An unguarded arm completes the machine and
must be last. An arm's target is a value, a terminal call (a call whose
result completes the machine, [state contracts](../../../../../wiki/spec/language/state_contracts.md#terminal-calls-in-transition-arms):
lowered as a call followed by a return) or Unit.

Scalar locals are SSA values: a binding or assignment names the value the
slot holds from then on. Every arm completes the machine, so the only joins
are the short-circuit joins, and the lowered form has no structural places:
the place, root and write tables are empty because the form implies none.

## Obligations

| Kind | Where | Created at |
| --- | --- | --- |
| `OperationSafety` | each `Exact` add, subtract, multiply, divide, remainder, shift (every primitive whose `integer_policy_bridge` has formation conditions) | the point before the operation |
| `CallRequirement` | one per callee `requires` clause per call, clause = position in the callee's canonical `requires` | the point before the call |
| `NormalGuarantee` | one per `ensures` clause per normal exit edge | that edge |

## Source correspondence

`MachineSources` holds one `SourceOrigin` per block, operation, edge and
obligation, indexed by the same identity and minted in lockstep with it.
An origin is the authored span, or `Unpositioned` with the reason
(`MachineEntry`, `ShortCircuitRight`, `ShortCircuitJoin`,
`DispatchContinuation`, `ImplicitReturn`, or `SpanAbsent` when the typed
tree's span is empty). It never borrows a neighbour's position.
`LoweredSourceCustody` was not used: it records call occurrences only and
depends on `checked-trees`.

## What is reused from the current pipeline

Pure helpers only: `numerics::integer_policy::integer_policy_bridge` (which
operations are partial), the Terminal Psi data types,
`terminal_codec::canonical_proposition_order_key` and
`canonical_scalar_term_order_key` (Terminal Psi's canonical order). Nothing
from Psi 05, validation, checked trees or Psi 06 is used or depended on.

## Findings

Each is a gap between this route and the contract it builds against; none was
worked around by adding to `terminal-check-tables`.

1. **`ensures` cannot name `result` by identity.** The typed trees hold the
   synthetic `result` as a name path with invalid symbols, recognised only by
   spelling (`typed-trees/src/values.rs`, `names_reserved_result`, which calls
   `Identifier::is_reserved_result`). The reader refuses any name without a
   symbol, so every `ensures` clause that mentions `result` is refused
   (`spelling name without a symbol`). Only clauses over formals elaborate.
2. **No `ValueOrigin` for the scalar result pseudo-value.**
   [Calls and outcomes](../../../../../wiki/spec/terminal-psi/calls_and_outcomes.md#normal-results):
   "Scalar | One typed value with a stable result pseudo-value". The module's
   `TerminalMachineResult::Scalar` needs a `ValueId`; `ValueOrigin` has no
   variant for it and that enum is not an extension point. The result gets the
   identity after the last body value and has no `values` row.
3. **One `ensures` obligation per clause in Terminal Psi, one per exit in the
   tables.** `MachineContract::ensures` holds a `ContractClause` with one
   `ObligationId` per clause (`terminal-psi/src/proof.rs`), while control flow
   and `ObligationKind::NormalGuarantee` have one per (normal exit, clause).
   The module's clauses carry the first exit's identities; the others exist
   only in the tables.
4. **No summaries.** `Summary::requires` and `result_relation` are
   `PropositionId`s into "the module's proposition table", but Terminal Psi
   keeps contract propositions inline in `MachineContract` and its only
   `PropositionId` table (`proposition_declarations`) is named propositions
   with a `String` name. `CheckTables::summaries` is left empty; the other
   axes (reach, termination, crash routes) are not structure either.
5. **`requires` is a canonical set in Terminal Psi.** The codec requires
   `MachineContract::requires` to strictly increase in canonical byte order,
   so clauses are sorted and a duplicate clause is refused. Clause positions
   in `CallRequirement` are canonical positions, not authored ones.
6. **The machine declaration is left `None`.** `CheckedMachineDeclaration`
   holds identity-table text; producing it is identity work this route does
   not do. The codec and the interpreter accept the module without it.
7. **A value arm's test is stored twice.** For `true -> X` on a captured
   subject, stage 03 keeps a `When(subject == true)` guard and, as a separate
   expression copy, a non-structural pattern whose `guard` and `condition` are
   one handle. The reader evaluates the guard once and refuses the arm unless
   the copy reads to the same computation (`same_computation`, over slots,
   literal values, operators and callee identities).
8. **Calls name the callee's entry state.** `target_symbol` and transition
   paths hold the `State` symbol; the machine is that symbol's parent, and the
   reader checks that the machine's one state is exactly that symbol. Every
   authored call also carries an authored selection occurrence and a static
   application whose registered operation is that state; both are accepted
   only when they resolve to exactly the callee and apply nothing.
9. **Stage 04 leaves some literals unlanded.** An integer literal with no
   landing takes the type its context expects (initializer, assignment,
   argument, return, arm result, or the other operand), per
   [literals and expected types](../../../../../wiki/spec/language/domains.md#literals-and-expected-types);
   with no expected type it is refused.
10. **Every machine carries a reach row.** A machine with no reach holds
    `ServiceReachRowTable::EMPTY_ROW`, not `NULL`; both are accepted, any other
    row is refused.
