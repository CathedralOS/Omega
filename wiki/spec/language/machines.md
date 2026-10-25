# Machines and contract refinement

A machine is a contracted transition system with declaration identity. Given
inputs, state, and authority, it produces an observable trace and may reach a terminal outcome.
A productive machine may run forever; a function returning a value is a special
case, not the definition.

Machines use named declarations. Static callbacks select those declarations
and pass ordinary explicit context data. Anonymous machine expressions and implicit
capture-environment construction are not part of the source language.

## Static declarations and explicit state

All machines are static declarations independent of data. Named implementations
satisfy traits; data types only occur as ordinary signature types or declared
trait arguments. There is no conformance data subject or `Self` type binding
([conformances](conformances.md#data-and-code-are-independent)).
There is no special `self` parameter,
implicit receiver, bound-method value, or receiver-inserting call syntax.
Predicate-only `self` in data/default-domain and named-domain predicates
([domains](domains.md#declaration-and-membership)) is not a machine parameter
and does not change this rule.
Named decision: `no-data-code-attachment`, ratified 2026-10-22. A data declaration
is not a machine owner or namespace. Declaring a machine under a data carrier,
such as `machine Counter::increment`, is retired even with an explicit parameter.
Machine qualification identifies an ordinary module, trait requirement, named
implementation or lexically visible nested declaration; it never binds code to a data type. Package visibility and exact
declaration identity remain independent of the types of explicit arguments.

```omega
data Counter { value: u64; }

machine clear(counter: &mut Counter) {
    counter.value = 0;
}

machine example() {
    let mut counter: Counter;
    clear(&mut counter);
}
```

Every argument, including context and capabilities, is ordinary and explicit.
This includes the toolchain-authored build API: `Build::application(builder,
"example")` and `Build::depend(builder, source)` pass the build context explicitly.
Owned, shared, mutable and write-only access retain their normal contracts;
removing receiver syntax removes no ownership, invariant, effect or proof check.
Trait requirements and satisfiers use the same explicit parameter correspondence:
a satisfier declares no parameter its requirement lacks. A boundary provider is
therefore its satisfier machines, with no provider value or private state;
state that outlives a call is a value the service returns and the caller passes
back.
Nested machines likewise receive explicit arguments on every invocation.

Invocation scratch belongs in ordinary local bindings or local data instances.
Pass values needed by another machine as arguments; retain data in a record
only when that record actually owns the required state or resource lifetime.
This introduces no machine-scoped temporary storage, implicit capture, or new
local lifetime rule. Implementations must support the ordinary local-storage and
borrow path instead of requiring authors to put temporary buffers in a persistent
context record. Entry state uses ordinary stack or static storage. An explicitly
installed entry contract may instead supply arguments, as with UEFI handoff;
declaring a mutable context parameter requests no automatic allocation. See
[entry arguments and storage](../build/entry_roots.md#entry-shape-and-arrival-bridge).

This is the source contract. Remaining receiver-based or carrier-qualified
compiler paths, samples and documentation examples are migration debt, not an
alternative accepted form.
An implementation must reject retired `self` declarations and expressions rather
than silently translate them or enable a compatibility mode. A rejected legacy
fixture does not verify the behavior that its eventual static form must exercise.
The `STATIC-MACHINE-MIGRATION` item in [the execution board](../../../TASKS.md)
tracks their removal and end-to-end validation.

## Declaration forms

A fixed operator token after `machine`, as in `machine + Vec2::add(...)`, binds
expression syntax to the same named machine. This is not a separate executable
species or an alternate contract system. [Operator declarations](expressions.md#operator-declarations)
owns token selection and family ownership; the ordinary supply rules below own
the body or authorized realization.

Runtime calls, [semantic evaluation](evaluation.md),
[proof citation](../proofs/contracts.md), task activation, trait satisfaction,
and boundary import/export consume the same semantic construct. Eligibility
comes from its complete contract and context, not separate `async`, `proof`,
or `const` machine species. Evaluation may erase execution but cannot change
meaning. Logical conditions, mathematical abstractions, and compiler-owned
proof term formers are not additional executable supply modes.

## Call preconditions

Every machine application establishes the selected contract's `requires` under
the exact argument substitution. This applies equally to named and token-bound
calls, runtime execution, compile-time evaluation, and mathematical use. A
mathematical application must establish those premises before forming its result
term, not only when importing `ensures`. An erased or discarded result does not
waive the preconditions. A total implementation behind a restricted contract
does not make that selected contract total over all inputs.

The implementation establishes its result's validity and guarantees assuming its
declared premises; the caller establishes those premises. Existing assumptions,
guards, and checked evidence may discharge them. This obligation is not limited
to conditions decidable by the current automatic prover: unavailable evidence
prevents acceptance, but failed search does not establish falsity.

Recursive proof applications must both establish the callee's substituted
premises and satisfy the exact well-founded descent obligation. Neither condition
substitutes for the other. See [citation and induction](../proofs/contracts.md#citation-and-induction).

## Supply

The settled decision `boundary-operations-are-trait-requirements` retires the
standalone carrier-owned boundary-requirement declaration. Boundary operations
are requirements in boundary traits; one-member traits provide independent
selection without making data a code owner.

| Supply | Source and obligation |
| --- | --- |
| Checked body | An ordinary machine with a checked `{ ... }` body, including generated/default bodies after generation. |
| Required body | A bodyless trait/component machine requirement; a boundary operation belongs to a boundary trait, including a one-member trait when independent selection is required. |
| External provider | A bodyless `boundary machine` satisfying an exact requirement under its pinned contract and admission policy. |
| Admission-bearing declaration | A bodyless `boundary machine` publishing an unproved theorem or external-behavior guarantee for separate owner acceptance. |
| Compiler-supplied primitive | A bodyless exact toolchain declaration with an authorized closed-catalog implementation and fixed semantic contract; not selectable by naming or user registration. |

Required-body forms share one supply meaning. Checked artifacts distinguish
required, checked, compiler-supplied, externally supplied, and admission-bearing
behavior explicitly;
a `boundary` Boolean cannot represent those distinctions. A claim-free bodyless
free machine outside the authorized compiler catalog has no supply mode and
rejects. The catalog exception is automatic, not a Build provider selection.
Executable bodies always use braces, never expression-body syntax. A bodyless
`ensures` is a claim, not a theorem by
virtue of its name. Accepted theorem claims use the existing boundary surface,
not a separate fact declaration.

Token-bearing machines follow these same supply distinctions. Ordinary direct
declarations own checked bodies and trait requirements use named
implementations. The standalone carrier-owned `boundary requirement
Carrier::operation(...);` spelling is retired: data and code are independent,
and a one-member boundary trait already provides an independently selectable
operation slot. Carrier and authority values remain ordinary explicit
parameters. `satisfies` does not attach an implementation to an otherwise
bodyless ordinary direct declaration.

`boundary` marks a crossing, not its direction. Imports and exports are
artifact-relative; requirements and composition determine provision or demand.
`satisfies` names an exact trait requirement and inherits its contract. `via`
only supplies a compile-time-evaluated binding payload not derivable from
declaration and target; it is neither a supply keyword nor trust authority.
Compiler intrinsics use the sealed declaration/signature/target catalog without
a binding payload. See [foreign bindings](../build/foreign_bindings.md) and
[provider selection](../build/provider_selection.md).

Composite providers remain ordinary checked bodies; only irreducible foreign
leaves use external supply. The toolchain derives their selected conformance
closure, coverage, dependencies, reach, and admission inputs. Programs cannot
assemble provider-plan rows or reauthor inherited ceilings in a binding.

### Target-scoped families

A machine declaration prefixed with a target name is one body of a family keyed
by target under one path. Families are data: every body is resolved, typed,
checked, and published in Terminal Psi with its target tag in every
compilation, and realizing a target selects that target's body
([multi-target compilation](../build/configuration.md#multi-target-compilation)).
A family is never filtered before checking, and no realization set narrows
checking. A realized target without a body for a called family rejects that
realization; it is not a checking error for the program.

## Substitution

The public contract includes input/state/result relations, failure/crash and
cancellation, service reach and authority, progress and operational ceilings,
atomicity/reentrancy, context-visible resources, and observable calling/
representation plans. A provider refines a requirement when every provider
trace, under the permitted observation projection, is allowed by the requirement
and asks no more from its caller:

| Axis | Substitution condition |
| --- | --- |
| Preconditions | Requirement premises imply provider premises. |
| Result/state guarantees | Provider guarantees imply requirement guarantees. |
| Service reach | Provider row is a subset of the requirement ceiling. |
| Suspension/blocking | Each possibility is permitted independently by the corresponding ceiling. |
| Failure, crash, cancellation | Visible outcomes are a subset of permitted outcomes. |
| Termination/progress | Every positive guarantee is preserved under no stronger premises. |
| Context-visible resources | Demand fits the promised ceiling. |
| Atomicity, reentrancy, calling plan | Behavior is compatible with or stronger than the pinned promise. |

Admission conjoins the independently checked axes; matching signatures or one
successful analysis is insufficient. A stronger implementation does not acquire
the requirement's identity. Remote execution, handoff, or scheduling that
changes an observable item needs a different declared contract.

Internal steps may stutter only under the declared observation projection and
the calling context's observation floor. Worker occupation cannot become
invisible to evade a no-block context; neither can authority use, resource cost,
failure, cancellation, or required temporal behavior. Direct-call audit markers
follow [operational ceilings](effects.md#call-site-acknowledgements) without
changing machine identity or invocation mode.

## Nested machines and identity

Named machines may nest to arbitrary depth. There is one machine declaration
kind: the source `state` declaration is retired, not an alias or a second kind
of machine. Nesting restricts name visibility; it does not allocate data,
capture runtime bindings, retain an enclosing activation, or impose a stack
layout. Every nested declaration has its own signature, contract and identity.

A body consists of its executable portion followed by zero or more nested
machine declarations. The executable portion may finish with a transition,
result, or permitted Unit completion. A transition is not mandatory merely
because declarations follow. Once declarations begin, executable statements
cannot resume. This layout rule applies recursively. Nested declarations are
visible throughout their enclosing machine body, including before their textual
declaration, and within its nested declaration scopes. Duplicate declarations
follow ordinary name/overload rules; nesting is not a new overload mechanism.

Name resolution uses lexical scope. A helper is unavailable outside the machine
whose scope declares it; qualification cannot bypass this restriction. A sibling
can name another visible sibling, but cannot reach into that sibling's private
descendants. An ancestor's visible declaration may be called recursively; this
invokes its entry, not a saved activation or a private point inside its body.
Nested declarations are not separately package-exportable; package export of an
enclosing machine does not export its helpers.

Runtime parameters and locals of enclosing invocations are **not captured**.
Pass them explicitly, including references, capabilities and lifetime-bearing
values. Enclosing generic/static declaration bindings remain lexical context;
resolution and specialization retain their exact identities without creating an
implicit runtime environment. Nested contracts can use that static context and
their own parameters, not another invocation's local values or live facts.

An ordinary call resumes its continuation on normal return. A machine call in a
terminal `transition` arm forwards its normal result as the current invocation's
result, with no continuation in that invocation. A nested machine's return is
local to that invocation: it does not return from arbitrary lexical ancestors.
Both forms use the same contract, exact call acknowledgements and explicit
arguments. The distinction is call position, never nesting depth.
[Invocation contracts](state_contracts.md#terminal-calls-in-transition-arms)
owns argument, result, cleanup and fact obligations.

Resolution checks visibility and elaborates declaration context before erasing
lexical nesting into flat machine identities and explicit calls/transfers.
Source ancestry remains available for identity, diagnostics and debug mapping,
not as an executable parent stack. Downstream representations may have ordinary
basic blocks and jumps; these are not a revived source `state` construct.

A machine invocation does not prescribe a physical call frame. Inlining,
tail transfer, frame reuse and fusion must preserve observable contracts, loans,
cleanup and composed storage bounds. Runtime recursive cycles retain the
constant-stack requirement in [termination](termination.md#calls-and-productive-loops).
Replacing old state loops with frame-growing recursion is not an acceptable
intermediate behavior or an optimization left for later.

This is the ratified end state. Compiler/source migration is tracked by
`NESTED-MACHINE-UNIFICATION` in [the execution board](../../../TASKS.md).
Implementation may stage depth support and migrations, but cannot retain a
permanent state-only calling, contract, ownership or lowering path. Existing
source examples using `state` are migration debt, not an alternative language.

## Published identity

The normalized semantic contract is independent of syntax and lowering.
Published identity includes the substitutable contract, supply, and observable
boundary calling plan. Component manifests, proof artifacts, admission, and
replacement checks retain it when runtime lowering erases proof-only material.
The normalized compiler-derived reach dependency is part of this contract under
[automatic reach propagation](effects.md#static-callback-reach-dependencies).
An implementation body hash, source keyword, or opportunistic narrowing of an
opaque requirement cannot replace that identity. Private ranking evidence follows
the separate
[identity rule](termination.md#published-guarantees-and-private-witnesses).

`05_machine_contracts.json` exposes this split as sibling `contract` and
`implementation` objects. The former reports supply, normalized service and
operational ceilings, published termination, and contract identity; the latter
reports the checked summary and private witness. Binding and component tools
pin the contract rather than incorporating proof-local implementation fields.
