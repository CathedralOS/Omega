# Compiler pipeline and ownership

Psi reads source and makes every target-neutral judgment, ending in immutable
[Terminal Psi](../wiki/spec/terminal-psi/product.md). Omega turns that product
into native code, under realization authority supplied separately. A construct
a stage does not support is refused at that stage; no backend falls back to the
source shape or to a hand-assigned program. The old checker, interpreter and
lowering chain are removed. Source preparation and Build work that no producer
serves yet stop with `no product route yet`.

Cathedral (the downstream OS) owns OS data structures, policies, protocols and
lifecycle. Do not model page tables, schedulers or drivers as compiler-owned
Rust types; if Cathedral cannot express something, name the missing general
Omega primitive or mark the slice blocked.

## Compositional lowering

Never add code that compiles a program only when its source has one particular
shape. When you touch such a mechanism, generalize its operation or make its
data relationship explicit, and delete the old path in the same change. A task
promising later consolidation is no exception. If the general repair is
blocked, name that dependency instead of landing another special case. This
does not call for an unrelated whole-pipeline rewrite.

Supported assignment, call, branch and transfer operations must combine under
their real type, effect, ownership and control-flow rules. Do not add another
producer for an incidental arrangement such as "locals before calls", "call immediately followed
by case dispatch", or "the same body with an extra statement".
Replace the narrow recognizer or duplicated planner with ordinary operation
sequencing and explicit value, storage and control relationships. Clean X-to-Y
crate names do not make a pattern-specific implementation general.

Semantic distinctions, ABI constraints, missing proof obligations and explicit
implementation limits are real. Never remove their rejection checks just to
accept more shapes. Pattern matching suits an optional optimization with a
correct general path, not the only way to compile an ordinary combination.

When a program someone needs hits this, find the missing general operation or
join and delete the special path as that repair lands. When the missing
relationship is a requirement an earlier stage dropped, have that stage record
its still-open requirements on the value it emits and have the next stage read
that list. Saved scalars, write sets, crash routes, domain facts and the
selected body are entries on that list; a recognizer that rebuilds one of them
for one production is the path to delete. Before spreading a new
representation or lowering mechanism across stages, pick a small valid variant
that tests its assumptions and a related invalid program, and check
independently that the design accepts the first and rejects the second. Build
and run both early, not only after the original fixture passes. For a borrowed
call, surrounding computations and refusal of a conflicting access can expose a
bad design without testing every source permutation.

Build more variants in parallel only after one route through the new mechanism
works and is independently checked; do not copy an unproven representation
into several implementations first. Review, tests and work on established
contracts can still run in parallel.

The user's real command stays the final acceptance check; a passing isolated
helper does not fix an application that still fails. Do not add one task-board
item per permutation, or read this rule as permission for an unrelated
whole-IR rewrite.

## Verification placement

By default a compile trusts each stage to keep its input's meaning and meet its
output contract. It does not revalidate, replay or rebuild output the same
compile just produced. That trades safety for compile time and maintenance: an
independent validator can catch producer bugs in the same process, but each
recheck costs time and needs a second implementation of the checked rules.

Source checking, decoding and installation checks still run where they belong.
A compile checks its
[validation subjects](../wiki/spec/build/validation_subjects.md) in full and
their dependencies as far as the semantic closure requires. Review verifies a
new or changed foreign package, and an untrusted build script runs confined and
checked.
[Psi consumption](../wiki/spec/terminal-psi/product.md#publication-and-separate-consumption)
reuses the judgments in a trusted producer's exact artifact when the host
authorizes it, including artifacts accepted in a separate build. The receiver
neither rediscovers source theorems nor replays carried proof bytes. Canonical
decoding grants no execution or provider authority.

Native optimization and lowering need trusted input. The interpreter enforces
the safety of each actual operation even on trusted input. Running untrusted
artifacts in general waits on a containment audit of the interpreter, host
bindings and teardown; bounded structural decoding can still inspect them.

Machine-effect and register-constraint preparation keep the ISA producers'
catalogs and identities in shared immutable storage; ordinary consumers borrow
those tables instead of building and checking another copy. Physical-model
roots and platform reservation joins are still checked. Validators of a
supplied catalog still check its structure and compare it with the canonical
ISA catalog. Preparation is not an independent validation.

Focused tests run the independent validators that exist. Explicit [validator selection](../wiki/spec/build/command_line.md#independent-stage-validators)
runs the requested validators that apply and reports their real coverage
through the ordinary compiler driver. A stage that emits an invalid plan is a
compiler defect; passing source checking does not make later lowering or
optimization infallible. A default compile leaves the Psi pass selection
empty, so it runs no second lowered-form check. Once a nonempty selection is
served, the same checker must accept the optimized module before publication,
and a rejection there is reported as a compiler defect, not a source
diagnostic. No nonempty selection is served yet, so the default path is
unchanged
([optimization ordering](../wiki/spec/terminal-psi/product.md#optimization-ordering)).

| Boundary | Checks and evidence | What remains trusted |
| --- | --- | --- |
| Source to typed structure | Parsing, resolution and typing in Psi 00–03 retain source identities and declarations. Psi 04 runs its pass list over every program and judges nothing. | Frontend preservation of source meaning and exact source custody; typed structure is no acceptance certificate. |
| Unsealed Terminal Psi to accepted publication | Structural elaboration builds the module; the lowered-form checker reconstructs supported obligations on it and keeps their evidence, which publication carries. A nonempty selection must be checked again after its passes, which do not carry the first verdict; it refuses for now. Unsupported, incomplete or missing verdicts refuse. | Correct obligation reconstruction, the independent proof-admission kernel and explicitly admitted premises. Publication must preserve the exact module and evidence of the check that admitted the published program. The checker does not establish behavior preservation. |
| Terminal Psi to native execution | Target realization and publication checks; `omega run --both` compares native execution with the Terminal interpreter. | Backend correctness beyond the executions and observations compared, plus the interpreter and shared semantics. |
| Received artifacts and installed providers | Bounded canonical/structural decoding; explicit host authorization of the exact artifact and producer; preparation and installation checks; concrete interpreter operation and host-access checks. | Authorized producer judgments, admitted assumptions, faithful lowering, the interpreter and installed adapters. Future untrusted interpretation requires audited containment and does not certify source claims. |
| Foreign packages and build execution | Package review, exact acceptance and confined build services. | Accepted external claims and the enforcing review/execution machinery. |

Three kinds of information do different jobs:

- Local proof objects let the kernel check source proofs and supported local
  evaluation. Carried certificate encodings are decoded canonically; their
  presence grants no admission authority and requires no replay.
- Retained contracts, definitions, subject identities and custody relationships
  describe the program and tie each reusable judgment to its exact question.
- Host authorization lets a trusted producer's judgments be reused. An artifact
  cannot authorize itself with a flag, digest or producer name. The
  interpreter's checks of actual operations and host permissions do not depend
  on those claims.

The [admission and preparation plan](../wiki/drafts/designs/psi_claim_admission.md)
records the execution-API gate, codec split, shared limits, static operation plans,
performance measurements and containment work. Optional stage validators check
compiler output independently; they neither replace required external admission
nor certify that the executable matches the source.

`--both` runs the same Terminal artifact through the interpreter and natively.
Agreement is evidence for the inputs, paths and observations compared, not a
proof for every run. An error in source checking or in lowering before Terminal
Psi affects both routes the same way, so expected-result tests, rejection tests
and focused frontend tests must check that part of the compiler separately.

## Connected program route

Each row names a stage owner and what it hands to the next. Private analyses
and target setup are not extra public stages. The map does not mean the whole
route works: typing stops that are not rebuilt, unserved source evaluation and
product-authority requests currently refuse with `no product route yet`. Default
Build and native completion remain work on [TASKS.md](../TASKS.md).

| Input → output | Owner |
| --- | --- |
| Source files → tokens | [source-files-to-tokens](psi/compilation/pipeline/00_source-files-to-tokens/lexer.rs) |
| Tokens → syntax trees | [tokens-to-syntax-trees](psi/compilation/pipeline/01_tokens-to-syntax-trees/src/lib.rs) |
| Syntax → symbol-resolved trees | [syntax-trees-to-symbol-resolved-trees](psi/compilation/pipeline/02_syntax-trees-to-symbol-resolved-trees/src/lib.rs) |
| Resolved → typed trees | [symbol-resolved-trees-to-typed-trees](psi/compilation/pipeline/03_symbol-resolved-trees-to-typed-trees/src/lib.rs) |
| Typed trees → prepared typed trees (one ordered pass list) | [typed preparation](psi/compilation/pipeline/04_typed-trees-to-typed-trees/README.md) |
| Typed structure → unsealed Terminal Psi | [typed-trees-to-terminal-psi](psi/compilation/pipeline/05_typed-trees-to-terminal-psi/src/lib.rs) |
| Unsealed Terminal Psi → accepted closure | [terminal-psi-to-terminal-psi](psi/compilation/pipeline/06_terminal-psi-to-terminal-psi/src/lib.rs) |
| Accepted closure → sealed Terminal artifact | [accepted source factory](psi/compilation/terminal-production/src/accepted.rs) and [codec](psi/compilation/representations/terminal-psi/codec/src/lib.rs) |
| Terminal Psi → abstract operations | [terminal-psi-to-abstract-operations](omega/compilation/pipeline/00_terminal-psi-to-abstract-operations/README.md) |
| Abstract → abstract operations | [abstract-operations-to-abstract-operations](omega/compilation/pipeline/01_abstract-operations-to-abstract-operations/src/lib.rs) |
| Abstract → target operations → prepared execution plan | [abstract-operations-to-target-operations](omega/compilation/pipeline/02_abstract-operations-to-target-operations/README.md) |
| Prepared execution plan → selected instructions | [target-operations-to-selected-instructions](omega/compilation/pipeline/03_target-operations-to-selected-instructions/README.md) |
| Selected → selected instructions | [selected-instructions-to-selected-instructions](omega/compilation/pipeline/04_selected-instructions-to-selected-instructions/src/lib.rs) |
| Selected instructions → register homes | [selected-instructions-to-register-homes](omega/compilation/pipeline/05_selected-instructions-to-register-homes/README.md) |
| Register homes → post-allocation machine | [register-homes-to-post-allocation-machine](omega/compilation/pipeline/06_register-homes-to-post-allocation-machine/src/lib.rs) |
| Post-allocation machine → selected-form encoding | [post-allocation-machine-to-selected-form-encoding](omega/compilation/pipeline/07_post-allocation-machine-to-selected-form-encoding/src/lib.rs) |
| Selected-form encoding → resolved layout | [selected-form-encoding-to-resolved-layout](omega/compilation/pipeline/08_selected-form-encoding-to-resolved-layout/src/lib.rs) |
| Resolved → resolved layout | [resolved-layout-to-resolved-layout](omega/compilation/pipeline/09_resolved-layout-to-resolved-layout/src/lib.rs) |
| Resolved program → machine bytes | [machine-emission](omega/compilation/backend/machine-emission/README.md) |
| Machine bytes → object/image evidence | [image-emission](omega/compilation/backend/images/image-emission/src/lib.rs) |

The Psi rows show the empty pass selection: elaboration, one lowered-form
check, then the factory. [Optimization ordering](../wiki/spec/terminal-psi/product.md#optimization-ordering)
places selected passes after the accepted closure, checked again before
sealing; a nonempty selection refuses, so the table has no pass row.

<a id="one-driver-one-pass-per-target-realization"></a>

## Two drivers, one pass, per-target realization

Tests can call each stage crate directly; production code goes through the
compiler facades. Psi 00–03 load, parse, resolve and type source. Every program
then passes through Psi 04, which runs one ordered list of passes over the typed
trees and freezes the result without judging it. Each pass either does its work
or refuses its own request by name; requested deferred layout, opaque-selection
and wire work has no pass yet and refuses.

The served checking route is Psi 05 (`typed-trees-to-terminal-psi`)
→ Psi 06 (`terminal-psi-to-terminal-psi`) → publication. Stage 05 prepares
every needed generic form and conformance witness in a shared Terminal packet;
stage 06 depends on neither stage 05 nor typed trees. The accepted-source
factory runs them in order, keeps the frozen typed owner and source-only
publication correspondence outside the checker's input, then publishes the
same module and evidence. That is the default, empty Psi selection: no passes,
no second check. A nonempty selection adds the passes and a second check
([optimization ordering](../wiki/spec/terminal-psi/product.md#optimization-ordering));
`ProductTerminalPublication::accepted` refuses it for now with
`no product route yet: selected Psi optimizations have no second lowered-form check`.
There is no legacy optimization or publication fallback. Psi's published
product is target-neutral; Build, keyed by target, separately supplies the
realization inputs ([multi-target compilation](../wiki/spec/build/configuration.md#multi-target-compilation)).
Given those inputs and a published product, Omega realizes each target through
provider selection and installation, Omega 00–09 and image emission. An
unserved Build, provider or entry producer stops that sequence; default Build
completion requires serving them.

Package-local Build rows declare native bindings:
`Build::bind<Requirement>(builder, target, locator)` names one designated
product requirement and plain typed locator data with inferred byte lengths. It
runs no product code and installs no Build-host supply. The compiler derives
native candidates and plans, keeping requirement, target, supplier, calling
policy and authored provenance through review and realization. Source `via` clauses and per-symbol forwarding realizers are
retired; they are not a second supply route.

The two drivers are the only production entry points to the stages. Code
needing source semantics calls the Psi driver with explicit source, package and
Build custody; code needing machine output calls the Omega driver with Terminal
Psi plus target and realization inputs. Package management, the
application, interpreters, inspection and test runners do not depend on stage
or intermediate-representation crates, so they cannot invent a stage order,
skip checks or keep sidecars. Backend libraries such as machine and image
emission do not call stages either; they read the representation the driver
hands them. `mbx xtask check-boundary` refuses any production dependency on an
Omega stage outside the native driver, except for a fixed list of remaining
exceptions that may only shrink. Focused tools stop the driver at an explicit
point or inspect what it returns; they are not other compile modes.

This limits orchestration, not use of shared libraries. Each transform consumes
its declared input and output representations and uses the semantic,
foundation and backend libraries that own reusable operations. Public Terminal
artifact and view types are shared contracts for producers, realization,
interpretation and presentation. A later stage that needs an established fact
gets it in the previous stage's output; hiding a call back into an earlier
stage behind a driver does not fix that dependency.

One Psi invocation owns its arenas and lends a lifetime-bound Terminal artifact
view to immediate consumers. Publication moves the same artifact into an owned
encoded form; an external reader decodes and admits it once. Omega realization
and interpretation read one common borrowed view of either the retained or the
decoded artifact, so same-invocation realization never serializes, decodes or
clones Terminal Psi just to cross an API.

The [public command router](../wiki/spec/build/command_line.md) calls portable
artifacts and their inspection subsystem Psi; Terminal Psi is an internal stage
name.

The shipped product exposes four facades: application coordination, package
management, the two compiler drivers, and Psi interpretation. Package
management owns acquisition, the resolved graph, review proposals/decisions
and `omega.lock`, then hands source custody to Psi. It does not return
checked trees, settle compiler products or call native production. The
interpreter owns Terminal loading, entry setup, installation of supplied host
bindings, execution resources and observations; it runs an artifact directly or
asks the Psi driver to compile source into one. The application depends on
these facades and shared request/artifact contracts, never their internal
stages, representations, backend or package-compilation helpers.

One `CompileRequest` assembles sources, runs Build, finishes authored/generated
typing and checks Product before its target loop. Targets borrow the same
checked arenas and sealed source custody and settle providers without
rewriting source contracts or rehashing the source closure. Targets selecting
the same Psi optimizations share Terminal production and storage, even with
different Build entry roots. Lowering keeps
every authored root and optimization preserves them. Each application picks its
retained entry through source-signature custody; native and interpreted runs do
not rewrite the artifact's default entry. Nominal callback bodies join the same
portable closure and keep their selected entries beside their native ABI
adapters. Main and callbacks borrow one provider context; realizing a callback
never starts another Psi production.

Product constant applications and their checked
conditions cross into Omega in canonical Terminal Psi. Selected realization
completes them without copying or rewriting checked source, and selected
providers cannot inject completion roles into checked source. Psi keeps each
const argument's complete expression, lexical owner and result carrier; type
and body copying carry that link through exact occurrence maps, and substituted
array fields refer to the same retained extent. Preparation turns closed
expression inputs, including those inside generic owners, into ordinary
evaluation entries without dropping the template's checking obligations.
Canonical publication reads those entries, never the first nested helper in
place of the complete expression. Selected execution still owes provider
admission, completion and every recorded bound.

Accepted package roles instead enter source preparation before checking. Psi admits their declarations and confines their
nomination before matching retained checks; realization only rejoins the
resulting source declarations to selected provider rows. Selected-provider
cycle policy reads Psi's checked invocation summaries from canonical declaration
headers, including dormant ones, instead of reopening source or using
permission ceilings in place of actual calls. Receiving provider admission
independently checks every installed canonical instance,
including uncalled closed tuples, with the same whole-plan graph rule. Opaque
rows contribute their declared ceilings; an uninstantiated source template
grants no executable instance. `terminal-installation` owns the retained
synchronous call closure (ordinary, no-normal, cleanup, invoked dynamic slots
and selected target members). Native authority review borrows that closure to
classify physical operations, with no second call traversal and no trust in an
earlier source check. Psi also owns the provider-selection
declarations and the shared decoder for static defaults and executed Build
choices. Checked Product selection returns all authored defaults before root
pruning; Omega only selects among the retained rows.

Resolver and typing continuation lives in the [Psi compiler](psi/compilation/source-compiler/src/source_compilation.rs):
one owner keeps the deferred resolver, typing arenas and evaluation obligations
together across Build and generated-source resumption. Product records cannot
reorder those stages or replace half of the continuation. The same facade owns
preparation, retained-judgment matching and checking for both Build and
Product; product pruning reads its checked result. Shared executable
projections and operator contract queries live in semantic checking, with no
stage dependency. The facade also owns source-frontier loading, lexing, parsing
and parser-page publication. Package assembly supplies admitted frontiers and
finds imports in the returned syntax; generated extension units keep separate
forests. Build role projection and dependency editing use the same Psi parsing
entry point, not their own lexer and parser.

The compiler invocation owns prepared sources,
completed Pure Builds and exact prior language judgments across package targets
and review rounds. Package callers supply source custody and fresh grants, not
frontend or proof cache slots. A reused language judgment lends its checked
arenas; equal Product root selections share the pruning output. Package native
requests complete as one roster through the same Terminal producer as
standalone requests. Package-manager batches supply dependency-ordered custody
and consent plans; the compiler owns generated-output joins, Build and Product
continuations and independent component publication. Target plans enter
together, and matching post-Build root continuations share source completion.
The captured Build prefix keeps its deferred Product frontend, keyed by exact
source and semantic inputs. Siblings validate live custody before borrowing it,
then rejoin their own Build selections and receipts; a reused frontend never
stands in for another occurrence's return or grants. Package operations supply
requested products and exact root permission consent and receive reports or
inspections, not checked activations to resume. Standalone and session products
share the same sequence: complete source-side selection, Terminal production,
then permission and trust admission and artifact completion. No phase that
reads source follows Terminal production. Review descriptions borrow the same
production facts without a second producer mode. Demanded components and lazy
review descriptions enter the same product sequence as root compilation.
Component completion owns its exported plans, description and permission
evidence; there is no second Terminal batch loop or public entry point for
publishing checked source. Terminal cohorts keep each admitted application
beside its exact entry, callback roots and exclusion policy. Source-sharing
comparisons use those requests directly; only the caller's order is restored
after cohort work, with no parallel entry tables or empty outcome slots.

The [multi-target observation fixture](omega/compilation/compiler/tests/README.md#shared-multi-target-pipeline-observations)
records each native coordinator call by exact target profile. For each
application it keeps entry counts for Omega 00–09 and for fragment and image
emission attempts, including the stages a failed call entered. Independently
admitted callable bodies stay separate observed units. Collection uses the
existing optional phase profiler and feeds nothing into semantics or admission.
The fixture also checks shared Terminal allocation, byte equality for a single
target, and native versus reloaded-Terminal execution.

Composition admission runs once per application, after canonical
publication. Psi projects the ordered entry, callback and physical-adapter roots
with deferred source diagnostics and keeps their pre-optimization semantic
operations in Terminal. Omega prepares the exclusion and provider context once
and judges each root's selected semantic closure, without calling product
policy from Psi or depending on optional optimization. Per-root attribution and
the order of first refusals survive, including missing-body checks when no
authored service exclusion resolves. This does not remove source-side callback
selection or replace its full dormant-declaration checks with reachability of
emitted calls.

Before shared production, each application prepares
the exact target-provider candidates, nominal owners and authored family
declarations for its Build rows. After production, provider publication reads
that owned preparation and the artifact's demand catalog, not typed trees. It
uses the ordinary whole-provider selector and canonical roster join; unused
competing candidates do no harm, family coordinates stay complete and
unavailable targets keep their refusal rows. Preparation is local to each
application: a source cohort does not imply equal Build defaults, overrides or
assumptions. This removes late source reconstruction from publication, not
every source dependency from provider planning.

Psi owns the target-neutral service surface: inherited
applications, signatures, completion, claims, reach and progress premises. Its
projection reads the original simultaneous type and region scopes and keeps the
exact ordered requirement correspondence and supplying ownership. Omega's
`ServiceSchema` shares that declaration and attaches paired calling-report and
commitment evidence by requirement coordinate; inherited receiver comparison
still includes the complete target calling contract. The final immutable
Product owner prepares the full source census once, before target planning;
early Build prepares a separate census for its unchanged provisional
projection. Abstract generic declarations, live applied roots, retained named
applications, operators, top-level requirements and sealed accessor
declarations stay distinct origins. Unsupported surfaces and exact
trait-requirement-closure refusals stay deferred data, not preparation
failures. Target provider derivation and declaration custody query those
records without projecting source schemas again. Candidate eligibility and
capture coalescing stay separate operations; presence in the catalog grants
neither selection nor execution. Independent package-review candidates prepare
their own source census instead of borrowing custody coordinates. Selected
physical calling reads the exact provider schema and ordered row
correspondence instead of searching source declarations by rendered name. Psi
keeps declaring-owner eligibility as deferred per-method data: an ordinary
inherited trait does not become a boundary owner, and an unused such method
does not fail census preparation. Foreign extraction and component ABI
preparation keep the full provider and provenance relationship; toolchain
settlement emits the same derived-plan carrier, not a separate plan-plus-symbol
record. Target calling-plan validation and commitment checks stay separate from
this source correspondence. Native Build supply and canonical-host settlement
attach calling evidence to the same prepared Product declarations instead of
projecting neutral schemas per target. Canonical-host admission still requires
the exact accepted package, path and complete target-augmented schema digest.
Native supplies keep exact declared-signature checks and validate every
authored target's locator before narrowing realization; neither catalog
presence nor a Product row supplies Build-host authority. Entry-role
nomination, physical-entry planning and deferred Build adapter lookup use those
prepared declarations too; their existing source-shape, source-custody and
checked-body admission stay explicit. Reference dispatch carries no retired
`Binding<R>` receiver applications, nor calling-plan inputs only they used.

The compiler privately stores each native
component's canonical description together with every source-free input needed
to derive it again. Package admission supplies an exact compiler/library,
source/dependency, purpose/target and snapshot key; the compiler adds the
current generated bytes, accepted bindings, descriptions and optimization
inputs. A fresh Build admission comes before lookup. Recovery runs the ordinary
entry, calling-plan and provider producers again on the embedded canonical
artifact; it never deserializes a validated wrapper. Receiving policy, live
backing, host bindings and resource charges stay inputs of the current
invocation. Consumers in the same invocation borrow the produced object
directly; it is encoded only when component persistence was requested.

Package preparation advances in dependency-first rounds
across target rosters; demanded component continuations join the same source
batch as their siblings, instead of one target finishing its whole dependency
chain first. Build admission and charges stay local to each occurrence. The
invocation also owns one source catalogue and Psi's canonical pre-Build
frontend snapshots. Admission assigns coordinates before parsing; a dependency
continuation keeps resolution, typing and pending evaluations paired while the
consumer adds fresh declarations. Source membership, imports, aliases and
declaration authority must still match. Anonymous Build-to-Product sealing
admits a new source instance and issues its exact correspondence; executed
descriptions keep their original records and Product lookup reads that
correspondence. The correspondence alone transfers no checking judgment,
executed Build return or evaluation charge.

Source preparation, parsed frontiers and checking judgments live only as long as
the compiler invocation; no source or checking record is written to disk. A later
invocation prepares and checks its source again. Within an invocation, a
dependency judgment enters another consumer only when the producer dependency
context matches: package identity, dependency scope, and callee body hashes. A
body hash alone does not admit it. Persisted native Terminal products still
require fresh source, dependency, compiler, and invocation admission before a
recorded judgment is used.

| Input → output | Owner |
| --- | --- |
| Admitted source frontier → parsed syntax | [Psi source parsing](psi/compilation/source-compiler/src/lib.rs); [package source preparation](omega/packages/package-compilation/src/source/source_assembly.rs) owns import discovery and custody between frontiers |
| Assembled syntax → completed typing | [Psi source continuation](psi/compilation/source-compiler/src/source_compilation.rs), interrupted only for admitted Build execution and generated-source admission |
| Completed typing → source-preparation result | [Psi preparation facade](psi/compilation/source-compiler/src/source_checking.rs), through [typed preparation](psi/compilation/pipeline/04_typed-trees-to-typed-trees/README.md), the one path to frozen typed trees |
| Typed structure → accepted closure | [Structural elaboration](psi/compilation/pipeline/05_typed-trees-to-terminal-psi/src/lib.rs) and [lowered-form checking](psi/compilation/pipeline/06_terminal-psi-to-terminal-psi/src/lib.rs) |
| Accepted closure → Terminal artifact and evidence | [Accepted-source factory](psi/compilation/terminal-production/src/accepted.rs); the [product coordinator](omega/compilation/compiler/src/terminal/terminal_artifact.rs) consumes this shared product |

Package source preparation assembles source sets, imports and custody; Build
evaluation does not discover or assemble sources. The Psi facade keeps the
source continuation across admitted generated-source returns. Elaboration,
checking and publication own the hand-off from source to Terminal Psi, with
pass selection as described [above](#verification-placement). A producer that
is not served refuses instead of rebuilding a removed source-checker result.
The remaining product and default-Build work stays on [TASKS.md](../TASKS.md).

## Build and compiler ownership migration

Provider settlement is finished only when target policy reads canonical Psi
declarations and explicit Build and package inputs without reopening checked
source. The remaining migration steps are:

1. Publish candidate/default and requirement declaration inputs before target
   choice; keep whole-plan, target and grant selection in Omega.
2. Publish exact calling applications, signatures and checked contracts so
   Build-sponsored completion and target ABI/materialization consume records.
3. Move entry, selected-family reach and source-cycle obligations onto canonical
   declaration/call evidence, including dormant obligations for rootless checks.
4. Complete owner-bound service/qualification/role review and deferred constant
   recipes, then remove selected-source policy from Terminal inventory discovery.

Each step deletes the source-reading route it replaces, for every consumer; no
step is handed off separately. Native receiving admission stays source-free.
Check and declaration review must still work without executable publication,
and a reviewed candidate never replaces the original compiler custody.

The owner approved this plan on 2026-10-24 as the acceptance for
**PIPELINE-ARCHITECTURE-ENFORCEMENT**. Build suspension and helper compilation
also keep the acceptance of **TERMINAL-BUILD-SEED**.

Boundary-value layout works from a source-free graph of storage recipes.
Omega's `layout::TerminalBoundaryRecipes` builds that graph from retained
Terminal storage declarations and explicit closed scalar and extent inputs;
`layout::BoundaryValueLayout` packs and classifies it for the target without
reading typed source. Ordinary calling signatures, entry storage and table
dispatch use this route, including retained reference descriptors, runtime
members, admitted fixed-storage alternatives and opaque carrier
correspondence. The graph describes storage only; it grants no type or contract
authority and no representation.

The same source catalog keeps each native parameter and callback telescope
(the ordered list of parameters) before a target is chosen. Psi owns nominal
parameter identities, static-machine ordinal gaps, callback requirements,
synchronous-invocation flags and declared native positions. Default ABI lookup
uses the Product-eligible subset of declarations; authored policy evaluation
keeps the full census of checked inputs, Build copies included. Omega reads
these records instead of walking signatures again. Projection refusals wait
until an ABI request, and runtime type recipes still build one parameter at a
time. The telescopes and their separate source correspondence come from the
catalog the invocation owns. Canonical Terminal keeps the calling telescopes
of the emitted boundaries; native callback placement reads those exact
declaration rows, native-only function-pointer arguments included, instead of
taking identity and order from the source-prepared ABI application, which must
agree with them. Canonical Terminal also keeps exact nominal record and field
associations and full native layout reports, including invariant and
target-specific private-slot selections, and native layout identity is derived
from those retained or decoded records. Canonical signature projection also
resolves native-only callback arguments, binder requirements and private
inline-record paths against the borrowed selected field storage. Completed
scalar extent inputs are explicit; missing backing and unfinished extents
refuse instead of implying empty callback demand.

The same immutable Psi catalog also keeps the full census of explicit
calling-policy applications: the owning vocabulary, concrete boundary
arguments, exact named-map capture, inherited signature order and simultaneous
type bindings. Requests and their refusals stay in source order. Omega follows
these arena-backed associations instead of walking trait requirements or
rebuilding application scopes per target. Omega still selects each
deployment's policy body and admits the closure the compiler runs. The compiler
publishes one immutable policy-code catalog per checked source through ordinary
Psi production; target children borrow the same artifact as the final Product.
Signature preparation completes the native telescope and callback geometry
before the policy's single activation, which runs the shared catalog in
Terminal with finite work and metered storage and results, not in the
checked-source interpreter. Nothing evaluates an incomplete signature first or
replaces the plan later.

### Destination

The `omega` executable is a router. It parses a command, hands it to the one
owner of that command, and turns the returned outcome into output and an exit
status; it owns no compilation, execution, scheduling or package workflow. The
owner calls the other owners it needs:

| Command | Owner | Calls |
| --- | --- | --- |
| `omega <root.omg>` (build), `--check`, `omega psi inspect` | compiler | the interpreter engine, to execute Build programs |
| `omega run` | interpreter runtime | the compiler, for the program it executes |
| `omega packages install`, `omega packages update`, `omega packages audit` | package manager | the compiler, for review |

The interpreter engine is a library beneath both the compiler and the
interpreter runtime, so neither command owner depends on the other's entrance.
Settled with the owner on 2026-11-01.

The compiler's Build coordinator chooses and schedules the needed work. The two
compiler drivers sequence the stages. The Terminal interpreter owns
execution and suspended continuations. Package management owns source
resolution, acquisition, review and lock decisions. Shared representations,
foundations, product reports and publication keep their own owners.

| Entrance | Responsibility |
| --- | --- |
| `omega/application/src/main.rs` | Parse, route to the command's owner, display outcomes |
| `omega/build-system/orchestration/src/build.rs` | Request admission, Build lifecycle, requested products, required tests and publication |
| `omega/build-system/orchestration/src/compile/build_execution.rs` | Dependency jobs and Build activation readiness; mandatory final join |
| `omega/build-system/orchestration/src/compile/source_compilation.rs` | One-shot source consumers: prepare an opaque compiler invocation, execute Build, complete the requested products |
| `omega/build-system/orchestration/src/build_session.rs` | Invocation compiler lifetime beside independently package-owned staging and aggregate sponsors |
| `omega/build-system/orchestration/src/package/package_commands.rs` | Admitted install/update target-review work; complete findings returned to package-owned publication |
| `omega/build-system/orchestration/src/package/package_review.rs` | Admitted review dependency passes, compiler calls and suspended-Build cache recovery, shared with audit |
| `omega/build-system/orchestration/src/package/package_inspection.rs` | Read-only audit session lifetime and ordered target execution; package-owned pinned resolution and reporting |
| `psi/compilation/source-compiler/src/lib.rs` | Source processing and checking through Terminal Psi |
| `omega/compilation/compiler/src/compiler.rs` | Terminal plus explicit realization inputs through native emission |
| Terminal interpreter entrance | Load or borrow code, execute, suspend, install admitted bindings, resume |
| `omega/interpreter/native-provider-runtime/src/native_provider_runtime.rs` | Sponsor isolated native provider incarnations, immutable mapped homes and exact compiled frame adapters; consume source-free products |

These are the target owners, not extra facades over the current compiler.
Split the existing product coordinator among its real owners. Reuse the
source-free `native-compiler` implementation as the Omega driver. Move a path
only when its responsibility and callers move; do not create a second live
driver to match this table. Build-effect evaluation keeps one owner outside the
language transformation stages.

### Public contracts

The router depends on the command owners' entry points, never on compiler
stages or source or intermediate IRs. Package management produces no program
judgments or native images. Psi acquires no packages or permissions and never
starts native compilation. Omega cannot depend on source IRs, source checkers
or interpreters. A stage cannot call the coordinator or a neighbouring stage to
rebuild input facts it is missing. Enforce these production dependency edges;
independent representations and explicit test-only consumers are fine.

Psi 03 reads resolved continuation records and prepared requirement selections
from `symbol-resolved-trees`, not stage 02 APIs. The Psi driver pairs deferred
resolver state with typing checkpoints; allocation and source-activation
handoffs keep the exact predecessor, and pending syntax stays in resolution.

Native code generation must keep the entry, selected functions and admitted
callbacks tied to the same program and target. Omega 00 publishes `VerifiedPsiOptimizationInput` in the shared
`verified-optimization-input` representation. Omega 01 consumes it and passes
its sealed `AdmittedLoweringFacts` to Omega 02. The representation keeps the
entry, native function selection, provider choices, callbacks and interrupts in
private fields. Omega 02 borrows and checks that same record without calling
an earlier stage or rebuilding missing facts, and ends by publishing the
execution plan: target operations joined to their accepted proofs, concrete
types and surviving installations. Omega 03 reads that plan and the
installations the driver bound to it; it cannot name the abstract program,
the optimization unit or the sealed module. This needs no exception to the
stage dependency rules; `check-boundary` enforces them unchanged.

Consumers ask for a meaningful outcome: checking, Build or helper Terminal
production, product completion, native realization or inspection. Do not expose
raw stage numbers or retired checkpoints as a build-system API. Psi owns any
source continuation across Build and generated-source admission and exposes
only the operation that resumes it with admitted inputs. Sessions keep storage
and reusable judgments, not alternative compilation paths.

Provider choices, representation selections, native intent, grants,
declarations and exclusions issued from source keep their exact authored
operands, activation, executed roster and dependency custody. These records are
inputs, not judgments. The retired source-checker admission sequence is gone;
when no producer yet serves nonempty source authority or Product policy, the
request returns `no product route yet`. The Product coordinator neither
resolves one source operand at a time nor invents acceptance from recorded
choices. A served producer must recover original lexical authors and static
environments, authenticate dependency selections and keep exact application
identities. Assumed relationships need the consuming root's
authority; neither an old Build return nor a dependency's records transfer it.
Package review owns audit projection, the Psi checker owns wire-compatibility
obligations, and Omega owns runtime provider compatibility.

ProgramEntry descriptions and source compatibility stay with Psi. The target
catalog supplies source-visible slot requirements without receiving typed trees
through a compatibility callback. A served producer must keep lexical
ownership, the original slot identity and exact core Product identity;
physical arrival belongs to the selected target schema. Until that producer
exists, description, binding and settlement requests refuse. Hosted source
descriptions alone do not make physical arrival run.

Authored grant and exclusion records belong to language semantics. Shared crash
causes and physical-class vocabulary keep their canonical encoding; naming a
restriction does not prove a behavior absent. Runtime provider compatibility,
selected executable closure and physical mechanism checks stay with Omega and
read published declarations and admitted plans, with no source-IR backend
exception.

Retained opaque selections live in the typed-tree data owner and carry no
semantic-copy receipt or checking authority. Nonempty representation admission
refuses for now; a future producer must establish the required obligations on
the lowered form and keep exact application custody. Cached source data or an
old source judgment cannot replace that evidence.

Interpretation and native realization read the common Terminal view
[above](#one-driver-one-pass-per-target-realization). Keep suspended activation
storage alive while helpers compile separately. Target/Build companion inputs
travel explicitly; consumers cannot reopen checked source. Matching inputs may
share work, but host Build and product contexts are not interchangeable just
because source bytes match.

Callback and physical-entry adapters join their exact selected declarations to
canonical Terminal bodies. Each body keeps its combined scalar and structural
runtime order, so native preparation derives the formal correspondence from the
artifact itself, also after reload. These adapters make no late Psi signature
query.

Entry custody keeps declared arguments and physical/semantic calling contracts,
not service fields synthesized on an application record. Provider
roots come from actual calls and callback demand. Selected entries, private
native derivation records and runtime provisions carry no synthetic
field-establishment roster; older private records are rejected at their schema
boundary. Ordinary provider admission, access-view inputs and physical entry
adapters stay independent.

Component publication prepares its exported calling applications before
Terminal production. The retained product carries them to the ordinary provider
catalog join, which checks them against imported and explicit boundary policies
without reopening typed source. Native proposal construction builds no
component policies and keeps no second catalog merge.

Independent component selection keeps its exact verified artifact, native
proposal, selected plan and export/calling-application association. Terminal
production checks the retained availability; native preparation borrows the
completed join instead of searching the checked source product again. A missing
product is still a native-only refusal, including for direct compiler requests
with no package session to attach it to. The invocation-local selection is not
serialized and is not an installed capability. Admission of the real receiving
artifact still checks every selected export at the cross-artifact edge.

Checked-provider binding also packs the exact selecting declarations and
provider associations into a source-free record beside the complete selected
plans. Native request projection borrows that record instead of reopening typed
declarations. Preparation keeps a refusal as data until the native handoff
needs it, so a checking-only stop does not take on native-only obligations.
Physical realization and receiving admission still belong to the Psi consumer.

Ordinary provider binding keeps closed intrinsic and syscall applications but
does not replay checked-adapter instances. Native realization and unoptimized
behavior-exclusion admission join those instances from Terminal's canonical
provider associations and exact prepared declarations. Check and inspection
stops, nominal callback preparation, checked-reference interpretation and
package review ask for their checked-source projection explicitly. Review
derives it from the original compiler custody, never from the supplied semantic
candidate. The selected plans, Import application custody and exact owner
identities stay unchanged; each completed instance roster recomputes its
aggregate commitment. That source projection has a declaration producer and a
source-free policy consumer. Psi records exact inherited receiving-application
candidates and generic specialization identities, keeping individual failures
inside their candidate groups. `provider-planning/src/provider_instances.rs`
supplies original plan and review renderings of Psi's checked identity handles,
original plan/row correspondence and exact adapter judgments;
`provider-selection` owns candidate cardinality, template-row agreement, full
closed-instance coverage and attachment to the selected plans. Review still
prepares from the original compiler program, not the candidate under review.
These are invocation-local declarations, not another portable format or
permission to run a provider. `SelectedProviderSource` owns the immutable
checked program and the complete selected-plan roster together. It prepares
that declaration payload lazily, once; callback, Check, review and reference
consumers borrow it and run source-free selection at their original admission
points. Empty callback rosters do not initialize it. Clones share preparation,
equality ignores only derived cache state, and no caller can replace the
program without its plans.

Psi's `PreparedProviderInventory` keeps the immutable typed owner and its
service declarations for inspection. The target-supply walk instead follows
boundary calls from the accepted Terminal module, and refuses a demand for a
Build-selected target implementation until that supply has a producer.

One Psi declaration producer supplies numeric meaning and external
satisfaction, shared by Terminal retention and rootless package projection.
Checked-provider binding keeps that projection before selection. Package review
prepares its supplied candidate on its own, then joins nominal requirement,
realization and selecting-owner identities against immutable custody. A library
needs no ProgramEntry or executable lowering to review its uncalled supplied
declarations. Private checking-only coordinates stay invocation-local, never
portable owners. The selected row does not duplicate numeric execution
identity: review and native settlement read the checked meaning through the
shared source-free external-satisfaction join. Host and access authority stay
independent. Psi also keeps the requirement's exact schema, method and owner
coordinates; numeric settlement checks them against the selected whole plan
through the same join, and early typed Build planning adapts the source-owned
view to that check. Eligibility is kept separately from numeric meaning, so a
missing meaning cannot fall through into another execution category.

Direct-call preparation keeps the original plans and provenance once, with
foreign ABI results or deferred refusals attached to their original row
coordinates. Selected execution resolves accepted host bindings and builds
complete target-companion leaves from those borrowed rows; there is no pass
that patches placeholder leaves and no per-leaf copy of the source plan. The
projected callable contract stays distinct from the original satisfied schema
and the accepted public-plan identity; native demand and receiving authority
still decide which supplies may run.

On the lowered route, provider settlement leaves one small record per target:
`selected_dispatch::SelectedExecution`, holding the target and its settled
provider plans, built only from the settled supply (`SelectedExecution::settle`).
A plan has one checked adapter row for every requirement of its slot's schema,
naming the machine the accepted module states as the chosen provider's
candidate for it (`provider_planning::target_supply`). It carries no grants,
callbacks, review provenance, host roles or direct-call supplies: a product
derives what it needs from those plans and the accepted Terminal artifact where
it uses them, and a product whose inputs have no producer yet is refused by
name (`compiler/src/product_execution.rs`). There is no intermediate
checked-provider result and no source installation phase after Terminal.
Conversion to `CheckedCompilation` only assembles the completed custody.
Checking-only task validation stays separate from native task demand read from
the artifact. Source and role failures can therefore come before Terminal or
task diagnostics; native source failures stay deferred, and receiving
permission, trust admission and actual native demand stay where they are.

Installation-reach preparation reads the selected provider's exact retained
requirement and realization rows, including inherited and dormant
declarations. It projects checked bounds and dependency identities once;
source-free `provider-selection` owns fixed-point substitution across the whole
selected closure. A missing source realization is not an empty service row, and
a pending competing publisher cannot become a unique fallback. Substitution
neither searches typed declarations nor rebuilds provider names. Existing
selected fact attachment validates the bound; independent package review still
compares the resolved row with its own checked-source projection.

Accepted-service preparation likewise records the exact nominal owner, complete
target-side schema and ordered requirement identities once, at checked-provider
binding. Resolved semantic bindings carry that record, not a source-arena
symbol. Requirement-only role resolution, permission validation and candidate
nomination read it without rebuilding typed schemas. Independent package review
projects the supplied program into its own catalog, compares complete records
against immutable custody and uses only its own source correspondence for the
remaining contract capture. Unsupported or unowned unused declarations keep
deferred projection refusals.

Original provider rows keep the declaration facts shared by selected review and
direct-call completion: exact nominal owners, bundled-source eligibility,
signature, effect and supply checks, and sealed placed-operation contracts. Each
row owns its optional direct-call input; bodyful checked completion adapters
keep their host-role facts without gaining a direct leaf. Selected settlement
rejoins the exact original plan and provenance and reads prepared placed-catalog
and exit-role facts. Selected-row review projects the supplied source through
the same producer; independent package review still projects its candidate
afresh. Direct-call completion joins target and accepted public-plan custody
without reopening an earlier representation, and handles deferred shape
failures only after the original ownership and target gate. Numeric execution
still comes from Psi's external declarations. The accepted public plan, the
original row and the projected callable plan stay distinct. Planning candidates
enter completion only after source-owned row preparation has consumed their
original ownership and derived these facts from the shared checked program.
Product scalar completion follows canonical publication; it neither rewrites
those declarations nor substitutes a target-specific checked snapshot.
Retained declarations do not bring back retired standalone boundary
requirements or source `via` bindings.

Target preparation starts from the accepted Terminal module and follows the
boundary requirements its entries and chosen providers reach.
`provider_planning::target_supply` applies the Build overrides and
target-package defaults to those requirements. The compiler judges the chosen
providers for that target and settles their plans.
`SelectedExecution::provider_proposal` joins those plans to the produced
Terminal artifact through `TerminalTargetProviderSelection::from_plans`. Native
completion takes the proposal and uses `realization/provider-selection` to
admit the already chosen boundary applications and bodies. Completion takes an
explicit native-proposal request and the produced application, not a checked
compilation. The application keeps physical-adapter selection; completion still
binds its exact arrival contract against the artifact. The native compiler owns
this source-free preparation in `application.rs`, plus admission of the
separately supplied package-permission rows. Product coordination supplies
those inputs and requests preparation; it has no direct dependency on numbered
Omega stages. Artifact-only and component publication use the same preparation
without emitting an image.

Product scalar completion reads canonical Terminal applications, declaration
conditions and exact lexical identities, not a private rewrite of an earlier
representation. Source completion records deferred recipes and complete
initializer entries in the typed program before checking. Product carriers keep
neither a second authored forest nor source-evaluation continuations for later
target closure. Early Build evaluation still consumes its pending source work
before admitting the Build activation; that is separate from Product scalar
completion. Source checking roots complete initializer entries before ordinary
preparation. It exposes no target-proof closure that rebuilds an earlier
program or refreshes its execution catalog, and callable descriptions are
published inside the checking stage, not through a second source-settlement
operation the coordinator can call. Psi keeps the computations and obligations;
its compile-time evaluator runs dependency-ready canonical jobs and closes each
authored proof scope. The application supplies exact provider selections,
target families, address semantics and carrier geometry, and keeps evaluation
accounting even on refusal. Native preparation first works out the work and
scalar demand for the root and its components; the application completes that
demand, and native compilation reads the same preparation and completed inputs.
Native production has no interpreter dependency or evaluator callback and
cannot silently recompute a missing value.

`build-system/orchestration/src/compile/native_completion.rs` owns this
lifecycle for standalone and package products. Source compilation hands over an
owned `PendingNativeProduct` after admission and Terminal preparation and
releases its checked source. The shared `build-settings` scalar operation also
serves descriptive Check, inspection and component publication; those stops
take on no native execution obligations. Declaration-only Check publishes a
rootless Terminal product when conditions need completion; a Check with no
retained scalar obligations stops before publication. Demanded components
complete all declaration conditions against their shared Application artifact
before publishing descriptions or entering the component cache, without first
producing a throwaway declaration artifact per target. Lazy review keeps its
earlier checking boundary because review evidence may be used without any
component being requested. Named inspection joins the same batch, with its
selected machine as part of production identity. Equal selections share
portable code and source-issued Build companions; different inspected roots
stay distinct. Inspection completes declaration scalar and task obligations
against that product without a ProgramEntry or native installation. Inspection
of an external artifact keeps only the admitted artifact and cannot recreate
the source-issued companions. Published checking, inspection and component
completion read the artifact's complete task-demand bundle and identity table,
including declarations outside the selected entry's executable reach, and do
not lower those obligations from checked source again. A checking stop without
publication still projects its task demands directly; full checked-provider
instance coverage stays a separate source judgment.

Build-helper production publishes the checked source unchanged. The waiting
Build activation admits each explicitly named provider slot, finds its scalar
and inline-storage demands, then completes canonical applications under the
admitted Build execution profile and that slot's exact provider selection. It
never selects the dependency's Product defaults. Installed components own their
completed scalar admission after temporary helper preparation is released.
Evaluation charges the waiting invocation directly; retained observations are
not charged twice. Backing that is already installed skips completion before
resumption.

Numeric intrinsic classification is decided in one place, owned by source: the
exact requirement's token or named catalog row supplies both the signature
obligations and the closed execution identity. Provider planning and selected
dispatch use that classification; diagnostic strings do not decide support.
Package review still rejoins its supplied candidate to retained selection
custody on its own. Psi publishes numeric meaning and its exact IEEE format in
canonical Terminal boundary declarations; native proposal creation reads that
meaning and joins the selected external realization to canonical checked
satisfaction edges. The module-level catalog does not depend on executable
call shapes, so directly lowered comparisons keep the same relation. It keeps
requirement, realization and selecting-owner declarations, authored target
membership, package identities and structural binding mechanisms independent
of target selection. Native retention drops the duplicate numeric execution
sidecar; host and access authority judgments stay separate. Portable decoding
validates declaration coordinates and the complete numeric signature. This
meaning neither selects nor authorizes a provider. Provider selection and
target geometry come from outside Psi; Product scalar publication is
target-neutral.

Independent component emission uses the same source-free native compiler entry
point as root emission. Product preparation supplies each verified component,
retained proposal, complete receiving plan and accepted package permissions.
Native compilation keeps each emitted image beside its admitted call, joins the
root's settlements, and keeps component products in the resulting runtime
provisions. There is no public component-emission entry point and no outer
native orchestration before or after it. Receiver policy stays separate from
package acceptance.

An anonymous Build runs before its final source graph has a declaration owner,
so the Psi production request separates invocation-only code from portable
publication while using the same stages, Terminal representation and
interpreter. Invocation-local coordinates keep their live code owner; neither
artifact encoding nor native production accepts that private product. Build
companions need both the exact code semantics and the same invocation owner.
After the dependency join, package coordination must set the final owner before
Product resolution and typing, as required by
[anonymous owner identity](../wiki/spec/build/declarations.md#default-builds-and-anonymous-owner-identity).
Retagging already-typed declarations or replaying Build does not do that.

### Visible execution

The Build entry point shows, in order: request admission, activation
preparation, dependency settlement, frozen Build outputs, product compilation,
required tests, publication and outcomes. Its execution loop handles real
dependency requests, provider readiness, completed helpers, Build return and
failure/cancellation; it is not a generic event engine for compiler phases.
Dependencies may start while Build runs; waiting resumes the same activation
without replaying effects. Build return joins every request before
product completion. Product bindings do nothing during Build. Generated source
cannot change admitted authored meaning. Ordinary Build helpers need Psi
execution, not native product compilation.

Demanded Product descriptions pause the same activation while the application
acquires and settles its issued Product dependencies. The compiler's opaque
query context shares the captured source prefix, loads authored Product
declarations through the source-assembly owner and answers in the query
author's lexical scope. It neither gives an unfinished root its final
package identity nor runs Product bodies. Final binding rejoins the selected
source declaration instead of reusing an old arena handle or looking up its
name. Query dependencies join the same demanded-source ledger as executed
helpers, including purpose-qualified workspace-catalog records. Entry,
data-schema and named-implementation provider descriptions share lexical
resolution; role-specific inspection authenticates the activation's issued
description and returns an accounted immutable byte view, without reopening
source or running Product code.

Execution preparation and Product completion take different inputs. Source
inventories, named inputs, required outputs, resource sponsors and restricted
grants must join the activation before its effects. A returned Build does not
satisfy a later execution request just because its source graph matches. After
return, the coordinator joins final source custody and chooses Product targets;
it must not drop execution obligations while turning occurrence plans into
retained Product work. Build's execution profile stays independent of those
targets.

The compiler's `settle_package_build` takes the exact returned activation and
checks its source-request ledger and required outputs before returning opaque
`CompletedBuild` custody. `prepare_package_product` admits each requested
Product context from that settled output. Review proposals, accepted semantic
bindings and compilation purpose belong to that context, not to the executed
Build. The source graph and execution observations stay fixed. Product source
is resolved and typed under its own admissions; a sibling's typed declarations
are never retagged. Default Builds keep immutable parsing for changed
source-routing inputs. The ordinary `complete_package_build` route runs the
same settlement and Product-preparation sequence without forking a sibling that
does not exist. An already prepared `PackageBuild::select_target` is still
limited to equal source and semantic inputs; it selects realization, not a new
Product judgment.

Ordinary CLI and SDK build, check, run and inspection use this lifecycle; SDK
callers submit `BuildRequest` instead of preparing a second, statically
projected graph.

Package recovery begins before acquisition; one application session runs the
actual declarations and selected dependency requests, keeps their source
captures and settles each Build. Package-owned Product admission keeps the
source, compiler and lock checks and target-specific refusals, but issues no
second execution request. Its inert execution requirement must match the
pre-effect host profile, snapshot, named inputs and required-output roster kept
with the Build. Discovery runs with private snapshot and staging authority
only: a restricted request stops before effects, and neither an accepted
Product row nor a discarded late grant can authorize it. Matching Product
admissions share one prepared source continuation; realization still validates
each target's own generated bundles and permissions. Named Git selection
settles the actual workspace catalog and observes member declarations from
authenticated subtrees. Those observations are not completed Builds or
Products: only selected members join their dependencies and prepare Products,
using their original returned activations. Relative Paths stay within
registered member navigation at the frozen repository resolution.
Install/update/resume run their staged candidates through the same discovery
loop and keep settled Builds across review passes and targets.
Proposal identity, original local dependency bases, execution inventories,
review decisions and accepted-pair publication stay with the package system.
Semantic-role discovery and accepted bindings stay part of Product judgment
reuse, never copied into a bound admission. The retired entry-field tolerance
mode is absent from compiler requests, continuations and review facts; ordinary
entry contracts establish actual arguments, never provisional fields. Older
review-cache formats miss instead of importing a provisional verdict.
Observational audit uses the same executed source roster and retained Product
review. Its accepted graph admits dependency requests before acquisition, keeps
exact Git pins and permits only root-content changes; per-target findings never
recover proposals or publish acceptance. A source failure stays an unavailable
finding for every requested target. Review has no fresh-execution or
sponsor-issuing route.

Standalone batches, diagnostic checks and named inspection use the same
application execution owner. The compiler's opaque `PreparedCompilation` and
`SourceCompilation` keep source continuations and target policy alive; their
completion accepts the exact returned Build, never an execution callback or a
sidecar of an earlier representation. Target-local refusals and post-Build
discovery stay in their outcomes. Explicit provider readiness keeps the
suspended activation while its named dependency is acquired, settled and
compiled to Terminal Psi, then installs that exact implementation before
resuming. Helper Product preparation uses an explicit dependency-first
frontier, keeping generated bundles by package and purpose without recursive
preparation calls. Source execution, provider and query dependencies, graph
resolution and workspace catalogs also use application-owned pending work.
Each paused Build keeps its original invocation, interpreter activation and
exact host adapters. Immutable Terminal code and source-issued call tables stay
with their existing owners without encoding, decoding or rebuilding. Host
issuance and activation start happen once even when caller handles are cloned
or dropped. The final source join also reconciles helper graphs kept with
workspace declaration observations; pending Product requests of unselected
members do nothing. Package locks keep catalog execution custody separate from
Product routing. Fixture test harnesses use this same executed discovery and
keep settled Builds through target selection and Product admission. Their
explicit fixture consent neither builds a second source graph nor grants late
execution inputs; named inspection reads retained products as well.

Each compiler entry point shows named representations and real stage calls in
execution order. A separate `pipeline.rs` is justified only if it owns real
sequencing, not forwarding. Use ordinary locals, loops and matches for
lifecycle decisions; keep per-target failures and ordered outcomes without
splitting identities from products into parallel tables. Run independent jobs
or target realizations in parallel, not every phase. There is no line quota.

### Execution order and acceptance

1. Record current consumers and their behavior: ordinary and package
   compilation/review, inspection, interpreted/native execution, artifact-only
   outputs and mixed passing/failing targets.
2. Move real stage sequencing into the two compiler owners; remove the moved
   outside stage calls and duplicate sequencing routes.
3. Move Build lifecycle orchestration into its application owner and migrate
   package/CLI consumers with it, deleting the replaced orchestration.
4. Finish source-free Terminal inputs and the accepted Terminal Build/helper
   continuation contract.
5. Remove obsolete packages, exports and adapters; align dependency guards,
   documentation and tests with the new ownership.

Each step keeps working consumers working and lands through normal
validation; the outcome stays open until every obligation is shown. Acceptance
includes readable build/stage entry points, no forbidden production edges,
exactly-once Build effects across helper suspension, rejection of
failed/cyclic dependencies, generated-source isolation, shared source/Terminal
storage across applicable targets, execution of loaded and retained artifacts,
meaningful output stops, focused runtime/refusal tests and no material
iteration-time regression. Record current progress on the existing task, not a
parallel task board.

## Placement and semantic ownership

On 2026-09-26 the owner rejected merging stages into a few large crates.
Durable IRs stay independent packages under `representations/`; transformation
crates consume and produce them without owning their public types. Shared
foundations and utilities stay independent libraries. Do not fold them into a
producing stage or the application crate to cut crate count or directory depth.
Subsystem folders show their coordinators and supporting packages without
merging their APIs or dependency boundaries.
Omega groups packages under six domains: `application/`, `build-system/`,
`compilation/`, `interpreter/`, `packages/` and `foundation/`. Compiler
machinery, including `pipeline/`, `representations/`, `realization/` and
`backend/`, lives in `compilation/`; the Build coordinator lives in
`build-system/`. Hosted execution (the Terminal runtime and the native-provider
runtime) lives in `interpreter/`. The domains are directory groupings; they do
not merge package boundaries.

Psi groups its packages under `compilation/`, `interpreter/` and `foundation/`.
Source transforms, program representations, semantic checking, proof and
compile-time elaboration belong to compilation; the interpreter domain holds
only execution engines. The groupings keep IR and semantic library APIs
independent and add no compiler driver.


| Responsibility | Owner |
| --- | --- |
| Current program forms and their attached records/codecs | Each half's `compilation/representations/`, grouped by program family |
| Program judgments | Psi `compilation/pipeline/06_terminal-psi-to-terminal-psi/` over structurally elaborated Terminal Psi; independent mathematical checking in `compilation/proof-admission/` |
| Terminal encoding, semantic reconstruction and work | Independent packages beneath Psi `compilation/representations/terminal-psi/` |
| Transformation, rewrite, private scratch and analyses | Each half's `compilation/pipeline/`, in the owning `X-to-Y` or `X-to-X` package |
| ISA, ABI, runtime carriers, object/relocation/image mechanics | Omega `compilation/backend/` |
| Shared target-neutral source/arena/numeric primitives and optimization selections | Psi `foundation/` |
| Shared host primitives, native function identities and optimization vocabulary | Omega `foundation/` |
| Application workflows and presentation | Omega `application/`, depending only on product facades and shared request/result contracts |
| Source/package custody, lock updates and review decisions | Omega `packages/package-manager/`; no checking or product production |
| Source → Terminal product sequencing | Psi 00–03, the explicit 04 preparation step, structural elaboration, lowered-form checking and accepted-source publication (the empty Psi selection; a nonempty one refuses) |
| Terminal + target/Build inputs → realized product sequencing | Omega compiler facade over Omega 00–09 and emission |
| Requested outputs, configuration admission, retained outcomes and publication | Omega `compilation/products/` |
| Source acquisition and package storage primitives | Omega `packages/` behind package-manager requests |
| Terminal Psi loading, supplied-binding installation and execution | Psi interpreter facade under `interpreter/` |
| Target-aware interpreter host adapters | Omega-owned host integration outside Psi, reusing independent backend ABI/mechanism libraries; not a compiler driver |
| Build-time execution inputs and accounting | Psi `compilation/build-time-evaluation/`; actual canonical artifacts use Terminal execution, while unsupported source jobs refuse |
| Accepted lowered-form closure → published Terminal artifact | `psi/compilation/terminal-production/`, keeping the same source owner |
| Ordinary library reference implementations | `library/`, outside the compiler halves |

### Program families and support packages

Psi keeps tokens, syntax trees, resolved trees, typed trees and Terminal Psi.
Structural elaboration and the lowered-form checker work on the same Terminal
module, which is published with its evidence ([verification placement](#verification-placement)).
The retired facts and intermediate Lowered Psi packages are gone. Checker tables belong to
`psi/compilation/representations/terminal-psi/check-tables/`.
Shared effect-summary records and pass-selection vocabulary live in
`psi/foundation/flow-effects/` and `psi/foundation/optimization/`.

Omega has six top-level program families: abstract operations, target
operations, selected instructions, register homes, post-allocation machine and
machine code. They are not six consecutive passes. Same-form optimization
passes keep their input form; `machine-code` owns separate selected-form
encoding, resolved-layout and final machine-code roots. The
[native representation map](omega/compilation/representations/README.md) names the actual
producer and consumer of each form.

The `optimization-unit` package under `abstract-operations/` holds the derived
CFG/SSA and replay form used during abstract optimization. The
`legalized-operations` package under `target-operations/` holds the internal
form between legalization and selection inside Omega 03. Both are independent
packages; nesting names the family and adds no dependency on a producing stage.
Keep these forms and manifests separate, and do not invent pipeline steps to
make folder counts match.

In backend, target descriptions, calling conventions and register models
belong to backend itself, installation and task evidence to backend runtime,
boundary-application companions to backend artifacts, and provider authority
evidence to backend plans. Target-independent records of opaque representation
selections belong to the `typed-trees::opaque_representations` data owner; they
keep source choices, not a checking verdict. None of these is a pipeline
program IR.

### Subsystem entrances and supporting packages

[`omega/compilation/products/`](omega/compilation/products/README.md) owns
invocation configuration, product eligibility, finished report payloads and
artifact publication. It also owns the retained evaluated-Build records and the
checked compilation product: source-custody sealing, immutable stage results
and their metadata and subject projections. The Build evaluator produces this
shared data contract; product consumers depend on neither evaluator nor
compiler orchestration. Sealing grants neither package
admission nor native publication authority, and consumers must still rebuild
what they check. The checked product is a compilation result and output
lifecycle, not another IR or pipeline phase. The coordinator consumes
validated requests and keeps the target execution loop; normalizing the set of
exact target names belongs to the backend target catalog. The CLI publishes
through this owner directly.

Product source, generated extensions included, declares ordinary named
implementations and explicit requirements. Build records target-keyed provider
selections separately; source declarations have no target modifier or implicit
same-path body family. Provider planning joins exact checked requirement and
implementation identities, never a roster of source names or a target-selected
source body. Build helpers use their separately admitted execution profile.

Checked qualification evidence names its source declaration and exact boundary
requirement, not a selected provider's receipt number. Provider grant binding
borrows that evidence and returns selected plans and grants separately; it
never rewrites or clones checked Psi to stamp target-specific authority into
facts. Psi prepares each authorized contract issuer's export identity once,
before sharing the checking baseline with flow workers. Admitted fact
occurrences carry that stable issuer handle; the inventory alone creates no
obligation. Omega's source-free grant policy reads those occurrences, including
uncalled bodies at the Check stop, and keeps the declaring-owner preference and
the refusal of ambiguous grants without rescanning typed traits or signatures.

Psi keeps checked operator declaration eligibility separate from closed
application body candidates. Open or dormant declarations do not root bodies or
establish executable coverage. Omega admission and checked dispatch join
selected plans to that inventory through one source-free choice operation that
keeps provider and package identity, effective target context, scope and
application coordinates. Neither rediscovers specialization or source
satisfaction; the exact application's existing body span supplies its
executable alternatives.

`omega/compilation/compiler/` owns only compilation orchestration.
[`omega/compilation/native-compiler/`](omega/compilation/native-compiler/README.md)
owns the source-free sequence from Terminal to image. Its entry is `compile.rs`;
`pipeline.rs` owns the shared abstract-to-object sequencing for application
entries, callbacks and component exports. Realization captures runtime
provisions before the abstract, target and physical stages produce the image;
those provisions belong to `native-artifact`, not to the source compilation
products. Provider evidence contains no source-IR helpers: conversion from
source to evidence stays with the source-aware producer, and lifetime partition
normalization belongs to shared language semantics. The native compiler's
transitive production dependencies are enforced by `mbx xtask check-boundary`,
including optional, build and target-specific edges.
[`omega/build-system/build-settings/`](omega/build-system/build-settings/README.md)
owns Build admission, consent and execution; it is not a pipeline phase. The
shared package-input contracts live with the package system, not compiler
orchestration.

A root grant in `build.omg` is the root's acceptance; dependency claims are
accepted through `omega.lock`. Pipeline execution does not load or mutate a
separate admissions file.

[`omega/compilation/realization/provider-planning/`](omega/compilation/realization/provider-planning/README.md)
owns target supply selection and provider plan formation.
[`provider-selection/src/boundary.rs`](omega/compilation/realization/provider-selection/src/boundary.rs)
admits canonical boundary applications against the chosen plans.

Psi projects checked task activation requirements, exact specialization and
carry evidence, and the reachable frame graph into Terminal Psi. State-resident
storage and staged arguments at each suspension crossing stay separate
alternatives. Native proposal construction joins those demands to exact
selected runtime plans and target geometry without reopening source.
Checking-only stops ask for the same portable projection without lowering
executable bodies and use the same source-free join. Instruction lowering and
task storage share the backend calling-conventions geometry owner. Static
activation planning is not a task executor.

[`selected-dispatch/`](omega/compilation/realization/selected-dispatch/README.md)
owns selected execution preparation and settlement. It imports no Psi
transform: the source coordinator owns stage requests and supplies operator
selections. Adapter dispatch reads the canonical backing through a Product
declaration and occurrence selection instead of cloning and pruning a separate
program. Build copies stay outside that selected frontier. Provider grants
resolve through backend provider evidence.

Optional x86 FMA admission reads demanded Terminal builtin proposals at native
provider admission. Checked source keeps no separate FMA association list: an
operation in an unselected target body creates no deployment demand for the
selected body. Each realized FMA still needs exact profile and feature
admission, and its ordinary target-operation record carries that custody.

Normal Product operator and requirement admission borrows the checked external
catalog and its exact source-to-nominal correspondence instead of rescanning
typed numeric meaning or satisfaction edges. That catalog is prepared before
selected plans are bound, so malformed declarations may be reported before plan
or qualification diagnostics; both are refusals before execution. Product
boundary-provider admission also reads Psi's complete requirement inventory and
occurrence applicability. Checking records slot and package identity, the
difference between dormant-operator and direct-call eligibility, and whether a
use belongs to a generic machine, so admission does not rebuild those facts
from typed declarations. Requirement rows exist without implementations or
uses. Product root pruning projects direct-call rows and template
applicability with the machine roster and keeps independent operator
declarations. The prepared Psi declaration owner projects that inventory into
a borrowed, source-free review view. The full dormant, operator and direct-call
admission sequence and plan cardinality belong to `provider-selection`.
Missing application or numeric evidence stays data until the selected binding
needs it; projection never rejects an unused declaration or a skipped generic
template. Declaration strings and body eligibility are borrowed from the exact
immutable owner, with no second cache or demand for portable publication.
Emitted demand cannot replace unused declarations or dead concrete uses.

`provider-selection/boundary_applications.rs` owns the canonical executable
application operation: exact selected-row and provenance joins, checked body
choice and numeric execution admission. Omega 00 builds operation-bound
physical coverage from its result. Source and canonical admission share numeric
satisfaction and callable, package and target selection; source eligibility
also checks Product, provider application and lifetime coordinates, while
canonical admission checks the nominal requirement and closed application.
Neither invents the other's metadata. Unused invocation-local declarations are
not forced into portable publication. Complete selected-plan binding belongs to
`provider-selection/selected_binding.rs`: grants, qualification, boundary
admission, reach resolution, physical-leaf aliases and Import application
attachment run in their original order without typed trees or check facts. Psi
projects selected declaration evidence from the exact source associations,
deferring unavailable evidence until its admission phase. Named-provider
derivation keeps the original declaration and capture scope and declaration-row
order, so source projection never repeats target selection or uses an open
declaration scope in place of a captured receiving application. The temporary
view borrows the existing selected plan and provenance instead of building a
second candidate cache. Early and reference instance projection share Psi's
closed-leaf roster but stay separate from ordinary Product preparation.

Callback-root selection likewise belongs to source-free `provider-selection`.
Psi freezes the complete final checked nominal-use roster once, in the existing
prepared declaration product. Canonical arena rows keep registration
classification, exact requirement and body declarations, separate published,
actual and refinement contracts, and checked entry-resource report axes.
Dormant and symbolic uses stay present, and unavailable declaration projection
stays deferred, so checking-only products need no emitted callback body.
Calling declarations resolve through the product's authenticated owner/member
correspondence, keeping exact empty-argument matching and duplicate counts
without ABI publication. Source preparation freezes exact adapter results once
per original selected plan and row; trait and top-level instance expansion read
those results at their original diagnostic sites, repeated error reports
included. The returned callback-provider product owns projected instance
metadata and borrows only source-free plan/row custody. Callback admission
itself runs in `provider-selection` without checked source: use iteration,
exact selected-row lookup, calling counts and entry policy share one visible
loop. Full plan/row equality rejoins the projection's clone; report hashes or
source pointers cannot replace it. Missing preparation refuses a nonempty use
roster, while a completed empty selection is valid. A registration failure
suppresses only that occurrence's plan diagnostics. Selected roots borrow the
same identity table in occurrence order.

Terminal Psi owns the prepared root records and their exact retained-body join;
its constructor refuses invocation-local declaration owners before those
records cross into Omega. Psi projects the requests and diagnostic
associations, and the application carries exact declarations, not callback
source symbols. Composition rejoins each complete
owner/signature/contract/specialization to exactly one unoptimized body,
lazily, after the semantic entry and before the physical adapter. Diagnostic
names and spans stay payload; Psi still projects ordinary callee and provider
names and root-specific caller chains. This transient companion changes no
portable schema and does not replace dormant admission with emitted
reachability. These producer-owned records are not independently admitted
decoded evidence.

Early and reference source adapters stay distinct from Product policy.
`SelectedExecution` keeps no reference dispatch table, and ordinary Product
installation does not scan source to prepare an engine it will not run.
Terminal and native provider admission read canonical demand and selected rows
independently. Reference preparation never copies or mutates checked Psi;
Build's required comparison preparation stays separate. Requests with matching
source owner, entry and Psi optimizations share one Terminal production and
backing allocation, also in retained native images. Each target separately
admits the retained pre-optimization composition and keeps its own entry
receipt and native proposal. Distinct roots and callbacks stay in that same
whole-program Terminal roster.

Executable callback placement belongs to Omega 00: canonical callback
applications name their registrar operation, exact body, requirement and
checked entry admission; target calling applications keep exact static binder
ordinals, requirement identities and synchronous invocation membership. Fresh
and recovered native inputs derive placements through the same join; neither
the realization request nor its persisted record carries callback placements
produced from source. Source preparation still selects callback roots and
checks dormant declarations before behavior admission; emitted occurrences
alone cannot replace that complete source roster. Psi identifies callback uses
from their declared calling policy and keeps the exact checked entry resource
receipt. Native ABI plan hashes are not callback checking facts; Omega binds the
checked entry to its evaluated calling application without rewriting the
nominal-use facts. Evaluated calling applications belong to provider
settlement, not typed trees. Schema derivation receives them explicitly;
package review joins the exact schema commitment to the retained realization.
Callback materialization reads only canonical Terminal uses and completed
calling applications, without reopening source.

Boundary requirement applications and occurrence receipts are Psi data,
independent of the selected composition mode; source has no provider value or
receiver to carry them. Provider settlement does not mutate typed trees to
authorize erasure. Omega joins the entry's exact requirement to its selected
provider and establishes runtime custody. Native entry storage supports only
Fused establishment for now and rejects other modes.

Physical data-layout queries stay symbolic during shared Product checking.
Canonical scalar applications keep `size_of`/`align_of` observations through
Terminal publication. Completion uses explicit selected-target carrier
geometry, not interpreter storage or host pointer widths. Reference and view
carriers and sum packing keep their physical layout distinctions. Build-helper
completion uses its separately admitted execution profile at provider
installation. Ordinary authored machines with those names stay ordinary calls.

Calling-policy evaluation keeps its validated ABI application as backend-owned
output. Native entry and registrar settlement read that application, not the
evaluator's typed handles, source spans or policy execution state, and their
contract joins do not rerun calling-plan validation. Entry signatures and Psi
entry receipts join by exact package/toolchain declaration identity, not
source-arena handles. Source coordinates and Extent layout capture stay with
provider planning; backend entry plans and image emission read the completed
signature.

[`omega/compilation/realization/trust-model/`](omega/compilation/realization/trust-model/README.md)
owns reconstruction and admission of trust obligations over source-free Psi
declarations and selected-provider evidence. Psi's immutable provider inventory
publishes every eligible accepted-machine declaration with exact template and
contract commitments; missing or duplicate evidence stays deferred until a
grant needs it. Package admission and final reconstruction borrow that roster.
The workspace dependency guard rejects any path from this owner to typed or
checked source. Host file writing and timing do not depend on these semantics.

`omega/packages/package-manager/` owns package operations. The independent
`source-acquisition`, `resolver-execution`, and `review-evidence` crates are its
siblings in `omega/packages/`. The [build declarations](omega/packages/build-declarations/README.md),
[build outputs](omega/packages/build-output/README.md) and
[compilation inputs](omega/packages/package-compilation/README.md) packages own
the shared project grammar and the source and output custody contract.
Compilation inputs also own source assembly: one ordered Psi 00/01 loading
loop, immutable parse checkpoints, and exact generated-source and import joins,
without running Build machines or doing resolution, typing or checking.
Compilation and realization use these input packages directly; none depends on
the package-command application or the compiler coordinator. Package
operations call compiler services, never the reverse. Review-evidence
projections read compilation products, not compiler orchestration; the
compiler uses their exact permission-consent comparison before production.
Shared process and filesystem utilities stay under `omega/foundation/`.

`psi/interpreter/` contains the `terminal-interpreter` engine.
`psi/compilation/build-time-evaluation/` keeps Build-time request data,
accounting and canonical-artifact execution inputs. The old source and
checked-program evaluators are removed. Unsupported source execution and
result elaboration refuse before any effect or result; the retained API does
not mean an ordinary Build program can run.
`psi/compilation/terminal-production/` owns accepted-source publication and
source companions below the source facade. It keeps the exact frozen source
owner from before checking, publishes the checked module and its evidence, and
attaches per-application roots; `ProductTerminalPublication::accepted` refuses a
nonempty Psi selection. Entry receipts belong to
`terminal-codec::entry_receipt`. Missing product capabilities refuse with
`no product route yet`; no intermediate Lowered Psi carrier remains. Exact
consumed-source and toolchain identities belong to the shared
`psi/foundation/source/src/source_identity.rs` owner; package review and early
Psi use the same byte protocol without importing each other's orchestration.

The lowered-form checker owns program judgments. Admission at the Terminal
receiving boundary is separate: the independent
[`terminal-installation`](psi/compilation/terminal-installation/src/lib.rs)
package admits provider selections and assumed links against an exact Terminal
artifact and owns shared structural-entry backing descriptions. It reads no
source trees, target layout, compiler driver or execution engine. Native
lowering and product records keep that sealed selection evidence; only
interpretation builds host-import dispatch from it. This library is not a
source checker or a transform stage.

The old opaque representation planner is deleted. Source preparation and
product policy refuse nonempty representation inputs until the lowered-form
route can supply their obligations. Retained application records live in
[`typed-trees::opaque_representations`](psi/compilation/representations/typed-trees/src/opaque_representations.rs);
keeping them grants no checking or layout authority.
`psi/compilation/proof-admission/` owns the mathematical kernel and checking of
supplied certificates. The Terminal-specific codec, semantic reconstruction and
work schedule packages live beside Terminal Psi under
`psi/compilation/representations/terminal-psi/`, each with its own manifest and
dependency direction. Nesting gives these companions a subject, not a program
stage. A general `psi/semantics/` bucket would hide these separate
responsibilities.

The independent `optimization-unit-semantics` checker lives at
`omega/compilation/representations/abstract-operations/optimization-unit-semantics/`,
beside the representation whose meaning and rewrites it checks. Its manifest
depends on representation and semantic libraries, not on the stages that
produce candidates, and the representation does not depend on the checker.
Placing it in the domain does not make checking IR data or justify a general
`verification/` bucket.

The compiler owns component descriptions, candidates, emitted products and
generic installation evidence. Live deployment, replacement, entry routing, era
retirement and update policy belong to an ordinary runtime or library package
or to Cathedral; no compiler runtime crate models them. A topology package's
graph policies, plan codec, verifier and installer are ordinary Omega library
code, not compiler primitives or backend realization plans.

Omega `foundation/` owns shared host primitives and native compiler vocabulary:
bounded process execution, filesystem custody, function identities and
optimization contracts and selections. These packages depend on host libraries
and Psi foundations, never on producing stages or product coordinators. Native
function identities also use the public Terminal identity vocabulary, not
program contents or an intermediate representation. Psi keeps its
source/arena/numeric and optimization-selection foundations; do not duplicate
them in Omega. Target, ABI and register descriptions and retained realization
evidence belong to their backend owners. Psi must not depend on Omega.

The settled decision `hosted-interpreter-abi-library-dependency` keeps the
interpreter and its binding-dispatch interface in Psi and the target-aware host
adapters outside Psi. The hosting application supplies those adapters through
the interpreter's explicit binding interface. Adapters may depend on
independent Omega ABI, locator and mechanism libraries, but must not pull
compiler stages, source checking or native production into interpretation.
Expose host integration through its owning API, not through application imports
of backend internals. Keep exact selected foreign identity, calling and storage
correspondence and authority checks before effects; copying the ABI catalog or
weakening admission is no substitute for this split. See [embedding](../wiki/spec/build/embedding.md#target-environments-and-execution-engines).

A coordinator forwards complete typed results; it does not own package
loading, build evaluation, stage algorithms, artifact formatting or a generic
orchestration subsystem. Allocation counters and phase-report deltas belong to
the independent host [artifacts](omega/foundation/artifacts/README.md) utility,
not program representations; it depends only on diagnostic foundations, not
semantic reports, backend encoders or producing stages.

Keep `X-to-Y`, `Y-to-Y`, `Y-to-Z` easy to follow on disk and in the executable
route. A named calculation or a crate name containing `to` does not make a
reusable invariant boundary. Add a crate only once its module boundary has
settled and it has a real independent responsibility. Do not keep umbrella or
helper crates, orphan outputs, competing successors or compatibility wrappers
just to keep an old package count.

Backend runtime-ABI carriers own descriptor field layout and accessors;
calling-conventions owns value passing. Layout and selection use those owners
instead of re-deriving offsets, sizes or ABI rules. Object sections, symbols and
relocations stay distinct from final-image layout and installation. Startup and
entry mechanics belong under backend runtime ownership when implemented, not in
placeholder crates. Direct image construction does not assume a system linker.
The optional `source/library/std` package has no compiler privilege; provider
requirements and checked adapters use ordinary explicit bindings.

A provider carries no value or state ([machines](../wiki/spec/language/machines.md#static-declarations-and-explicit-state)):
the installation adapter calls the selected satisfier machine with exactly the
requirement's arguments.

### Portable materialization and consumer ownership

`post-handoff-writer-ownership` follows the existing portable boundary: Psi
describes operations, relationships and evidence; Omega lowers them and the
interpreter interprets them. A writer's generated execution plan, private
invocation ABI, reusable fragments and byte application are consumer machinery,
not portable format just because several targets use them. Native writer
derivation and realization belong to Omega; interpretation belongs to the
interpreter and needs no native writer lowering.

A checker may share small predicates and primitives with the producer or
consumer whose output it checks, but not the decision procedure that produces
that output. Neither current crate placement nor target
independence alone settles ownership.

One named root file beside `lib.rs` defines each current representation and
leads into its actual concepts. Shared vocabulary need not invent an aggregate
program. Keep producer history in explicit replay evidence, off the path
ordinary consumers walk to get current data. See [native representation ownership](omega/compilation/representations/README.md).
Arena handles and spans are the default for durable repeated children; after
resolution source text is diagnostic/debug payload, user literals stay program
payload, and linker/display names are edge metadata. Scoped symbol-tree lookup
is the baseline; an extra lookup map needs a measured reason.

Recursive terms are flat in memory, not only on the wire. A scalar term,
proposition, proof term or any other value whose children are values of its own
kind is stored as a postorder table whose entries name earlier entries by handle,
never as a tree of nested `Box`/`Vec` children. Every walk over one, including
drop, clone and comparison, is a loop over that table, so a deep term costs
memory, not stack (the owner's ruling of 2026-11-12, extending the
[no-depth-limit ruling](../wiki/spec/build/compiler_request.md)).

Working state belongs to an explicit compilation, checking or worker
invocation. Put durable indexes beside their owning arena, and derived memos,
recursion stacks and scratch in an invocation-owned context passed by borrow.
Production checking must not hide ownership in `thread_local!`, `RefCell`,
process-global registries or invalidation by program address or fingerprint.
Partitioned workers get disjoint mutable contexts over shared immutable arenas;
runtime borrow checks do not replace an ownership structure that Rust and
Omega can both express.

When one operation may receive either an already-retained value or bytes from
an external trust boundary, make that custody explicit as borrowed or owned
input: the in-process producer lends its retained value, and byte ingress
decodes once into owned storage. Do not serialize and decode a value just to
cross an internal API. After resolution, semantic decisions use symbols,
handles, interned identities or explicit representation classes; comparing a
source or display string, such as a builtin type's spelling, is an
architecture violation even when that spelling is reserved.

Keep places, values, facts, loans, moves, drops, calls, transitions, reach and
boundary identities recognizable as their resolution changes, without forcing
one mega-IR. Parsing recognizes syntax, resolution identifies declarations,
typing fixes type and signature meaning, and checking establishes proof, flow,
ownership and effect validity. Later stages preserve, refine and realize those
judgments. Native layout neither authorizes an access nor creates a language
value.

Deterministic normalizers own published identities; proof search can change
acceptance, not interface hashes, specialization keys or compatibility meaning.
See [domain identity](../wiki/spec/language/domains.md#aliases-and-identity) and
[effect identity](../wiki/spec/language/effects.md#published-identity-and-installation-rows).
Sharing arithmetic helpers does not merge ownership of normalization and
entailment. Before merging two concepts, check that their composition,
inference and weakening laws agree beyond one familiar example; equal
projections do not prove equal semantic axes.

## Projections and replacement work

Package admission is a checked observation, not a new `Chi` stage. Its
[projection owner](omega/packages/review-evidence/README.md) reads each fact
from the earliest representation where it is semantically complete and joins
any later checked evidence it needs. Unresolved syntax and diagnostic strings
are not admission evidence. The boundary is the canonical package projection,
not the compiler-private handles used to build it; no single IR has to hold the
whole report.

When replacing code, preserve language behavior, ABI, ownership, proof,
effects, resource bounds and independent publication checks, not internal APIs
that get in the way or obsolete test shapes. A replacement may explicitly
reject unsupported cases while they are rebuilt on the common route; it must
not silently change what they mean. Reuse encoders, small primitives,
independent validators and semantic test cases where they fit. Do not restore
alternative physical routes or recursive whole-body payloads inside selected
instructions. Land coherent, verified checkpoints, not temporary breakage or
cosmetic owner moves. Machine bytes may change under a valid realization; a
purely mechanical move should keep them.

Remaining convergence and behavior work belongs on the task boards; this map
is not a second migration ledger. [Optimization](optimization.md) owns its implementation
contracts, while [compiler coordination](omega/compilation/compiler/README.md) owns
operational reports and product stopping boundaries.

## Psi implementation and deferred human audit

Human review and audit of Psi are deferred until the owner asks for them.
Implementers choose the IR, operation vocabulary, encoding and reconstruction
needed to express all accepted Omega behavior, without waiting for owner
approval or filing owner questions. Choose cohesive representations, update
their specifications and versioned schemas, and carry the change through
producers, consumers and the user's acceptance check.

Investigating compile cost is implementation work too. An old slowdown does not
set an accepted performance budget or need an owner decision before profiling.
Measure a current baseline, keep custody and source behavior, and remove
avoidable recomputation. The `custody-join-build-cost` report is not a pending
architecture approval.

Only human design review is deferred, not compiler checking or required
evidence. Keep the verifiers at [trust boundaries](#verification-placement),
rejection tests and the tests that apply, and do not declare deferred audits
complete. Preserve source-language meaning, observable behavior, trust
guarantees and the Psi/Omega firewall. Escalate missing language or trust
decisions, not how existing behavior is represented in Psi.


## Crate placement rule

Workspace crate names encode the pipeline. Internal package and folder names
omit the enclosing `omega-` or `psi-` namespace; keep the shipped `omega`
package name. Cargo names are unique across the workspace, so use descriptive
ownership names instead of repeating generic ones (Psi's `semantic-vocabulary`
and `flow-effects`, for example). Within both halves:

- Psi `foundation/` — shared vocabulary, arenas, symbols, diagnostics.
- Omega `foundation/` — shared host primitives, function identities and
  optimization contracts/selections.
- `compilation/representations/` — pipeline program families, with independent
  internal or derived forms nested under their family. Shared vocabulary,
  configuration and standalone evidence are not program stages.
- `compilation/pipeline/` — transforms only; crate names read literally as `X-to-Y`
  (`source-files-to-tokens` → `tokens-to-syntax-trees` →
  `syntax-trees-to-symbol-resolved-trees` →
  `symbol-resolved-trees-to-typed-trees` →
  `typed-trees-to-typed-trees`). Structural elaboration, lowered-form checking
  and accepted-source publication then produce the Terminal artifact; no Psi
  pass crate is in this sequence yet ([verification placement](#verification-placement)).
  Native transforms continue with `terminal-psi-to-abstract-operations` →
  `abstract-operations-to-target-operations` →
  `target-operations-to-selected-instructions` →
  `selected-instructions-to-selected-instructions` →
  `selected-instructions-to-register-homes` → image emission.
  Optimization stages use literal `X-to-X` names: for example,
  `abstract-operations-to-abstract-operations`. They consume and produce
  the same representation; do not invent a `PreOptimized`/`PostOptimized` pair.
  See [optimization phases](../wiki/spec/build/optimizations.md#phase-and-product-boundaries).
- Psi `compilation/pipeline/05_typed-trees-to-terminal-psi/` — typed structure to unsealed Terminal Psi.
- Psi `compilation/pipeline/06_terminal-psi-to-terminal-psi/` — the compilation's only program verdict. It checks the unoptimized module and, once selected passes run, their output, without knowing which passes ran. Rejecting that output is a compiler defect, not a source diagnostic.
- Psi `compilation/pipeline/04_typed-trees-to-typed-trees/` — the ordered
  preparation pass list over typed trees; a requested pass that has no
  implementation refuses.
- Psi `compilation/proof-admission/` — the independent mathematical checker.
- Terminal companion packages live under `compilation/representations/terminal-psi/`:
  `wire/`, `codec/`, `semantics/` and `work/`. `wire/` holds the canonical
  encoding and the vocabulary encoders native forms use for their identities;
  `codec/` adds the sealed module (`ClosedTerminalModule`) and the artifact
  envelope and re-exports `wire/`. No form after stage 02's preparation may
  reach `codec/` (`mbx xtask check-boundary`). Representation data and
  candidate production stay independent of checking.
- Psi `compilation/terminal-production/` — keeps the exact frozen source from
  before checking and publishes the accepted module, its evidence and its real
  companions. Omega consumers supply product policy and observation, read the
  same artifact and check exclusions against retained behavior. Missing
  producers refuse; they do not export or rebuild deleted source-checker results.
- Psi `compilation/build-time-evaluation/` — request/accounting data and canonical
  artifact execution inputs; unsupported source work refuses.
- Psi `interpreter/` — Terminal execution.
- `compiler/` — compilation coordination.
- Omega `package-manager/` — package operations and independent acquisition,
  resolver execution, and review packages.
- `backend/` — target, ABI, layout, object, linker, image.

Each stage uses the form that fits its resolution level and keeps stable links
back to the shared semantic spine. Coordinators stay boring: they run typed
phases in order and stop.

Organize the areas under each representation's root file around its actual
control flow, values, storage, calls, ownership and evidence. Do not force one
directory template onto different representations or collect unrelated types
in `model/`. Pipeline crates own transformations and private working state, not
public program structs that contain previous-stage objects. Optimization
history is explicit evidence; it must not select a different downstream
representation.


## Discoverability architecture

Every crate needs an obvious starting point whose code explains what the crate
does. In a crate that owns an operation, the domain entry file owns the
high-level flow: preparing input, ordering or dispatching phases, delegating
subordinate work, and handling results and errors. `lib.rs` wiring,
re-exports and a prose file map do not replace that orchestration.

[main.rs](omega/application/src/main.rs) shows startup and typed command
dispatch. Compiler entry points must likewise show the real stage order, and
the Build entry point its lifecycle. The current product coordinator is not
a completed example of the target architecture; see the
[migration acceptance](#build-and-compiler-ownership-migration). Judge visible
orchestration, not filenames, compact expressions or line counts.
Representation and utility crates may start with their main data structure and
its operations; do not invent an execution pipeline where there is none.

Show the crate's distinct domains beside its coordinator. A directory that
repeats the coordinator's name and holds nearly the whole crate hides those
domains instead of organizing them. The file tree shows responsibility, not the
call stack: shared grammar or policy belongs to a shared owner, not under the
first consumer that needed it. Moving directories upward is not enough; change
the actual ownership and dependency direction. This neither bans same-named
files and folders nor prescribes a layout.

Apply the same structure inside each meaningful subordinate flow. A child that
owns a multi-step operation shows its own sequencing and decisions, then
delegates narrower mechanisms below it. One good crate root with the rest
scattered among sibling folders does not meet this rule. Siblings must name
distinct responsibilities at the same level of abstraction, not hold leftover
code. Stop at cohesive leaves; this does not require a new folder or
forwarding function for every operation.

Before reorganizing, name the reader's concrete question and where navigation
fails. Afterwards, follow the changed route from the crate entry through
subordinate orchestration to the actual decision and result handling, and say
what became easier to find. Merging, moving orchestration upward, or leaving a
cohesive file alone may beat splitting it. Smaller files, more topic folders
and the same source chunks under new labels are not evidence of better
architecture.

Group tests by the behavior or contract they verify, with easy-to-find
fixtures. Splitting tests up does not by itself finish a
production-discoverability assignment. When moves change module-qualified test
names or reader paths, update the documented commands, filters and
source-reading checks, and check that the intended tests still run. Do not take
on unrelated cleanup just to produce another organization commit.
