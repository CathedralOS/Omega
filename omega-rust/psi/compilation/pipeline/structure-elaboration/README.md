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
[outcome.rs](src/outcome.rs) then works out, callees first, whether each
machine can complete normally and which crash routes it publishes;
[lower.rs](src/lower.rs) turns each body into blocks, values, operations,
edges and invocations. [canonical.rs](src/canonical.rs) holds the one route to
Terminal Psi's canonical form for every proposition the crate builds.

## The subset

Machines with one state and a complete body whose formals, locals and result
are builtin integers (`i8` to `u64`) or `bool`, using: scalar locals and
assignment to them; integer and Boolean literals; builtin `Exact` integer
arithmetic, bitwise operations, shifts and comparisons; `!`, `~`, `&&` and
`||`; direct calls of other such machines with `requires` clauses over their
formals and `ensures` clauses over their formals and `result`; `crashes Trap`
and `crashes Abort` clauses with or without route guards over the formals;
`crashes none` on a body that reaches no crash;
`crash Trap;` and `crash Abort;`; guarded transition arms; and returns. A call
of a machine that may return is an operation wherever it stands, and carries
the callee's crash routes when it publishes any. A call of a machine with no
normal completion is served only in terminal position (an arm target or a
call statement), where it becomes the block's no-normal invocation.
Everything else is refused with `RefusalReason::Unsupported` and a detail
naming the construct (`expression match`, `type reference`, `call no-normal
call inside an expression`, ...): a reference, record, slice, loop, recursion
through a cycle, generic, callable value, suspension, assembly, cast, float,
domain or service reach is never skipped, approximated or passed through. A
refusal anywhere in the closure refuses the whole closure.

## How identity is fixed

- **Types** are scalar only through `SymbolTable::builtin_type_atom` on the
  resolved symbol of a `Named` type: the compiler-installed root slot. The
  typed trees' `PrimitiveType::from_name` classifies by spelling and is not
  used. A `Named` type with an invalid symbol is refused as a spelling; a
  declaration spelled `u32` is a different symbol and is refused as nominal.
  Integer literals take their type from the landing stage 04 recorded.
- **Names** are slots only when the path has one member and a valid symbol
  equal to a formal's or local's symbol. A name without a symbol is refused.
- **`result`** in a contract is the result pseudo-value only through the name
  path's `reserved_result_of`, the contract owner that stage 02 records where
  it decides the name is the reserved result (`symbols.rs`,
  `assign_contract_call_symbols`). The owner must be the machine's one state,
  the clause an `ensures`, and the machine scalar. A name spelled `result`
  with no recorded owner is refused as a spelling. The pseudo-value is value
  `formals + 1` with `ValueOrigin::MachineResult`, so the module's
  `TerminalMachineResult::Scalar`, its `ensures` propositions and the value
  table name one identity.
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

A crash is a `Crash` terminator with its cause and its site guard: the exact
incoming conjunction of branch decisions on the path to it, each the branch
value equal to the Boolean its edge took ([crash](../../../../../wiki/spec/terminal-psi/calls_and_outcomes.md#crash):
"The guard retains the exact incoming conjunction"). Canonical consequences
used as route witnesses are proof information and are not added here. A call
of a machine with no normal completion is an `InvokeNoNormal` terminator whose
crash continuations are the callee's published routes with each formal
replaced by the actual at its position; nothing is dropped or simplified.
Both edges go to `Destination::NoNormal`. A call of a machine that may return
and also publishes crash routes stays a `Call` or `CallUnit` operation and
carries the same substituted routes as its crash continuations: the block
goes on after it, so the crash site is the operation
(`CrashSite::Operation`), not the block. Their dispositions are empty: the
subset mints no conserved obligation, and if one existed the edge is refused
rather than given a guessed fate (a crash abandons; `Fate::Abandoned`).

What a machine publishes ([guarded crashes](../../../../../wiki/spec/language/effects.md#guarded-crashes)):
an authored `crashes` clause is its ceiling; a public body with none publishes
no route; a private body with none publishes each cause its body may reach,
from a `crash` statement or a callee's route, as the alternative `Truth`.
`Truth` is the widest route of that cause, so inference never hides a crash a
caller must cover; a narrower guard needs proof.

An authored `crashes none` is not an omitted clause. It leaves no contract
row, so the reader takes it from `Machine::declares_no_crash_route`, the fact
stage 01 records where it parses the clause and stages 02 and 03 copy. Such a
machine publishes no route and nothing is inferred for it. When its body can
reach a crash by structure, from a `crash` statement or a callee's route, the
closure is refused with ``contract `crashes none` on a body with a crash
route``: effects.md says an explicit empty value "promises the empty value and
rejects any body or selection that violates it", and keeping the promise
would need each route disproved, which is the checker's work.

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
| `PublishedGuarantee` | one per `ensures` clause, the identity `ContractClause` carries | the entry block |
| `NormalGuarantee` | one per `ensures` clause per normal exit edge, naming its `PublishedGuarantee` | that edge |
| `CrashCoverage` | one per crash site and cause. `CrashSite::Block`: a `Crash` terminator (its one cause), or an `InvokeNoNormal` terminator (each cause among its continuations). `CrashSite::Operation`: a `Call` or `CallUnit` whose callee may return and publishes crash routes (each cause among its continuations) | the point before the terminator or the call |

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

1. **The current checker accepts a private `crashes none` body that
   crashes.** Stage 01 publishes no contract row for `crashes none`, so
   stage 05 reads it on a private body as an omitted clause and infers the
   routes the author forbade
   ([effects](../../../../../wiki/spec/language/effects.md#static-binder-contract-dependencies):
   an explicit empty value "promises the empty value and rejects any body or
   selection that violates it"). The typed trees now keep the authored fact as
   `Machine::declares_no_crash_route`, and this crate refuses such a body
   (above). Stage 05 was left as it is; the witness is
   `tests/omega/pending/effects/private_crashes_none_body_crashes`.
2. **Inferred routes are `Truth` per cause.** A narrower guard would be over
   body values, not formals, and stating it over formals is proof work.
3. **One `ensures` identity per clause, one obligation per exit.**
   `MachineContract::ensures` holds a `ContractClause` with one `ObligationId`
   per clause (`terminal-psi/src/proof.rs`), while control flow requires every
   normal return to establish each guarantee. Lowering mints one
   `PublishedGuarantee` per clause, which the `ContractClause` carries, and one
   `NormalGuarantee` per (normal return edge, clause), which names it through
   its `published` field. The published clause holds when every return
   edge's guarantee holds.
4. **No summaries.** `Summary::requires` and `result_relation` are
   `PropositionId`s into "the module's proposition table", but Terminal Psi
   keeps contract propositions inline in `MachineContract` and its only
   `PropositionId` table (`proposition_declarations`) is named propositions
   with a `String` name. `CheckTables::summaries` is left empty; published
   crash routes live in `MachineContract::crash_routes` only.
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
