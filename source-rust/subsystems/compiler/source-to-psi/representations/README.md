# Source pipeline representations

Tokens, syntax trees, resolved trees, typed trees, prepared typed trees, the lowered program and the checked program are the forms passed between source stages. The checked program keeps the exact lowered program and its evidence. Stage 07 publishes the [Psi artifact](../../psi/).

## Program forms along the pipeline

The following table
shows which records its operations produce and what the next operation reads.

| Program form | Producer | Next consumer |
| --- | --- | --- |
| [Tokens](tokens/token_stream.rs) | [Lexer](../pipeline/00_source-files-to-tokens/src/lib.rs) | [Parser](../pipeline/01_tokens-to-syntax-trees/src/lib.rs) |
| [Syntax trees](syntax-trees/syntax_trees.rs) | [Parser](../pipeline/01_tokens-to-syntax-trees/src/lib.rs) | [Name resolution](../pipeline/02_syntax-trees-to-symbol-resolved-trees/src/resolver.rs) |
| [Resolved source](symbol-resolved-trees/symbol_resolved_trees.rs) and its [symbol table](symbol-resolved-trees/symbol-table/symbol_table.rs) | Name resolution | [Typing](../pipeline/03_symbol-resolved-trees-to-typed-trees/src/lowerer.rs) |
| [Typed source](typed-trees/typed_trees.rs) | Typing | [Stage 04 preparation](../pipeline/04_typed-trees-to-prepared-typed-trees/src/lib.rs) |
| [Prepared typed trees](prepared-source/lib.rs) | [Stage 04](../pipeline/04_typed-trees-to-prepared-typed-trees/src/lib.rs) runs specialization and domain preparation, retaining typing's record-pattern identities | [Stage 05 lowering](../pipeline/05_prepared-typed-trees-to-lowered-program/src/compilation.rs) |
| [Lowered program](lowered-program/lowered_program.rs), bound to its source in a [lowered source publication](lowered-source-publication/lowered_source_publication.rs) | [Stage 05 lowering](../pipeline/05_prepared-typed-trees-to-lowered-program/src/compilation.rs) | [Stage 06 judgment](../pipeline/06_lowered-program-to-checked-program/src/publication.rs) reads the root closure, demanded forms and witnesses together. It never reads the source. |
| [Checked program](checked-program/checked_program.rs) | The [lowered-form checker](../pipeline/06_lowered-program-to-checked-program/src/publication.rs) returns it with the source of the lowering it judged. | The [accepted-source factory](../pipeline/07_checked-program-to-psi/src/production/accepted.rs) takes that judgment for [stage 07 publication](../pipeline/07_checked-program-to-psi/src/source/publication.rs). |
| [Published target providers](published-target-providers/publication.rs) | [Stage 07](../pipeline/07_checked-program-to-psi/src/provider_supply/demand.rs) binds the checked provider demand and completed Build bindings to a Psi manifest and deployment. | [Native stage 00](../../psi-to-native/pipeline/00_psi-to-admitted-native-inputs/provider-demand/src/lib.rs) receives them for that manifest and target. |

When a dependency's Build chooses a provider or a native binding, the package stores the [declarations and arguments it selected](retained-product-selections/selections.rs). Build keeps the [provider choices](../../build/inputs/package/provider_selections.rs) and [native bindings](../../build/inputs/package/native_bindings.rs) in the dependency’s package handoff. The source compiler checks them against the consuming program.

The [Terminal interpreter](../../../interpreter/README.md) runs the published
program. The published program includes its operation, invocation, capability and reach summaries. [Optimization policy](../../psi/artifact/optimization-selections/src/lib.rs)
defines pass selections and their identities.

The shared Psi representation has companion packages for [canonical encoding](../../psi/artifact/codec/), [operation semantics](../../psi/semantics/) and [logical work bounds](../../psi/semantics/work/). The source [lowered program](lowered-program/) owns the tables consumed by stage 06.

## Who can build a product

Stages 04, 05 and 06 each hand the next stage a value that says a decision was
made: the passes ran, this program was lowered from this source, the checker
accepted it. A stage that could build such a value itself could skip the
decision. So each of these three products has private fields and one
constructor, and the constructor takes a capability:

| Product | Constructor | Capability | Only linked by |
| --- | --- | --- | --- |
| [Prepared typed trees](prepared-source/lib.rs) | `PreparedTypedTrees::new` | [`PreparationIssuer`](prepared-source/issuer/issuer.rs) | stage 04 |
| [Lowered source publication](lowered-source-publication/lowered_source_publication.rs) | `LoweredSourcePublication::from_lowering` | [`LoweringIssuer`](lowered-source-publication/issuer/issuer.rs) | stage 05 |
| [Checked program and its judgment](checked-program/checked_program.rs) | `CheckedProgram::from_judgment`, `CheckedPublication::from_judgment` | [`CheckerIssuer`](checked-program/issuer/issuer.rs) | stage 06 |

Each capability is the only item of a crate of its own. Rust lets a crate name
only the crates its manifest lists, so a stage that does not list the issuer
cannot write the constructor call. `mbx xtask check-boundary` refuses the
issuer in any other manifest, including as a test dependency. The same pattern
guards the [bound framed machine](../../psi-to-native/representations/bound-framed-machine/issuer/issuer.rs)
in the native pipeline.

## Records and facts

| Subject | What the representation must keep |
| --- | --- |
| [Domains](../../../../../wiki/spec/language/domains.md) | Carrier/index identity, predicate body, semantic roles, aliases and permitted establishment routes as separate records; a hybrid needs no duplicate declaration. |
| [Machines](../../../../../wiki/spec/language/machines.md) | Normalized supply and the complete public contract; consumption eligibility is derived, not a separate Boolean that can drift. |
| [Conformances](../../../../../wiki/spec/language/conformances.md) | The exact closed selected-row map and complete application; satisfying those rows never creates whole-trait evidence. |
| [Termination](../../../../../wiki/spec/language/termination.md) | Public guarantee and premises, apart from the checked local summary and private ranking witness. |
| [Ownership](../../../../../wiki/spec/language/ownership.md) | Exact places, establishment, access, multiplicity, loans, provenance, outstanding payload obligations and ordered edge actions. |
| [Resources across suspension](../../../../../wiki/spec/resources/suspension.md) | Live places, loan origins and backing across a possibly suspending call. The compiler still encodes the retired carry policy. |
| [Effects](../../../../../wiki/spec/language/effects.md) | Service identity, synchronous invocation, suspension, blocking, crash routes and positive progress, separate even when inference computes them in one graph walk. |

The [resolved domain](symbol-resolved-trees/nodes/declarations_and_machines.rs)
and [typed domain](typed-trees/definitions/declarations.rs) keep
predicate, role, route, alias, classification and semantic-identity records.
The [typed machine](typed-trees/callables/calls_and_control_flow.rs) carries
normalized supply, termination plan, service row and separate operational
clauses. Shared vocabulary belongs in
[semantic vocabulary](../../foundation/semantic-records/src/lib.rs). Having these records does not mean every later proof,
normalization or use is implemented.

Flow-proven membership and a binding's qualification are different inputs.
Operator selection reads binding and signature qualification across all
operands, not flow facts found later. Normalization
fixes the published identity; proofs show legality without rewriting it.
Index equality keeps the exact algebra, normalized public operation contract
and canonical expression. Implementation versions and compatibility evidence
stay separate from semantic identity. Dropping a qualification cannot
discharge a resource claim or a carry obligation.

## Contracts versus implementation evidence

Keep public ceilings and guarantees separate from local inference, ranking,
proof artifacts, source markers and the chosen execution. An export that omits
a guarantee publishes none; private inference may derive a tighter summary. Changing the prover, private witness or provider cannot
silently change that interface. Evidence keeps every accepted premise a
derived fact depends on; one checked input cannot hide another dependency.

Cleanup plans and crash plans differ. Cleanup orders exact transfer and
disposal actions on normal edges. A crash ends explicitly with no successor
and no cleanup; it records only a lower bound of known live claims, never that
survivors are safe.
[Terminal calls and outcomes](../../../../../wiki/spec/terminal-psi/calls_and_outcomes.md)
states how both are published and replayed. A missing action or mapping means
lowering is incomplete; it does not permit abandoning the resource.

Target calling and layout, provider plans, executable trusted-base
manifests, installed entries and task activation plans belong to Omega's target planning and installation stages. They meet exact normalized semantic demands through the
bound companion; they must not become per-target Psi semantic fields. A static
demand or decoded report never replaces installation or single-use invocation
custody.

## Validation and remaining work

A representation change must keep the semantic records above separate
through every stage that produces or consumes them. [TASKS.md](../../../../../TASKS.md)
tracks the unfinished source-to-Terminal and native work; do not copy
migration milestones or lists of missing fields here.
