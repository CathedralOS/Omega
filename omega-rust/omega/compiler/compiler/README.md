# Compiler coordination

[lib.rs](src/lib.rs) exposes source checking, compilation products and explicit
multi-target requests. The coordinator sequences typed owner results; it does
not implement package loading, build evaluation, transformation algorithms or
visualization semantics. See the [pipeline map](../../../pipeline.md).

[compiler.rs](src/compiler.rs) owns request admission, the shared checked
continuation, and the Check / Terminal / Native product dispatch.
[targets.rs](src/compiler/targets.rs) owns exact-target batching and immutable
preparation reuse; every child returns through that same product continuation.

Checked-only consumers supply one `CheckedCompileRequest` to `compile_to_checked`.
Package inputs, build staging, session sponsors and replay evidence are request
data, not alternate compilation entrypoints. The request enters the same prepared
source continuation used by production and target batches.

[Native compilation](src/compiler/native.rs) prepares and realizes the native
product; it is not owned by optional optimization or report writing. Re-entry
from retained Terminal Psi uses `RetainedNativeRealizationRequest`, with the
receiving policy and image request supplied explicitly. `CompileReport` is the
normal returned product record. Executable publication remains a later operation.

## Product boundaries and observations

[Request validation](src/compiler/request.rs) checks the requested product and
rollback selection before acquiring source or opening output directories.
Checking permits no rollback; Terminal publication permits only applicable
Psi-stage rollback; native products permit their applicable phase set. Rollback
is subtractive, not a new optimizer selection or a fallback backend.

A retained native-artifact request runs the checked frontend and canonical
Terminal producer, then the same source-free
[native realization](../native-realization/README.md) used for component staging.
The non-clonable result retains Terminal identity, target, selected providers,
object/relocation evidence, encoded text and independently replayed image.
It writes no primary output and grants no executable path, installation,
publication, deployment or runtime authority. Pending component progress cannot
be silently discarded. Unsupported Terminal vocabulary rejects without another
source-shaped path. [Native artifacts](../../backend/artifacts/native-artifact/README.md)
owns the current physical evidence limits.

`ArtifactEmissionPolicy::OutputOnly` suppresses auxiliary HTML/JSON/text/Markdown,
timing and disassembly, not semantic validation. Wire compatibility, capability
and trust consistency, owner admissions, lock enforcement and executable-footprint
checks still run where required by the product. Requested primary output and
semantically required installation records are independent of this policy.
An output-only check or retained-artifact compile need not create a build directory.

[Checked admission](src/compiler/admission.rs) reconstructs trust obligations,
settles exact owner admissions, and validates the derived trust report without
performing observation I/O. Its result borrows the exact checked program.
[Observation writing](src/pipeline/reporting/checked_observations.rs) consumes that
validated view: Full mode writes trust first, ordered checked snapshots next and
timing last; OutputOnly writes nothing. A writer failure does not revise trust
admission. Checked results
retain first-seen ordered, repeated-stage-aggregated timing observations, but
nondeterministic measurements do not enter semantic equality.

Boundary reporting captures source target/contract/policy rows once and later
joins the same carrier to checked capability facts; it does not retain a syntax
clone merely to reconstruct them. Native reporting similarly captures a checked
surface only for a full native compilation. Suppressed or non-native products
have canonical absence, not a raw optional report passed through every stage.
The private same-invocation checked/native observation seam rejoins source count,
target profile, artifact and production manifest before sealing its non-clone
carrier. The ordinary report consumes the checked half; report data does not
grant checked trees native authority.

Production defaults to full observations. Corpus cases may select output-only
when their assertions concern diagnostics, checked results or primary output;
report-content tests must retain full mode. This is orchestration policy, not
language semantics.

## Intrinsic settlement conversion

[intrinsic_settlements.rs](src/compiler/intrinsic_settlements.rs) validates all
selected plan provenance, then joins sorted borrowed intrinsic rows to lexical
Terminal demand. Original plan indices remain attached to each row. Duplicate
demanded rows reject; undemanded rows need no executable proposal. One exhaustive
optional mapping converts planner classifications into the native catalog and
rejects unsupported executions, as required by the
[boundary contract](../../../../wiki/spec/terminal-psi/boundary_calls.md).

## Immutable source reuse and exact-target children

[Explicit targets](src/compiler/request/targets.rs) accepts a nonempty supplied
set, normalizes supported aliases, deduplicates and orders exact profiles by the
trusted catalog, and rejects wildcards, empty or unknown selections. It neither
infers targets nor certifies platform support.

[Source preparation](src/pipeline/source_assembly/checkpoint.rs) retains immutable
physical sources, unconditional imports and parse results once. Its package
source-input projection includes root roles, exact package identities/names,
physical roots, build-visible metadata and requester-local dependencies. It is
a private checkpoint equality guard, not durable package identity or a receipt.
Each target child must match that input before joining generated bundles,
generated-only imports or selected target imports. Prepared checked input shares
the source frontier and parse timings, not mutable semantic/build state, sponsor,
evaluation replay or target authority. The ordinary one-target route uses the
same child continuation. [Generated source](generated_source.md) owns append
custody; [checked settlement](checked_settlement.md) owns its later ordered joins.

[Multi-target admission](src/compiler/request/multi_target.rs) takes a targetless
child-request factory; the explicit set is the only source of child target
identity. Root, product, observation policy and package source projection must
agree, and declared child build directories must differ before acquisition.
That detects deterministic collisions, not host filesystem aliases or races.
Compilation collects one ordered outcome per target without fail-fast collection;
a shared preparation failure supplies the same diagnostics to all children.
A target-specific malformed generated unit fails its child, not an unrelated
sibling. Success retains the ordinary standalone artifact/manifest identity.
The collection grants no batch manifest, support, test or audit claim.

[Native preparation reuse](../native-realization/README.md#multi-target-reuse)
has its own exact artifact/profile/selection key. Shared parsing never permits
reusing one child's checked target, providers, admissions or physical evidence
as another child's authority.
