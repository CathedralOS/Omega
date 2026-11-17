# Decisions for the complete migration plan

The [migration plan](plans/foundation-and-backend.md) now specifies **one
coordinated change set and one cutover**, after the architecture is resolved.
The task remains plan first; no compiler implementation is authorized by this
page. Internal work packages can run in dependency order or in parallel, but
are not separate migration rollouts. The six [owner rulings](rulings.md) remain
unchanged.

All recommendations below are **judgment**. Source links describe compiler
snapshot `3f9ce43f0c9d179048911d18faa91ba93f60176d`; the plan separately identifies
known later changes. “Unverified” means a concrete design obligation must close
before implementation, not that the old behavior may be dropped.

## Q-A. Where does shared host support belong?

**Recommendation:** a narrow host-support owner outside compilation. Package
resolvers, runtimes, CLI and tools may use generic process/filesystem mechanisms;
CLI-only observations stay with CLI, and products owns publication. Update the
runtime dependency rule explicitly with these consumers. Placing a compiler
coordinator or source lookup behind a generic host interface is not allowed.

**Planning obligation:** classify the `bounded-process`, `platform-custody` and
filesystem-authority exports by real consumer and operation. The
[graph](evidence/data/graph.json) proves shared declarations, not runtime calls.
The exact packages/names follow that classification; they are not already
ratified by the owner.

## Q-B. Which Build/source information may cross the boundary?

**Recommendation:** immutable semantic facts and narrow provenance, never live
source custody or a mutable Build result. Source settlement receives admitted
bytes, scope/consent and neutral Build selections. It publishes exact chosen
identities, prepared facts and accepted code. Later native phases cannot acquire
source, activate a body or rejoin a symbol table.

[RetainedDeclaration includes SourceFile][retained-declaration], whose
[contents include text and scope][source-file]; its [rejoin operation][retained-rejoin]
is not an identity-only leaf. `terminal-psi/vocabulary` must distinguish
stable definitions/carriers from producers and checks. Access/layout construction
and semantic encoding do not become primitives because they are pure.

**Unverified:** complete the per-field projection and provider-policy input
inventory. An opaque callback or retained Arc to a source owner fails the
contract even if its public type is small.

## Q-C. What replaces the current source/evaluation interleaving?

**Recommendation:** one explicit source-settlement transition owns the current
Psi 00–06 dependency closure. Source acquisition/admission remains before it;
canonical sealing and native work remain after it. Inside settlement, lazily
lex/parse activated units, resolve/type selected bodies, prepare applications,
lower/check the required program or evaluation closure, execute accepted
invocations, and discharge pending dependencies. Preserve the one lowered-form
checker and independent certificate admission.

This deliberately changes stage granularity. [Source continuation already
alternates resolution and typing][source-loop], and [evaluation preparation][source-job]
requires later source operations. Wrapping the old stage API calls in a new
coordinator is not the replacement. Eagerly parsing every available file would
also change the demand/error contract.

The executor receives immutable accepted code and a sealed invocation admission
binding caller, arguments, target/supplies, permitted observations and account.
It returns values/status and receipts. It receives no mutable syntax, source
consent object or source lookup callback. Source-job construction and
[retained-product preparation][product-preparation] move into settlement.
The [existing dependency][source-dependency] prevents simply reversing the
facade call without replacing both interfaces.

**Unverified:** specify activation/extension order, dependency-cycle behavior,
charging/reuse and stable accepted answers. Distinct private operations stay
named and testable; source settlement must not absorb package acquisition,
native compilation or publication as an all-purpose stage.

## Q-D. Which definitions may later representations share?

**Recommendation:** share explicit acyclic identity/vocabulary leaves and
unchanged declarations by content identity; carry computed facts in the current
form. A later phase may not use an earlier executable graph as a database.
Re-export chains and renamed wrappers do not establish this distinction.

The declaration-selection ledger [already passes through typed trees][ledger-transfer].
Its definition's old location is not a missing handoff. In contrast,
[EmissionFacts retains an earlier abstract program][emission-facts]. Inventory
its actual consumers and replace those reads with justified current fields.

**Unverified:** finish the producer/consumer field map and constructor-authority
model, including runtime provision and independent verifier inputs. No driver
or earlier-stage callback belongs in a program representation.

## Q-F. Where does all executable code become complete?

**Recommendation:** physical planning represents every ordinary and generated
body; common encoding produces all executable fragments before layout, object
and image construction. Image work is placement/relocation/container production,
not a second instruction/frame generator. This replaces the old proposal to
move entry/foreign/hosted/writer generators wholesale into an O14 stage.

[Entry bridges currently obtain effects from completed body objects][entry-input]
and then choose [scratch/frame/encoding details][bridge-encoding]. O08 also
[re-encodes callback materializations][callback-encoding]. Their replacement
needs explicit recipes, carried body effects, complete encoding alternatives and
fixups. Preserve body-first relaxation and later frame placement using already
encoded frame fragments.

**Unverified:** prove the generated-body and fixup contracts for both ISAs and all
formats. A late value may choose only a carried encoding alternative; it may not
create a body, change allocation or increase its own resource requirement.
Exported post-handoff helpers need real caller evidence before assigning a
production phase.

## Q-G. How do provider closure, retained bodies and installations fit?

**Recommendation:** logical provider/body discovery closes inside source
settlement from admitted Psi-neutral Build selection records. Omega then admits
physical ABI plans over those fixed selections and already accepted code. It
cannot select a new source body when a target check fails.

The [observed provider loop][closure-loop] grows demand through selected exact
bodies; [policy preparation still has source custody][policy-input]. Splitting
those responsibilities while preserving choices is a required proof, not a
simple folder move. Calling-policy code needed later must already be accepted.

Keep available, demanded and retained body sets distinct. Native planning closes
the references surviving relevant optimization over the accepted code catalog;
unused availability must not demand target support. The [current callback walk][callback-demand]
uses emitted object references, so an early conservative roster is not enough.
The plan absorbs demand-changing native planning operations into one owner and
requires proof that encoding/layout cannot discover more bodies.

Physical planning also includes entries/adapters/handlers and their effects.
Binding uses a complete resource summary: [transitive body demand, bridge
storage, call bias, alignment and fault premises][stack]. The current root waits
for handler objects before selection; a frame-size-only replacement cannot
preserve that contract. [O03's numeric guard consumer][guard] must instead read
a symbolic requirement until a proved stable binding point.

**Unverified:** close the logical/ABI selection split, retained-body closure,
generated-body effect model and numeric-binding stability. If a bound changes
register constraints, helper reach or its own stack demand, revise the phase
contract before implementation. Do not hide recursive handler compilation in a
new binding stage.

## What resolves these questions

The [planning gates](plans/foundation-and-backend.md#8-planning-completion-gates-before-implementation)
require concrete fields/APIs, actual producer/consumer inventories and an
ordering argument. Once those gates close, the [integrated execution plan](plans/foundation-and-backend.md#9-one-integrated-execution-plan)
implements all replacements and retires the old routes together. These pages do
not announce that feasibility, implementation or compiler validation has already
been completed.


[bridge-encoding]: https://github.com/CathedralOS/Omega/blob/3f9ce43f0c9d179048911d18faa91ba93f60176d/omega-rust/omega/compilation/backend/machine-emission/src/native_entry_bridge.rs#L701
[callback-demand]: https://github.com/CathedralOS/Omega/blob/3f9ce43f0c9d179048911d18faa91ba93f60176d/omega-rust/omega/compilation/native-compiler/src/realization/callback_thunks.rs#L182
[callback-encoding]: https://github.com/CathedralOS/Omega/blob/3f9ce43f0c9d179048911d18faa91ba93f60176d/omega-rust/omega/compilation/pipeline/08_selected-form-encoding-to-resolved-layout/src/ordinary.rs#L676
[closure-loop]: https://github.com/CathedralOS/Omega/blob/3f9ce43f0c9d179048911d18faa91ba93f60176d/omega-rust/omega/compilation/compiler/src/execution_settlement.rs#L260
[emission-facts]: https://github.com/CathedralOS/Omega/blob/3f9ce43f0c9d179048911d18faa91ba93f60176d/omega-rust/omega/compilation/native-compiler/src/pipeline.rs#L261
[entry-input]: https://github.com/CathedralOS/Omega/blob/3f9ce43f0c9d179048911d18faa91ba93f60176d/omega-rust/omega/compilation/backend/images/image-emission/src/entries.rs#L1251
[guard]: https://github.com/CathedralOS/Omega/blob/3f9ce43f0c9d179048911d18faa91ba93f60176d/omega-rust/omega/compilation/pipeline/03_target-operations-to-selected-instructions/src/selection_interrupts.rs#L1145
[ledger-transfer]: https://github.com/CathedralOS/Omega/blob/3f9ce43f0c9d179048911d18faa91ba93f60176d/omega-rust/psi/compilation/pipeline/03_symbol-resolved-trees-to-typed-trees/src/lowerer.rs#L1622
[policy-input]: https://github.com/CathedralOS/Omega/blob/3f9ce43f0c9d179048911d18faa91ba93f60176d/omega-rust/omega/compilation/compiler/src/terminal/provider_execution.rs#L14
[product-preparation]: https://github.com/CathedralOS/Omega/blob/3f9ce43f0c9d179048911d18faa91ba93f60176d/omega-rust/psi/compilation/build-time-evaluation/src/product/retained_provider_selections.rs#L120
[retained-declaration]: https://github.com/CathedralOS/Omega/blob/3f9ce43f0c9d179048911d18faa91ba93f60176d/omega-rust/psi/foundation/symbols/src/retained_declaration.rs#L67
[retained-rejoin]: https://github.com/CathedralOS/Omega/blob/3f9ce43f0c9d179048911d18faa91ba93f60176d/omega-rust/psi/foundation/symbols/src/retained_declaration.rs#L106
[source-dependency]: https://github.com/CathedralOS/Omega/blob/3f9ce43f0c9d179048911d18faa91ba93f60176d/omega-rust/psi/compilation/source-compiler/Cargo.toml#L16
[source-file]: https://github.com/CathedralOS/Omega/blob/3f9ce43f0c9d179048911d18faa91ba93f60176d/omega-rust/psi/foundation/source/src/lib.rs#L144
[source-job]: https://github.com/CathedralOS/Omega/blob/3f9ce43f0c9d179048911d18faa91ba93f60176d/omega-rust/psi/compilation/build-time-evaluation/src/source_jobs.rs#L616
[source-loop]: https://github.com/CathedralOS/Omega/blob/3f9ce43f0c9d179048911d18faa91ba93f60176d/omega-rust/psi/compilation/source-compiler/src/source_compilation.rs#L1091
[stack]: https://github.com/CathedralOS/Omega/blob/3f9ce43f0c9d179048911d18faa91ba93f60176d/omega-rust/omega/compilation/backend/images/image-emission/src/object_artifact.rs#L1499
