# Design Brief: Machine Taxonomy

Settled 2026-07-18 (frozen decision 20). This brief settles
what a machine *is* and how the one construct is supplied and consumed. It did
not itself settle the effects algebra; decision 22 has since done so.
Decision 23 has since settled termination guarantees, private ranking
witnesses, and opaque boundary progress profiles. Suspension lowering and
component-versioning policy remain downstream contracts over this model.

## One semantic construct

A **machine** is a named, contracted transition system. Given its inputs,
state, and authority, it produces a contract-observable trace and may produce
a terminal outcome. A productive machine is allowed to run forever, so
"function from arguments to a return value" is an important special case,
not the definition.

The same machine can be called at runtime, evaluated during compilation,
cited as a proof, spawned as a task, used to satisfy a trait requirement, or
provided across a boundary. These are not different machine kinds. They are
different ways to supply or consume the same contracted transition system.

Proof-time use and runtime use therefore compose naturally. A machine whose
body and contract are eligible for compile-time evaluation can establish a
theorem; the same body may also be called at runtime. Evaluation context may
change lowering or erase the execution entirely, but it may not change the
contract's meaning.

## The substitutable contract

A machine's public contract includes every behavior a caller is allowed to
observe or rely upon:

- input, state, and result relations;
- failure, trapping, and cancellation behavior;
- effects and required authority;
- progress and temporal guarantees, including whether it may suspend or
  occupy a worker thread;
- atomicity and reentrancy promises;
- resource bounds exposed by the calling context; and
- the calling/representation plan where a boundary or artifact makes it
  observable.

An implementation may vary freely only within that complete contract. A
provider refines a requirement when every trace admitted by the provider,
after the allowed observation projection, is admitted by the requirement and
the provider asks no more of its caller. Handoff, remote execution, or a
scheduler strategy that changes an observable item is a different declared
contract, not a silent implementation choice.

## Supply modes

Supply answers **where the implementation or accepted evidence comes from**:

1. **Checked body.** Ordinary authored machine code, checked against its
   contract.
2. **Required body.** A trait or component requirement to be supplied by a
   conforming implementation. A default or generated body is still checked
   body supply after generation.
3. **External provider.** A boundary implementation supplies the behavior;
   admission is gated by the pinned boundary contract and trust policy.
4. **Accepted declaration.** The program deliberately accepts a theorem or
   external behavior at a named trust boundary. The artifact must report this
   trust expenditure.

These modes must be explicit in checked artifacts. A boolean such as
`boundary` is not a sufficient semantic representation: it does not say
whether the artifact requires, provides, checks, generates, or merely trusts
the machine.

## Consumption modes

Consumption answers **how a valid machine is used**:

- runtime call;
- compile-time evaluation;
- proof citation;
- concurrent spawn;
- trait conformance;
- boundary import or export.

Eligibility is derived from the complete contract and the consuming context,
not declared through parallel species such as `async machine`, `proof
machine`, or `const machine`. For example, compile-time evaluation rejects
effects or unbounded work that its context forbids; an interrupt context
rejects a machine whose effect/resource ceiling exceeds its own; spawning
adds ownership and capacity obligations.

Imports and exports are artifact-relative. A `boundary trait` describes a
requirement; a provider/implementation satisfies it; a body-bearing exported
machine is a provision. The accepted-proof spelling (`boundary machine`
versus a possible `boundary fact`) remains a surface decision, but its supply
mode does not.

## Observation and internal transitions

Compiler, runtime, and provider steps may be internal (`tau`) only after
projection through the machine's declared observation surface, subject to the
observation requirements imposed by its calling context. Declared effects,
authority, resource bounds, failure, cancellation, and temporal guarantees
remain observable wherever the context requires them.

This gives implementations room to refine or stutter internally without
letting a machine self-certify away a real cost. In particular, a provider
cannot call worker-thread occupation "unobservable" to become admissible in a
no-block context. The context's effect/resource ceiling is the observation
floor.

## States and transitions

States are internal nodes of the machine transition system. A transition is a
jump inside the current machine and does not create a new machine identity or
call frame. A call enters another machine. This distinction remains visible in
control-flow and storage planning even though both participate in the same
contract proof.

## Representation law

The checked representation must carry a normalized **machine semantic
contract** independently of syntax and lowering. Its identity includes the
full substitutable contract above plus supply mode and boundary-facing calling
plan. Runtime lowering may erase proof-only material, but component manifests,
proof artifacts, provider admission, and hot-swap checks must continue to
reference the normalized contract identity.

Do not re-derive this taxonomy from booleans, keyword presence, or whichever
effects happen to be implemented. See
`architecture/semantic_taxonomy_representation.md` for the compiler migration.

## Acceptance register

1. One checked machine can be evaluated at compile time and called at runtime
   without acquiring two semantic identities.
2. A productive transition loop is a valid machine even though it has no
   terminal return.
3. A provider with a stronger result relation and no stronger requirements is
   admitted; one with a hidden extra effect is rejected.
4. A machine cannot hide `Block` merely by omitting it from its chosen
   observation vocabulary when the calling context forbids blocking.
5. Internal scheduling steps may stutter without changing meaning when the
   contract deliberately abstracts them and the context permits that
   abstraction.
6. Trait requirement, checked implementation, external provider, and accepted
   declaration remain distinguishable in the checked artifact.
7. Runtime call, compile-time evaluation, proof citation, and spawn consult
   the same normalized machine contract.
8. A transition remains internal to its machine; a call enters a different
   machine contract.
9. A boundary import/export manifest pins semantic contract identity rather
   than a source keyword or implementation body hash alone.

## Deferred design spaces

- Effects, authority, and observation are now settled by frozen decision 22;
  see [Effects, Authority, And Observation](effects_authority_and_observation.md).
  Its compiler representation remains engineering work.
- Continuation lowering and suspension-safe loans.
- Accepted-proof surface spelling.
- Component coexistence, import-slot refinement, and continuation migration.

Termination and progress are now settled by frozen decision 23; see
[Termination, Ranking, And Progress](termination_ranking_and_progress.md).
