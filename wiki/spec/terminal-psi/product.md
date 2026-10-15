# Terminal Psi product

This is the current portable-product contract, independent of the implementation
language. The [documentation index](../../README.md) lists its vocabulary subjects.
[Boundary calls](boundary_calls.md), [byte views](byte_views.md),
[structural access and owned byte values](structural_access.md), and
[observations](observations.md), [calls and outcomes](calls_and_outcomes.md), and
[fact validity](fact_validity.md) have their
own references. [Mathematical proof values](mathematical_values.md) and
[integer certificates](integer_certificates.md) define their proof vocabulary.
[Structural predicates](structural_predicates.md) defines path and aggregate terms.
Implementation coverage is not implied by a specified
contract; unfinished integration belongs on the [execution board](../../../TASKS.md).

## Semantic boundary

Psi owns source parsing, resolution, typing, checking, proof construction,
expression lowering, selected target-neutral optimization, and publication of
Terminal Psi. Omega consumes that product and owns provider selection and
installation, target realization, its own optimization phases, ABI lowering,
native emission, and execution machinery. Parsing is not an Omega backend step.

Terminal Psi is immutable, self-contained, concrete, and post-instantiation.
Its meaning requires no source syntax, typed-tree arena, producer process,
instruction selector, or generic execution machinery. Execution, proof
questions, evidence, diagnostics, fuel, and lowering refer to the same stable
values, places, operations, and edges. Each executable choice affecting behavior
or obligations has a closed static identity. Author-declared hardware geometry
remains semantic; target-selected layout, ABI classes, registers, storage regions,
and instructions are later realization facts.

The portable root names either one concrete machine or an independent
target-family contract. A family contract has no executable body: the receiving
installation resolves its exact tagged member from the retained catalog before
creating execution state or a native plan. The root, every member, and the
canonical semantic identity remain unchanged across installations. Selection
establishes catalog membership only; entry backing, provider authority and
checked source-entry correspondence remain separate admission obligations.

Selected boundary-operator applications belong to these portable semantics.
Each demand names its Terminal operation, exact-owner requirement identity,
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
in Terminal Psi. A compilation-local occurrence number joins target placement
to this row and survives pruning; it is not independently authoritative. Several
callback binders may share one registrar. Native settlement consumes these
rows, not a parallel source-call catalog. A callback artifact's entry declaration
must match the selected body in the caller's demand; no source-side lowering
receipt is required. Callback body correspondence and
target ABI/installation evidence remain separate obligations. Native callback
function identities are scoped by the caller's canonical Terminal identity and
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

## Publication and separate consumption

Compilation may stop at Terminal Psi. A later interpreter or native lowerer
may consume it in a different process, on another machine, under separately
supplied authority. The first-party source-to-native path crosses this same
boundary; checked frontend state is not a privileged alternative input.

The portable envelope contains canonical semantics and pre-Terminal selection
provenance, plus optional canonical debug information. It carries no proof
evidence. Program meaning, installation decisions, and debug information have
distinct identities. The manifest is reconstructed from the sections, not
accepted as redundant authoritative input.

Nominal declarations retain their original owner and owner-local identity,
including [content-identified anonymous roots](../build/declarations.md#default-builds-and-anonymous-owner-identity).
Dependency references preserve those identities across separate consumption;
importing a product does not re-own its declarations. This provenance is distinct
from the exact product's semantic digest and its later native realization.
Reloading or moving a product, or clearing a compiler cache, cannot rename its
declarations. No global catalog of previous compilations establishes identity
or acceptance. Products missing required provenance must reject the affected
nominal import rather than guess it from a path or same-spelled declaration.

Optimization validation retains the exact Psi-local selection and input/output
semantic identities. Decoding rejoins retained output identity to the
semantic module. Producers and
consumers use the current pre-release vocabulary;
stale encodings reject rather than being guessed compatible.

A consumer decodes and performs ordinary structural, contract and authority
validation before creating execution state or native realization requirements.
A public in-memory module cannot bypass those checks. Consumption relies on
Psi checking for the logical obligations the producer discharged; it does not
repeat them. Failure to find a proof does not refute its conclusion.

An authoritative vocabulary extension must define encoding, execution,
reconstructed obligations and authorized admissions, proof rules and their
soundness argument, interpretation, lowering requirements, and fuel identity.
An incomplete consumer rejects unsupported forms; a metadata-only producer
does not establish complete executable support.
Obligation-affecting policies are closed operation variants, not ordinary values
that must be constant-folded to discover the question. Additional sound proof
lemmas do not change operation or program identity.

## Build-owned companion

The target-neutral module carries every target-scoped machine body under its
family path with its target tag; selecting a body is a realization fact, not a
Terminal branch. The realization target set travels beside the module in an
exact owned build companion, and per target: profile, ProgramEntry,
target-constrained provider plans, external-binding requirements,
compiler-builtin proposals and pending Omega optimization selections, as rows
keyed by target. Rows are not executable semantics inserted into Terminal,
and possession does not grant realization authority.

The companion binds each target's pending selection to the complete
build-selection identity. Terminal independently retains the Psi selection already executed.
A receiving consumer must admit the exact proposed pending selection using its
own target catalog and local authority, or reject realization. Rejection does
not invalidate the portable semantics. With no target companion, subsequent
physical selection belongs to the receiving authority.

Source-declaration receipts may separately establish author intent, entry
identity, provider selection, and target closure. They must rejoin the canonical
artifact; they cannot supply executable choices absent from Terminal.

## Optimization ordering

The stage order is lowering, selected Psi optimization, then Terminal
publication. An empty selection is the identity transformation of the same
phase: validate input and output without constructing rule candidates,
transformations, or pass manifests. Selection does not choose an alternate
pipeline.

Later Omega phases optimize their own representations while retaining the
canonical Terminal identity. They cannot mutate the published module or
reselect a Psi/checked-tree pass. Rollback may subtract a selection only at a
phase the invocation actually executes; the report distinguishes effective
Psi selection from pending native selection.

A schema-derived semantic plan belongs in Terminal only when its carrier and
validator are representation-level or lower, contain no backend-owned types,
and are independently reconstructible before assignment or emission.
Assigned homes, final call placement, relocations, and emitted bytes remain
later facts.

## Accounting and subsequent products

Semantic/program identity and fuel-schedule identity are independent.
Changing a fuel schedule changes accounting, not program meaning. Semantic
caches bind semantics and program identity; cost evidence additionally binds
the schedule. [Logical work](../resources/logical_work.md) and
[spatial resources](../resources/storage.md) own those separate contracts.

Executable publication stages and replays retained image bytes. Installation
is a further authority-bearing operation. Neither native lowering nor the
portable companion silently performs deployment admission.
See [native products and component publication](../build/component_publication.md)
for installed provider/progress closure, runtime custody, and transactional
publication. [Content custody](../resources/content_custody.md) defines installed
root accounting and conservation; [placed access](../resources/placed_access.md)
defines resident occurrences.
