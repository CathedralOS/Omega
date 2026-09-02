# Pipeline Architecture

Omega's compiler pipeline is a sequence of durable representation boundaries.
Each stage should have one primary job, one input representation, and one output
representation.

The target architecture has one named language/realization boundary. Psi owns
Omega-file parsing and every target-neutral stage through immutable terminal
Psi. Omega consumes terminal Psi and owns installation, optimization, ABI and
storage realization, target operations, and native artifacts. See
[Terminal Psi Architecture](terminal_psi.md). The stage list and matrix below
describe the one production path. Unsupported Terminal-Psi vocabulary rejects;
the compiler does not retain a second source-shaped backend as a fallback.

The same semantic nouns should be recognizable across stages, but their data
shape changes as they become more resolved. Source-shaped IR can only say "this
syntax looks like a place." Checked IR can say "this place overlaps this loan."
Backend IR can say "this place is stack slot plus offset."

## Stage Questions

Every stage document should answer:

- Input representation.
- Output representation.
- Primary responsibility.
- Places, values, facts, loans, moves, drops, calls, transitions, reach, and
  boundary edges.
- What this stage must not own.
- Known gaps.

## Semantic Spine

- Places: location-like expressions that can be read, written, borrowed, moved,
  or invalidated.
- Values: produced runtime or compile-time objects with type, initialization,
  ownership, and storage/lowering consequences.
- Facts: proven or accepted assertions at a program point.
- Loans: active borrows over places or views.
- Moves: ownership transfers that may make a source unusable.
- Drops: lifetime-ending cleanup events.
- Calls: invocations of machines, states, operators, helpers, or imported
  boundary entries.
- Transitions: control and argument transfers between states or exits.
- Reach: externally visible service classes such as allocation, IO, process
  exit, or host interaction.
- Boundary edges: points where Omega accepts a declared contract from
  compiler/runtime/host/toolchain code.

## Ownership Rule

The stage that first creates durable, queryable data for a noun owns that noun's
semantic meaning. Later stages preserve, refine, schedule, lower, encode, or
report that data. Earlier stages may parse or carry syntax for the noun, but
they should not make semantic decisions about it.

Use this rule when a pass starts to sprawl:

- If it discovers identity, it belongs near symbol resolution.
- If it decides type/signature compatibility, it belongs near typing.
- If it proves obligations, records facts, creates loans, or validates reach,
  it belongs near checked trees.
- If it chooses storage, ABI, instruction, relocation, or image form, it belongs
  in the backend lowering stages.

## Cross-Stage Compiler Projections

Some compiler-owned consumers are total observations assembled after successful
checking rather than transformation stages. Package admission is the principal
example. Its projector reads each fact from the earliest existing representation
where that fact is semantically complete, then joins structural identity to any
later checked acceptance, effect, proof, or realization evidence it requires.
"Earliest" does not mean unchecked or merely convenient: unresolved syntax and
diagnostic renderings are never admission evidence.

No single IR must contain the complete package report. The versioned canonical
projection is the compiler/package boundary; the representations and handles
used to derive it stay compiler-private and may evolve with the compiler. A
downstream Psi stage may repeat an invariant as a backstop without becoming the
mandatory place from which the projector reconstructs an already-settled fact.

Do not introduce a nominal `Chi` stage merely to collect these queries or give
them an internally stable interface. Add a named representation only when work
discovers a reusable semantic invariant boundary with its own consumers or
transformations. Prefer an existing coherent representation, including `Exact`,
when it already carries the required meaning.

## Operational Artifact Emission

Pipeline viewers and diagnostic reports are observations of a compilation, not
semantic gates. `ArtifactEmissionPolicy::OutputOnly` therefore suppresses the
HTML, JSON, text, Markdown, timing, and disassembly bundle used for interactive
inspection while preserving every fail-closed validation that normally feeds
those reports. Wire compatibility demands, capability-ledger checks, trust
consistency checks, trust-lock enforcement, and final executable-footprint
certification still run. If output installation is requested, the primary
object or executable and any installation records required by its semantics
still exist; otherwise an output-only check need not create a build directory.
The complete checked result enters one typed checked-observation reporter.
That reporter always reconstructs trust obligations, settles the exact
owner-supplied admissions, constructs the derived trust report, and validates
its target/report joins before consulting observation policy. Its single
policy branch creates the artifact writer only for `Full`, writes the trust
report first, writes the ordered checked snapshots next, and writes
`00_timings.html` last. `CheckedCompilation` retains the first-seen ordered,
repeated-stage-aggregated timing rows needed by that final observation, but
these nondeterministic measurements are explicitly absent from checked
semantic equality. `OutputOnly` therefore follows the same validating path
without a reporting filesystem effect.
Boundary reporting captures its ordered source target, contract, and policy
rows once, writes that initial observation at the source boundary, and later
consumes the same carrier when checked capability facts become available. It
does not rebuild those rows from a retained syntax-tree clone, and checked
capability settlement remains validating under report suppression.
Backend reporting likewise captures one checked-surface observation only for a
full native compilation and consumes it once the corresponding backend plan is
available. Suppressed and non-native products retain canonical absence; the
pipeline driver neither couriers a raw optional report surface nor owns its
conditional publication policy.

Production entry points retain full artifacts by default. Corpus schedulers may
select output-only mode for independent pass/fail compiles whose assertions are
the diagnostics, checked result, or installed primary output. Tests that inspect
a report continue to use full emission. This keeps observability selectable at
the orchestration boundary without turning report generation into language
semantics or duplicating policy through every representation stage.

One private compiler-owned native observation seam retains the exact targetful
`CheckedCompilation` beside the ordinary native `CompileReport` from the same
invocation. Native realization borrows that checked value before the sealed,
non-clone carrier is formed; construction rejoins source count, target profile,
native target, retained artifact, and any production manifest. The sole public
`compile(request)` entry consumes the checked half when it returns the ordinary
report; the lower compilation-report crate does not depend on compiler
representations. Crate-local verification may inspect the pair without growing
a test-only production wrapper. The receipt grants no backend, publication, or
runtime authority to checked trees.

`RequestedCompileProduct::NativeArtifact` is a distinct stopping boundary. It
runs the Psi-owned checked frontend and canonical Terminal producer, then gives
that complete artifact to the source-free native realization path
shared with component staging. It returns exactly one non-clonable payload
owning the canonical Terminal identity, checked target, selected-provider
projection, object and relocation evidence, encoded text, and independently
replayed final executable image. Its report has no executable path,
publication, installation, terminal deployment, or runtime authority and
records that no primary output was written. Unsupported Terminal vocabulary
rejects at this boundary without legacy fallback, and pending component
progress rejects rather than being discarded. Auxiliary observations remain
controlled independently by `ArtifactEmissionPolicy`; with `OutputOnly`,
retained-artifact compilation creates no build directory.

Forwarded local dynamic descriptors preserve this same stop/resume boundary.
Terminal Psi carries only the existential interface, ordered requirements,
descriptor arguments, and parameter-slot dispatch. The selected native
lowerer chooses the `{data, table}` entry placement, generates role-identified
erased-data adapters, and emits distinct forwarded tables. Object and image
replay bind caller-to-table, table-to-adapter, and adapter-to-realization
relocations. Canonical installation format 64 retains the application/row/
realization identities and exact code/data spans needed to rejoin those facts.
Both a direct local selection and a once-rebound descriptor may supply the
one-hop argument under their distinct semantic source identities; forwarding
does not manufacture a rebound merely to obtain a physical descriptor.
Its forwarding rows retain an exact semantic and physical scalar result when
one exists, while Unit rows explicitly retain neither; they do not invent
Terminal machine identities for native-only adapters. The bounded mutable form additionally carries `&mut
self` as one no-copy pointer and replays one Boolean or fixed-integer literal
store into either a direct field or a primitive field one record projection
below it, followed by an independently identified direct scalar field read and
return. Target assignment independently resolves that bounded path and its
accumulated byte offset before native emission. Boolean-returning forwarded
calls use an exact one-byte result home and direct Boolean control carrier;
they do not pass through an invented integer comparison.

Local rebound Unit requirements use the same descriptor/table/receiver call
record without a result carrier. Target assignment reserves only the aligned
two-word descriptor region; machine and object replay require the call plan to
have no result and publish no scalar home. Forwarded Unit requirements use the
same result-neutral rule across two distinct roles: the attached caller
materializes the descriptor and calls the transparent helper, and the helper
calls through its incoming table slot before returning Unit. The generated
adapter reads the realization ABI from the Unit body's native call plan.
Function parameter evidence retains authored access, so a borrowed receiver is
not mistaken for an owned affine value at return.

D32 makes this boundary the owner of physical children for every settled
boundary occurrence. The canonical Terminal artifact remains immutable while
the validated optimization projection identifies the surviving executable
occurrences. Each child's role-tagged `PhysicalChildParent` is either a D29
operator-application coverage reference or a complete replayable D41 boundary-
trait settlement. Equal D29 applications may share one semantic parent, but
each occurrence has exactly one physical child. That child binds its domain-
separated parent and distinct optimized-operation identity while retaining the
target-lowering, instruction-selection, assignment, relocation, and emitted-
span joins. Replay rejects a missing, duplicate, stale, substituted, padded,
or role-swapped child; a verified optimizer elimination alone permits omission.
Semantic settlement and native physical coverage stay distinct even when one
envelope carries both.

The implemented no-optimization identity projection covers Linux ELF
`CompilerBuiltin(LinuxExitGroupI32)` and supported D29 operator applications.
The compiler retains complete reconstructible D29 demand/realization coverage,
including exact-empty custody, and explicit optimizer handoff never enters this
scope. The native artifact independently derives separate role-tagged D29 and
D41 Terminal occurrence sets. Fresh emission derives one child per occurrence;
standalone replay checks exact parents, optimized occurrences, spans, byte
digests, and relocation disposition. D41 `exit_group` children additionally
replay selected-plan, target/catalog, scalar ABI, and direct-byte custody. D29
nearest-FMA children exercise direct-instruction custody end to end. The
checked-body call path rejoins the exact Terminal callee, emitted call record,
semantic relocation owner, target, zero addend, kind, and changed-byte
interval, although ordinary Linux lowering still rejects the reviewed checked-
operator-plus-exit shape before an end-to-end canary can reach that path.

An artifact with an explicit optimization, port effect, normalized foreign
call, admitted native provider, or another unsupported executable evidence
role remains usable but carries no D32 evidence, so later final-realization
admission fails closed instead of mistaking partial coverage for a complete
child set. Checked-body backend enablement, admitted-provider parents, and
non-identity optimized projections remain explicit follow-on lanes.

Cross-field product admission precedes that frontend. The request owner
consumes `CompileRequest` into a private validated request and rejects a
nonempty optimization rollback unless the selected product is
`NativeArtifact`. This preserves rollback as a native-realization-only release
overlay and guarantees an invalid product combination reads no source and
creates no report/output directory. The product driver consumes only the
validated request; it does not inspect rollback contents or own the policy
diagnostic.

D54's multi-target request boundary begins with the compiler-owned
`ExplicitTargetSet`. It accepts one nonempty caller-supplied list, normalizes
legacy CLI aliases, deduplicates and orders only those exact profiles by the
trusted target catalog, and rejects `all`, `*`, empty, or unknown inputs. It
does not infer targets or expand the catalog. This value does not itself fan
out compilation: orchestration must first expose the shared immutable
source/parse/build checkpoint, so wrapping the existing target-sensitive
compiler invocation in a loop would violate the stage boundary.

Package-aware compilation now projects one exact in-memory
`PackageCompilationSourceInputs` value containing every source-routing input:
root role, package identities and canonical names, physical roots, canonical
build-visible source metadata, and requester-local dependency edges. Generated
source bundles and accepted semantic bindings remain exact-target child inputs.
This projection is a checkpoint equality guard, not a durable package identity
or source receipt.

The compiler now forms and validates the corresponding immutable source/parse
checkpoint. Physical roots and unconditional physical imports are parsed once;
an exact child must match the retained package source inputs before it may add
its dependency-generated sources, resolve generated-only imports, or load its
selected target imports. The ordinary one-target path uses that same child, so
it cannot drift from later multi-target orchestration. The checkpoint remains
compiler-private and does not itself loop targets or publish a batch result.

Checked compilation now consumes that source/parse result through the
compiler-private `PreparedCheckedSource` boundary. Shared parse timings and the
immutable source frontier remain on the prepared value; target, exact package
inputs, build staging, filesystem/evaluation sponsors, and replay custody enter
only through a fresh child execution. Reusing the prepared value for a sibling
therefore cannot retain the first child's mutable build or semantic state, and
the ordinary standalone route is one child through the same continuation.
Canonical target-set orchestration and its ordered outcomes remain above this
boundary.

The compiler's `MultiTargetCompileRequest` takes one `ExplicitTargetSet` and a
targetless child-request factory. The set is therefore the only declaration of
target identity; the compiler installs each canonical profile while the
factory supplies that child's package inputs, build directory, admissions, and
lowering policy. Admission requires one root, product, artifact-emission
policy, and package source-input projection, plus distinct declared build
directories, before source acquisition. This catches deterministic sibling
collisions; it does not attest host filesystem isolation or defeat ambient
same-user aliasing and races. `compile_targets` prepares source once and returns
one `ExactTargetCompileOutcome` per canonical profile without fail-fast
collection. A shared preparation failure is repeated as the same diagnostic
set for every requested profile. The outcome set is orchestration data only:
it mints no batch manifest or support/test/audit claim, while successful
children retain their ordinary standalone production reports.

Package-aware controls supply different exact-target generated-source bundles
over one equal package source projection. A malformed generated source rejects
only its child. Native package controls also compare the successful child's
retained artifact and production-manifest identities with and without an
undeclared failing sibling; both remain exact. These are orchestration
invariants, not evidence that either target was audited or supported.

## Representation Root Shape

Durable representations should make their semantic spine visible at the root.
When a representation has both executable/data shape and preserved semantic
evidence, those should be separate named roots rather than a flat bag of arenas.

Current preferred shapes:

- Source-shaped representations use `roots` plus `tables` when identity and
  contiguous storage are the main concerns, for example `TypedTrees`.
- Checked representations use a source/program root plus a facts root, for
  example `CheckedTrees { typed, facts }`.
- Omega operation and artifact representations use a code/shape root plus the
  retained evidence needed to replay the next boundary. Orchestration passes
  complete typed artifacts between stages rather than rebuilding semantic facts
  from source-shaped trees.

This is not ceremony. It makes it obvious whether a pass is changing executable
shape, preserving semantic evidence, or doing both. If a stage starts reaching
through unrelated roots to answer a question, that is a sign the query belongs
behind a unified view or helper instead of being reconstructed ad hoc.

Concrete field layout for backend-ABI carriers is a separate concern from these
representation roots. Fat-descriptor field layout (for slices and text windows,
the shared `{ ptr, len }` carrier) is a backend-ABI concern owned at the
runtime-abi boundary, not redefined by later lowering. Owned and borrowed
carriers share that layout and differ only by an ownership tag in the semantic
spine. Layout and instruction-selection stages consume the descriptor shape
through its owner rather than re-deriving offsets and sizes.

## Semantic Ownership Matrix

This table is intentionally blunt. Each cell says the main relationship between
the stage and the noun: `none`, `syntax`, `identity`, `typed`, `checked`,
`scheduled`, `lowered`, `assigned`, `encoded`, `artifact`, `metadata`, or
`final`.

| Stage | Places | Values | Facts | Loans | Moves | Drops | Calls | Transitions | Reach | Boundaries |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| Source Files To Tokens | none | none | none | none | none | none | none | none | none | token |
| Tokens To Syntax Trees | syntax | syntax | syntax | none | none | none | syntax | syntax | syntax | syntax |
| Syntax Trees To Symbol Resolved Trees | identity | identity | identity | none | none | none | identity | identity | identity | identity |
| Symbol Resolved Trees To Typed Trees | typed | typed | typed | type surface | planned | planned | typed | typed | typed | typed |
| Typed Trees To Checked Trees | checked | checked | checked | checked | checked | checked | checked | checked | checked | checked |
| Checked Trees To Terminal Psi | lowered | lowered | preserved | lowered | lowered | lowered | lowered | lowered | preserved | lowered |
| Terminal Psi To Abstract Operations | lowered | lowered | metadata | assertion | abstract op | abstract op | abstract op | abstract op | op metadata | metadata |
| Optimization Run To Abstract Operations | projected | projected | validated metadata | assertion | projected op | projected op | projected op | projected op | projected metadata | validated metadata |
| Abstract Operations To Target Operations | target | target | metadata | assertion | target op | target op | target op | target op | target op | target metadata |
| Target Operations To Selected Instructions | selected | selected | metadata | none | selected | selected | selected | selected | selected | selected metadata |
| Selected Instructions Through Allocation | assigned | assigned | metadata | none | assigned | assigned | assigned | assigned | assigned | assigned metadata |
| Target Operations To Assigned Target Operations | assigned | assigned | metadata | none | assigned | assigned | assigned | assigned | assigned | assigned metadata |
| Assigned Operations To Machine Code | encoded | encoded | metadata | none | metadata | metadata | encoded call bytes | encoded branch bytes | encoded bytes | encoded metadata |
| Machine Code To Native Artifact | final | final | final layout | none | metadata | metadata | final import/fixup | final branch fixup | final artifact | final metadata |

Current deliberate gaps:

- Terminal Psi is the sole Psi/Omega boundary. Unsupported aggregate, cleanup,
  transfer, boundary, loop, suspension, and ordering slices reject until their
  canonical lowering exists; they do not revive a tree-consuming backend.

- Moves and drops now have durable checked/control-flow event plumbing, but
  event production still needs type-aware precision plus transition and nested
  call coverage.
- Checked values preserve through Terminal Psi, abstract operations, target
  operations, physical assignment, machine code, and native artifacts. Missing
  semantic slices fail at their owning stage.

## Stages

- [Terminal Psi target architecture and migration](terminal_psi.md)

- [Source Files To Tokens](stages/source_files_to_tokens.md)
- [Tokens To Syntax Trees](stages/tokens_to_syntax_trees.md)
- [Syntax Trees To Symbol Resolved Trees](stages/syntax_trees_to_symbol_resolved_trees.md)
- [Symbol Resolved Trees To Typed Trees](stages/symbol_resolved_trees_to_typed_trees.md)
- [Typed Trees To Checked Trees](stages/typed_trees_to_checked_trees.md)
- [Optimization Run To Abstract Operations](stages/optimization_run_to_abstract_operations.md)
- [Abstract Operations To Target Operations](stages/abstract_operations_to_target_operations.md)
- [Target Operations To Selected Instructions](stages/target_operations_to_selected_instructions.md)
- [Selected Instructions To Liveness](stages/selected_instructions_to_liveness.md)
- [Liveness To Live Ranges](stages/liveness_to_live_ranges.md)
- [Live Ranges To Allocation Legality](stages/live_ranges_to_allocation_legality.md)
- [Allocation Legality To Fixed-View Copies](stages/allocation_legality_to_fixed_view_copies.md)
- [Fixed-View Copies To Reanalyzed Legality](stages/fixed_view_copies_to_reanalyzed_legality.md)
- [Allocation Legality To Register Homes](stages/allocation_legality_to_register_homes.md)
- [Target Operations To Assigned Target Operations](stages/target_operations_to_assigned_target_operations.md)
