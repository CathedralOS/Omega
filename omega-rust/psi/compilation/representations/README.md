# Semantic representation ownership

The front end resolves source spelling once into lasting identities and
separate semantic records. Later stages carry or recheck those facts; they
never guess meaning from body presence, keywords, declaration names, runtime
layout or provider choice.
[Terminal Psi](../../../../wiki/spec/terminal-psi/product.md) is what the
compiler publishes, not these intermediate trees.

## Program forms along the pipeline

Stage 01 holds resolved and typed source until judgment. The following table
shows which records its operations produce and what the next operation reads.

| Program form | Producer | Next consumer |
| --- | --- | --- |
| [Tokens](tokens/token_stream.rs) | [Lexer](../pipeline/01_source-compilation/src/operations/lexing/lexer.rs) | [Parser](../pipeline/01_source-compilation/src/operations/parsing/parser.rs) |
| [Syntax trees](syntax-trees/syntax_trees.rs) | [Parser](../pipeline/01_source-compilation/src/operations/parsing/parser.rs) | [Name resolution](../pipeline/01_source-compilation/src/operations/resolution/resolver.rs) |
| [Resolved source](../pipeline/01_source-compilation/src/source_resolved/src/lib.rs) | Name resolution | [Typing](../pipeline/01_source-compilation/src/operations/typing/src/lowerer.rs) |
| [Typed source](../pipeline/01_source-compilation/src/source_state/src/lib.rs) | Typing | [Source preparation](../pipeline/01_source-compilation/src/operations/preparation/README.md) and [structural lowering](../pipeline/01_source-compilation/src/operations/lowering/src/lib.rs) |
| [Terminal Psi](terminal-psi/src/lib.rs) | Structural lowering and the [lowered-form checker](../pipeline/01_source-compilation/src/operations/checking/src/verdict.rs) | The [accepted-source factory](../pipeline/01_source-compilation/src/operations/production/src/accepted.rs) passes its judgment to [stage 02 publication](../../../omega/compilation/pipeline/02_portable-publication/src/lib.rs). |

The [Terminal interpreter](../../interpreter/README.md) runs the published
program. Operational, invocation, capability and reach summaries live in
[source effects](source-effects/src/lib.rs). [Optimization policy](optimization-policy/src/lib.rs)
defines pass selections and their identities.

Terminal Psi also has separate companion packages: `codec/` for canonical
artifacts, `semantics/` for operation judgments, `work/` for logical costs and
bounds, and [`check-tables/`](terminal-psi/check-tables/README.md) for the
lowered-form checker's tables. None is another IR stage. Their manifests keep
the dependency boundary between the representation and the checker.

## Records and facts

| Subject | What the representation must keep |
| --- | --- |
| [Domains](../../../../wiki/spec/language/domains.md) | Carrier/index identity, predicate body, semantic roles, aliases and permitted establishment routes as separate records; a hybrid needs no duplicate declaration. |
| [Machines](../../../../wiki/spec/language/machines.md) | Normalized supply and the complete public contract; consumption eligibility is derived, not a separate Boolean that can drift. |
| [Conformances](../../../../wiki/spec/language/conformances.md) | The exact closed selected-row map and complete application; satisfying those rows never creates whole-trait evidence. |
| [Termination](../../../../wiki/spec/language/termination.md) | Public guarantee and premises, apart from the checked local summary and private ranking witness. |
| [Ownership](../../../../wiki/spec/language/ownership.md) | Exact places, establishment, access, multiplicity, loans, provenance, outstanding payload obligations and ordered edge actions. |
| [Carry](../../../../wiki/spec/resources/carry.md) | Suspension, CPU, host-thread and address demands, combined separately, each with the origin that explains it. |
| [Effects](../../../../wiki/spec/language/effects.md) | Service identity, synchronous invocation, suspension, blocking, crash routes and positive progress, separate even when inference computes them in one graph walk. |

The [resolved domain](../pipeline/01_source-compilation/src/source_resolved/src/declarations_and_machines.rs)
and [typed domain](../pipeline/01_source-compilation/src/source_state/src/declarations.rs) keep
predicate, role, route, alias, classification and semantic-identity records.
The [typed machine](../pipeline/01_source-compilation/src/source_state/src/calls_and_control_flow.rs) carries
normalized supply, termination plan, service row and separate operational
clauses. Shared vocabulary belongs in
[semantic vocabulary](semantic-vocabulary/src/lib.rs). Having these records does not mean every later proof,
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
[Terminal calls and outcomes](../../../../wiki/spec/terminal-psi/calls_and_outcomes.md)
states how both are published and replayed. A missing action or mapping means
lowering is incomplete; it does not permit abandoning the resource.

Target calling and layout, provider plans, executable trusted-base
manifests, installed entries and task activation plans belong to Omega's target planning and installation stages. They meet exact normalized semantic demands through the
bound companion; they must not become per-target Psi semantic fields. A static
demand or decoded report never replaces installation or single-use invocation
custody.

## Validation and remaining work

A representation change must keep the semantic records above separate
through every stage that produces or consumes them. [TASKS.md](../../../../TASKS.md)
tracks the unfinished source-to-Terminal and native work; do not copy
migration milestones or lists of missing fields here.
