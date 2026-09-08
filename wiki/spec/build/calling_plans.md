# Boundary calling plans

A boundary requirement selects its calling policy. Omega's internal convention
is compiler-owned and is not selected through this surface. Terminal Psi retains
target-neutral demand; evaluated target plans travel as
[realization inputs](../terminal-psi/boundary_calls.md), not target policy inside
the portable module.

## Authorship and evaluation

`Calling<C, Policy>` names a convention subject `C` and the exact named
conformance `Policy: C satisfies CallingPolicy`. Its compile-time `plan` machine
receives a normalized `BoundarySignature` and returns
`Accepted(BoundaryEntryPlan)` or `Rejected(CallingPolicyRejection)`.

Evaluation is deterministic, terminating, and build-time-admissible. A rejection
identifies the incompatible signature feature at the relationship's source
span and creates no accepted contract identity. An accepted value is still
subject to compiler validation and canonicalization.

Packages may author policies under the same rules as platform packages. The
`std::calling` vocabulary describes normalized signatures, placement, machine
state, acceptance, and rejection. Its quantities use `u64`, not `addr`; narrowing
to a compiler field requires a checked range conversion. Policies choose from
closed primitives; they cannot emit instructions, supply relocation bytes,
inspect private carriers, or bypass validation. Extending the primitive
register, placement, control, or machine-state vocabulary requires compiler
support, not another policy declaration.

Boundary service parents contribute service reach. Ordinary policy parents
contribute contract identity without service reach. A reusable boundary trait
may parameterize its convention and named policy; each closed application has
its own contract. Providers cannot independently choose conventions for the same
requirement. Mixed conventions require distinct applications or composed facets.

Static machine selection and direct invocation do not require runtime function
values. Calling-policy evaluation does not depend on general function-value,
environment-capture, or code-address facilities. A boundary machine retains its
ordinary declaration and `satisfies` relationship; policy is on the requirement,
not an extra modifier on the machine.

## Plan structure

| Record | Owns |
| --- | --- |
| `CallPlan` | Parameter/result placement, private callback materialization, ordinary clobbers, stack alignment and shadow space, entry/return control. |
| `StatePlan` | Initial machine regime, interrupted state, saved/restored state, and permitted transitive machine-state use. |
| `BoundaryEntryPlan` | The call and state plans, retaining their independent identities. |
| `BoundaryPlanApplication` | Exact requirement, ordered native parameters and their origins/shapes/placements, callback destinations, and evaluated entry plan. |

An ordinary call's register/stack ABI does not describe hardware entering an
already-live activation. Agreement between the two facets for ordinary calls
does not merge their meanings.

One native-parameter identity space contains semantic formals and private
callback entries. Declaration order fixes native argument position; declared
names establish nominal identity under the owning requirement. A multi-register
aggregate remains one parameter with several physical locations. Whole-parameter
and private-field destinations refer to the same identity space. See
[callback destinations](private_callbacks.md#declared-native-destinations).

## Shape and placement

[Boundary shapes](boundary_shapes.md) defines what the policy may classify.
[Structural access](../terminal-psi/structural_access.md) separately requires
every borrowed parameter to preserve its referent; an owned value's indirect
ABI placement does not create a borrow.

When a runtime opaque value is passed by value,
[representation selection](opaque_representations.md) closes its exact shape,
movement, and lifecycle application before calling-policy evaluation. A policy
classifies that application; it neither chooses nor reconstructs a private
representation. Reference-only pointees need no by-value application.

## Validation

The same validated plan governs outbound argument/result handling and inbound
entry/exit machinery. Validation checks:

- every native parameter and result is placed exactly once and compatibly;
- stack ranges, alignment, shadow space, and register classes agree;
- ordinary clobbers respect the ABI regime;
- saved/restored state covers the state commitment;
- entry/exit control is valid for the initial regime; and
- target and provider applicability match the requirement.

Unordered register/clobber sets normalize before fingerprinting. Equivalent
encodings produce the same normalized plan. Instructions changing regime require
their starting regime and establish the next; regions on either side retain
separate plans rather than one mixed-mode plan.

## Identity and selection

The canonical evaluated promise, not policy source, construction order, or a
friendly convention name, defines ABI identity. The complete public contract has
a domain-separated SHA-256 commitment; compact fingerprints are report
coordinates. Refactoring policy code while preserving its normalized result
preserves the ABI promise. Observable placement or state changes do not.

The reusable physical-plan identity is weaker than its application identity.
Application identity additionally binds the exact requirement, complete ordered
nominal parameter mapping, callback materializations, and any opaque
representation applications. Swapping equally shaped parameters cannot preserve
application identity merely because the register recipe is unchanged.

[Provider selection](provider_selection.md) chooses declared `satisfies` edges.
An external `via` contributes only payload that the exact declaration, signature,
and target do not determine. A locator, syscall number, or target label cannot
select calling policy. A descendant boundary trait may own a selected inherited
requirement's schema while the row retains the declaring requirement's identity.

All selected import, vtable, and service-table consumers retain the evaluated
plan through layout and emission. They cannot reselect it from the output target
or duplicate its arity in a dispatch recipe. Dispatch-only operands are added
only where the selected topology requires them. Missing or unrealizable plans
reject. A wider compiler scratch slot cannot redefine a retained foreign scalar
type; insufficient storage rejects.

The plan is the published promise. The final artifact's
[machine-state evidence](machine_state_evidence.md) establishes that a particular
implementation honors it without becoming part of that promise.

Current policy-source and normalized-model support is documented beside
[calling conventions](../../../omega-rust/omega/representations/calling-conventions/README.md).
