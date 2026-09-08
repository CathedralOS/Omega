# Scalar computation and call production

Public contracts: [calls and outcomes](../../../../wiki/spec/terminal-psi/calls_and_outcomes.md)
and [verification](../../../../wiki/spec/terminal-psi/verification.md).
This is the implementation map, not another call representation.

## Authored occurrence to computation

Call-bearing initializers, assignments, guards, state arguments, and returns use
arena-backed checked computation plans, separate from proof-side pure expressions.
Value leaves retain the source namespace; call nodes retain generational authored
expression handles, exact flow calls, and occurrence ordinals. Conditional nodes
select a result; pure templates consume completed operands.

Rejoin each root with its exact state, statement, destination role, expression,
carrier, and declaration namespace. Pure roots use the same locator and require
one unambiguous binding/expression pair. Missing or stale handles, duplicate call
occurrences, swapped arms, conflicting rows, and reordered symbols reject.
Occurrence preorder is identity, not execution order. Zero-argument calls and
transfers still rejoin their outer target. A static qualifier names the exact
attached data owner without manufacturing a receiver or discarding a value one.

Private typed blocks evaluate arguments left-to-right and carry each result
before evaluating the next. They preserve short-circuit selection and do not
manufacture source states, locals, or argument slots in the caller namespace.
Completed comparisons may reverse values for `>`/`>=`, never evaluation order.
Cast-wrapped calls and indexed reads remain behind their original selection
boundary; a later cast cannot hoist its call ahead of an earlier operand.

Assignments evaluate against the pre-write storage environment and commit only
after RHS completion. Earlier immutable snapshots survive. Mutable scalar formals
use state-local storage initialized from exact incoming operands, not immutable
entry aliases recovered by spelling. Collect cast facts before invalidating old
destination facts. Final-value postconditions need their own transport; entry
requirements and crash conditions remain incoming-value contracts.

Result/literal fixed-integer postconditions retain the actual result occurrence,
builtin comparison meaning, and checked adjacent-endpoint normalization for
strict bounds. Calls import those guarantees only after their evaluated actuals
prove the full canonical requirement conjunction, including enforced parameter
ranges. Nested cast certificates cite emitted return equations and instantiated
guarantees, not a literal tautology or the result carrier alone. Named-state
forwarding needs an unambiguous immutable origin on every incoming edge.

## Unit and boundary integration

Statement arguments select a pure plan or exact computation root. Dense scalar
ordinals differ from authored argument positions; structural operands advance
the latter, implicit receivers do not. Structural places stay in the enclosing
Unit machine until the outer call commits. Finalize arithmetic proofs only after
the selected module's borrowed places, cleanup, and complete closure are assembled.

A final Unit call expression shares operand evaluation with a semicolon call,
then returns normally. It does not imply tail-call optimization or authorize
discarding a scalar result. Calls before transitions retain statement syntax.
Scalar/structural result initializers establish a binding only after successful
completion; statement positions and dense binding ordinals remain separate.
Boundary refusal preserves interpreter custody, not external effects or ownership
the host may already have changed; see the [host response contract](../../../../wiki/spec/terminal-psi/boundary_calls.md#host-result-validation).

Whole owned affine results and bounded shared reads use the existing result
owner and continuation cleanup. Anonymous results do not become synthetic locals.
Structural scalar calls use `CallStructuralScalar`, not a widened scalar `Call`;
exact projection/transfer events and machine-local claim IDs survive the scalar
result. General projected/linear results, mixed temporary consumers, and richer
construction remain separate from whole plain-owned result support.

The structural scalar-return body plan also retains an ordered effect prefix.
One direct primitive-reference assignment followed by a scalar return uses the
same checked store and Terminal emission as a Unit body. Free bodies have no
fabricated attachment. The current prefix accepts an exclusive primitive borrow
and a literal or direct scalar-parameter RHS; it neither discards the borrow nor
turns the store into a scalar-producing operation. Its complete authored statement
roster, destination, RHS, and return coordinates are rejoined before emission.
Authored contracts, published crash routes, and constrained input/result types
remain unsupported on this prefix until their exact contracts are carried too.
The source-to-artifact execution regression is
[`primitive_store_return_source.rs`](../../pipeline/checked-trees-to-lowered-psi/tests/primitive_store_return_source.rs):
`cargo nextest run -p checked-trees-to-lowered-psi --test primitive_store_return_source --no-fail-fast`.
This establishes the body of an operand callee such as
`machine reset(value: &mut u64) -> u64 { value = 0; 0 }`.
Ordinary Unit callers retain these bodies in their existing shared scalar-callee
catalog and invoke them with `CallStructuralScalar`. Independent primitive-store
bodies are discovered before Unit closure; nominal-cleanup-dependent return bodies
remain in the later discovery phase. Exact authored structural actuals and dense
scalar positions survive the call, including a result binding after scalar inputs.
The selected callee's type closure is validated in the shared allocated namespace;
unrelated retained bodies do not add types or machines to that artifact.
[`borrowed_scalar_call_source.rs`](../../pipeline/checked-trees-to-lowered-psi/tests/borrowed_scalar_call_source.rs)
checks observable callee/caller writes, returned values, suspension without replay,
and rejection of substituted borrowed actuals or callee custody. Both store owners
rejoin their exact authored assignment and RHS namespace; a direct call initializer
must retain its invocation even when its result is unused.
Ordinary Unit statement sequences establish mutable primitive locals as real
referents, separately from scalar inputs and immutable result bindings.
Borrowed calls mutate that storage; subsequent reads observe the current value,
while an earlier immutable snapshot retains its original value. Local assignments
use the same primitive-store operation as reference parameters. Source replay
retains declaration, initializer, destination, borrow occurrence, and read identity.
The interpreter uses fresh activation-local identities and preserves them across
fuel suspension. Primitive-local establishment and reads still require native
realization; unsupported native lowering rejects explicitly.
Mixed scalar/structural computation nodes still need borrowed operand staging and
the same call-closure integration before the guarded customer below can close.

## One complete call closure

Ordinary and composed Unit catalogs are pruned together: an unavailable transitive
body removes upstream callers. Allocate shared type, boundary, service, machine,
header, value, place, operation, and edge identities before emitting each body
once. Do not concatenate independently lowered modules or turn states into
synthetic machines. Shared scalar helpers keep one identity across roots, callees,
dynamic continuations, and boundary operands. Finalize proofs after assembly.

Preserve source-to-machine ownership for suspension, conformance, and float-source
metadata. Boundary declarations may coalesce only when identical; conflicting
declarations reject. Root attachment requirements equal actual direct boundary
calls, not transitive reach or unused retained fields. Callee services remain
owned by their bodies. Provider-field lookup rejoins inherited storage stamps.

Selected operator realizations additionally rejoin strong plan identity, exact
checked adapter machine/state, authored application, contract, and result against
retained plan facts. Another same-signature conformer cannot replace that row.
No conformance scan supplies a missing selected application.

General scalar/shared-byte-view state traversal preserves selected-edge operands,
subslice descriptors, joins, and authored ranking. A failed check on that selected
route cannot fall back to a shape matcher. Qualification/ownership, implicit
receiver, and sum-payload routes have separate admission; interpreter payload
inspection is not supplied by operand evaluation. Composed internal calls still
have narrower scalar/requirement support than ordinary direct mixed calls.

Longer computed dispatches need explicit shared-subject planning. Two opposite
Boolean literal arms evaluate one subject once; independent guards are not merged
because their expressions look alike. Guards finish before destinations, and a
fallback crash retains its exact source site and no-successor behavior. Failed
earlier guards contribute only facts whose storage/version remains valid.

Remaining selected operators, arithmetic policies, projected writes, mutable
origin/snapshot transport, structural returned-call integration, and broader
computed-argument coverage stay on [STATE-LOCAL-VALUE-FRONTIER](../../../../TASKS.md).
Retire older flat guarded-argument normalization as those paths acquire complete
computation plans; do not describe a source classifier as finished execution.

## Guarded primitive-reference operand

This continuation customer still fails source checking at its rank range. Save
it as `main.omg` and run `omega --check --target macos_arm64 main.omg`:

```omega
machine reset(value: &mut u64) -> u64 { value = 0; 0 }

data Limits {
    limit: u64;
    divisor: u64 [3..=5];
}

machine walk(remaining: u64 [0..=5], limits: Limits, marker: u64)
terminates by remaining -> Nat::Descending in 0..(limits.limit % limits.divisor + 6);
-> u64 {
    let mut scratch: u64 = 0;
    transition remaining > 0 {
        true -> walk(remaining - 1, limits, reset(&mut scratch))
        false -> remaining
    }
}
```

The existing argument hoist splits the edge into a rank-preserving hop and a
decrement without its co-located guard. Integrate the ordinary primitive-store call route
with checked structural operand computations and their shared Terminal closure,
then retire the hoist for the supported route independently of ranking annotations.
Keep the original guard, mutable local, and exact `limits` forwarding. Adding
provenance around generated states or suppressing normalization without executing
the checked computations does not close this customer. After source checking,
verify call selection, operand order, mutations, and descent through Terminal.
