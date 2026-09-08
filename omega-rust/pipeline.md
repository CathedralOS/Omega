# Compiler pipeline and ownership

Psi owns source processing and target-neutral judgments through immutable
[Terminal Psi](../wiki/spec/terminal-psi/product.md). Omega consumes that product
under separately supplied realization authority. Unsupported vocabulary rejects
at its owning boundary; there is no source-shaped or assigned-program backend
fallback. Checked-tree interpretation remains a Psi reference/build-time service,
not another route to native publication.

## Connected program route

Each link below enters an actual transform owner. Private analyses and target
setup are not additional public program stages.

| Input → output | Owner |
| --- | --- |
| Source files → tokens | [source-files-to-tokens](psi/pipeline/source-files-to-tokens/src/lib.rs) |
| Tokens → syntax trees | [tokens-to-syntax-trees](psi/pipeline/tokens-to-syntax-trees/src/lib.rs) |
| Syntax → symbol-resolved trees | [syntax-trees-to-symbol-resolved-trees](psi/pipeline/syntax-trees-to-symbol-resolved-trees/src/lib.rs) |
| Resolved → typed trees | [symbol-resolved-trees-to-typed-trees](psi/pipeline/symbol-resolved-trees-to-typed-trees/src/lib.rs) |
| Typed → checked trees | [typed-trees-to-checked-trees](psi/pipeline/typed-trees-to-checked-trees/README.md) |
| Checked trees → lowered Psi | [checked-trees-to-lowered-psi](psi/pipeline/checked-trees-to-lowered-psi/src/lib.rs) |
| Lowered Psi → lowered Psi | [lowered-psi-to-lowered-psi](psi/pipeline/lowered-psi-to-lowered-psi/src/lib.rs) |
| Lowered Psi → Terminal Psi | [lowered-psi-to-terminal-psi](psi/pipeline/lowered-psi-to-terminal-psi/src/lib.rs) |
| Terminal Psi → abstract operations | [terminal-psi-to-abstract-operations](omega/pipeline/terminal-psi-to-abstract-operations/README.md) |
| Abstract → abstract operations | [abstract-operations-to-abstract-operations](omega/pipeline/abstract-operations-to-abstract-operations/src/lib.rs) |
| Abstract → target operations | [abstract-operations-to-target-operations](omega/pipeline/abstract-operations-to-target-operations/README.md) |
| Target operations → selected instructions | [target-operations-to-selected-instructions](omega/pipeline/target-operations-to-selected-instructions/README.md) |
| Selected → selected instructions | [selected-instructions-to-selected-instructions](omega/pipeline/selected-instructions-to-selected-instructions/src/lib.rs) |
| Selected instructions → register homes | [selected-instructions-to-register-homes](omega/pipeline/selected-instructions-to-register-homes/README.md) |
| Register homes → post-allocation machine | [register-homes-to-post-allocation-machine](omega/pipeline/register-homes-to-post-allocation-machine/src/lib.rs) |
| Post-allocation machine → selected-form encoding | [post-allocation-machine-to-selected-form-encoding](omega/pipeline/post-allocation-machine-to-selected-form-encoding/src/lib.rs) |
| Selected-form encoding → resolved layout | [selected-form-encoding-to-resolved-layout](omega/pipeline/selected-form-encoding-to-resolved-layout/src/lib.rs) |
| Resolved → resolved layout | [resolved-layout-to-resolved-layout](omega/pipeline/resolved-layout-to-resolved-layout/src/lib.rs) |
| Resolved program → machine bytes | [machine-emission](omega/backend/machine-emission/README.md) |
| Machine bytes → object/image evidence | [image-emission](omega/backend/images/image-emission/src/lib.rs) |

[Terminal production](psi/compiler/terminal-production/README.md) sequences its
Psi stages; [native realization](omega/compiler/native-realization/README.md)
sequences the separately admitted native continuation. Selected passes execute
at their explicit X-to-X phase. Empty and supported nonempty selections use the
same downstream representation and publication route. No construction stage
depends on a later optimizer; baseline layout construction and optional layout
relaxation are separate transforms over the same current layout vocabulary.

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
and phase-report deltas belong to [artifacts](omega/tooling/artifacts/src/lib.rs),
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
contracts, while [compiler coordination](omega/compiler/compiler/README.md) owns
operational reports and product stopping boundaries.
