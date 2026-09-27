# Semantic representation ownership

Source spelling is resolved once into durable identities and independent semantic
axes. Later stages carry or independently validate those facts; they do not
recover meaning from body presence, keyword combinations, declaration names,
runtime layout, or provider selection. [Terminal Psi](../../../wiki/spec/terminal-psi/product.md)
is the published boundary, not these intermediate trees.

## Program forms along the pipeline

These seven independent packages hold the successive program forms. The
transformations live in the linked pipeline stages.

| Program form | Produced by | Next pipeline consumer |
| --- | --- | --- |
| [Tokens](tokens/src/lib.rs) | [00: source files → tokens](../pipeline/00_source-files-to-tokens/src/lib.rs) | [01: tokens → syntax trees](../pipeline/01_tokens-to-syntax-trees/src/lib.rs) |
| [Syntax trees](syntax-trees/src/lib.rs) | [01: tokens → syntax trees](../pipeline/01_tokens-to-syntax-trees/src/lib.rs) | [02: syntax → symbol-resolved trees](../pipeline/02_syntax-trees-to-symbol-resolved-trees/src/lib.rs) |
| [Symbol-resolved trees](symbol-resolved-trees/src/lib.rs) | [02: syntax → symbol-resolved trees](../pipeline/02_syntax-trees-to-symbol-resolved-trees/src/lib.rs) | [03: resolved → typed trees](../pipeline/03_symbol-resolved-trees-to-typed-trees/src/lib.rs) |
| [Typed trees](typed-trees/src/lib.rs) | [03: resolved → typed trees](../pipeline/03_symbol-resolved-trees-to-typed-trees/src/lib.rs) | [04: typed → checked trees](../pipeline/04_typed-trees-to-checked-trees/README.md) |
| [Checked trees](checked-trees/src/lib.rs) | [04: typed → checked trees](../pipeline/04_typed-trees-to-checked-trees/README.md) | [05: checked trees → lowered Psi](../pipeline/05_checked-trees-to-lowered-psi/README.md) |
| [Lowered Psi](lowered-psi/src/lib.rs) | [05: checked trees → lowered Psi](../pipeline/05_checked-trees-to-lowered-psi/README.md) | [06: lowered Psi → lowered Psi](../pipeline/06_lowered-psi-to-lowered-psi/README.md), then [07: lowered Psi → Terminal Psi](../pipeline/07_lowered-psi-to-terminal-psi/README.md) |
| [Terminal Psi](terminal-psi/src/lib.rs) | [07: lowered Psi → Terminal Psi](../pipeline/07_lowered-psi-to-terminal-psi/README.md) | [Omega 00: Terminal Psi → abstract operations](../../omega/pipeline/00_terminal-psi-to-abstract-operations/README.md) |

Stage 06 optimizes lowered Psi in the same representation. The independent
[checked interpreter](../interpreter/checked-interpreter/README.md) and
[Terminal interpreter](../interpreter/terminal-interpreter/README.md) consume
their respective program forms without creating another pipeline representation.

Constituent semantic records have separate package owners:
[facts](checked-trees/facts/src/lib.rs) holds fact contexts, places and evidence;
[flow-effects](../foundation/flow-effects/src/lib.rs) holds operational,
invocation, capability and reach summaries. The shared
[optimization selections](../foundation/optimization-selections/src/lib.rs)
package owns pass names, canonical selections and their identities. These
packages support the route above; none is an additional program stage.

Terminal Psi also owns a family of independent companion packages: `codec/`
for canonical artifacts, `semantics/` for operation judgments, `verification/`
for receiving checks, and `fuel/` with `fuel/certification/` for logical costs
and bounds. These are contracts over the same Terminal program, not additional
IR stages. Their manifests preserve the representation-to-checker dependency
boundary.

## Records and facts

| Subject | Representation obligation |
| --- | --- |
| [Domains](../../../wiki/spec/language/domains.md) | Carrier/index identity, predicate body, semantic roles, aliases, and authorized establishment routes are independent records; hybrids need no duplicated declaration. |
| [Machines](../../../wiki/spec/language/machines.md) | Carry normalized supply and complete public contract; consumption eligibility is derived, not a separate drifting Boolean. |
| [Conformances](../../../wiki/spec/language/conformances.md) | Retain the exact closed selected row map and complete application; exact satisfaction never manufactures whole-trait evidence. |
| [Termination](../../../wiki/spec/language/termination.md) | Public guarantee and premises are separate from checked local summary and private ranking witness. |
| [Ownership](../../../wiki/spec/language/ownership.md) | Exact places, establishment, access, multiplicity, loans, provenance, active payload debt, and ordered edge dispositions are retained. |
| [Carry](../../../wiki/spec/resources/carry.md) | Suspension, CPU, host-thread, and address axes compose independently, with the origins needed to interpret each demand. |
| [Effects](../../../wiki/spec/language/effects.md) | Service identity, synchronous invocation, suspension, blocking, crash routes, and positive progress remain separate even when inference shares one graph traversal. |

The [resolved domain](symbol-resolved-trees/src/symbol_resolved_trees/declarations/domain.rs)
and [typed domain](typed-trees/src/typed_trees/declarations/domain.rs) retain
predicate, role, route, alias, classification, and semantic-identity records.
The [typed machine](typed-trees/src/typed_trees/control_flow/machine.rs) carries
normalized supply, termination plan, service row, and independent operational
clauses. Shared vocabulary belongs in
[language-semantics](../foundation/language-semantics/src/lib.rs), not in a
later native stage. These records' existence does not establish every downstream
proof, normalization, or source-consumption route.

Flow-proven membership and binding qualification are different inputs. Operator
selection consumes binding/signature qualification across the complete operand
tuple, not newly discovered flow facts. Normalization defines published identity;
proofs establish legality without rewriting it. Index equality retains the exact
algebra, normalized public operation contract, and canonical expression.
Implementation versions and compatibility evidence remain distinct from semantic
identity. Qualification forgetting cannot discharge a resource claim or carry
obligation.

## Contracts versus implementation evidence

Keep public ceilings and guarantees separate from local inference, ranking,
proof artifacts, source markers, and selected execution. Exported omission means
the published negative/no-guarantee case; private inference may derive a tighter
summary. Changing the prover, private witness, or provider cannot silently change
that interface. Evidence retains all accepted premises that a derived fact
depends on; one checked input cannot conceal another admitted dependency.

Checked cleanup and crash plans are distinct. Cleanup orders exact transfer and
disposal actions on normal edges. A crash has an explicit no-successor/no-cleanup
termination and only a lower bound of known live claims, never a survivor-safety
claim. [Terminal calls and outcomes](../../../wiki/spec/terminal-psi/calls_and_outcomes.md)
own the publication/replay requirements for both. Absence of an action or mapping
is incomplete lowering, not implicit abandonment authority.

Target calling/layout, provider plans, executable-TCB manifests, installed entry
occurrences, and task activation plans belong to Omega's build/realization side.
They join exact normalized semantic demands through the bound companion; they
must not become target-selected Psi semantic fields. A static demand or decoded
report never substitutes for installation or single-use invocation custody.

## Validation and remaining work

Representation changes must preserve the independent semantic distinctions
above through their producing and consuming stages. [TASKS.md](../../../TASKS.md)
owns actual unclosed source-to-Terminal and native consumers; do not reproduce
migration milestones or stale missing-field inventories here.
