# Psi product

Psi is the public name of this portable product, independent of the implementation
language. The compiler's Terminal Psi stage produces it; that internal stage
name is not the artifact, interpreter or CLI name. The
[documentation index](../../README.md) lists its vocabulary subjects.
[Boundary calls](boundary_calls.md), [byte views](byte_views.md),
[structural access and owned byte values](structural_access.md), and
[observations](observations.md), [calls and outcomes](calls_and_outcomes.md), and
[fact validity](fact_validity.md) have their
own references. [Mathematical proof values](mathematical_values.md) and
[integer certificates](integer_certificates.md) define their proof vocabulary.
[Structural predicates](structural_predicates.md) defines path and aggregate terms.

## Semantic boundary

Psi owns source parsing, resolution, typing, proof construction, expression
lowering, lowered-form checking, selected target-neutral optimization, and
publication of Psi. [Optimization ordering](#optimization-ordering) is the
stage order. Omega consumes that product and owns provider selection and
installation, target realization, its own optimization phases, ABI lowering,
native emission, and execution machinery. Parsing is not an Omega backend step.

Psi is immutable, self-contained, concrete, and post-instantiation.
Its meaning requires no source syntax, typed-tree arena, producer process,
instruction selector, or generic execution machinery. Execution, proof
questions, evidence, diagnostics, work, and lowering refer to the same stable
values, places, operations, and edges. Each executable choice affecting behavior
or obligations has a closed static identity. Author-declared hardware geometry
remains semantic; target-selected layout, ABI classes, registers, storage regions,
and instructions are later realization facts.

The optional portable default root names either one concrete machine or an independent
target-family contract. A family contract has no executable body: the receiving
installation resolves its exact tagged member from the retained catalog before
creating execution state or a native plan. The root, every member, and the
canonical semantic identity remain unchanged across installations. Selection
establishes catalog membership only; entry backing, provider authority and
checked source-entry correspondence remain separate admission obligations.

A declaration-only checking product has no default root or root-service reach.
It can retain real scalar-evaluation machines and supplied callable declarations;
it does not invent an executable entry. Execution requires an explicitly admitted
callable or an existing default root. Native emission likewise requires its
selected executable application.

Selected-composition [behavior exclusions](../build/behavior_exclusions.md)
must not depend on optional optimization. Psi therefore retains the
pre-optimization semantic operations for each produced body: direct and boundary
invocations, instruction service contributions, crash causes, and nominal
cleanup calls. These are typed operation relationships, not a producer's
policy verdict. The consumer reconstructs the selected closure against the
provider catalog and conservative opaque contracts. Historical sites may no
longer be executable after optimization, including when a whole machine is
deleted; its historical graph node remains. Missing history supplies no absence
evidence. Canonical ingress checks referenced declarations and ensures retained
history covers the executable body's contributions. This structural check does
not independently establish the producer's historical lowering correctness.

Selected boundary-operator applications belong to these portable semantics.
Each demand names its Psi operation, exact-owner requirement identity,
overload coordinate and declaration-ordered closed type/constant arguments.
Psi produces the rows before optimization; their canonical encoding participates
in the program identity. Optimizers preserve the selected operation and its
demand unless they can transport the complete relationship. Provider choice,
selected-plan commitments and executable realizations remain Omega inputs;
they do not determine or reconstruct the program's demand roster.

Psi also retains the checked body candidates for each distinct closed application:
the selecting callable and package identities, any target tag, exact body and
entry identities, checked contract commitment and optional specialization identity.
This is a catalog of eligible bodies, not a selected provider. Omega joins the
target's Build row to that catalog; it does not reopen checked source or ask the
publication stage for a source-index companion. Requirement package identity is
explicit, so same-spelling requirements from different packages cannot collide.

The provider catalog retains the exact nominal selecting declaration separately
from the realizing callable: two named maps may share code without becoming one
provider. Boundary declarations retain their exact schema owner so whole-interface
demand filtering also uses canonical identities. Receiving checked selections
name that declaration and the artifact-local boundary/body IDs; display labels
alone do not establish owner or package identity.
Each candidate also retains the original requirement and realization callable
coordinates selected by a provider-plan row, beside their closed executable
coordinates. Unspecialized coordinates are identical; a specialization names
its immediate checked template, including any existing outer application.
That template's package coordinate remains separate from callable text and
must agree exactly with the selected row, including a toolchain's absent package.
Receiving policy enumerates the complete canonical roster for that exact
original pair and choosing declaration. A proposal cannot redefine the expansion
by omitting an uncalled tuple, and equal runtime signatures establish no join.
Admitted selections retain the canonical choosing declaration for policy
agreement; projecting a selection to its execution body does not erase that
declaration from the sealed installation.
The source-free Build proposal retains that exact selecting declaration with
each complete provider plan; a plan's display label and optional managed package
are not sufficient to recover a toolchain source owner. Proof-only foreign loan
rows carry ContractEvidence rather than Callable role: their schema owner creates
no provider demand, and receiving execution cannot install or invoke those rows.
The in-process native proposal retains complete declaration-qualified checked
choices, their full originating plan digest and original row, and closed
specialization/family coordinates. Lowering consumes that projection once;
scope and installation match its choosing declaration against canonical candidates.
Shared physical code does not merge choices: native call custody rejoins the
exact installed conformance rather than inferring a provider from the callee.
Its physical settlement retains and commits the exact choosing declaration.

Source-produced machine bodies retain a portable checked declaration identity:
exact-owner callable and initial state, contract commitment, and optional
specialization template/commitment. Optimization preserves this identity on the
surviving machine. It identifies the checked input; it is not a substitute for
the body's semantic proof or native ABI/resource admission.

Checked callback applications retain their registrar operation, static-machine
binder ordinal, requirement identity, published contract and selected body identity
in Psi. A compilation-local occurrence number joins target placement
to this row and survives pruning; it is not independently authoritative. Several
callback binders may share one registrar. Native settlement consumes these
rows, not a parallel source-call catalog. A callback artifact's entry declaration
must match the selected body in the caller's demand; no source-side lowering
receipt is required. Callback body correspondence and
target ABI/installation evidence remain separate obligations. Native callback
function identities are scoped by the caller's canonical Psi identity and
its retained placement ordinal. Source-arena coordinates and separately supplied
private symbol strings are not native identity; linker names derive from that
portable identity.

The reference interpreter executes the portable semantics directly. Native
code realizes the same semantics. Differential agreement is evidence about
implementations, not a definition of the language.

[Checked assembly](../language/assembly.md#interpreted-execution) crosses this
same boundary as typed operations with exact target semantics, operand/place
identities, authority, persistent-state effects and control exits. Reuse ordinary
operations where they completely express the instruction; otherwise retain the
catalog identity and its executable semantics, not raw text, backend register
allocation or a clobber-only summary. Scratch state cannot become an implicit
cross-block input. Consumer admission checks the selected instruction/environment
coverage; a native encoder alone does not establish interpreted support.

Concrete service closure retains fixed declaration contributions, including
services contributed by declared synchronous invocations, separately from
installation-selected requirement bounds. Checking unions those contributions
and retained calls from the selected entry. An inert checked callback does not
erase its declared Console promise. A Console declaration remains concrete even
when an installation requirement also has Console in its upper bound; subtracting
the bound from a flattened published ceiling cannot recover that distinction.
These declarations are semantic contract inputs, not proof certificates or a
generic-substitution receipt.

Callable declaration headers retain source supply and remaining type-parameter
arity independently of emitted bodies. A concrete emitted body alone does not
establish a nongeneric source declaration. Headers also retain checker-normalized
realized synchronous invocations. Each invocation target names its boundary
declaration and package, with an exact application when concrete and explicit
absence when abstract. An unavailable summary retains its refusal reason;
it is not an empty call set. Selected-provider cycle policy consumes these
summaries, while receiving executable admission checks installed call closure
independently.

Under [selected-provider cycle policy](../language/effects.md#direct-synchronous-invocation),
retained requirement dependencies are not edges to every eligible supplier.
Composition resolves each provider-controlled call for its exact application
and target while preserving concrete direct calls and conservative callback or
native alternatives. Unselected suppliers add no hypothetical wiring. Preserve
these distinctions through source checking, lowering, publication and replacement;
an abstract or unavailable target cannot become a fabricated empty call set.

## Publication and separate consumption

Named decision: `psi-execution-admission`, ratified 2026-11-07, superseding
`psi-claim-admission`'s requirement to rediscover published proofs.

Compilation may stop at Psi. A later interpreter or native lowerer may consume
the exact artifact under host-supplied authority. Trust attaches to that artifact
and its producing process, including an accepted separately built library. It
is supplied by the receiving host; an artifact's digest, producer name or
assertion that it was checked does not authorize execution by itself.

| Input and requested use | Admission |
| --- | --- |
| Trusted producer's exact artifact, interpreted | Reuse checked judgments; prepare execution and enforce concrete operation checks. |
| Trusted producer's exact artifact, native | Reuse checked judgments for optimization and lowering under the selected installation contracts. |
| Untrusted artifact, interpreted | Available only through a containment-audited interpreter and host bindings. Until that route is established, execution requires host authorization of producer trust. |
| Untrusted artifact, native | Refuse. This contract provides no guarded native lowering route. |
| Untrusted bytes, inspection | Bounded canonical decoding and structural validation; no execution or installation authority. |

Every public execution entry, including embedding APIs accepting bytes or
caller-supplied modules, enforces this distinction. An execution trust witness
records the host's authorization for the exact artifact digest and producer;
copying those identifiers from the artifact is insufficient. Internal stages may
reuse their trusted immutable outputs without repeating their checking. Changed
bytes or a different producing process require a new admission decision.

The portable envelope retains canonical semantics, contracts, mathematical
claims, exact subjects, selected operations, assumptions and dependencies. It
also retains pre-Terminal selection provenance and optional debug information.
The existing mathematical-certificate forms remain carried and canonically
decoded as specified by [encoding](encoding.md#mathematical-certificate).
Their presence grants no truth, execution authority or producer trust, and
receipt does not require kernel replay. Local source proving keeps its kernel.

A trusted producer's established judgments may cross the artifact boundary with
their exact subjects, substitutions and assumption dependencies. The consumer
does not rediscover source theorems or require transported derivations. Trust in
the producer does not waive the receiver's assumption, installation or authority
policy. Source-to-artifact correspondence and faithful native realization remain
trusted compiler responsibilities; see the
[verification table](../../../omega-rust/pipeline.md#verification-placement).

The interpreter enforces actual bounds, storage liveness, access permissions,
operand safety and admitted resource limits independently of asserted facts.
An invalid operation produces a defined guest fault even when the artifact
declares no crash route. Published crash guards describe permitted program
failure; they do not replace these enforcement checks. Budget exhaustion retains
its defined pause or refusal outcome. Neither faults nor refusal imply rollback.

Host grants and installed bindings bound actual reach. Untrusted guest metadata
cannot grant capabilities or establish a qualification needed by a host adapter.
A crossing offered to untrusted execution must enforce its required permissions,
limits, custody and supported value conditions dynamically, or be unavailable.
Containment establishes safe execution within those limits, not the truth of
every source theorem or a crash-free guest. Only trusted inputs may supply facts
to optimization or native lowering.

[Loading and preparation](../build/embedding.md#loading-and-preparation) separate
inspection from execution admission. Both have shared input and aggregate work
limits. Preparation computes declaration-dependent check plans once; dispatch
checks actual values without proof search or repeated declaration-table scans.
The [implementation plan](../../drafts/designs/psi_claim_admission.md) owns the
API changes, codec split, performance validation and containment acceptance.

Program meaning, installation decisions, and debug information have distinct
identities. The manifest is reconstructed from the sections, not accepted as
redundant authoritative input.

Nominal declarations retain their original owner and owner-local identity,
including [content-identified anonymous roots](../build/declarations.md#default-builds-and-anonymous-owner-identity).
Dependency references preserve those identities across separate consumption;
importing a product does not re-own its declarations. This provenance is distinct
from the exact product's semantic digest and its later native realization.
Reloading or moving a product, or clearing a compiler cache, cannot rename its
declarations. No global catalog of previous compilations establishes identity
or acceptance. Products missing required provenance must reject the affected
nominal import rather than guess it from a path or same-spelled declaration.

Optimization validation retains exact Psi-local selections and input/output
semantic identities. Decoding rejoins the retained output identity to the
semantic module. Consumers use the current pre-release vocabulary; unsupported
or stale forms reject.

An authoritative vocabulary extension must define encoding, execution,
reconstructed obligations and authorized admissions, proof rules and their
soundness argument, interpretation, lowering requirements, and work identity.
An incomplete consumer rejects unsupported forms; a metadata-only producer
does not establish complete executable support.
Obligation-affecting policies are closed operation variants, not ordinary values
that must be constant-folded to discover the question. Additional sound proof
lemmas do not change operation or program identity.

## Build-owned companion

The target-neutral module carries every supplied named implementation under its
ordinary declaration identity and explicit requirement-satisfaction edges. It
does not group source bodies into target-tagged same-path families or synthesize
a portable family head. The realization target set travels beside the module in an
exact owned build companion, and per target: profile, ProgramEntry,
target-constrained provider plans, external-binding requirements,
compiler-builtin proposals and pending Omega optimization selections, as rows
keyed by target. Rows are not executable semantics inserted into Psi,
and possession does not grant realization authority.

Omega selects the exact implementation through those Build provider rows, not
through a target modifier on a source declaration. Every implementation remains
checked independently of selection; selected ABI, import, layout, and ISA
applicability is a realization obligation. Retained target-family rows are
not accepted Psi product semantics.

An unused representation application is not a validated concrete target plan.
[Demand-driven layout formation](../layouts/plans.md#layout-demand-and-validation)
retains ordinary source checking while deferring target geometry until a
consumer, entry or selected Build-row closure demands it. Another declared
target row remains a consumer even if it is outside the emitted target set.
No later consumer may treat absence of an earlier demand as layout acceptance.

The companion binds each target's pending selection to the complete
build-selection identity. The artifact independently retains the Psi selection already executed.
A receiving consumer must admit the exact proposed pending selection using its
own target catalog and local authority, or reject realization. Rejection does
not invalidate the portable semantics. With no target companion, subsequent
physical selection belongs to the receiving authority.

Source-declaration receipts may separately establish author intent, entry
identity, provider selection, and target closure. They must rejoin the canonical
artifact; they cannot supply executable choices absent from Psi.

## Optimization ordering

The stage order is structural elaboration, the lowered-form check of the
unoptimized program, selected Psi passes in one canonical order, the same
lowered-form check of the optimized program, then Terminal publication. The
unoptimized check decides source validity. An optimization can never make an
invalid program valid. The published Terminal Psi carries the verdict and
evidence of its own check, never evidence borrowed from its input. There is
no evidence transport through passes. Those passes are the Psi phase, and
they run before publication, in the one canonical order of
[phase and product boundaries](../build/optimizations.md#phase-and-product-boundaries).
A later consumer reuses the published verdict under
[publication and separate consumption](#publication-and-separate-consumption).

An empty selection is the literal identity. It runs no passes and no second
check. Output equals input, and the default path is unchanged. It constructs
no rule candidates, transformations, or pass manifests. Selection does not
choose an alternate pipeline.

Every Psi pass consumes and produces well-formed lowered Psi in the same
representation, the unsealed Terminal module structural elaboration built.
The checker has no knowledge of which passes ran and has no pass-specific or
combination-specific branches or adapters. A pass whose output the second
check rejects is a compiler defect, reported as such and not as a source
diagnostic.

The lowered-form check does not establish behavior preservation. Per-pass
input and output validators, and Psi-versus-native agreement, do that work.
Those validators run in the corpus runner and the tests, not on the default
compile path ([verification placement](../../../omega-rust/pipeline.md#verification-placement), owner commit `b1423f5385d`).

A nonempty Psi selection has no product route yet.
`ProductTerminalPublication::accepted` refuses it with
`no product route yet: selected Psi optimizations have no second lowered-form check`.

Later Omega phases optimize their own representations while retaining the
canonical Psi identity. They cannot mutate the published module or
reselect a Psi pass. Rollback may subtract a selection only at a
phase the invocation actually executes; the report distinguishes effective
Psi selection from pending native selection.

A schema-derived semantic plan belongs in Psi only when its carrier and
validator are representation-level or lower, contain no backend-owned types,
and are independently reconstructible before assignment or emission.
Assigned homes, final call placement, relocations, and emitted bytes remain
later facts.

## Accounting and subsequent products

Semantic/program identity and work-schedule identity are independent.
Changing a work schedule changes accounting, not program meaning. Semantic
caches bind semantics and program identity; cost evidence additionally binds
the schedule. [Logical work](../resources/logical_work.md) and
[spatial resources](../resources/storage.md) own those separate contracts.

Executable publication stages and replays retained image bytes. Installation
is a further authority-bearing operation. Neither native lowering nor the
portable companion silently performs deployment admission.
See [native products and component publication](../build/component_publication.md)
for installed provider/progress closure, runtime custody, and transactional
publication. [Content custody](../resources/content_custody.md) defines installed
root accounting and conservation; [access policies](../resources/access.md)
defines resident occurrences.
