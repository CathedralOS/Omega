# One coordinated migration to the linear compiler

This plan replaces the foundation and backend architecture in **one coordinated
change set with one cutover**. It does not propose a sequence of small migration
PRs. Source settlement, provider selection, generated code, physical resources,
representations, runtimes and validators must agree before the new route replaces
the old one. Internal work has dependencies and can be divided among agents;
partial routes are not separately rolled out.

The current task is **plan first**. No compiler migration is implemented by this
document. The six [owner rulings](../rulings.md) remain authoritative and unchanged.
The destination below is a concrete recommendation applying them. Proposed
boundaries and names are **judgment**; feasibility obligations are **unverified**
until the named evidence is supplied. Those obligations must be settled in this
plan before implementation starts. A new folder name is not evidence that a
contract is possible.

## 1. Scope, evidence and the single cutover

The change covers the Rust compiler from source input through accepted Psi,
native planning, code generation, object/image construction and publication,
including the consumers affected by dissolving foundation and backend. It also
updates their documentation, boundary rules, tests, recovery codecs and public
facades together. Package acquisition, live interpreter execution and filesystem
publication remain real owners outside the transformation pipeline; they may
supply inputs or consume outputs, not perform hidden compiler stages.

The Omega-written compiler and bootstrap chain are [separate implementations](../subsystems/bootstrap.md).
This plan does not claim to rewrite them incidentally. Their dependencies,
commands and artifact contracts must still work after the Rust compiler cutover.

| Evidence at compiler snapshot `3f9ce43f0c9d179048911d18faa91ba93f60176d` | Count |
| --- | ---: |
| Psi + Omega foundation | 26 packages; 96,223 tracked Rust lines |
| Backend | 29 packages; 223,507 tracked Rust lines |
| Realization | 5 packages; 36,409 tracked Rust lines |
| Workspace metadata | 139 packages; 1,479 path dependency declarations |

[Counts](../evidence/data/counts.json), [graph](../evidence/data/graph.json),
[usage](../evidence/data/usage.json) and [backend usage](../evidence/data/usage_be.json)
are pinned by the [manifest](../evidence/data/manifest.json). Counts include tests;
`src_rs_lines` is a path scope, not production-only code. The backend has 221,538
lines under `src/`. Dependencies are declarations: 1,318 normal, 1 build and
160 dev. Neither a Cargo edge nor a text reference proves a production call.
“Unobserved” is not “unused”; removing an edge requires source and configuration
checks. The scanners now include the real backend `target/` crate.

This document was rewritten on atlas content base
`c6cae034e48d8725927f9a7df0582d28266ae154`. Unless stated otherwise, source links
below describe the older compiler snapshot, not moving `main`. One known delta
matters: by that content base, the native driver supplied a
[stage-audit object](https://github.com/CathedralOS/Omega/blob/c6cae034e48d8725927f9a7df0582d28266ae154/omega-rust/omega/compilation/native-compiler/src/stage_audit.rs#L1)
to machine emission. That relocates calls but does not establish a data-only
handoff. [EmissionFacts still retains the earlier program](https://github.com/CathedralOS/Omega/blob/c6cae034e48d8725927f9a7df0582d28266ae154/omega-rust/omega/compilation/native-compiler/src/pipeline.rs#L278).
Implementation planning must reconcile every inventory row against its chosen
base once; it must not reintroduce work already removed since the snapshot.

A single cutover means all production entry points switch to the completed new
contracts together, old paths and temporary adapters are deleted in that change
set, and the full boundary checks are enabled when it lands. Development commits
and parallel branches are permitted working material; none is an independently
shipped migration milestone. If final validation fails, the old released route
remains in place. This is not a runtime fallback between two compilers.

## 2. What the observed code requires us to solve

The [Psi](../subsystems/psi.md) and [Omega](../subsystems/omega.md) maps retain the
full snapshot walkthroughs. These observations determine the new contracts:

| Observed mechanism | Evidence | Consequence for the complete design |
| --- | --- | --- |
| Source continuation retains seeded typing, deferred bodies and pending evaluations; selected bodies alternate resolution and typing. | [State][source-state], [loop][source-loop] | Preserving all old Psi boundaries and promising one global pass is not credible. Source settlement needs an explicit internal closure. |
| Source preparation evaluates layout/access/const work; an evaluation job itself prepares, lowers and checks an invocation. | [Preparation][source-prepare], [job][source-job] | Source-job preparation belongs to settlement; execution receives only an accepted artifact invocation. |
| Prepared domain applications are computed inside typed-tree methods. | [Producer][prepared-applications], [consumer][application-consumer] | Move computation, not just the record definition. Later lowering must consume the prepared theory and reject missing/open results. |
| Declaration selections already pass through typed trees. | [Transfer][ledger-transfer], [read][ledger-read] | Preserve that handoff; moving a type out of foundation does not fix an imaginary bypass. |
| Provider demand grows through selected exact bodies; policy preparation still takes checked source custody. | [Closure][closure-loop], [input][policy-input] | Close logical provider/body discovery before publishing the source result; later native work must not request source activation. |
| Native proposals perform provider specialization, task realization and ownership checks. | [Constructor][proposal] | A representation constructor cannot remain an unnumbered producer under a new name. |
| The root pauses after target lowering while handlers are compiled to objects; their requirements are then bound before selection. | [Root route][object-route], [handlers][handler-demand] | Replace this interleaving with explicit whole-program dependencies and resource records. |
| Installed requirements include transitive calls, bridge storage, call bias and fault premises. | [Requirement][stack], [call graph][transitive-stack], [fault analysis][fault-premises] | A frame-size table alone is insufficient. |
| Entry transport selects registers, allocates frames and emits instructions after a body object exists. | [Creation][entry-generation], [body effects][entry-input], [encoding][bridge-encoding] | All generated bodies must enter the physical program before common encoding; image writing cannot remain a second code generator. |
| Callback reach is currently recovered from emitted root/child object references. | [Demand walk][callback-demand] | An early roster must preserve pruning; available bodies cannot automatically become native obligations. |
| Emission receives an earlier abstract program and lowering facts alongside the machine form. | [EmissionFacts][emission-facts] | Carry justified facts in current forms; neither an Arc nor an opaque callback repairs earlier-program access. |
| Explicit replay needs retained originals; runtime provision capture can itself create foreign adapters. | [Audit][audit], [adapter][foreign-adapter] | Verification and runtime helpers need complete routes in the new architecture, not exemptions discovered after moving crates. |

## 3. Recommended destination and stage contracts

**Proposed · unverified contracts.** This figure states the intended whole-program
handoffs for one coordinated cutover. The table below supplies their invariants;
§8 names the feasibility work that must finish before implementation.

```mermaid
flowchart LR
  A[Admitted source bytes and requests]
  B[Source settlement: lazy source and evaluation closure]
  C[Accepted canonical Psi]
  D[Native program planning: retained-body closure]
  E[Physical program planning: all generated bodies]
  F[Installation and resource binding]
  G[Common encoding]
  H[Layout and relaxation]
  I[Object construction]
  J[Image construction]
  K[Product publication]
  A -.-> B
  B -.-> C
  C -.-> D
  D -.-> E
  E -.-> F
  F -.-> G
  G -.-> H
  H -.-> I
  I -.-> J
  J -.-> K
```

Independent verification is a separate consumer of current forms and the
originals its request explicitly retains. It does not produce program meaning.

**Judgment:** keep the public transformation boundaries where a complete output
can be stated without asking a previous phase for more work. Coarsen the current
boundaries that participate in an actual dependency loop. Keep their algorithms
as named, separately testable private operations; do not preserve obsolete
public stage APIs behind a coordinator wrapper.

The names below describe contracts, not final directories or numbers. Assign
numbers after the feasibility work in §8 establishes these boundaries. Current
Psi/Omega numbers are used only to locate existing code.

Every program representation contains its identity, provenance required by its
consumers, selected target when applicable, current body membership and all
facts needed next. Immutable leaf definitions may be shared. A phase receives
its declared predecessor and admitted inputs, such as native planning's accepted
code catalog. It must not retain an older executable graph beside its current
input to recover missing facts, or receive a source lookup capability, mutable
Build context or callback into another producer. Explicit verification may read
the originals its request declares. Private worklists may contain intermediate
forms while their owning phase runs; they are not hidden inter-stage products.

| Phase / owner | Complete input | Output | Required invariant |
| --- | --- | --- | --- |
| Source acquisition and admission — packages/Build, before the compiler | Requested source/package roots, permissions and explicit Build input | Immutable admitted source-byte/custody reservoir and requests | Availability and authorization are recorded; acquisition does not imply semantic acceptance or activation of every file. |
| Source settlement — Psi producer | That reservoir, scoped roots, neutral Build selection records, admitted observations and budgets | Accepted target-neutral program, closed logical provider/body selections, prepared application facts and narrow provenance | All source and logical-body discovery for this result is closed; only the lowered-form checker can accept it; dormant availability is not a new obligation. |
| Portable sealing — Psi representation producer | Accepted program and its publication policy | Canonical Psi artifact, identity, acceptance/evidence and required summaries | Encoding cannot confer semantic acceptance or host execution authority. No live source owner escapes in the artifact. |
| Native program planning — Omega producer | Canonical accepted code catalog, exact logical selections, selected target description, component admissions and native request | Selected native program with explicit retained-body references, call/ABI plans, obligations and generated-body recipes | Native work is source-free. It closes target demand and optimization-related body reach over already accepted code; it cannot select a new source body or silently change a logical provider. |
| Physical program planning — Omega producer | Selected native program and immutable target environment | Complete physical program including ordinary bodies, required adapters/entries/handlers, register choices, frame recipes, effects and symbolic resource uses | All executable bodies and their effects are represented. Generated code is program data, not a later writer's private byte sequence. |
| Installation/resource binding — Omega producer | Complete physical program, admitted installations and complete resource summaries | Bound physical program | Transitive stack demands, bridge storage, call bias, fault premises and placement identities agree; binding cannot invalidate allocation, effects or frame choices. |
| Common encoding — Omega producer | Bound physical program | Encoded bodies/fragments, all legal alternatives, frame encodings and complete fixup descriptors | Every executable byte generated by this compilation has this producer, including entry/foreign/writer helpers. Imported pre-encoded components remain explicit admitted inputs. No new body may appear downstream. |
| Layout and relaxation — Omega producer | Encoded program, target placement rules and relocations | Laid-out text/fragments with resolved internal references | Choose/patch carried alternatives only. Preserve existing body/frame relaxation semantics and checked range/alignment rules. |
| Object construction — Omega producer | Laid-out program, symbols and object-format request | Object plus attribution, entry and evidence records | No source/ABI rediscovery or helper generation; composition uses declared, already encoded bodies or admitted component objects. |
| Image construction — Omega producer | Objects, container request, external binding/loader inputs and carried entry plans | Final image, regions, symbols, relocation evidence and runtime/provision carriers | Container placement, relocation, signing/format metadata only; no fresh register, frame or instruction decisions. |
| Publication — products/host support | Completed retained product and output request | Installed files/packages and receipts | File installation does not invoke compiler stages; failure retains the existing publication guarantees. |
| Independent verification — explicit consumer route | Current form plus the exact optional originals/evidence required by that validator | Verification result/diagnostics | Does not choose providers or produce program meaning. Missing required replay inputs refuse the requested audit. |

Native planning absorbs the current early realization/Omega 00–04 operations
that must cooperate to close retained-body demand. Physical planning owns
allocation, physical alternatives, frames and generated-body construction.
This is a recommended starting boundary, **not a proved claim** that all current
optimizations can close reach before encoding. The proof and adjustments are
part of §8; a downstream discovery cannot be excused as “just a helper.”

## 4. Source settlement, evaluation and provider-body closure

### Why the old source stage boundaries change

The recommendation is to replace the externally sequenced Psi 00–06 source
route with one source-settlement transition. Source acquisition remains outside
it; canonical publication and all native work remain after it. Its private
operations retain useful names and tests: lexical/syntax preparation, name and
body resolution, typing, application preparation, structural lowering,
obligation generation, proof search and independent certificate admission.
The lowered-form checker remains the sole semantic verdict. Combining ownership
does not merge proof search with its checking authority or weaken a refusal.

Lexing/parsing must be demand-aware too. Settlement starts with admitted bytes,
not an eagerly parsed forest of all available files. It activates a unit only
under a recorded root/dependency request, lazily prepares its syntax and retains
that result in its private state. This avoids both a callback into an earlier
numbered parser and new errors from unused files. The exact activation/error
order is a compatibility obligation, not an opportunity to parse more input.

The private state owns admitted source custody, active scope/bindings, syntax,
resolved and typed rows, prepared applications, pending evaluation jobs,
accepted invocation artifacts and charging records. A work item names its
inputs, requester and unresolved dependency. Completing it publishes a new
immutable result for dependants; it does not mutate an already admitted answer.
Equivalent preparation requests may share immutable code and established facts.
Invocation deduplication is allowed only where the proved request identity and
existing accounting contract permit it. Distinct deployments, sponsors and
actual invocations retain their required execution and receipts; the current
[invocation accounting][invocation-accounting] is not blanket memoization.
A dependency cycle or exhausted budget follows the existing refusal semantics;
there is no guessed constant, default provider or ambient host observation.

The old numbered source crates cease being callable stages. Their useful
algorithms move into this owner or independent checking packages. Keeping the
same stage calls inside a new `settle()` wrapper is specifically not completion.
The scope is bounded by one target-neutral source result; package acquisition,
process execution, native lowering and file publication are not absorbed into
an all-purpose compiler stage.

### Evaluation is an explicit operation on accepted code

Settlement constructs, lowers and checks the exact required invocation closure,
then gives the executor immutable accepted code plus sealed invocation admission.
That admission binds caller, entry, arguments, target/supplies, permitted
observations and work account. It is the result of source-consent checks, not
`InvocationSourceConsent`, a source owner or a lookup callback. The executor
validates the binding, invokes the interpreter and returns values/status with
invocation-bound observation and work receipts. Settlement alone decides how
those values update source preparation.

The current [source-job preparation][source-job] and
[retained-product preparation][product-preparation] move to settlement.
The actual [invocation engine][source-job-execution] remains an execution service.
Simply making `build-time-evaluation` depend on `source-compiler` creates a
cycle because the reverse [dependency already exists][source-dependency]. Both
APIs must be replaced together in this migration.

### Close logical providers without violating the firewall

Available declarations are not roots. Settlement receives admitted Build
selection/binding descriptions in Psi-neutral data: exact declaration IDs,
source scope, target keys and language-level requirements. It owns discovery
and preparation of each selected provider body and its transitive source
dependencies. The output contains the exact logical selections, required body
catalog and their acceptance/provenance. Nothing later can request another
source file, activate a declaration or rejoin a source `SymbolTable`.

Omega then evaluates/validates physical ABI plans over those closed identities.
Failure cannot trigger an alternative logical provider or reopen settlement.
The current [closure loop][closure-loop] and [calling-policy inputs][policy-input]
must therefore be split by meaning, not moved wholesale before a renamed O00.
Calling-policy code needed later must already be an admitted immutable artifact.

**Unverified planning obligation:** demonstrate that logical provider choice can
be separated from target ABI realization with the same choices and refusals.
Where a choice requires a target observation, state the neutral admitted input
and its owner explicitly. Passing an Omega ABI object into Psi, hiding a call
behind a trait, or preparing all candidate source bodies is not an answer. If
this separation cannot preserve behavior, revise the phase contract before
implementation; do not ship a backedge or quietly accept fewer programs.

## 5. Whole-program reach, generated code and resources

### Three body sets, with different obligations

| Set | Meaning | What membership does not imply |
| --- | --- | --- |
| Available | Admitted source or accepted code that a valid request could select. Source availability and native code availability are separate catalogs. | It does not demand typing, proof, ABI realization, native support or emission merely by existing. |
| Demanded | A recorded semantic/native dependency of a selected root, with the requester and selected binding. | It does not guarantee the body survives allowed optimization, nor permit a different source selection. |
| Retained | A body/reference still required after the relevant optimization and closure step. | It does not let later phases invent another body or erase its obligations. |

Source settlement closes the source decisions. Native planning closes the
native retained set over accepted code. An accepted body can be available for
native planning without yet requiring target lowering; this is necessary to
preserve current pruning. The [current callback walk][callback-demand] uses
emitted object references, so “all bodies up front” is not a sufficient design.

Native planning must derive explicit reference edges from the current program
and keep their relationship to optimization. Each newly retained native body
is processed within that phase's worklist. Changes to an already settled
logical binding refuse. Work/identity records distinguish shared processing
from repeated execution. At phase exit, a certificate or independently
checkable invariant identifies the closed native body/reference set.

**Unverified:** prove that all later encoding/layout references are a projection
of this set. If an existing optimization changes whether a callback is needed,
its demand-changing part belongs before closure, or the planning boundary must
move. It may not survive as an image-time discovery followed by a recursive
compiler invocation. Preserve harmless unavailable target support in bodies
that the old route prunes; a larger conservative native roster can change
acceptance even when its final bytes omit those bodies.

### Generated bodies join the program before encoding

Entries, foreign adapters, callback transports, interrupt entries, stack-switch
helpers, floating-control/hosted sequences and any live post-handoff writers
must have explicit identities, inputs, effects, calls and resource contracts.
Native planning declares their recipes; physical planning instantiates them
using carried ABI plans and actual body effects. Fixed target byte templates
are allowed only as fully described encoding inputs, with effects and fixups;
they cannot hide an instruction-selection algorithm in target description.

This replaces [entry generation from a completed object][entry-generation].
Today its [request obtains body effects][entry-input], then the bridge chooses
[scratch registers][bridge-registers] and [frame/encoding details][bridge-encoding].
Those dependencies must be exposed in the physical program. Image construction
may resolve addresses or format relocations, but may not call these generators.
The old suggestion to place their whole implementations in an O14 writer is
withdrawn.

The [post-handoff plan][writer] already distinguishes address-free transfer
steps from invocation relocations. Classify actual callers before claiming a
production route: an exported library function or its tests alone do not prove
that image construction uses it. Where required, its transfer program follows
the common encoding route; otherwise retire or retain it only with an evidenced
non-production owner.

### Binding uses a complete resource summary

The current [stack requirement][stack] contains body demand, bridge bytes,
call-stack bias, alignment, entry plan and fault premises. [Transitive demand][transitive-stack]
includes called/private bodies; [fault analysis][fault-premises] reads physical
instruction correspondence. The replacement record must name the same program,
target, entry, installation and body version and preserve all these dimensions.
It must distinguish a finite admitted bound from unknown/unbounded demand;
“missing” is never zero.

Physical planning establishes register/effect and frame facts for ordinary and
generated bodies. Resource analysis then combines the whole retained call graph,
entry/bridge costs, arrival nesting and fault premises. Installation binding
checks that summary against admitted cohorts, environment assumptions and
placement requirements and publishes one bound physical program.

Today [O03 constructs numeric stack guards][guard] before this point. The
recommendation is to represent those requirements as symbolic operands with
reserved constraints until binding. Binding must not change body membership,
register pressure, clobbers, control-flow shape or frame demand. Encoding may
choose a carried width alternative after the value is known.

**Unverified:** prove this stability for every current numeric consumer. A late
literal patch is insufficient if the value chooses a different instruction
sequence, creates a helper or alters its own stack bound. The plan must either
move that decision into physical planning with a proved finite dependency
order, or revise the boundary. It cannot claim a global pass while secretly
compiling handlers through later stages and returning to the installer.

### Encoding and layout have a closed executable input

Common encoding owns ordinary instructions and all generated executable
fragments, including frame prologues/epilogues. It emits legal alternatives and
fixup data: bases, signed ranges, alignment, bitfield geometry, external symbols,
callback/image addresses and identity bindings. Layout never asks an earlier
encoder to regenerate a branch or callback sequence; O08's current
[callback re-encoding][callback-encoding] is part of this migration too.

Preserve the present reason for body-first relaxation before frame insertion.
The layout phase can perform those internal placement steps using already
encoded frame fragments; moving frame *encoding* earlier need not force a new
relaxation algorithm. Exact bytes, branch choices and failures must be compared.
The object and image phases then consume closed encoded content and explicit
relocations. No stage after encoding may create a new executable body.

## 6. Data ownership, runtimes and independent checks

### Classify every export before assigning a home

| Kind | Permitted owner | Test for the distinction |
| --- | --- | --- |
| Generic primitive | Foundation | No language, program, target or source-authority meaning. |
| Definition | Narrow representation vocabulary | Names a stable semantic or target concept; holds no live source/body lookup. |
| Carried record | The first complete program form that establishes it | Includes identities/provenance needed to interpret it, not an earlier program used as a database. |
| Producer | The phase whose complete input permits the calculation | Creates a plan, offset, choice, commitment, body or authority-bound result. “Pure” does not make it data. |
| Checker | Admission boundary or explicit independent verifier | Checks evidence against a stated input contract; it does not choose missing semantics. |
| Live host operation | Packages, executor/runtime, products or host support | Performs acquisition, execution or publication, not a compiler transformation. |

`terminal-psi/vocabulary` (TPV) is a proposed family of small definition/carrier
leaves, not a destination for 40k lines of mixed semantic code. Split identities,
scalar/path definitions, declarations, access/resources and layout by their
actual dependencies. The old 40–45k figure was an inventory estimate, not an
approved API or a size target.

[RetainedDeclaration contains SourceFile][retained-declaration];
[SourceFile contains text and ownership/scope data][source-file];
[rejoining consults the source symbol table][retained-rejoin]. Those capabilities
stay inside source settlement. Publish only necessary IDs, selected declaration
summaries and immutable error provenance. Likewise,
[application preparation][prepared-applications] and
[access-plan commitment construction][access-construction] are producers.
[Layout defaults][layout-rules] are realizations, not automatically language
promises. Moving these modules unchanged into TPV would reproduce the problem.

Generic byte cursors, record encoding and digest bytes may become `wire`.
[Scalar identity encoding][identity-encoder] and
[field/case path encoding][path-encoder] assign semantic tags and stay with their
representation codecs. A [SymbolHandle-keyed alias][symbol-map] stays with
symbol vocabulary even if its generic hasher moves to foundation.

### Carry what emission actually reads

For every `EmissionFacts` consumer, record the field read, meaning, producer,
first available phase, version/identity and final consumer. Replace access to
whole abstract/Terminal programs with those current-form records. Current
immutable structural declarations may be shared by content identity where
unchanged; executable graphs and source owners may not be retained as lookup
shortcuts. The deletion condition is that consumers no longer recover facts
from an earlier program, not that the struct or Arc changed its name.

### Runtimes consume products and admitted provision records

Keep sealed admitted carrier definitions separate from their issuing producer;
[terminal-runtime already names an admission type][runtime-type]. The runtime
must not import a producing stage merely to read it. Native/Psi execution
provisions carry exact program/target/binding identities and the necessary host
plans; only the corresponding admission phase may issue them.

The Psi execution route does not automatically require a native version of the
main program. It may require generated foreign adapters: [current capture emits
one][foreign-adapter]. Model those as explicit auxiliary native roots with their
own completed pipeline results. They use the common planning/encoding route,
then join the provision output. Do not revive a private adapter compiler inside
a runtime carrier constructor, and do not reject a valid Psi program because
an unused native main body lacks support. Independent components remain
separately admitted products; the migration does not merge their address spaces
or change their isolation/ABI contracts.

### Verification has explicit inputs and independent authority

Retain producer admission at trust boundaries and object/image consistency
checks. Keep explicit correspondence/rewrite replay separate from ordinary
production. Its request identifies which originals must be retained; absence
refuses that audit. Moving a validator beside a representation does not remove
its dependency on a producing stage or make it independent.

The verifier route receives current data and retained evidence directly through
a verification dispatcher. Program representations hold no callback into the
native driver or an earlier stage. Separate shared vocabulary/codecs from
independent checking logic; do not deduplicate a verifier into the producer
whose error it must catch. Proof search, certificate admission, source acceptance,
canonical decoding and host execution authorization remain distinct contracts.

## 7. Complete ownership and retirement inventory

The tables account for every package in the pinned foundation, backend and
realization groups. Counts are generated package totals, not estimated savings.
Each destination is **judgment** and requires a per-export classification from
§6. No row authorizes moving all functions into a passive representation.

### Foundation: 26 packages

| Existing package | Tracked Rust lines | Destination and required split — judgment |
| --- | ---: | --- |
| [access-plans](https://github.com/CathedralOS/Omega/blob/3f9ce43f0c9d179048911d18faa91ba93f60176d/omega-rust/psi/foundation/access-plans/Cargo.toml#L1) | 5,790 | Split access vocabulary and plan records from construction and checking. Portable records belong with the accepted program; access/extent commitment derivation belongs to source or native planning when its inputs exist; runtime checks stay with the interpreter. No whole-crate move into TPV. |
| [arena](https://github.com/CathedralOS/Omega/blob/3f9ce43f0c9d179048911d18faa91ba93f60176d/omega-rust/psi/foundation/arena/Cargo.toml#L1) | 8,402 | Keep generic arenas, handles and dense storage primitives. Receive generic ID tables and hashing only; no SymbolHandle-specific aliases. |
| [artifacts](https://github.com/CathedralOS/Omega/blob/3f9ce43f0c9d179048911d18faa91ba93f60176d/omega-rust/omega/foundation/artifacts/Cargo.toml#L1) | 345 | Move measurement/report carriers to a products leaf package that package compilation can use without depending on the product coordinator. |
| [bounded-process](https://github.com/CathedralOS/Omega/blob/3f9ce43f0c9d179048911d18faa91ba93f60176d/omega-rust/omega/foundation/bounded-process/Cargo.toml#L1) | 3,823 | Move the reusable process mechanism to explicit host support; CLI-only private observation stays with CLI. Permit this leaf for package resolvers, runtimes and tools; it owns no compilation. |
| [build-time-values](https://github.com/CathedralOS/Omega/blob/3f9ce43f0c9d179048911d18faa91ba93f60176d/omega-rust/psi/foundation/build-time-values/Cargo.toml#L1) | 91 | Put immutable evaluation result values beside the executor contract; source settlement interprets them. Move native-binding description production to Build. |
| [compiler-storage](https://github.com/CathedralOS/Omega/blob/3f9ce43f0c9d179048911d18faa91ba93f60176d/omega-rust/psi/foundation/compiler-storage/Cargo.toml#L1) | 1,599 | Keep generic record encoding/decoding. Semantic wire tags remain with the representation they encode. |
| [compiler-storage-derive](https://github.com/CathedralOS/Omega/blob/3f9ce43f0c9d179048911d18faa91ba93f60176d/omega-rust/psi/foundation/compiler-storage/derive/Cargo.toml#L1) | 168 | Keep the generic record derive package beside compiler-storage; count it as a separate package. |
| [diagnostics](https://github.com/CathedralOS/Omega/blob/3f9ce43f0c9d179048911d18faa91ba93f60176d/omega-rust/psi/foundation/diagnostics/Cargo.toml#L1) | 598 | Keep plain diagnostic values and formatting. Source parsing helpers move into source settlement; stages retain exact provenance needed for errors. |
| [evaluation-accounting](https://github.com/CathedralOS/Omega/blob/3f9ce43f0c9d179048911d18faa91ba93f60176d/omega-rust/psi/foundation/evaluation-accounting/Cargo.toml#L1) | 1,499 | Separate invocation/work records carried by accepted programs from evaluation sponsorship, limits and charging owned by source settlement/execution services. |
| [execution-admission](https://github.com/CathedralOS/Omega/blob/3f9ce43f0c9d179048911d18faa91ba93f60176d/omega-rust/psi/foundation/execution-admission/Cargo.toml#L1) | 138 | Keep generic worker/stack budget machinery if export review confirms no language policy. It is not authority to accept a program. |
| [extents](https://github.com/CathedralOS/Omega/blob/3f9ce43f0c9d179048911d18faa91ba93f60176d/omega-rust/psi/foundation/extents/Cargo.toml#L1) | 4,915 | Shared extent IDs and rights go with portable vocabulary; semantic bound derivation goes to its producing phase; mutable ledgers, root grants and host issuance stay with the interpreter/runtime owner. |
| [filesystem-authority](https://github.com/CathedralOS/Omega/blob/3f9ce43f0c9d179048911d18faa91ba93f60176d/omega-rust/psi/foundation/filesystem-authority/Cargo.toml#L1) | 2,670 | Separate source admission/capture, evaluation filesystem sponsorship and product-output obligations. Package/source acquisition, source settlement and products own these respectively; no runtime-stage dependency. |
| [flow-effects](https://github.com/CathedralOS/Omega/blob/3f9ce43f0c9d179048911d18faa91ba93f60176d/omega-rust/psi/foundation/flow-effects/Cargo.toml#L1) | 1,132 | Move carried invocation, dependency, capability and installation-reach records to accepted/current forms. Construction and contract checking stay with source/native producers or explicit independent checkers. |
| [function-identity](https://github.com/CathedralOS/Omega/blob/3f9ce43f0c9d179048911d18faa91ba93f60176d/omega-rust/omega/foundation/function-identity/Cargo.toml#L1) | 151 | Move stable machine-function identities into a native leaf. Keep body membership and mappings in current program records. |
| [language-core](https://github.com/CathedralOS/Omega/blob/3f9ce43f0c9d179048911d18faa91ba93f60176d/omega-rust/psi/foundation/language-core/Cargo.toml#L1) | 6,175 | Split source syntax vocabulary, portable semantic definitions and producer algorithms. Shared atomic/intrinsic/float definitions may be portable leaves; source-only spelling and resolution helpers stay inside source settlement. |
| [language-semantics](https://github.com/CathedralOS/Omega/blob/3f9ce43f0c9d179048911d18faa91ba93f60176d/omega-rust/psi/foundation/language-semantics/Cargo.toml#L1) | 11,821 | Split declaration/semantic records, source preparation, Build descriptions and independent checks. Logical provider declarations needed after source settlement must be published in the accepted program; no later source lookup. |
| [layout-plans](https://github.com/CathedralOS/Omega/blob/3f9ce43f0c9d179048911d18faa91ba93f60176d/omega-rust/psi/foundation/layout-plans/Cargo.toml#L1) | 8,205 | Separate data such as StorageGeometry from target/default selection and layout computation. Producers own offsets and defaults; portable/current forms carry their results; ingestion/runtime checks keep their own owner. |
| [mutation-matrix](https://github.com/CathedralOS/Omega/blob/3f9ce43f0c9d179048911d18faa91ba93f60176d/omega-rust/psi/foundation/mutation-matrix/Cargo.toml#L1) | 113 | Move to test support and migrate optional test-support users. Remove the optimization-core re-export once all test imports move in the same change set. |
| [numerics](https://github.com/CathedralOS/Omega/blob/3f9ce43f0c9d179048911d18faa91ba93f60176d/omega-rust/psi/foundation/numerics/Cargo.toml#L1) | 6,799 | Keep BigInt/BigRational as generic bignum. Put numeric type definitions, language arithmetic and float meaning with portable vocabulary; put evaluation/realization algorithms in the relevant producer. |
| [optimization](https://github.com/CathedralOS/Omega/blob/3f9ce43f0c9d179048911d18faa91ba93f60176d/omega-rust/psi/foundation/optimization/Cargo.toml#L1) | 437 | Merge only generic wire helpers into wire. Carry Psi optimization selections/identity with the accepted representation. |
| [optimization-core](https://github.com/CathedralOS/Omega/blob/3f9ce43f0c9d179048911d18faa91ba93f60176d/omega-rust/omega/foundation/optimization-core/Cargo.toml#L1) | 5,818 | Split generic wire helpers, optimization definitions, current decision/work records, reporting and checker logic. Native vocabulary owns definitions; actual rewrites belong to planning; products owns report requests/rollback presentation. |
| [platform-custody](https://github.com/CathedralOS/Omega/blob/3f9ce43f0c9d179048911d18faa91ba93f60176d/omega-rust/omega/foundation/platform-custody/Cargo.toml#L1) | 1,146 | Move filesystem installation and executable-bit/publication operations to package/host support used by products. It is not a native transformation. |
| [semantic-vocabulary](https://github.com/CathedralOS/Omega/blob/3f9ce43f0c9d179048911d18faa91ba93f60176d/omega-rust/psi/foundation/semantic-vocabulary/Cargo.toml#L1) | 12,136 | Keep generic ID tables/hash bytes in primitives; semantic IDs, scalar terms, proposition/path definitions and their codecs in narrow portable leaves. Build/source-only operations and live source authority stay with their owners. |
| [source](https://github.com/CathedralOS/Omega/blob/3f9ce43f0c9d179048911d18faa91ba93f60176d/omega-rust/psi/foundation/source/Cargo.toml#L1) | 2,556 | Keep primitive positions/spans separate from file contents and acquisition custody. Source settlement owns admitted bytes, source scopes and consent. Later forms receive only required immutable identity/provenance. |
| [source-audit](https://github.com/CathedralOS/Omega/blob/3f9ce43f0c9d179048911d18faa91ba93f60176d/omega-rust/psi/foundation/source-audit/Cargo.toml#L1) | 107 | Move dev-only source guards to test support. Preserve existing guards and their selectors; no production dependency. |
| [symbols](https://github.com/CathedralOS/Omega/blob/3f9ce43f0c9d179048911d18faa91ba93f60176d/omega-rust/psi/foundation/symbols/Cargo.toml#L1) | 9,589 | Move symbol-table mutation, resolution, builtins and retained source declarations to source settlement. Publish stable symbol IDs and selected declaration summaries only. RetainedDeclaration/SourceFile are not portable identity leaves. |

The retained primitive set is generic arena/storage, record codec and derive,
plain diagnostics, text positions, generic wire/digests, bignum and generic worker
budget support. There is no `omega/foundation/` at cutover. Host/process support
is explicitly outside compilation. Support needed by Psi must remain reachable
without a dependency on Omega; a host-support label does not waive the firewall.
Its boundary rule is updated with its users,
not an unexplained package exception for the runtime.

### Backend: 29 packages

| Existing package | Tracked Rust lines | Destination and required split — judgment |
| --- | ---: | --- |
| [backend-plan](https://github.com/CathedralOS/Omega/blob/3f9ce43f0c9d179048911d18faa91ba93f60176d/omega-rust/omega/compilation/backend/plans/backend-plan/Cargo.toml#L1) | 2,726 | Split selected-provider/proposal carriers from canonical-schema construction and closure. Logical body selection moves into source settlement; target realization/admission into native planning. |
| [bound-installations](https://github.com/CathedralOS/Omega/blob/3f9ce43f0c9d179048911d18faa91ba93f60176d/omega-rust/omega/compilation/backend/runtime/bound-installations/Cargo.toml#L1) | 348 | Carry bound installation facts in the bound physical program; constructors belong to installation binding. Records must not expose an earlier whole program. |
| [boundary-applications](https://github.com/CathedralOS/Omega/blob/3f9ce43f0c9d179048911d18faa91ba93f60176d/omega-rust/omega/compilation/backend/artifacts/boundary-applications/Cargo.toml#L1) | 1,350 | Carry canonical artifact/application identities and selected rows in native input. Selection/construction belongs to source/native settlement; checking remains explicit. |
| [calling-conventions](https://github.com/CathedralOS/Omega/blob/3f9ce43f0c9d179048911d18faa91ba93f60176d/omega-rust/omega/compilation/backend/calling-conventions/Cargo.toml#L1) | 11,063 | Target-invariant ABI vocabulary goes to target description. Signature-specific call plans, callback placement and boundary materialization are produced by native/physical planning and carried forward. |
| [component-description](https://github.com/CathedralOS/Omega/blob/3f9ce43f0c9d179048911d18faa91ba93f60176d/omega-rust/omega/compilation/backend/artifacts/component-description/Cargo.toml#L1) | 2,542 | Keep imported component carriers, codec and independent verifier separate from the producer/publisher. Native admission consumes checked descriptions; products publishes them. |
| [effects](https://github.com/CathedralOS/Omega/blob/3f9ce43f0c9d179048911d18faa91ba93f60176d/omega-rust/omega/compilation/backend/plans/effects/Cargo.toml#L1) | 7,782 | Split effect vocabulary and selected evidence records from derivation and authority issuance. Physical planning publishes actual clobbers/effects; admission/binding checks the corresponding promises. |
| [encoded-scalar-plan](https://github.com/CathedralOS/Omega/blob/3f9ce43f0c9d179048911d18faa91ba93f60176d/omega-rust/omega/compilation/backend/plans/encoded-scalar-plan/Cargo.toml#L1) | 324 | Move encoder logic into common encoding; put its required plan data in the bound physical/encoded forms. Later stages receive fixups rather than calling the encoder. |
| [image](https://github.com/CathedralOS/Omega/blob/3f9ce43f0c9d179048911d18faa91ba93f60176d/omega-rust/omega/compilation/backend/images/image/Cargo.toml#L1) | 5,281 | Image representation owns bytes, regions, symbols and relocation facts; image construction owns placement/relocation algorithms; independent validators are separate consumers. |
| [image-elf](https://github.com/CathedralOS/Omega/blob/3f9ce43f0c9d179048911d18faa91ba93f60176d/omega-rust/omega/compilation/backend/images/image-elf/Cargo.toml#L1) | 22,191 | Move ELF container production under image construction, with ELF data/codec and independent validation separated as needed. No instruction selection or fresh helper generation there. |
| [image-emission](https://github.com/CathedralOS/Omega/blob/3f9ce43f0c9d179048911d18faa91ba93f60176d/omega-rust/omega/compilation/backend/images/image-emission/Cargo.toml#L1) | 43,553 | Distribute object construction, image construction, early generated-body recipes, runtime carriers and explicit verification. No umbrella replacement library owns all these routes. |
| [image-macho](https://github.com/CathedralOS/Omega/blob/3f9ce43f0c9d179048911d18faa91ba93f60176d/omega-rust/omega/compilation/backend/images/image-macho/Cargo.toml#L1) | 4,710 | Move Mach-O container/signature production under image construction; preserve format-specific validation and metadata semantics. |
| [image-pe](https://github.com/CathedralOS/Omega/blob/3f9ce43f0c9d179048911d18faa91ba93f60176d/omega-rust/omega/compilation/backend/images/image-pe/Cargo.toml#L1) | 1,904 | Move PE container production under image construction; preserve format-specific entry/import/relocation validation. |
| [installation-evidence](https://github.com/CathedralOS/Omega/blob/3f9ce43f0c9d179048911d18faa91ba93f60176d/omega-rust/omega/compilation/backend/runtime/installation-evidence/Cargo.toml#L1) | 357 | Separate admitted early obligations, full resource summaries and final bound facts. Put each in the first current form that can establish it; preserve sealed construction. |
| [isa-aarch64](https://github.com/CathedralOS/Omega/blob/3f9ce43f0c9d179048911d18faa91ba93f60176d/omega-rust/omega/compilation/backend/isa/isa-aarch64/Cargo.toml#L1) | 25,746 | Target register/effect catalogs become description data; all body/entry/adapter/frame/writer encoders move to common encoding. Physical recipes are established before encoding; validators remain independent. |
| [isa-conformance](https://github.com/CathedralOS/Omega/blob/3f9ce43f0c9d179048911d18faa91ba93f60176d/omega-rust/omega/compilation/backend/isa/isa-conformance/Cargo.toml#L1) | 579 | Keep the existing ISA conformance suite as encoding test support; relocate selectors/imports in the same change set. |
| [isa-x86_64](https://github.com/CathedralOS/Omega/blob/3f9ce43f0c9d179048911d18faa91ba93f60176d/omega-rust/omega/compilation/backend/isa/isa-x86_64/Cargo.toml#L1) | 32,987 | Same split as AArch64: catalogs, planned physical operations, common encoding and independent checks. Hosted byte sequences cannot hide late program construction. |
| [layout](https://github.com/CathedralOS/Omega/blob/3f9ce43f0c9d179048911d18faa91ba93f60176d/omega-rust/omega/compilation/backend/layout/Cargo.toml#L1) | 1,934 | Shared layout records become leaves; signature/program layout calculation belongs to native planning. Do not retain a realization/layout library that continues to perform unnumbered stage work. |
| [machine-emission](https://github.com/CathedralOS/Omega/blob/3f9ce43f0c9d179048911d18faa91ba93f60176d/omega-rust/omega/compilation/backend/machine-emission/Cargo.toml#L1) | 17,781 | Physical planning owns frames, exit contracts and bridge recipes; common encoding owns every executable fragment; layout owns placement/frame insertion/fixups. Explicit audit work moves to verifier consumers. Delete the crate after every part moves together. |
| [native-artifact](https://github.com/CathedralOS/Omega/blob/3f9ce43f0c9d179048911d18faa91ba93f60176d/omega-rust/omega/compilation/backend/artifacts/native-artifact/Cargo.toml#L1) | 16,394 | Move final/runtime carriers to image/provision representations, early entry admission to native planning, physical evidence production to its actual phase, explicit verification to verifier packages, and filesystem publication/receipts to products. |
| [normalized-foreign-call-plan](https://github.com/CathedralOS/Omega/blob/3f9ce43f0c9d179048911d18faa91ba93f60176d/omega-rust/omega/compilation/backend/plans/normalized-foreign-call-plan/Cargo.toml#L1) | 1,202 | Normalization belongs to native/physical planning; encoding-specific interpretation belongs to common encoding. Carry results so layout never reopens signatures. |
| [object-file](https://github.com/CathedralOS/Omega/blob/3f9ce43f0c9d179048911d18faa91ba93f60176d/omega-rust/omega/compilation/backend/object-file/Cargo.toml#L1) | 3,552 | Move object/name/container construction into the object phase and data into the object representation. The driver owns sequencing, not a product wrapper. |
| [post-handoff-writer-plan](https://github.com/CathedralOS/Omega/blob/3f9ce43f0c9d179048911d18faa91ba93f60176d/omega-rust/omega/compilation/backend/plans/post-handoff-writer-plan/Cargo.toml#L1) | 960 | Classify live consumers first. Where used, lower symbolic materialization into explicit writer recipes before common encoding; carry invocation relocations separately. Do not infer an image-time production caller from a library export. |
| [program-entry-plan](https://github.com/CathedralOS/Omega/blob/3f9ce43f0c9d179048911d18faa91ba93f60176d/omega-rust/omega/compilation/backend/plans/program-entry-plan/Cargo.toml#L1) | 1,300 | Entry selection/admission and physical entry recipes belong to source/native/physical planning at their respective boundaries; later forms carry selected records. |
| [register-environment](https://github.com/CathedralOS/Omega/blob/3f9ce43f0c9d179048911d18faa91ba93f60176d/omega-rust/omega/compilation/backend/register-environment/Cargo.toml#L1) | 5,171 | Derive immutable target-only environment from target description before native planning; pass it explicitly. Program-specific reservation decisions belong to the current planning phase. |
| [register-model](https://github.com/CathedralOS/Omega/blob/3f9ce43f0c9d179048911d18faa91ba93f60176d/omega-rust/omega/compilation/backend/register-model/Cargo.toml#L1) | 3,514 | Move declarative register, constraint and preservation vocabulary to target description. Computation from a specific body remains a producer responsibility. |
| [runtime-abi](https://github.com/CathedralOS/Omega/blob/3f9ce43f0c9d179048911d18faa91ba93f60176d/omega-rust/omega/compilation/backend/runtime/runtime-abi/Cargo.toml#L1) | 2,356 | Separate target geometry and carrier definitions from live host descriptor/network/numeric operations. Geometry belongs to target description, current values to program/provision records, host operations to runtime. |
| [target](https://github.com/CathedralOS/Omega/blob/3f9ce43f0c9d179048911d18faa91ba93f60176d/omega-rust/omega/compilation/backend/target/Cargo.toml#L1) | 3,921 | Move target identity/catalogs into the selected input description. Build owns target-set selection; products/request records own per-product application metadata; review-evidence owns package review descriptions. |
| [task-plans](https://github.com/CathedralOS/Omega/blob/3f9ce43f0c9d179048911d18faa91ba93f60176d/omega-rust/omega/compilation/backend/runtime/task-plans/Cargo.toml#L1) | 1,901 | Put realized activation records in native/current forms and move realize_task_activations into native planning. Preserve admission and stack-contribution checks. |
| [x86_64-encoding](https://github.com/CathedralOS/Omega/blob/3f9ce43f0c9d179048911d18faa91ba93f60176d/omega-rust/omega/compilation/backend/isa/x86_64-encoding/Cargo.toml#L1) | 78 | Move primitive instruction encodings to common encoding, including users that currently reach them through native-artifact. Image construction receives encoded fragments and fixups. |

No replacement `backend` umbrella remains. All ordinary and generated executable
construction has a phase owner; final data and format-specific validators may
have separate packages. Deleting machine-emission waits until frames, fragments,
bridges and all audit consumers have moved within the same integrated change.

### Realization: five packages

| Existing package | Tracked Rust lines | Destination and required split — judgment |
| --- | ---: | --- |
| [provider-admission](https://github.com/CathedralOS/Omega/blob/3f9ce43f0c9d179048911d18faa91ba93f60176d/omega-rust/omega/compilation/realization/provider-admission/Cargo.toml#L1) | 26,491 | Move admission operations to native planning and late installation binding; split sealed carrier definitions for runtimes and representations. Preserve who can issue each record. |
| [provider-planning](https://github.com/CathedralOS/Omega/blob/3f9ce43f0c9d179048911d18faa91ba93f60176d/omega-rust/omega/compilation/realization/provider-planning/Cargo.toml#L1) | 7,231 | Separate logical requirement/body discovery from target ABI realization. The former is resolved in source settlement through neutral Build records; the latter belongs to native planning over closed accepted code. |
| [provider-selection](https://github.com/CathedralOS/Omega/blob/3f9ce43f0c9d179048911d18faa91ba93f60176d/omega-rust/omega/compilation/realization/provider-selection/Cargo.toml#L1) | 1,874 | Publish exact selected declaration/body/application identities during logical source settlement; native planning may validate these but may not activate source. |
| [selected-dispatch](https://github.com/CathedralOS/Omega/blob/3f9ce43f0c9d179048911d18faa91ba93f60176d/omega-rust/omega/compilation/realization/selected-dispatch/Cargo.toml#L1) | 105 | Replace the external wrapper role with the selected native input contract. Preserve target agreement and canonical plan custody; public from_realization is a current construction route, not unique settle authority. |
| [trust-model](https://github.com/CathedralOS/Omega/blob/3f9ce43f0c9d179048911d18faa91ba93f60176d/omega-rust/omega/compilation/realization/trust-model/Cargo.toml#L1) | 708 | Run trust-obligation reconstruction in native admission from carried declarations, grants and evidence; carry the result. It must not reconstruct source facts later. |

### Module and representation obligations that crate moves can miss

| Current surface | Required treatment |
| --- | --- |
| `declaration_selection` | Keep exact occurrence/body identities; preserve the existing resolved → typed → lowering ledger. Logical selection mutation belongs to source settlement. |
| `RetainedDeclaration`, `SourceFile`, source scopes/consent | Keep source content and authority private to source settlement. Only explicit provenance projections enter portable leaves. |
| `callable_applications`, semantic domains and prepared theories | Producer builds closed application records; consumers never reconstruct an unapplied family. Missing preparation preserves refusal. |
| `float_operations`, scalar/proposition/numeric tables | Shared definitions stay portable. `float_operations` has both terminal-semantics and [target][float-target] users; a single-stage move is wrong. |
| `layout_reports`, aggregate geometry, access/extent plans | Split records, default/target choices, computation and validation. Carry computed geometry and commitments; retain actual runtime ledgers in the runtime. |
| `build_requests`, native binding descriptions, compiler evaluation | Neutral records cross the appropriate input boundary; Build/source production and artifact execution do not share mutable contexts. |
| `VerifiedPsiOptimizationInput`, native proposal constructors | Passive accepted/current records after extracting provider closure, task realization and admission. Preserve sealing authority. |
| `native-vocabulary`, `target-vocabulary` | Stable leaves, not routes to abstract/target programs. Eliminate reverse program-form dependencies and producer imports. |
| Selected instruction analyses / register homes | Split shared analysis records from allocation-specific output; an earlier form cannot depend on a later whole program. |
| `FramedMachineProgram`, encoded/resolved machine roots | Distinct forms remain explicit even if private phases coarsen. Carry effects, resources, encoding and fixup data with versioned identities. |
| `EmissionFacts`, fragment/object replay wrappers | Inventory actual reads; remove earlier executable-program access from production. Keep requested replay originals in verifier-owned evidence only. |
| Target environment/cache and per-product metadata | Immutable target-only facts are explicit input; product application identity is a separate carried request. Program-specific reservations remain current facts. |
| Entry/foreign/hosted/frame/post-handoff sequences | Declare recipes early, include generated physical bodies, encode once, relocate later. Evidence decides whether an exported helper has a live producer at all. |
| Publication wrappers and receipts | Move filesystem publication to products; remove duplicate stage sequencing from native-artifact and other product helpers. |

## 8. Planning completion gates — before implementation

These are not deferred cleanup tasks. They are the remaining proof/design work
needed to turn the recommendation into an implementation-ready contract. Each
must end with a concrete field/API model, source-backed consumer inventory and
an ordering argument; a diagram alone does not close it.

| Gate | Required result | What would make the proposed boundary fail |
| --- | --- | --- |
| Source activation and closure | Exact lazy source activation, extension, evaluation and charging state transitions; preserve error order and checker verdicts. Show that private operations replace, rather than call, the old source stages. | Eager errors from unused files; dynamic source discovered after portable publication; a callback to an old stage. |
| Logical provider separation | Map every provider-selection input to neutral admitted Build/source data or later ABI admission. Close selected source bodies and calling-policy code before source publication. | A later target decision needs another source body, an ABI object enters Psi, or moving checks changes provider choice/refusal. |
| Native demand closure | Show available/demanded/retained membership and every generated reference; prove later encoding/layout cannot discover new bodies. Preserve optimization-pruned unavailable targets. | A conservative roster introduces new native obligations, or object symbols remain the authoritative source of reach. |
| Generated physical program | Model entries/adapters/handlers/hosted/writer code, their body-effect inputs and caller constraints in one physical form. Prove ordinary body effects remain valid after adding helpers. | Late scratch/frame choices, missing helper clobbers or image-time code generation. |
| Resource/binding stability | Inventory every numeric consumer; model transitive demand, bridge/bias/arrival costs, fault premises and identity joins. Prove finite ordering and unchanged allocation/effects after binding. | A resolved bound changes its own frame, instruction constraints, helper set or resource graph. |
| Encoding/layout closure | Complete alternative/fixup schema for both ISAs, callbacks, frames, entries and formats; account for exact old relaxation decisions. | A layout phase must invoke an encoder or the new order changes bytes/refusals without authorization. |
| Carried-fact and authority interfaces | Finish per-export/per-field classification, including EmissionFacts and source custody; specify allowed constructors and runtime/verifier inputs. | A representation exposes an earlier graph, source lookup, producing stage or driver callback. |

A gate may require changing the proposed boundary. Resolve that change in the
plan and its consumer contracts before coding. Do not hide the conflict with an
extra shared library, blanket allowance, speculative new acceptance rule or a
“temporary” runtime branch intended to survive cutover.

## 9. One integrated execution plan

After §8 is closed, implementation is one coordinated effort on a fixed base.
The following work packages are internal dependencies, not PR rollout steps.
They may proceed in parallel where their agreed contracts allow it. One
integration owner maintains the complete producer/consumer and retirement
matrix and resolves conflicts across all packages.

| Internal package | Depends on | Complete responsibility within the change set |
| --- | --- | --- |
| Contract and evidence baseline | Planning gates closed | Refresh the pinned inventory against the implementation base; freeze stage IO, authority, wire/identity and failure contracts; list all entry points and test selectors. |
| Primitive/leaf extraction | Agreed export/field model | Build the acyclic generic and representation leaves; update every consumer. No semantic algorithms or source custody move into primitives. |
| Source and evaluation replacement | Neutral Build/provider and executor contracts | Implement lazy source settlement, accepted invocation execution and closed logical body selection; replace old source stage APIs and facade continuations together. |
| Native demand/planning replacement | Accepted code/selection contract; target leaves | Implement source-free retained-body closure, native admission, ABI plans and explicit generated-body recipes. Consume components and target requests through their declared interfaces. |
| Physical/resource replacement | Selected program and generated-recipe contracts | Implement allocation/physical completion, complete resource analysis and stable installation binding for ordinary and generated bodies. |
| Encoding/layout/object/image replacement | Bound physical contract; format/fixup definitions | Route every executable body through common encoding, preserve layout behavior, and construct object/image data without new program decisions. |
| Runtime, products and independent verification | Carrier/authority contracts from all producers | Move provision consumers, auxiliary native roots, publication and explicit audit inputs; preserve runtime isolation and independent checks. |
| Integrated retirement and proof | All above complete | Switch every production facade together, remove old paths/adapters/allowances, enable final gates, run §10 validation on the exact change, and publish one reviewable cutover. |

Primitive extraction, verifier interface work and source/native implementations
can overlap after contracts are agreed. A consumer does not invent its own
version of an unfinished producer contract. No package is “done” while another
package still calls its old route. The final combined review, not individual
work-package completion, decides whether the compiler can switch.

### Retirement ledger at cutover

| Retire | Replacement required before deletion |
| --- | --- |
| Non-primitive foundation crates and `omega/foundation/` | Every row in the foundation table assigned and all imports/codecs/tests moved. |
| `omega/compilation/backend/` and umbrella re-exports | Every backend row assigned to a phase, representation, verifier, product/runtime or evidenced test owner. |
| Old source stage entry APIs and split continuation/evaluation coordination | Single source-settlement producer with demand-aware syntax, exact checker acceptance and artifact-only execution. |
| External provider-to-source completion and stage work in proposal constructors | Closed logical selections/accepted code plus source-free native planning. |
| Root/handler recursive native stage interleaving | Closed current native program, full resources and stable binding; no later-stage output fed to an earlier installer. |
| Image-time entry/adapter/frame/writer program generation | Explicit physical bodies and common encoding, including runtime auxiliary artifacts. |
| EmissionFacts and equivalent earlier-program access | Every consumer supplied by justified current-form fields; no renamed wrapper around the old graph. |
| Driver callbacks embedded in program/audit carriers | Explicit verifier dispatch and evidence inputs; production program forms are data. |
| Product-owned stage sequencing and filesystem writes inside compiler stages | Driver sequence plus final product publication only. |
| Temporary adapters, duplicate types and migration allowances | Final dependency/authority rules enabled in the same change set; no compatibility path selected in production. |

The integration also updates `AGENTS.md`, pipeline documentation, applicable
skills, crate READMEs, Cargo workspace paths, command/test selectors and atlas
sources that describe the replaced architecture. Preserve the six owner rulings;
do not reinterpret them as permission to weaken language or trust contracts.

## 10. Validation and exact done criteria

This is the validation plan for implementation, **not a report of tests already
run**. Establish the baseline on the chosen source and run the replacement on the
same inputs, targets, optimization settings, permissions and budgets. Existing
focused tests stay with their operations; add only tests that prove changed
contracts or previously untested failure modes.

| Boundary | Required evidence for the cutover |
| --- | --- |
| Source and evaluation | Same selected source activation, values, work charging, diagnostic provenance and accepted/rejected/incomplete results; unused invalid/unavailable source remains governed by the same demand rules. Exact-budget and one-unit-short behavior survives. |
| Source acceptance and proof authority | Positive/negative twins preserve the sole lowered-form verdict and independent certificate checks. Wire decoding, proof search or a native request cannot substitute for acceptance. |
| Logical and native provider demand | Same exact providers/bodies/calling-policy outputs; previously selected choices cannot change. Unused or optimization-pruned target-unsupported callbacks do not become new errors; reached equivalents still refuse. |
| Generated bodies and installation resources | Compare ordinary/adapter/handler call graphs, register/effect contracts, finite bounds, bridge/bias costs, alignment and fault evidence. Preserve cyclic-resource, identity-substitution and missing-evidence refusals. |
| Physical/encoding/layout | Same semantic effects, ABI transfers, frames, branch choices, bytes, fixups and target refusal behavior for unchanged requests. Cover x86-64 and AArch64 and both ordinary/generated code. |
| Object/image/products | Byte-for-byte canonical artifacts and native outputs, identity/digest/relocation/signature contracts and publication receipts remain stable for identical deterministic inputs. Preserve failure atomicity and component isolation. Any proposed intentional format/byte change requires an explicit separate design decision, not an unexplained comparison waiver. |
| Runtime routes | Native/Psi agreement for the affected accepted programs on the four hosted targets; Psi-only operation and required auxiliary adapters retain their own support contract. Runtime packages import carriers/host support, not producing stages. |
| Independent validators | Tampered facts/identities/bytes refuse; valid retained evidence passes; missing requested originals refuse. Production has no hidden replay/driver callback and does not lose required trust-boundary checks. |
| Architecture and deletion | Check complete dependency graph, per-export ownership and all public entrances; no old backend/non-primitive foundation path, bypass, adapter, duplicate route or unexplained allowance remains. Text matching alone is not proof of behavior. |

Use repository-required `mbx` commands for actual Rust builds/tests and the
shared host budget. The exact changed crates and programs determine the focused
validation set; the integrated cutover also exercises every affected public
route: compile/check, native and Psi execution, component production/import,
explicit validators and publication. A passing library test is not evidence
that an untested public route migrated.

The migration is done only when all planning gates have closed, every inventory
row and retirement item is accounted for, all producers and consumers use the
same final contracts, the exact combined change passes its behavior/identity and
architecture checks, and the old route is deleted at the single cutover. A page
build, a smaller crate count, a renamed stage or a partially connected native
example does not establish that outcome.

## 11. Decisions still to settle

[Questions](../questions.md) keeps Q-A/B/C/D/F/G aligned with this design. The
recommendation is explicit: narrow host support; no source custody in leaves;
one source-settlement owner and artifact executor; shared definition leaves but
no earlier program access; one common encoding route; logical provider closure
before native work and full resource binding before encoding. The open items
are the concrete feasibility proofs in §8, not authorization to perform a
piecemeal migration.


[access-construction]: https://github.com/CathedralOS/Omega/blob/3f9ce43f0c9d179048911d18faa91ba93f60176d/omega-rust/psi/foundation/access-plans/src/plan.rs#L212
[application-consumer]: https://github.com/CathedralOS/Omega/blob/3f9ce43f0c9d179048911d18faa91ba93f60176d/omega-rust/psi/compilation/pipeline/05_typed-trees-to-terminal-psi/src/read/types.rs#L167
[audit]: https://github.com/CathedralOS/Omega/blob/3f9ce43f0c9d179048911d18faa91ba93f60176d/omega-rust/omega/compilation/backend/images/image-emission/src/function_fragments/replay/validation.rs#L71
[bridge-encoding]: https://github.com/CathedralOS/Omega/blob/3f9ce43f0c9d179048911d18faa91ba93f60176d/omega-rust/omega/compilation/backend/machine-emission/src/native_entry_bridge.rs#L701
[bridge-registers]: https://github.com/CathedralOS/Omega/blob/3f9ce43f0c9d179048911d18faa91ba93f60176d/omega-rust/omega/compilation/backend/machine-emission/src/native_entry_bridge.rs#L521
[callback-demand]: https://github.com/CathedralOS/Omega/blob/3f9ce43f0c9d179048911d18faa91ba93f60176d/omega-rust/omega/compilation/native-compiler/src/realization/callback_thunks.rs#L182
[callback-encoding]: https://github.com/CathedralOS/Omega/blob/3f9ce43f0c9d179048911d18faa91ba93f60176d/omega-rust/omega/compilation/pipeline/08_selected-form-encoding-to-resolved-layout/src/ordinary.rs#L676
[closure-loop]: https://github.com/CathedralOS/Omega/blob/3f9ce43f0c9d179048911d18faa91ba93f60176d/omega-rust/omega/compilation/compiler/src/execution_settlement.rs#L260
[emission-facts]: https://github.com/CathedralOS/Omega/blob/3f9ce43f0c9d179048911d18faa91ba93f60176d/omega-rust/omega/compilation/native-compiler/src/pipeline.rs#L261
[entry-generation]: https://github.com/CathedralOS/Omega/blob/3f9ce43f0c9d179048911d18faa91ba93f60176d/omega-rust/omega/compilation/backend/images/image-emission/src/entries.rs#L1220
[entry-input]: https://github.com/CathedralOS/Omega/blob/3f9ce43f0c9d179048911d18faa91ba93f60176d/omega-rust/omega/compilation/backend/images/image-emission/src/entries.rs#L1251
[fault-premises]: https://github.com/CathedralOS/Omega/blob/3f9ce43f0c9d179048911d18faa91ba93f60176d/omega-rust/omega/compilation/backend/images/image-emission/src/object_artifact.rs#L1264
[float-target]: https://github.com/CathedralOS/Omega/blob/3f9ce43f0c9d179048911d18faa91ba93f60176d/omega-rust/omega/compilation/backend/target/src/float_realization.rs#L72
[foreign-adapter]: https://github.com/CathedralOS/Omega/blob/3f9ce43f0c9d179048911d18faa91ba93f60176d/omega-rust/omega/compilation/backend/artifacts/native-artifact/src/runtime_provisions.rs#L77
[guard]: https://github.com/CathedralOS/Omega/blob/3f9ce43f0c9d179048911d18faa91ba93f60176d/omega-rust/omega/compilation/pipeline/03_target-operations-to-selected-instructions/src/selection_interrupts.rs#L1145
[handler-demand]: https://github.com/CathedralOS/Omega/blob/3f9ce43f0c9d179048911d18faa91ba93f60176d/omega-rust/omega/compilation/native-compiler/src/realization/callback_thunks.rs#L117
[identity-encoder]: https://github.com/CathedralOS/Omega/blob/3f9ce43f0c9d179048911d18faa91ba93f60176d/omega-rust/psi/foundation/semantic-vocabulary/src/identity_encoding.rs#L5
[layout-rules]: https://github.com/CathedralOS/Omega/blob/3f9ce43f0c9d179048911d18faa91ba93f60176d/omega-rust/psi/foundation/layout-plans/src/lib.rs#L69
[ledger-read]: https://github.com/CathedralOS/Omega/blob/3f9ce43f0c9d179048911d18faa91ba93f60176d/omega-rust/psi/compilation/pipeline/05_typed-trees-to-terminal-psi/src/read/calls.rs#L515
[ledger-transfer]: https://github.com/CathedralOS/Omega/blob/3f9ce43f0c9d179048911d18faa91ba93f60176d/omega-rust/psi/compilation/pipeline/03_symbol-resolved-trees-to-typed-trees/src/lowerer.rs#L1622
[object-route]: https://github.com/CathedralOS/Omega/blob/3f9ce43f0c9d179048911d18faa91ba93f60176d/omega-rust/omega/compilation/native-compiler/src/realization/object_emission.rs#L81
[path-encoder]: https://github.com/CathedralOS/Omega/blob/3f9ce43f0c9d179048911d18faa91ba93f60176d/omega-rust/psi/foundation/semantic-vocabulary/src/structural_path_encoding.rs#L3
[policy-input]: https://github.com/CathedralOS/Omega/blob/3f9ce43f0c9d179048911d18faa91ba93f60176d/omega-rust/omega/compilation/compiler/src/terminal/provider_execution.rs#L14
[prepared-applications]: https://github.com/CathedralOS/Omega/blob/3f9ce43f0c9d179048911d18faa91ba93f60176d/omega-rust/psi/compilation/representations/typed-trees/src/callable_applications.rs#L1360
[product-preparation]: https://github.com/CathedralOS/Omega/blob/3f9ce43f0c9d179048911d18faa91ba93f60176d/omega-rust/psi/compilation/build-time-evaluation/src/product/retained_provider_selections.rs#L120
[proposal]: https://github.com/CathedralOS/Omega/blob/3f9ce43f0c9d179048911d18faa91ba93f60176d/omega-rust/omega/compilation/representations/abstract-operations/verified-optimization-input/src/native_proposal.rs#L240
[retained-declaration]: https://github.com/CathedralOS/Omega/blob/3f9ce43f0c9d179048911d18faa91ba93f60176d/omega-rust/psi/foundation/symbols/src/retained_declaration.rs#L67
[retained-rejoin]: https://github.com/CathedralOS/Omega/blob/3f9ce43f0c9d179048911d18faa91ba93f60176d/omega-rust/psi/foundation/symbols/src/retained_declaration.rs#L106
[runtime-type]: https://github.com/CathedralOS/Omega/blob/3f9ce43f0c9d179048911d18faa91ba93f60176d/omega-rust/omega/interpreter/terminal-runtime/src/execution/host_imports.rs#L365
[source-dependency]: https://github.com/CathedralOS/Omega/blob/3f9ce43f0c9d179048911d18faa91ba93f60176d/omega-rust/psi/compilation/source-compiler/Cargo.toml#L16
[source-file]: https://github.com/CathedralOS/Omega/blob/3f9ce43f0c9d179048911d18faa91ba93f60176d/omega-rust/psi/foundation/source/src/lib.rs#L144
[source-job]: https://github.com/CathedralOS/Omega/blob/3f9ce43f0c9d179048911d18faa91ba93f60176d/omega-rust/psi/compilation/build-time-evaluation/src/source_jobs.rs#L616
[source-job-execution]: https://github.com/CathedralOS/Omega/blob/3f9ce43f0c9d179048911d18faa91ba93f60176d/omega-rust/psi/compilation/build-time-evaluation/src/source_jobs.rs#L402
[source-loop]: https://github.com/CathedralOS/Omega/blob/3f9ce43f0c9d179048911d18faa91ba93f60176d/omega-rust/psi/compilation/source-compiler/src/source_compilation.rs#L1091
[source-prepare]: https://github.com/CathedralOS/Omega/blob/3f9ce43f0c9d179048911d18faa91ba93f60176d/omega-rust/psi/compilation/source-compiler/src/source_checking.rs#L119
[source-state]: https://github.com/CathedralOS/Omega/blob/3f9ce43f0c9d179048911d18faa91ba93f60176d/omega-rust/psi/compilation/source-compiler/src/source_compilation.rs#L906
[stack]: https://github.com/CathedralOS/Omega/blob/3f9ce43f0c9d179048911d18faa91ba93f60176d/omega-rust/omega/compilation/backend/images/image-emission/src/object_artifact.rs#L1499
[symbol-map]: https://github.com/CathedralOS/Omega/blob/3f9ce43f0c9d179048911d18faa91ba93f60176d/omega-rust/psi/foundation/symbols/src/fast_map.rs#L85
[transitive-stack]: https://github.com/CathedralOS/Omega/blob/3f9ce43f0c9d179048911d18faa91ba93f60176d/omega-rust/omega/compilation/backend/images/image-emission/src/object_artifact.rs#L5363
[writer]: https://github.com/CathedralOS/Omega/blob/3f9ce43f0c9d179048911d18faa91ba93f60176d/omega-rust/omega/compilation/backend/plans/post-handoff-writer-plan/src/lib.rs#L37
[invocation-accounting]: https://github.com/CathedralOS/Omega/blob/3f9ce43f0c9d179048911d18faa91ba93f60176d/omega-rust/psi/compilation/build-time-evaluation/src/source_jobs.rs#L164
