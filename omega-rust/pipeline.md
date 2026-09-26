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
| Source files → tokens | [source-files-to-tokens](psi/pipeline/00_source-files-to-tokens/src/lib.rs) |
| Tokens → syntax trees | [tokens-to-syntax-trees](psi/pipeline/01_tokens-to-syntax-trees/src/lib.rs) |
| Syntax → symbol-resolved trees | [syntax-trees-to-symbol-resolved-trees](psi/pipeline/02_syntax-trees-to-symbol-resolved-trees/src/lib.rs) |
| Resolved → typed trees | [symbol-resolved-trees-to-typed-trees](psi/pipeline/03_symbol-resolved-trees-to-typed-trees/src/lib.rs) |
| Typed → checked trees | [typed-trees-to-checked-trees](psi/pipeline/04_typed-trees-to-checked-trees/README.md) |
| Checked trees → lowered Psi | [checked-trees-to-lowered-psi](psi/pipeline/05_checked-trees-to-lowered-psi/README.md) |
| Lowered Psi → lowered Psi | [lowered-psi-to-lowered-psi](psi/pipeline/06_lowered-psi-to-lowered-psi/src/lib.rs) |
| Lowered Psi → Terminal Psi | [lowered-psi-to-terminal-psi](psi/pipeline/07_lowered-psi-to-terminal-psi/src/lib.rs) |
| Terminal Psi → abstract operations | [terminal-psi-to-abstract-operations](omega/pipeline/00_terminal-psi-to-abstract-operations/README.md) |
| Abstract → abstract operations | [abstract-operations-to-abstract-operations](omega/pipeline/01_abstract-operations-to-abstract-operations/src/lib.rs) |
| Abstract → target operations | [abstract-operations-to-target-operations](omega/pipeline/02_abstract-operations-to-target-operations/README.md) |
| Target operations → selected instructions | [target-operations-to-selected-instructions](omega/pipeline/03_target-operations-to-selected-instructions/README.md) |
| Selected → selected instructions | [selected-instructions-to-selected-instructions](omega/pipeline/04_selected-instructions-to-selected-instructions/src/lib.rs) |
| Selected instructions → register homes | [selected-instructions-to-register-homes](omega/pipeline/05_selected-instructions-to-register-homes/README.md) |
| Register homes → post-allocation machine | [register-homes-to-post-allocation-machine](omega/pipeline/06_register-homes-to-post-allocation-machine/src/lib.rs) |
| Post-allocation machine → selected-form encoding | [post-allocation-machine-to-selected-form-encoding](omega/pipeline/07_post-allocation-machine-to-selected-form-encoding/src/lib.rs) |
| Selected-form encoding → resolved layout | [selected-form-encoding-to-resolved-layout](omega/pipeline/08_selected-form-encoding-to-resolved-layout/src/lib.rs) |
| Resolved → resolved layout | [resolved-layout-to-resolved-layout](omega/pipeline/09_resolved-layout-to-resolved-layout/src/lib.rs) |
| Resolved program → machine bytes | [machine-emission](omega/backend/machine-emission/README.md) |
| Machine bytes → object/image evidence | [image-emission](omega/backend/images/image-emission/src/lib.rs) |

## One driver, one pass, per-target realization

The product compiler is one driver that feeds each stage's output into the
next. Psi's stages 00 through 07 run once per compilation and never observe a
target: target-scoped machine bodies are data checked for every target, and
the Build evaluates once into rows keyed by target
([multi-target compilation](../wiki/spec/build/configuration.md#multi-target-compilation)).
Omega then realizes each target in the realization set from the same Terminal
Psi and that target's rows: provider selection and installation in stage 00,
then stages 01 through 09 and image emission. A consumer that needs only a
prefix of the route, such as package review discovering bindings or a test
checking a fixture, calls the same stage functions itself; there are no
alternate entry points, mode flags, or preliminary passes.

The current Rust implementation does not meet this yet. Three orchestration
crates outside the pipeline drive the Psi stages, evaluate the Build per
target, check twice (a preliminary pass before build evaluation and a settled
pass after provider settlement), and settle providers on checked trees before
Terminal Psi:

| Input → output | Owner |
| --- | --- |
| Source files → assembled syntax | [source-files-to-assembled-syntax](omega/build/build-evaluation/src/sources/source_assembly.rs) |
| Assembled syntax → checked compilation | [assembled-syntax-to-checked-compilation](omega/compiler/src/checked/checking.rs) |
| Checked compilation → Terminal artifact | [checked-compilation-to-terminal-artifact](omega/compiler/src/terminal/terminal_artifact.rs) |

The [pipeline route items](../TASKS.md#pipeline-route) dissolve them: source
loading becomes stage 00 input preparation, the Psi-owned work between stages
moves inside stages 02 through 04, and provider settlement moves behind
Terminal Psi. Until then the layering test ranks these crates with the
compiler that schedules them.

## Placement and semantic ownership

| Responsibility | Owner |
| --- | --- |
| Durable current program, identities, raw evidence and codecs | `representations/` |
| Language meaning and independently reusable validity/proof | `semantics/` |
| Transformation, rewrite, private scratch and analyses | Owning `pipeline/X-to-Y` or `X-to-X` |
| ISA, ABI, runtime carriers, object/relocation/image mechanics | Omega `backend/` |
| Shared target-neutral source/arena/numeric primitives | Psi `foundation/` |
| Product sequencing, requested outputs and composition policy | Compiler/build owners |

Omega has no second general foundation bucket: dependency-light native identities
and carriers belong in its representations; target/runtime primitives belong in
its backend. Psi must not depend on Omega. A coordinator forwards complete typed
results rather than owning package loading, build evaluation, stage algorithms,
artifact formatting, or a generic orchestration subsystem. Allocation counters
and phase-report deltas belong to [artifacts](omega/tooling/artifacts/src/compile_timings/mod.rs),
not program representations or a dependency-floor core.

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
walk to obtain current data. See [native representation ownership](omega/representations/README.md).
Arena handles and spans are the default for durable repeated children; source
text is diagnostic/debug payload after resolution, user literals remain program
payload, and linker/display names are edge metadata. Scoped symbol-tree lookup
is the baseline; extra lookup maps require a measured reason.

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
[projection owner](omega/packages/review/evidence/README.md) reads each fact at the
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
contracts, while [compiler coordination](omega/compiler/README.md) owns
operational reports and product stopping boundaries.

## Psi implementation and deferred human audit

Until the owner requests otherwise, human review and audit of Psi are deferred.
Implementers own the IR, operation vocabulary, encoding,
and reconstruction choices needed to express all accepted Omega behavior. Do not
block those choices on owner approval or add owner questions for them. Choose
cohesive representations, update their specifications and versioned schemas, and
carry the change through producers, consumers, and customer acceptance.

This defers human design review, not compiler checking or required evidence.
Keep independent verification, rejection controls, and applicable tests; do not
declare deferred audits complete. Preserve source-language meaning, observable
behavior, trust guarantees, and the Psi/Omega firewall. Escalate genuinely missing
language or trust decisions, not how existing behavior is represented in Psi.


## Crate placement rule

Workspace crate names encode the pipeline. Within both halves:

Internal package and folder names omit the enclosing `omega-` or `psi-`
namespace. Keep the shipped `omega` package name. Cargo names are unique across
the workspace; use descriptive ownership names rather than duplicate generic
names (Psi's `semantic-vocabulary` and `flow-effects`, for example).

- `foundation/` — shared vocabulary, arenas, symbols, diagnostics.
- `representations/` — durable IR structs.
- `pipeline/` — transforms only; crate names read literally as `X-to-Y`
  (`source-files-to-tokens` → `tokens-to-syntax-trees` →
  `syntax-trees-to-symbol-resolved-trees` →
  `symbol-resolved-trees-to-typed-trees` →
  `typed-trees-to-checked-trees` → `checked-trees-to-lowered-psi` →
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
- `semantics/` — language meaning, validation, proof, interpreters.
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

Use [main.rs](omega/src/main.rs) and
[compiler.rs](omega/compiler/src/compiler.rs) as the gold
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

Source-shape admission is a code-rot warning: supported assignment, call, branch,
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
ordinary combination. Independent verification must reconstruct and check the
operation/evidence relationships, not trust a producer assertion or rediscover
the original source idiom at each stage.

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

