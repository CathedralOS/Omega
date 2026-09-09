# Machines and contract refinement

A machine is a contracted transition system with declaration identity. Given
inputs, state, and authority, it produces an observable trace and may reach a terminal outcome.
A productive machine may run forever; a function returning a value is a special
case, not the definition.

Machines use named declarations. Static callbacks select those declarations
and pass ordinary explicit context data. Anonymous machine expressions and implicit
capture-environment construction are not part of the source language.

Runtime calls, [semantic evaluation](evaluation.md),
[proof citation](../proofs/contracts.md), task activation, trait satisfaction,
and boundary import/export consume the same semantic construct. Eligibility
comes from its complete contract and context, not separate `async`, `proof`,
or `const` machine species. Evaluation may erase execution but cannot change
meaning. Logical conditions, mathematical abstractions, and compiler-owned
proof term formers are not additional executable supply modes.

## Supply

| Supply | Source and obligation |
| --- | --- |
| Checked body | An ordinary machine with a checked `{ ... }` body, including generated/default bodies after generation. |
| Required body | A bodyless trait/component machine requirement. |
| Top-level required body | An explicit carrier-owned `boundary requirement Carrier::operation(...);`. |
| External provider | A bodyless `boundary machine` satisfying an exact requirement under its pinned contract and admission policy. |
| Admission-bearing declaration | A bodyless `boundary machine` publishing an unproved theorem or external-behavior guarantee for separate owner acceptance. |

Required-body forms share one supply meaning. Checked artifacts distinguish
required, checked, externally supplied, and admission-bearing behavior explicitly;
a `boundary` Boolean cannot represent those distinctions. A claim-free bodyless
free machine has no supply mode and rejects. Executable bodies always use braces,
never expression-body syntax. A bodyless `ensures` is a claim, not a theorem by
virtue of its name. Accepted theorem claims use the existing boundary surface,
not a separate fact declaration.

`boundary` marks a crossing, not its direction. Imports and exports are
artifact-relative; requirements and composition determine provision or demand.
`satisfies` selects an exact requirement and inherits its contract. `via` only
supplies a compile-time-evaluated binding payload not derivable from declaration
and target; it is neither a supply keyword nor trust authority. Compiler
intrinsics use the sealed declaration/signature/target catalog without a binding
payload. See [foreign bindings](../build/foreign_bindings.md) and
[provider selection](../build/provider_selection.md).

Composite providers remain ordinary checked bodies; only irreducible foreign
leaves use external supply. The toolchain derives their selected conformance
closure, coverage, dependencies, reach, and admission inputs. Programs cannot
assemble provider-plan rows or reauthor inherited ceilings in a binding.

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

## States and identity

A state is an internal node. A transition jumps within the current machine and
creates neither a new machine identity nor a call frame. A call enters another
machine contract. [State contracts](state_contracts.md) govern arrivals and facts;
[termination](termination.md) governs ranked versus productive cycles.

The normalized semantic contract is independent of syntax and lowering.
Published identity includes the substitutable contract, supply, and observable
boundary calling plan. Component manifests, proof artifacts, admission, and
replacement checks retain it when runtime lowering erases proof-only material.
An implementation body hash, source keyword, or inferred narrow reach cannot
replace that identity. Private ranking evidence follows the separate
[identity rule](termination.md#published-guarantees-and-private-witnesses).

`05_machine_contracts.json` exposes this split as sibling `contract` and
`implementation` objects. The former reports supply, normalized service and
operational ceilings, published termination, and contract identity; the latter
reports the checked summary and private witness. Binding and component tools
pin the contract rather than incorporating proof-local implementation fields.
