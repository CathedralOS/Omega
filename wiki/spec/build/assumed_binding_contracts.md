# Assumed binding contracts

The consuming root may link one runtime boundary requirement to the selected
implementation of another while explicitly assuming a semantic relationship
that ordinary conformance checking has not proved:

```omega
builder.assume_binding_contract<Required::operation, Supplied::operation>();
```

This is a typed Build operation, not a language keyword, unchecked body, or
cast between `Binding` values. It records a runtime installation rule. Both
requirements retain their own identities and contracts; the actual implementation
remains checked against the supplied requirement. The assumption belongs to the
relationship at this crossing, not to that implementation on every call.

## Caller, implementation, and root

Here `Range` and its routed `Writable` qualification stand for a library's
complete range contract: valid backing, write authority, lifetime and required
aliasing guarantees. They are not compiler primitives, and naming a domain
does not substitute for its definition or the core authority required by memory
operations. The domain must authorize the exact claim requirement as an issuer.

```omega
pub boundary trait WritableMemory {
    machine claim(region: Range) -> Range in Writable;
}

pub boundary trait RangeSupplier {
    machine supply(region: Range) -> Range;
}

pub data EchoSupplier { }

machine EchoSupplier::supply(region: Range) -> Range
satisfies RangeSupplier::supply
{
    region
}

machine claim_memory(region: Range) -> Range in Writable
reaches WritableMemory;
{
    WritableMemory::claim(region)
}

machine build(builder: &mut Build) {
    Build::application(builder, "memory_editor");
    builder.select_provider<RangeSupplier, EchoSupplier>();
    builder.assume_binding_contract<WritableMemory::claim, RangeSupplier::supply>();
}
```

`EchoSupplier::supply` returns unqualified data. It neither proves nor assumes
write authority in its own body. The admitted caller-facing crossing establishes
the qualification on the exact returned subject. Calling the supplier directly
does not. Ordinary `satisfies WritableMemory::claim` would still require proof
of the complete contract and is not synthesized by this link.

The root may be wrong about both backing and authority, including deliberately
wrong about the external world. Checked consumers then operate under a false
premise. This is not a guarantee of a recoverable trap, confinement, or predictable
behavior after that trust fails. Memory instructions and ordinary code retain
their complete checks relative to the admitted premises; no `unsafe` block,
raw-instruction exception, or `admit_implementation` facility is introduced.

## Compatibility and direction

Resolve both static operands to exact requirements/applications under ordinary
visibility and package rules. The supplied side must have one selected complete
provider and executable realization. Equal spelling, a runtime address, or a
signature alone cannot select it. Static type/const/conformance arguments and
lifetime correspondence remain explicit parts of the checked relationship.

| Part | Required compatibility |
| --- | --- |
| Parameters and results | Same count, order, and underlying runtime carriers; parameter names may differ. |
| Passing modes | Preserve owned values, shared, mutable and write-only borrows, and their explicit subject correspondence. |
| Representation | Same validated runtime layout and calling plan; equal byte size alone is insufficient. |
| Ownership and lifetimes | Preserve transfers, borrow relationships, multiplicity and cleanup; no silent copy, drop or lifetime extension. |
| Semantic contracts | Differences in propositions, erased qualifications and authorized provenance need checked evidence or an exact root assumption. |
| Operational contracts | Check reach, suspension, blocking, crashes, invocation and termination compatibility; retain actual transitive implementation dependencies. |

A representation-changing qualification cannot be ignored as erased evidence.
Different runtime shapes require an ordinary checked adapter with a matching
entry; the build operation does not reorder arguments, recast scalar bits, or
use an ABI trampoline to disguise incompatible contracts. Ordinary boundary
dispatch remains available. Each parameter/result is mapped exactly once.

The two semantic checks are directional:

```text
entry: caller preconditions + admitted entry premises establish implementation preconditions
exit:  implementation postconditions + admitted exit premises establish caller postconditions
```

An implementation may therefore require a qualification absent from the caller's
parameter, or the caller may receive a qualification absent from the
implementation's result. Admission identifies each exact subject and missing
fact; it is not an ambient assertion that every value of that carrier is valid.
Routed authority still needs its domain-owner-authorized establishment route and
exact entry/result receipt. Fresh claims are explicit admitted issuance, not
unrecorded duplication of a transferred claim. These are the ordinary
[authority and custody obligations](../resources/authority.md), not a second
range-accounting scheme.

## Known contradictions at the trust crossing

An explicitly authorized and accepted `assume_binding_contract` relationship may
assume its semantic connection even when the checker can refute that connection
(`assumed-binding-refutable-claim`). The caller checks against the required
contract; the implementation checks against the supplied contract. The root
assumes the connection between them, not correctness of an unchecked body.

For example, the required operation may return `u32` with `result == 1` while
the supplied operation returns `u32` with `result == 0`. Their runtime shapes
agree, but their guarantees contradict each other. The explicit runtime link
may still be installed under the required root and receiving-policy acceptance.
Exposing the supplied guarantee makes the contradiction auditable; it does not
make the relationship ineligible merely because the lie is now demonstrable.

This is a scoped exception to the
[refutable-claim veto](../proofs/contracts.md#axioms-and-receiving-policy), not its
removal for ordinary grants, mathematical axioms, or checked `satisfies` edges.
It applies to the exact assumed entry/exit relationship and subjects only.
Neither the assumption nor conclusions derived from it can waive runtime-shape,
passing-mode, ABI, ownership/lifetime mapping, operational compatibility, either
body's own checking, or the build-time prohibition. Those checks remain
independent of the assumed semantic connection.

Record a detected contradiction with the precise conflicting statements,
subjects and facts establishing the refutation. Report it through the existing
extremely-high-risk review; the operation needs no separate contradiction flag
or new escape form. Failure to prove a relationship is not a refutation, and
failure to find a contradiction establishes neither correctness nor consistency.

Checked callers and optimizers may rely on the caller-facing contract. In the
example, a comparison of the returned value with `1` may be folded to true;
admission does not promise that a caller observes `0`, traps, or detects the lie.
Artifact guarantees remain conditional on the recorded assumption. Its falsity
invalidates those guarantees; no unconditional theorem that `0 == 1` or checked
conformance is created. Evidence checking verifies derivations and assumption
scope, not the truth of the root's assertion.

## Selection and retained evidence

The link supplies one caller-facing operation, not an inferred whole-trait
conformance. Every required member still needs exactly one supply. Explicit root
selection outranks target defaults under ordinary provider precedence. Competing
explicit checked and assumed supplies for the same operation reject rather than
silently preferring the assumption. Follow any supplied-side links to an actual
selected implementation, retaining all assumptions; cyclic binding aliases reject.

Retain both contracts and their exact applications, positional subject mapping,
proved versus assumed obligations, domain establishment authority, selected
provider/body and its semantic dependencies, calling/layout identity, root grant,
and runtime-only classification. Binding identity cannot hide the actual code
being invoked. Detected semantic contradictions and their supporting facts remain
attached to the assumed relationship. Inlining, wrappers, normalization, and
erasure cannot turn the relationship into a checked conformance or remove its
transitive trust record.

Psi retains the abstract requirement, contract evidence and runtime demands.
Omega's existing provider planning/installation validates and applies the
root-selected relationship. Reuse ordinary call and result custody; do not add a
parallel compiler pipeline, opaque instruction path, or package-owned verifier.
Independent verification checks the assumption and structural obligations under
the receiving policy; it does not certify the assumption's truth.

## Runtime only

Only the consuming runtime root may authorize the relationship. Dependencies
cannot authorize themselves, and this operation cannot widen delegated build
services. Root authorization and [project acceptance](../packages/acceptance.md#assumed-binding-contract-acceptance)
are both required where package acceptance applies.

`build.omg` may register the runtime relationship, but receives no callable
binding or proposition evidence from registration. Execution through that link,
or reliance on its assumptions to justify build-time computation, rejects in
build entries, imported helpers, generators, constants, proof/layout evaluation,
and automatic build-time tests. The prohibition follows the selected call and
trust closure, including erased premises; neither a host-service grant nor a
root flag can override it. Ordinary proof-admissible mathematical axioms remain
governed by their own admission rules.

Both static requirement operands use the existing
[non-executing product-reference rules](scoped_execution.md#selecting-product-declarations-without-executing-them).
Resolving them does not import the product implementation into the build-host
execution closure. The complete registered relationship is still reviewed before
runtime installation; mere visibility is not acceptance.

Inspecting/checking runtime declarations, producing a runtime artifact and its
conditional evidence, and recording non-executing product references remain
allowed. Independently checked calls to the underlying supplier can remain
build-admissible if they do not traverse or rely on the assumed link. A runtime
interpreter may install an accepted relationship under its runtime policy;
being interpreted does not turn runtime execution into build execution or make
guest address IDs into host pointers.

## Review and delivery

Package install, update and audit label every such relationship **Extremely high
risk - assumed binding contract**. Show the required and supplied contracts,
actual implementation, exact assumption difference, authority introduced, root
scope and transitive dependency path, including any demonstrated contradiction.
Do not describe a refuted connection as merely unproved. Changed relationship or
implementation identity needs fresh explicit acceptance. A dependency's own lock
or an ordinary provider selection is not that decision. Keep the compact readable
risk row in the existing lock model, not a second admission database or full
compiler dump.

Implementation must demonstrate both entry and result qualification differences,
an honestly checked supplier and caller, absence of facts on a direct supplier
call, and explicitly accepted refutable entry/exit relationships. Preserve
ordinary refutable-grant rejection, rejected shape/mode/custody mismatches,
root-only acceptance, transitive audit and update invalidation, and build-time
refusal through helpers, provider dispatch and erased evidence. Test the
contradiction's retained evidence and conditional claim, not a promised runtime
result after false trust. Assembly keeps its existing checked contract.
