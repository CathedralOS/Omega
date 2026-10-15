# Compiler pipeline and ownership

Psi owns source processing and target-neutral judgments through immutable
[Terminal Psi](../wiki/spec/terminal-psi/product.md). Omega consumes that product
under separately supplied realization authority. Unsupported vocabulary rejects
at its owning boundary; there is no source-shaped or assigned-program backend
fallback. Checked-tree interpretation remains a Psi reference/build-time service,
not another route to native publication.

Cathedral (the downstream OS) owns OS data structures, policies, protocols, and
lifecycle. Do not model page tables, schedulers, or drivers as compiler-owned
Rust types; if Cathedral cannot express something, name the missing general
Omega primitive or mark the slice blocked. `StateGraph` and `ControlFlowPlan`
predate the Terminal Psi cut and are **not** the public portable format.

## Connected program route

Each link below enters an actual transform owner. Private analyses and target
setup are not additional public program stages.

| Input → output | Owner |
| --- | --- |
| Source files → tokens | [source-files-to-tokens](psi/compilation/pipeline/00_source-files-to-tokens/src/lib.rs) |
| Tokens → syntax trees | [tokens-to-syntax-trees](psi/compilation/pipeline/01_tokens-to-syntax-trees/src/lib.rs) |
| Syntax → symbol-resolved trees | [syntax-trees-to-symbol-resolved-trees](psi/compilation/pipeline/02_syntax-trees-to-symbol-resolved-trees/src/lib.rs) |
| Resolved → typed trees | [symbol-resolved-trees-to-typed-trees](psi/compilation/pipeline/03_symbol-resolved-trees-to-typed-trees/src/lib.rs) |
| Typed → typed trees | [typed-trees-to-typed-trees](psi/compilation/pipeline/03b_typed-trees-to-typed-trees/README.md) |
| Typed → checked trees | [typed-trees-to-checked-trees](psi/compilation/pipeline/04_typed-trees-to-checked-trees/README.md) |
| Checked trees → lowered Psi | [checked-trees-to-lowered-psi](psi/compilation/pipeline/05_checked-trees-to-lowered-psi/README.md) |
| Lowered Psi → lowered Psi | [lowered-psi-to-lowered-psi](psi/compilation/pipeline/06_lowered-psi-to-lowered-psi/src/lib.rs) |
| Lowered Psi → Terminal Psi | [lowered-psi-to-terminal-psi](psi/compilation/pipeline/07_lowered-psi-to-terminal-psi/src/lib.rs) |
| Terminal Psi → abstract operations | [terminal-psi-to-abstract-operations](omega/compilation/pipeline/00_terminal-psi-to-abstract-operations/README.md) |
| Abstract → abstract operations | [abstract-operations-to-abstract-operations](omega/compilation/pipeline/01_abstract-operations-to-abstract-operations/src/lib.rs) |
| Abstract → target operations | [abstract-operations-to-target-operations](omega/compilation/pipeline/02_abstract-operations-to-target-operations/README.md) |
| Target operations → selected instructions | [target-operations-to-selected-instructions](omega/compilation/pipeline/03_target-operations-to-selected-instructions/README.md) |
| Selected → selected instructions | [selected-instructions-to-selected-instructions](omega/compilation/pipeline/04_selected-instructions-to-selected-instructions/src/lib.rs) |
| Selected instructions → register homes | [selected-instructions-to-register-homes](omega/compilation/pipeline/05_selected-instructions-to-register-homes/README.md) |
| Register homes → post-allocation machine | [register-homes-to-post-allocation-machine](omega/compilation/pipeline/06_register-homes-to-post-allocation-machine/src/lib.rs) |
| Post-allocation machine → selected-form encoding | [post-allocation-machine-to-selected-form-encoding](omega/compilation/pipeline/07_post-allocation-machine-to-selected-form-encoding/src/lib.rs) |
| Selected-form encoding → resolved layout | [selected-form-encoding-to-resolved-layout](omega/compilation/pipeline/08_selected-form-encoding-to-resolved-layout/src/lib.rs) |
| Resolved → resolved layout | [resolved-layout-to-resolved-layout](omega/compilation/pipeline/09_resolved-layout-to-resolved-layout/src/lib.rs) |
| Resolved program → machine bytes | [machine-emission](omega/compilation/backend/machine-emission/README.md) |
| Machine bytes → object/image evidence | [image-emission](omega/compilation/backend/images/image-emission/src/lib.rs) |

<a id="one-driver-one-pass-per-target-realization"></a>

## Two drivers, one pass, per-target realization

Independent stage crates remain directly testable, but production consumers do
not assemble them. The Psi driver feeds stages 00 through 07 once per
compilation and never observes a realization target: target-scoped machine
bodies are data checked for every target, and the Build evaluates once into
rows keyed by target
([multi-target compilation](../wiki/spec/build/configuration.md#multi-target-compilation)).
It returns one Terminal Psi artifact. The Omega driver realizes each target in
the realization set from that artifact and the target's Build rows: provider
selection and installation in stage 00, then stages 01 through 09 and image
emission.

The two drivers are the production dependency boundary around the stages. A
consumer needing source semantics calls the Psi driver with explicit source,
package and Build custody; a consumer needing machine output calls the Omega
driver with Terminal Psi plus target/realization inputs. Package management,
the application, interpreters, inspection and test runners do not depend on
stage or intermediate-representation crates and cannot invent a stage order,
skip checks or retain sidecars. Focused tools select an explicit driver stop or
inspect the driver's returned product; they are not alternate compile modes.

This limits orchestration, not access to independent libraries. Transforms
consume their declared input/output representations and use the semantic,
foundation and backend libraries that own their reusable operations. Public
Terminal artifact/view types are shared contracts for producers, realization,
interpretation and presentation. A downstream consumer needing an established
fact receives it in the preceding output; wrapping a call back into a producing
stage behind a driver does not repair that dependency.

One Psi invocation owns its arenas and lends a lifetime-bound Terminal artifact
view to immediate consumers. Publication transfers the same artifact into an
owned encoded form; external ingress decodes and admits that form once. Omega
realization and interpretation consume one common borrowed view over either the
retained or admitted-owned artifact. Same-invocation realization therefore
does not serialize, decode or clone Terminal Psi merely to cross an API.

The shipped product exposes four facades: application coordination, package
management, the two compiler drivers, and Terminal interpretation. Package
management owns acquisition, the resolved graph, review proposals/decisions and
`omega.lock`, then supplies source custody to Psi. It does not return checked
trees, settle compiler products or call native production. The interpreter owns
Terminal loading, entry establishment, installation of supplied host bindings,
execution resources and observations; it can execute an artifact
directly or ask the Psi driver to compile source to one. The application depends
on those facades and shared request/artifact contracts, never their internal
stages, representations, backend or package-compilation helpers.

The current Rust implementation does not meet this yet. Source preparation and
checked/Terminal coordination still sit outside the numbered stages. One
`CompileRequest` assembles sources, executes Build and completes authored/generated
typing and checks Product before its target loop. Targets borrow the same checked
arenas and sealed source custody, and settle providers without rewriting source
contracts or rehashing the consumed source closure. Checked-tree
pruning retains all Build-declared roots, not one target's selected entry.
Targets selecting the same Psi optimizations share Terminal production and
storage, including distinct Build entry roots. Lowering retains all authored
roots and optimization preserves them. Each application selects its exact
retained entry through source-signature custody; native and interpreted
execution do not rewrite the artifact's default entry. Nominal callback bodies
join that same portable closure and retain exact selected entries beside their
native ABI adapters. Main and callbacks borrow one provider context; callback
realization never starts another Psi production. Product constants
requiring selected-provider evaluation reject until portable recipes cross this
boundary; selected providers cannot inject completion roles into checked source.
Accepted package roles enter source preparation before checking instead.
Psi owns their declaration admission and confined nomination before matching
retained checks. Realization only rejoins the resulting source declarations to
selected provider rows. Synchronous provider-cycle checks consume Psi's checked
invocation facts rather than a second coordinator-owned inference sidecar.
Psi also owns exact provider-selection declarations and the shared decoder for
static defaults and executed Build choices. Checked Product selection returns
all authored defaults before root pruning; Omega only selects retained rows.
Resolver/typing continuation now lives in the [Psi compiler](psi/compilation/driver/src/source_compilation.rs):
one owner pairs the deferred resolver, typing arenas and evaluation obligations
across Build and generated-source resumption. Product records cannot rearrange
those stages or replace one half of the continuation. The same facade owns
preparation, retained-judgment matching and checking for both Build and Product;
product pruning consumes its checked result. Shared executable projections and
operator contract queries live in semantic checking, without a stage dependency.
The facade also owns source-frontier loading, lexing, parsing and parser-page
publication. Package assembly supplies admitted frontiers and discovers imports
from the returned syntax; generated extension units preserve separate forests.
Build role projection and dependency editing use the same Psi parsing entrance,
not independent lexer/parser pipelines.
The compiler's invocation owns prepared sources, completed Pure Builds and
exact prior language judgments across package targets and review rounds.
Package callers supply source custody and fresh grants, not frontend or proof
cache slots. A reused language judgment lends its checked arenas; equal Product
root selections share the pruning output. Package native requests complete as
one roster through the same Terminal producer as standalone requests.
Package-manager batches supply dependency-ordered custody/consent plans; the
compiler owns generated-output joins, Build/Product continuations and independent
component publication. Target plans enter together; matching post-Build root
continuations share source completion. Package operations supply requested
products and exact root permission consent, and receive reports or inspections,
not checked activations to resume. Standalone and session products share
selection, Terminal production, then installation; source checking no longer
installs the package product ahead of that sequence. Review descriptions supply
the same borrowed production facts without a second producer mode.
Package preparation advances dependency-first rounds across target rosters;
demanded component continuations enter the same source batch as their siblings,
rather than completing one target's entire dependency chain first.
Build admission and charges remain occurrence-local. Separate dependency rounds
and standalone/session preparation still prevent one complete source prefix:

| Input → output | Owner |
| --- | --- |
| Admitted source frontier → parsed syntax | [Psi source parsing](psi/compilation/driver/src/source_parsing.rs); [package source preparation](omega/packages/compilation-inputs/src/source_assembly.rs) owns import discovery and custody between frontiers |
| Assembled syntax → completed typing | [Psi source continuation](psi/compilation/driver/src/source_compilation.rs), interrupted only for admitted Build execution and generated-source admission |
| Completed typing → checked program | [Psi source checking](psi/compilation/driver/src/source_checking.rs); the product coordinator supplies source admission and seals package custody afterward |
| Checked compilation → Terminal artifact | [checked-compilation-to-terminal-artifact](omega/compilation/driver/src/terminal/terminal_artifact.rs) |

Package-owned source preparation feeds the Psi source-parsing facade;
Build evaluation does not own source discovery or assembly.
The [pipeline route items](../TASKS.md#pipeline-route) finish convergence: source
preparation supplies one source set, the Psi-owned work between stages
moves inside stages 02 through 04, and provider settlement moves behind
Terminal Psi. Source preparation remains package-owned input composition,
not a new language stage.

## Placement and semantic ownership

The owner rejected the fat-stages consolidation on 2026-09-26. Durable IRs
remain independent packages under `representations/`; transformation crates
consume and produce those representations without owning their public types.
Shared foundations and utilities retain independent library ownership. Do not
absorb them into a producing stage or the application crate to reduce crate
count or directory depth. Subsystem folders expose their coordinators and
supporting packages without merging their APIs or dependency boundaries.
Omega groups packages under four domains: `application/`, `compilation/`,
`packages/` and `foundation/`. Compiler machinery, including `pipeline/`,
`representations/`, `realization/` and `backend/`, lives within `compilation/`.
The domains are directory groupings; they do not merge package boundaries.

Psi groups its packages under `compilation/`, `interpreter/` and `foundation/`.
Source transforms, program representations, semantic checking, proof and
compile-time elaboration belong to compilation. The interpreter domain contains
only execution engines. These groupings preserve independent IR and semantic
library APIs; they do not introduce another compiler driver.


| Responsibility | Owner |
| --- | --- |
| Current program forms and their attached records/codecs | Each half's `compilation/representations/`, grouped by program family |
| Source-program judgments | Psi `compilation/checking/`; source proof planning/search in `compilation/proof/`, independent mathematical checking in `compilation/proof/admission/` |
| Terminal encoding, semantic reconstruction and fuel | Independent packages beneath Psi `compilation/representations/terminal-psi/` |
| Transformation, rewrite, private scratch and analyses | Each half's `compilation/pipeline/`, in the owning `X-to-Y` or `X-to-X` package |
| ISA, ABI, runtime carriers, object/relocation/image mechanics | Omega `compilation/backend/` |
| Shared target-neutral source/arena/numeric primitives and optimization selections | Psi `foundation/` |
| Shared host primitives, native function identities and optimization vocabulary | Omega `foundation/` |
| Application workflows and presentation | Omega `application/`, depending only on product facades and shared request/result contracts |
| Source/package custody, lock updates and review decisions | Omega `packages/package-manager/`; no checking or product production |
| Source → Terminal product sequencing | Psi compiler facade over stages 00–07 |
| Terminal + target/Build inputs → realized product sequencing | Omega compiler facade over stages 00–09 and emission |
| Requested outputs, configuration admission, retained outcomes and publication | Omega `compilation/products/` |
| Source acquisition and package storage primitives | Omega `packages/` behind package-manager requests |
| Terminal Psi loading, supplied-binding installation and execution | Psi interpreter facade under `interpreter/` |
| Target-aware interpreter host adapters | Omega-owned host integration outside Psi, reusing independent backend ABI/mechanism libraries; not a compiler driver |
| Compile-time interpretation and result elaboration | Psi `compilation/compile-time/` |
| Checked program → published Terminal artifact | Psi compiler facade; current Rust migration adapter: `omega/compilation/driver/terminal-production/` |
| Ordinary library reference implementations | `library/`, outside the compiler halves |

### Program families and support packages

Psi exposes seven successive program forms: tokens, syntax trees, resolved
trees, typed trees, checked trees, Lowered Psi and Terminal Psi. Checked fact
contexts have their own package at `psi/compilation/representations/checked-trees/facts/`.
Shared effect-summary records and pass-selection vocabulary live in
`psi/foundation/flow-effects/` and `psi/foundation/optimization-selections/`.

Omega exposes six top-level program families: abstract operations, target
operations, selected instructions, register homes, post-allocation machine and
machine code. These are not six consecutive passes. Same-form optimization
passes retain their input form; `machine-code` owns distinct selected-form
encoding, resolved-layout and final machine-code roots. The
[native representation map](omega/compilation/representations/README.md) names the actual
producer and consumer of each form.

The independent `optimization-unit` package beneath `abstract-operations/`
contains the derived CFG/SSA and replay form used during abstract optimization.
The independent `legalized-operations` package beneath `target-operations/`
contains the internal form between legalization and selection inside stage 03.
Nesting describes the family, not a new dependency on a producing stage. Preserve
these distinct forms and manifests; do not manufacture extra pipeline steps to
make folder counts match.

Target descriptions, calling conventions and register models belong to backend;
installation/task evidence belongs to backend runtime; boundary-application
companions belong to backend artifacts; provider authority evidence belongs to
backend plans. Target-independent opaque representation-selection records remain
an independent package beside their Psi admission owner at
`psi/compilation/checking/representation-planning/`. None of these is a pipeline program IR.

### Subsystem entrances and supporting packages

[`omega/compilation/products/`](omega/compilation/products/README.md)
owns invocation configuration and product eligibility, finished report payloads,
and artifact publication.
It also owns the retained evaluated-Build records and checked compilation product:
source-custody sealing, immutable stage results and their metadata/subject
projections. The Build evaluator produces this shared data contract; product
consumers do not depend on evaluator orchestration. Sealing grants neither
package admission nor native publication authority, and consumer reconstruction
remains mandatory. The checked product is a compilation result, not another IR.
The coordinator consumes validated requests and retains the target execution
loop; exact target-name set normalization belongs to the backend target catalog.
This product owner consumes artifact and custody owners without depending
on compiler orchestration or build evaluation. It is an output lifecycle, not a
new program representation or pipeline phase. The CLI publishes through it directly.

Product target families form during ordinary Psi resolution, including generated
source extensions. Typed machines retain their authored target independently of
canonical naming. Build helpers select their separately admitted execution target;
provider planning consumes typed identities, not a parallel source-name roster
carried through compilation. A canonical body is not a portable family contract.

Checked qualification evidence names its source declaration and exact boundary
requirement, not a selected provider's receipt number. Provider grant binding
borrows that evidence and returns selected plans and grants separately; it never
rewrites or clones checked Psi to stamp target-specific authority into facts.

`omega/compilation/driver/` owns only compilation orchestration.
[`omega/compilation/native-production/`](omega/compilation/native-production/README.md)
owns the source-free Terminal-to-image sequence. Its entry is
`native_compilation.rs`; realization captures runtime provisions before the
abstract, target and physical stages produce the image. Those provisions belong
to `native-artifact`, not the source compilation products. Provider evidence
contains no source-IR helpers: source-to-evidence conversion stays with the
source-aware producer, while lifetime partition normalization belongs to shared
language semantics. The native compiler's transitive production dependencies
are enforced by `python tools/check_native_boundary.py`, including optional,
build and target-specific edges. This boundary is implemented; the common Psi
prefix and package/application facade boundaries above remain unfinished.
[`omega/compilation/build-evaluation/`](omega/compilation/build-evaluation/README.md)
owns Build admission, consent and execution; it is not a new pipeline phase.
The shared package-input contracts live with the package system, not compiler
orchestration.

A root grant in `build.omg` is the root's acceptance; dependency claims are
accepted through `omega.lock`. Pipeline execution does not load or mutate a
separate admissions file.

[`omega/compilation/realization/provider-planning/`](omega/compilation/realization/provider-planning/README.md)
owns provider choices and calling, component and task planning.
[`selected-dispatch/`](omega/compilation/realization/selected-dispatch/README.md) owns selected
execution preparation and settlement.
It imports no Psi transform: the source coordinator owns stage requests and
supplies operator selections. Adapter dispatch reads the canonical backing
through a Product declaration/occurrence selection instead of cloning and
pruning a separate program. Build copies remain outside that selected frontier.
Provider grants resolve through backend provider evidence.
Product adapter dispatch and selected float executions live beside the checked
program in `SelectedExecution`; they never copy or mutate checked Psi. The
reference checked interpreter receives those explicit execution bindings.
Matching source-owner/entry/Psi-optimization requests share one Terminal
production and backing allocation, including in retained native images. Each
target independently admits the unoptimized composition and retains its own
entry receipt and native proposal. Distinct roots and callbacks are retained
in that same whole-program Terminal roster.
These remain checked-tree realization seams with package-input custody;
moving them out of the coordinator does not establish the intended Terminal-only
handoff. Callback proposal production now closes source placements into body,
ABI and argument records once; retained native re-entry borrows those records
without retaining or rejoining checked-source callback placements.
The native placement carries application/contract identity and its ABI recipe;
the source-selected machine stays with the producer that compiles its body.
Backend callback planning no longer depends on checked-tree records.
Psi identifies callback uses from their declared calling policy and retains
the exact checked entry resource receipt. Native ABI plan hashes are not
callback checking facts; Omega binds the checked entry to its evaluated calling
application without rewriting the nominal-use facts. Evaluated calling applications
belong to provider settlement, not typed trees. Schema derivation receives those
applications explicitly; package review joins the exact schema commitment to
the retained realization. Callback materialization borrows the checked program
and updates only its native application, without reopening the source owner.

Logical `Binding<R>` shapes and occurrence receipts are Psi data, independent
of selected composition mode. Provider settlement does not mutate typed trees
to authorize erasure. Omega joins the entry's exact carrier/requirement to its
selected provider and establishes runtime custody; a logical Binding marker
never grants zero-sized storage or authority. Native entry storage currently
supports Fused establishment and rejects other modes.

Physical data-layout queries must not rewrite typed source before checking.
The former target-selected `size_of`/`align_of` literal fold is removed. These
queries currently report an explicit implementation gap during source lowering;
restoration must retain a portable query and consume the exact selected layout
after the Psi boundary, including explicit target custody for constant evaluation.
Ordinary authored machines with those names remain ordinary calls.

Calling-policy evaluation retains its validated ABI application as backend-owned
output. Native entry and registrar settlement consume that application, not the
evaluator's typed handles, source spans or policy execution state. Their contract
joins do not rerun calling-plan validation. Entry signatures and Psi entry
receipts join by exact package/toolchain declaration identity, not source-arena
handles. Source coordinates and Extent layout capture stay with provider planning;
backend entry plans and image emission consume the completed signature. The
initial source-aware selection and planning remain migration work.

[`omega/compilation/realization/trust-model/`](omega/compilation/realization/trust-model/README.md)
owns trust obligation reconstruction and admission, consuming source
judgments and selected-provider evidence. The CLI supplies policy admissions;
the compiler sequences admission. Its checked-tree input remains a transitional
boundary. Host file writing and timing have no dependency on these semantics.

`omega/packages/package-manager/` owns package operations. Independent
`source-acquisition`, `resolver-execution`, and `review-evidence` crates are
siblings in `omega/packages/`. Independent [build declarations](omega/packages/build-declarations/README.md),
[build outputs](omega/packages/build-output/README.md) and
[compilation inputs](omega/packages/compilation-inputs/README.md) packages
own the shared project grammar and source/output custody contract. Compilation
inputs also own source assembly: one ordered Psi 00/01 loading loop, immutable
parse checkpoints, and exact generated-source/import joins. That preparation
does not execute Build machines or perform resolution, typing or checking.
Compilation and realization consume these input packages directly; none depends
on the package-command application or compiler coordinator. Package operations
consume compiler services, never the reverse. Independent review-evidence
projections consume compilation products, not compiler orchestration; the
compiler uses their exact permission-consent comparison before production.
Shared process and filesystem utilities remain under `omega/foundation/`.

`psi/interpreter/` contains the independent `checked-interpreter` and
`terminal-interpreter` engines. `psi/compilation/compile-time/` admits execution
through the checked interpreter and elaborates results into source and typed trees.
`omega/compilation/driver/terminal-production/` sequences Psi's checked-program
lowering, optimization and publication. Stage 05 owns input-ledger validation;
stage 07 owns entry receipts and source-correspondence judgments. Sequencing is
product-driver work, not an extra Psi stage or a second complete compiler driver;
there is no `psi/compiler/` umbrella.

`psi/compilation/checking/` owns reusable judgments, with source-program checking
separate from Terminal receiving-boundary admission. Its independent
[`terminal-installation`](psi/compilation/checking/terminal-installation/src/lib.rs)
package admits provider selections and assumed links against an exact Terminal
artifact and owns shared structural-entry backing descriptions. It consumes no
source trees, target layout, compiler driver or execution engine. Native lowering
and product records retain that sealed selection evidence; only interpretation
constructs host-import dispatch from it. This semantic library is not a source
checker or an additional transform stage.

Its independent
[`representation-planning`](psi/compilation/checking/representation-planning/README.md)
package admits source-level opaque representation relationships and checks their
transitive cleanup and copy properties. The caller supplies build authority;
backend layout derives target shape from the admitted carrier. Its independent
`selections/` package keeps consumers free of a dependency on admission.
`psi/compilation/proof/` constructs and solves source proof obligations; its
independent `admission/` package owns the mathematical kernel and
supplied-certificate checking. Terminal-specific codec, semantic reconstruction
and fuel schedule packages live beside Terminal Psi under
`psi/compilation/representations/terminal-psi/`. Each keeps its own manifest and
dependency direction. Nesting gives these companions a subject; it adds no
program stage. A general `psi/semantics/` bucket obscures these distinct
responsibilities.

The independent `optimization-unit-semantics` checker lives at
`omega/compilation/representations/abstract-operations/optimization-unit/semantics/`, beside the representation
whose meaning and rewrites it checks. Its own manifest depends on representation
and semantic libraries, not on candidate-producing stages. The representation
does not acquire a dependency on the checker. Domain-local placement does not
turn checking into IR data or justify a general `verification/` bucket.

The compiler owns component descriptions, candidates, emitted products and
generic installation evidence. Live deployment, replacement, entry routing,
era retirement and update policy belong to an ordinary runtime/library package
or Cathedral; no compiler runtime crate models them. The package-defined
topology implementation lives at `source/library/topology/`: its graph policies,
plan codec, verifier and installer are ordinary Omega library responsibilities,
not compiler primitives or backend realization plans.

Omega `foundation/` owns shared host primitives and native compiler vocabulary:
bounded process execution, filesystem custody, function identities and optimization
contracts/selections. These independent packages depend on host libraries and
Psi foundations, never on producing stages or product coordinators. Native
function identities also consume the public Terminal identity vocabulary,
not program contents or an intermediate representation.
Psi retains its source/arena/numeric and optimization-selection foundations; do
not duplicate them in Omega. Target/ABI/register descriptions and retained
realization evidence belong to their backend owners. Psi must not depend on Omega.

The settled decision `hosted-interpreter-abi-library-dependency` keeps the
interpreter and its binding-dispatch interface in Psi, and target-aware host
adapters outside Psi. The hosting application supplies those adapters through
the interpreter's explicit binding interface. Adapters may depend on independent
Omega ABI, locator and mechanism libraries; they do not pull compiler stages,
source checking or native production into interpretation. Expose host integration
through its owning API, not application imports of backend internals. Preserve
exact selected foreign identity, calling/storage correspondence and authority
checks before effects; neither copying the ABI catalog nor weakening admission
is an alternative to the ownership split. See [embedding](../wiki/spec/build/embedding.md#target-environments-and-execution-engines).

A coordinator forwards complete typed
results rather than owning package loading, build evaluation, stage algorithms,
artifact formatting, or a generic orchestration subsystem. Allocation counters
and phase-report deltas belong to the independent host
[artifacts](omega/foundation/artifacts/README.md) utility, not program
representations. It depends only on diagnostic foundations, not semantic
reports, backend encoders or producing stages.

Keep `X-to-Y`, `Y-to-Y`, `Y-to-Z` followable on disk and in the executable route.
A named calculation or a crate name containing `to` does not establish a reusable
invariant boundary. Add a crate only when its module boundary has settled and it
has a real independent responsibility. Do not preserve umbrella/helper crates,
orphan outputs, competing successors, or compatibility wrappers merely to retain
an old package count.

Backend runtime-ABI carriers own descriptor field layout and accessors;
calling-conventions owns value passing. Layout and selection consume those
owners rather than re-derive offsets, sizes or ABI rules. Object sections,
symbols and relocations remain distinct from final-image layout and installation.
Startup/entry mechanics belong under backend runtime ownership when implemented,
not in placeholder crates. Direct image construction does not assume a system
linker. The optional `source/library/std` package has no compiler privilege;
provider requirements and checked adapters use ordinary explicit bindings.

A provider carries no value or state ([machines](../wiki/spec/language/machines.md#static-declarations-and-explicit-state)):
the installation adapter calls the selected satisfier machine with exactly the
requirement's arguments, and the checked interpreter does the same. After
normal package review, the executable regression is:

```sh
omega run --both tests/omega/pass/providers/runtime_boundary_primitive_array_argument_exit/main.omg
```

On Linux x86_64 it returns native and interpreter exit 70, preserving the array
argument between scalar arguments.

### Portable materialization and consumer ownership

`post-handoff-writer-ownership` follows the existing portable boundary: Psi
describes operations, relationships, and evidence; Omega consumes and lowers
them, while the interpreter consumes and interprets them. A writer's generated
execution plan, private invocation ABI, reusable fragments, and byte application
are consumer machinery, not portable format merely because several targets use
them. Native writer derivation and realization belong to Omega; interpretation
belongs to the interpreter, without requiring native writer lowering.

Portable validity checking may share small semantic predicates with consumers,
but not the output-producing derivation it must independently check. Neither
current crate placement nor target independence alone establishes ownership.

One named root beside `lib.rs` defines each current representation and leads into
its actual concepts. Shared vocabulary need not invent an aggregate program.
Keep producer history in explicit replay evidence, not the path ordinary consumers
walk to obtain current data. See [native representation ownership](omega/compilation/representations/README.md).
Arena handles and spans are the default for durable repeated children; source
text is diagnostic/debug payload after resolution, user literals remain program
payload, and linker/display names are edge metadata. Scoped symbol-tree lookup
is the baseline; extra lookup maps require a measured reason.

Working state belongs to an explicit compilation, checking or worker invocation.
Put durable indexes beside their owning arena and derived memos, recursion
stacks and scratch in an invocation-owned context passed by borrow. Production
checking must not hide ownership in `thread_local!`, `RefCell`, process-global
registries or program-address/fingerprint invalidation. Partitioned workers get
disjoint mutable contexts over shared immutable arenas; runtime borrow checks
are not a substitute for an ownership structure Rust and Omega can both express.

When one operation may receive an already-retained value or bytes from an
external trust boundary, represent that custody explicitly as borrowed/owned
input: the in-process producer lends its retained value, while the byte ingress
decodes once into owned storage. Do not serialize and decode a value merely to
cross an internal API. After resolution, semantic decisions use symbols,
handles, interned identities or explicit representation classes; comparing a
source/display string such as a builtin type's spelling is an architecture
violation even when that spelling is reserved.

Preserve recognizable places, values, facts, loans, moves, drops, calls,
transitions, reach and boundary identities as their resolution changes, without
forcing one mega-IR. Parsing recognizes syntax; resolution identifies declarations;
typing fixes type/signature meaning; checking establishes proof, flow, ownership
and effect validity. Later stages preserve, refine and realize those judgments.
Native layout does not authorize an access or create a language value.

Deterministic normalizers own published identities; proof search can change
acceptance, not interface hashes, specialization keys or compatibility meaning.
See [domain identity](../wiki/spec/language/domains.md#aliases-and-identity) and
[effect identity](../wiki/spec/language/effects.md#published-identity-and-installation-rows).
Shared arithmetic helpers do not merge normalization and entailment ownership.
Before collapsing concepts, require their composition, inference and weakening
laws to agree beyond one familiar example; equal projections do not prove equal
semantic axes.

## Projections and replacement work

Package admission is a checked observation, not a new `Chi` stage. Its
[projection owner](omega/packages/review-evidence/README.md) reads each fact at the
earliest representation where it is semantically complete and joins whatever
later checked evidence it needs. Unresolved syntax and diagnostic strings are
not admission evidence. The canonical package projection is the boundary, not
the compiler-private handles used to derive it; no single IR must contain the
whole report.

Preserve language behavior, ABI, ownership, proof, effects, resource bounds and
independent publication checks, not obstructive internal APIs or obsolete test
shapes. A replacement may explicitly reject unsupported cases while they are
rebuilt in the common route; it must not silently change their meaning. Salvage
encoders, small primitives, independent validators and semantic test cases when
they fit. Do not restore alternate physical routes or recursive whole-body
payloads inside selected instructions. Land coherent verified checkpoints, not
unverified temporary breakage or cosmetic owner moves. Machine bytes may change
under a valid realization; a purely mechanical move should preserve them.

Producer and checker may share small predicates and primitives, not the
output-producing decision procedure the checker is meant to validate. Remaining
convergence and behavior work belongs to execution boards; this map is not a
second migration ledger. [Optimization](optimization.md) owns its implementation
contracts, while [compiler coordination](omega/compilation/driver/README.md) owns
operational reports and product stopping boundaries.

## Psi implementation and deferred human audit

Until the owner requests otherwise, human review and audit of Psi are deferred.
Implementers own the IR, operation vocabulary, encoding,
and reconstruction choices needed to express all accepted Omega behavior. Do not
block those choices on owner approval or add owner questions for them. Choose
cohesive representations, update their specifications and versioned schemas, and
carry the change through producers, consumers, and customer acceptance.

Compilation-cost investigation is also implementation work. A historical
slowdown does not establish an accepted performance envelope or require an owner
decision before profiling. Measure a current baseline, preserve custody and
source behavior, and remove avoidable recomputation. The named
`custody-join-build-cost` report is tracked by `REFERENCE-CUSTODY-JOIN-SCALING`
on the execution board; it is not a pending architecture approval.

This defers human design review, not compiler checking or required evidence.
Keep the verifiers that run at [trust boundaries](#verification-placement),
rejection controls, and applicable tests; do not declare deferred audits
complete. Preserve source-language meaning, observable
behavior, trust guarantees, and the Psi/Omega firewall. Escalate genuinely missing
language or trust decisions, not how existing behavior is represented in Psi.


## Crate placement rule

Workspace crate names encode the pipeline. Within both halves:

Internal package and folder names omit the enclosing `omega-` or `psi-`
namespace. Keep the shipped `omega` package name. Cargo names are unique across
the workspace; use descriptive ownership names rather than duplicate generic
names (Psi's `semantic-vocabulary` and `flow-effects`, for example).

- Psi `foundation/` — shared vocabulary, arenas, symbols, diagnostics.
- Omega `foundation/` — shared host primitives, function identities and
  optimization contracts/selections.
- `compilation/representations/` — pipeline program families, with independent internal or
  derived forms nested beneath the corresponding family. Shared vocabulary,
  configuration and standalone evidence are not additional program stages.
- `compilation/pipeline/` — transforms only; crate names read literally as `X-to-Y`
  (`source-files-to-tokens` → `tokens-to-syntax-trees` →
  `syntax-trees-to-symbol-resolved-trees` →
  `symbol-resolved-trees-to-typed-trees` →
  `typed-trees-to-typed-trees` → `typed-trees-to-checked-trees` →
  `checked-trees-to-lowered-psi` →
  `lowered-psi-to-lowered-psi` → `lowered-psi-to-terminal-psi`, then
  `terminal-psi-to-abstract-operations` →
  `abstract-operations-to-target-operations` →
  `target-operations-to-selected-instructions` →
  `selected-instructions-to-selected-instructions` →
  `selected-instructions-to-register-homes` → image emission).
  Optimization stages use literal `X-to-X` names: for example,
  `abstract-operations-to-abstract-operations`. They consume and produce
  the same representation; do not invent a `PreOptimized`/`PostOptimized` pair.
  The folders must expose the connected `X-to-Y`, `Y-to-Y`, `Y-to-Z` sequence,
  not merely name individually plausible calculations. See [optimization phases](../wiki/spec/build/optimizations.md#phase-and-product-boundaries).
- Psi `compilation/checking/` — reusable source-program judgments.
- Psi `compilation/proof/` — source obligation construction and proof search; independent
  `compilation/proof/admission/` owns the shared mathematical checker.
- Terminal companion packages live beneath `compilation/representations/terminal-psi/`:
  `codec/`, `semantics/` and `fuel/`.
  Representation data and candidate production remain independent of checking.
- Psi `compilation/terminal-production/` — sequences stages 05–07 through
  publication. Omega `compilation/driver/terminal-production/` supplies product
  policy and reporting; it does not assemble the stages. Exclusion admission
  borrows the unoptimized Terminal composition inside that sequence, without
  exporting or cloning its checked-source lowering.
- Psi `compilation/compile-time/` — evaluation admission and result elaboration.
- Psi `interpreter/` — checked and Terminal execution engines.
- `compiler/` — compilation coordination.
- Omega `package-manager/` — package operations and independent acquisition,
  resolver execution, and review packages.
- `backend/` — target, ABI, layout, object, linker, image.

Concepts stay visible across stages without being forced into one mega-IR: each
stage uses the form matching its resolution level while keeping stable links
back to the shared semantic spine. Coordinators stay boring — sequence typed
phases and stop. Do not add a crate until a module boundary has stopped moving.

Each program representation has one named root file beside `lib.rs`; the root
defines the current program and leads into subordinate concept-owned areas.
Organize those areas around the representation's actual control flow, values,
storage, calls, ownership, and evidence. Do not force an identical directory
template onto different representations or collect unrelated types in `model/`.
Pipeline crates own transformations and private working state, not public
program structs containing previous stage objects. Optimization history is
explicit evidence; it must not select a different downstream representation.


## Discoverability architecture

Every crate must have an obvious starting point that explains its responsibility
through code. For an operation-owning crate, its domain entry file owns the
high-level flow: input preparation, phase ordering or dispatch, subordinate work,
and result/error handling. `lib.rs` wiring, re-exports, and a prose file map do
not substitute for that orchestration.

Use [main.rs](omega/application/cli/src/main.rs) and
[compiler.rs](omega/compilation/driver/src/compiler.rs) as the gold
standard: the former shows startup and typed invocation dispatch; the latter
shows shared preparation, product selection, per-target realization, and outcomes.
Copy their visible orchestration principle, not their filenames or line counts.
Representation and utility crates may start with their principal data structure
and cohesive operations; do not invent an execution pipeline where none exists.

Make the crate's distinct domains visible beside its coordinator. A directory
that merely repeats the coordinator's name and contains essentially the entire
crate hides those domains rather than organizing them. The file tree represents
responsibility, not the call stack: shared grammar or policy belongs to a shared
owner, not beneath the first consumer that needed it. Moving directories upward
alone is insufficient; update the actual ownership and dependency direction.
This is not a blanket ban on same-named files and folders or a prescribed layout.

Apply the same structure recursively within each meaningful subordinate flow.
A child owning a multi-step operation must expose its own sequencing and decisions,
then delegate narrower mechanisms beneath that owner. One good crate root with
all remaining work scattered among sibling folders does not satisfy this rule.
Siblings must name distinct responsibilities at the same level of abstraction,
not become catch-all collections of leftover code. Stop at cohesive leaves;
recursive discoverability does not require a new folder or forwarding function
for every operation.

Before an organization change, name the reader's concrete question and the
current navigation failure. Afterward, follow the changed route from crate entry
through subordinate orchestration to the actual decision and result handling.
Explain what became easier to find. Consolidation, moving orchestration upward,
or leaving a cohesive file intact may be better than splitting it. Smaller files,
more topic folders, and unchanged contiguous source chunks with new labels are
not evidence of improved architecture.

Keep tests grouped by the behavior or contract they verify, with discoverable
fixtures. Test partitioning alone does not complete a production-discoverability
assignment. When moves change module-qualified test names or reader paths, update
their documented commands, filters, and source-reading checks and verify that the
intended tests are still selected. Do not expand into unrelated cleanup merely to
produce another organization commit.


## Compositional lowering

Reinforcing required source-shape admission is prohibited, not a cleanup
preference. Clean the touched mechanism as you go: generalize its operation or
explicit data relationship and delete the superseded path in the same change.
Another task promising later consolidation is not an exception. If the general
repair is blocked, identify that dependency rather than publishing another
required special case. This does not require an unrelated whole-pipeline rewrite.

Supported assignment, call, branch,
and transfer operations must compose under their actual type, effect, ownership,
and control-flow rules. Do not add another required producer family for an
incidental arrangement such as "locals before calls", "call immediately followed
by case dispatch", or "the same body with an extra statement". Replace the
restrictive recognizer or duplicated planner with ordinary operation sequencing
and explicit value/storage/control relationships. Clean X-to-Y crate names do
not make a pattern-specific implementation compositional.

Semantic distinctions, ABI constraints, missing proof obligations, and explicit
implementation limits remain real. Never remove their rejection checks merely
to admit more shapes. Specialized pattern matching is appropriate for optional
optimizations with a correct general path, not as the only way to compile an
ordinary combination.

When a customer exposes this pattern, identify the general operation or join that
is missing and remove the superseded special path as that repair lands. Before
propagating a new representation or lowering mechanism across stages, choose a
small valid variation that challenges its assumptions and a relevant invalid
control. Check that the design supports the valid case and rejects the invalid
one through independent checking. Implement and exercise them early, not only
after the original fixture passes.
For a borrowed call, surrounding computations and conflicting-access rejection
can expose a bad design without a Cartesian matrix of source permutations.

Parallel implementation of additional variants should follow one working,
independently checked route through the new mechanism. Do not replicate an
unproven representation across several implementations first. Independent review,
tests, and work using established contracts can still proceed in parallel.

Keep the actual customer's command as the outer acceptance check; a passing
isolated helper does not close an unchanged application failure. Do not expand
the task board with one item per permutation, or treat this rule as permission
for an unrelated whole-IR rewrite.

## Verification placement

Verification runs at trust boundaries:

- a new or changed foreign package is verified when it is reviewed;
- an untrusted build script is confined and checked while it executes.

Proof-carrying code is removed; see [PCC removal](../TASKS.md#pcc-removal).

A stage does not re-validate, replay or rebuild output that the same compile
just produced. The producer and its checker would be the same code running in
the same process on the same input, so the second pass cannot find anything
the first missed, and every such pass costs compile time and a second copy of
the stage's rules. A plan that violates its contract is a bug in the stage
that produced it, for the corpus and the tests to find.

The verifiers themselves stay. The corpus runner and the tests exercise them;
the default compile path does not call them.
