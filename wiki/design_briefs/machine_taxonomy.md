# Design Brief: Machine Taxonomy

Settled 2026-07-18 (frozen decision 20). This brief settles
what a machine *is* and how the one construct is supplied and consumed. It did
not itself settle the reach algebra; decision 22 has since done so.
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
cited as a proof, started through a task-runtime provider, used to satisfy a
trait requirement, or provided across a boundary. These are not different
machine kinds. They are different ways to supply or consume the same
contracted transition system.

Proof-time use and runtime use therefore compose naturally. A machine whose
body and contract are eligible for compile-time evaluation can establish a
theorem; the same body may also be called at runtime. Evaluation context may
change lowering or erase the execution entirely, but it may not change the
contract's meaning.

A `proposition` is the adjacent proof-formula category, not another machine
supply or consumption mode. It declares something a machine contract may
require or ensure; it never denotes an operation or transition system. Proof
machines remain ordinary machines: their checked contracts establish
proposition applications, and fact-only invocations erase. The dedicated
category also keeps a resultless machine requirement such as
`where machine Visit(item: &T);` unambiguously operational.

## The substitutable contract

A machine's public contract includes every behavior a caller is allowed to
observe or rely upon:

- input, state, and result relations;
- failure, trapping, and cancellation behavior;
- service reach and required authority;
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

At a direct call, possible suspension and blocking are acknowledged with the
independent contextual prefixes `suspend` and `block`. These prefixes mirror
the statically known call envelope and change neither the machine contract nor
the invocation model. They are call-site audit syntax, not additional machine
species.

This is refinement, not signature equality. Mechanically, each contract axis
keeps the same substitution direction:

| Axis | Provider admitted when |
|---|---|
| Caller requirements | the requirement's preconditions imply the provider's; the provider asks no more |
| Result/state guarantees | the provider's guarantees imply the requirement's; the provider promises no less |
| Service reach | the provider row is a subset of the requirement ceiling |
| Suspension | a provider may suspend only when the requirement declares `suspends` |
| Blocking | a provider may block only when the requirement declares `blocks` |
| Failure, trap, and cancellation | provider-visible outcomes are a subset of those the requirement permits |
| Termination and positive progress | the provider preserves every published guarantee under no stronger premises |
| Context-visible resources | provider demand is within the requirement ceiling |
| Atomicity, reentrancy, and calling plan | provider behavior is compatible with or stronger than the pinned promise |

The corresponding contract analyses prove those axis judgments; admission
conjoins them. A provider may be strictly more capable or predictable than its
requirement without acquiring the requirement's identity, but failure on any
one axis rejects substitution.

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

The source forms are deliberately distinct:

| Supply mode | Source form |
|---|---|
| Checked body | An ordinary machine with a `{ ... }` body |
| Required body | A bodyless machine declaration inside a trait |
| External provider | A machine that `satisfies` a requirement `via` a compile-time `Binding` value |
| Accepted declaration | A bodyless `boundary machine ... ensures ...;` declaration |

There are no expression-bodied machines. `{ ... }` is the sole executable
machine-body syntax; even a one-expression predicate uses braces. `via` is not
an expression-body operator. It selects the external-provider supply variant:

```omega
windows_x64 machine WindowsBindings::write_file() -> Binding {
    Binding::DllImport {
        import: DllImport::PeByName {
            library: "kernel32.dll",
            export: "WriteFile",
        },
    }
}

machine Kernel32::write_file(handle: WinHandle, bytes: &[u8]) -> WriteResult
    satisfies Kernel32Requirements::write_file
    via WindowsBindings::write_file();
```

Proposition declarations have their own non-executable forms: `;` introduces a
primitive fact, a witness-bearing declaration publishes one canonical
carrierless evidence interface, and `=` defines a transparent logical
expansion. The witness-bearing form uses an `evidence Interface;` clause after
the proposition signature. None is a machine body or machine supply mode.

The expression after `via` must be compile-time evaluable to a normalized
`Binding` value. Its normalized identity enters the derived provider plan;
plan derivation validates it structurally, and admission assigns trust from
the binding kind and evidence. Merely writing `via` asserts no trust class.

The realization machine already supplies the canonical Omega symbol.
`Binding::CompilerIntrinsic` therefore has no textual name payload: the
resolved realization symbol, normalized signature, and selected target key the
sealed intrinsic catalog. Other binding operands are ordinary typed compile-time
values. A DLL locator is one object-format-specific sum case containing all of
its name, ordinal, or version coordinates as owned `StaticBytes` and scalars.
The satisfied requirement's `Calling<C, Policy>` relationship separately
produces the evaluated `CallPlan`; `Binding` neither carries nor reselects it. Raw
linker bytes are target-package data and never Omega symbols, requirement keys,
ambient lookup strings, or provider selections. The complete evaluated binding,
producer closure, and target are fingerprinted; changing foreign bytes changes
the selected target/artifact identity, forces dependent relinking, and requires
fresh admission.

`satisfies` identifies the requirement and inherits its contract. The
requirement's service-reach row, `suspends`/`blocks` fields, and guarded
`crashes` buckets are public ceilings. The realization's checked provider behavior is derived from its
binding/provider contract and must refine every ceiling during validation and
admission; a `via` declaration does not author a second copy of them.
An installation-bound requirement may carry one path-keyed abstract reach row
with a written upper bound. Its realization supplies the exact row, and root
admission substitutes it through that installation closure; it cannot escape
as an unpinned ordinary import or remain unresolved in a final artifact.

Checked adapters remain ordinary machines. A Console operation that obtains a
handle and performs two writes is authored as an Omega body satisfying the
Console requirement; only its irreducible DLL/syscall/instruction leaves use
`via`. The toolchain derives `ProviderPlan` coverage, dependency closure,
reach, identity, and admission inputs from the explicitly selected
conformance closure. Programs never assemble plan rows imperatively.

The same rule applies to named boundary operators. A checked body may satisfy
one exact overloaded operator slot without `via`; its proved contract must
cover the public operator guarantee under positional parameter substitution
while asking no stronger premises. The
derived row uses `CheckedAdapter`, and execution redirects to the ordinary body
only after the exact selected plan identity has been retained. Compiler
intrinsics remain admitted leaves rather than masquerading as checked code.

## Consumption modes

Consumption answers **how a valid machine is used**:

- runtime call;
- compile-time evaluation;
- proof citation;
- concurrent activation through a task-runtime provider;
- trait conformance;
- boundary import or export.

Eligibility is derived from the complete contract and the consuming context,
not declared through parallel species such as `async machine`, `proof
machine`, or `const machine`. For example, compile-time evaluation rejects
reach or unbounded work that its context forbids; an interrupt context
rejects a machine whose effect/resource ceiling exceeds its own; concurrent
activation adds ownership, provider-admission, lifecycle, and capacity
obligations.

Imports and exports are artifact-relative. A `boundary trait` describes a
requirement; a provider/implementation satisfies it; a body-bearing exported
machine is a provision. Accepted theorems use the existing bodyless
`boundary machine` surface; there is no `boundary fact` construct. The checked
artifact distinguishes an accepted proof claim from an executable external
provider through supply/eligibility and its normalized contract, not a second
declaration species.

## Observation and internal transitions

Compiler, runtime, and provider steps may be internal (`tau`) only after
projection through the machine's declared observation surface, subject to the
observation requirements imposed by its calling context. Declared service
reach, operational and guarded-crash ceilings, authority, resource bounds, failure,
cancellation, and temporal guarantees remain observable wherever the context
requires them.

This gives implementations room to refine or stutter internally without
letting a machine self-certify away a real cost. In particular, a provider
cannot call worker-thread occupation "unobservable" to become admissible in a
no-block context. The context's service/operational/resource ceilings are the
observation floor.

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
reach happens to be implemented. See
`architecture/semantic_taxonomy_representation.md` for the compiler migration.

## Acceptance register

1. One checked machine can be evaluated at compile time and called at runtime
   without acquiring two semantic identities.
2. A productive transition loop is a valid machine even though it has no
   terminal return.
3. A provider with a stronger result relation and no stronger requirements is
   admitted; one with a hidden extra effect is rejected.
4. A machine cannot hide blocking merely by omitting it from its chosen
   observation vocabulary when the calling context forbids blocking.
5. Internal scheduling steps may stutter without changing meaning when the
   contract deliberately abstracts them and the context permits that
   abstraction.
6. Trait requirement, checked implementation, external provider, and accepted
   declaration remain distinguishable in the checked artifact.
7. Runtime call, compile-time evaluation, proof citation, and task start consult
   the same normalized machine contract.
8. A transition remains internal to its machine; a call enters a different
   machine contract.
9. A boundary import/export manifest pins semantic contract identity rather
   than a source keyword or implementation body hash alone.

## Deferred design spaces

- Reach, authority, and observation are now settled by frozen decision 22;
  see [Reach, Authority, And Observation](effects_authority_and_observation.md).
  Its compiler representation remains engineering work.
- Continuation lowering and suspension-safe loans.
- Task-runtime activation planning, linear lifecycle claims, provider
  provenance, and transactional start; see
  [Task Runtime And Lifecycle](task_runtime_and_lifecycle.md).
- Component coexistence, requirement-binding refinement, and continuation migration.

Termination and progress are now settled by frozen decision 23; see
[Termination, Ranking, And Progress](termination_ranking_and_progress.md).
