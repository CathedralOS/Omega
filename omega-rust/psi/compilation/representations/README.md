# Semantic representation ownership

The front end resolves source spelling once into lasting identities and
separate semantic records. Later stages carry or recheck those facts; they
never guess meaning from body presence, keywords, declaration names, runtime
layout or provider choice.
[Terminal Psi](../../../../wiki/spec/terminal-psi/product.md) is what the
compiler publishes, not these intermediate trees.

## Program forms along the pipeline

These packages define the program forms; the linked stages transform them.
Source data and checker evidence live in different packages.

| Program form | Produced by | Next pipeline consumer |
| --- | --- | --- |
| [Tokens](tokens/src/lib.rs) | [00: source files → tokens](../pipeline/00_source-files-to-tokens/lexer.rs) | [01: tokens → syntax trees](../pipeline/01_tokens-to-syntax-trees/parser.rs) |
| [Syntax trees](syntax-trees/src/lib.rs) | [01: tokens → syntax trees](../pipeline/01_tokens-to-syntax-trees/parser.rs) | [02: syntax → symbol-resolved trees](../pipeline/02_syntax-trees-to-symbol-resolved-trees/src/lib.rs) |
| [Symbol-resolved trees](symbol-resolved-trees/src/lib.rs) | [02: syntax → symbol-resolved trees](../pipeline/02_syntax-trees-to-symbol-resolved-trees/src/lib.rs) | [03: resolved → typed trees](../pipeline/03_symbol-resolved-trees-to-typed-trees/src/lib.rs) |
| [Typed trees](typed-trees/src/lib.rs) | [03: resolved → typed trees](../pipeline/03_symbol-resolved-trees-to-typed-trees/src/lib.rs) | [04: typed preparation](../pipeline/04_typed-trees-to-typed-trees/README.md), then [structural lowering](../pipeline/05_typed-trees-to-terminal-psi/src/lib.rs) |
| [Terminal Psi](terminal-psi/src/lib.rs) | [structural lowering](../pipeline/05_typed-trees-to-terminal-psi/src/lib.rs), [judgment](../pipeline/06_terminal-psi-to-terminal-psi/src/lib.rs) and [accepted-source publication](../terminal-production/src/accepted.rs) | [Omega 00](../../../omega/compilation/pipeline/00_terminal-psi-to-abstract-operations/README.md) |

Stage 04 runs one ordered pass list and freezes the typed trees, refusing
requests it does not serve; the result is prepared, not proved. Terminal Psi
needs structural lowering, checking and publication; default Build and native
compilation still wait on [TASKS.md](../../../../TASKS.md).

The separate [Terminal interpreter](../../interpreter/README.md) runs
Terminal Psi directly and adds no pipeline representation.

The old facts package is gone; checking facts live in the lowered-form
checker's tables. Other semantic records have their own packages:
[flow-effects](../../foundation/flow-effects/src/lib.rs) holds operational,
invocation, capability and reach summaries, and
[optimization selections](../../foundation/optimization/src/lib.rs) holds
pass names, canonical selections and their identities. Neither is a program stage.

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

The [resolved domain](symbol-resolved-trees/src/declarations_and_machines.rs)
and [typed domain](typed-trees/src/declarations.rs) keep
predicate, role, route, alias, classification and semantic-identity records.
The [typed machine](typed-trees/src/calls_and_control_flow.rs) carries
normalized supply, termination plan, service row and separate operational
clauses. Shared vocabulary belongs in
[language-semantics](../../foundation/language-semantics/src/lib.rs), not in a
later native stage. Having these records does not mean every later proof,
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
manifests, installed entries and task activation plans belong to Omega's build
and realization side. They meet exact normalized semantic demands through the
bound companion; they must not become per-target Psi semantic fields. A static
demand or decoded report never replaces installation or single-use invocation
custody.

## Validation and remaining work

A representation change must keep the semantic records above separate
through every stage that produces or consumes them. [TASKS.md](../../../../TASKS.md)
tracks the unfinished source-to-Terminal and native work; do not copy
migration milestones or lists of missing fields here.
