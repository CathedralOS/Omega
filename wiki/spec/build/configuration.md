# Build configuration and target selection

[Declarations](declarations.md) establish project identity and dependencies;
[execution](execution.md) admits build work. This contract separates activation
authority from durable selections and exact-target results.

## Activation filesystem scope

Build receives one immutable source root for the exact package occurrence and
one fresh writable staging root for the build occurrence. Sponsored operations
use compiler-owned facets, not runtime `FilesystemHost`. Roots, handles, and
rooted paths expire with the activation and cannot enter runtime data or the
durable build result.

[Snapshot and staging semantics](scoped_execution.md#snapshot-and-staging-protocol)
require an explicit captured inventory, deterministic logical observations, and
append-and-seal files. Extra captured inputs have separately scoped roots; no
ambient consumer tree or live-host authority follows from a dependency. Existing
live-filesystem modes must be explicitly admitted and identified as transitional,
not represented as snapshot-isolated execution or silently cached as such.

A facet binds canonical relative bytes to an exact root occurrence. An erased
qualification over bare bytes cannot replace that identity. Reject absolute
input, traversal outside the root, ambiguous membership, and symlink escape
before host access. Authorized path-returning operations, including
canonicalization, preserve the same root or reject. `read_link` returns inert
target bytes; following them requires new checked resolution. An outside link
may be inspected but not traversed by the grant.

Staging a write does not include it in compilation. Explicit handoff follows
successful evaluation and evidence custody; failure discards the staging
occurrence. Evidence paths such as `/source/assets/font.bin` and `/output/font.bin`
are serialization spellings, not prefixes that can mint authority. Identity
retains root role/occurrence and relative bytes, never a host path or working
directory.

## Durable result

Normalized Build carries pipeline-consumed target, dependency, root, provider,
optimization, and output selections. It does not serialize source/staging roots,
handles, or admitted host-service authority. Image/application facts remain
build-owned; signing-affecting versus publication-only metadata follows
[application publication](macos_application.md).

Optimizations form an exact, empty-by-default set of individually named,
semantics-preserving families. During the experimental phase only the root's
authoritative `build.omg` enables them, not dependency metadata or embedding
defaults. Selection is independent of target, debug data, diagnostics, assertions,
and packaging. There are no debug/release or O1/O2/O3 optimization categories.
[Optimization phases](optimizations.md#phase-and-product-boundaries)
owns phase vocabulary, canonical selection, and fail-closed execution.

Hosted/freestanding environment, default providers, calling policies, fault and
resource supply belong to the selected target profile, not duplicated mutable
Build flags. Project role, roots, dependencies, outputs, image intent, and
provider selection remain build facts. Target-qualified bindings select facts;
they do not redefine target policy.

## Exact target requests

Accept one exact profile or a caller-supplied nonempty set, normalized to
canonical order with duplicates removed. Reject `all`, `*`, empty sets, and
inference from source, dependencies, or the complete toolchain catalog. That
catalog includes abstract/local modes and cannot define deployment intent.
Authored target-policy/activation declarations are not admitted source syntax
and do not define a support matrix.

A CLI Host convenience resolves before semantic evaluation; Host is not artifact
identity. Each child sees immutable `Build.target` and cannot assign, substitute,
or multiply it, or branch dependency discovery on it. Target schemas and
observation vocabulary are closed and toolchain-owned; concrete profiles come
from validated target packages in the selected toolchain closure, not authored
reinterpretation or a forever-fixed compiler enum. Use exact canonical spellings,
not extra aliases such as `windows_x64` for `windows_x86_64`.

Admissibility depends on product role and output mode: an executable application
needs its selected ProgramEntry; an explicitly artifact-only application requires
completed artifacts and rejects executable root/provider selections. A component
closes declared slots; a library or target-neutral artifact does not acquire a
native entry merely to pass checking. Artifact-only mode still binds an explicit
target and does not imply target-neutral identity. Target
semantics, ABI/layout, resources, and reach must validate. This is mechanical
closure, not a claim of human testing. Target assumptions must be checked facts.

## Staged multi-target execution

Share acquisition, immutable source, parsing, and syntax-projectable project
role/name and unconditional dependency rows. Fan out at each first target-
sensitive consumer: roots, declaration filtering, semantics/admission,
provider/foreign selection, build execution, and native realization. Parsed
target-qualified rows may be shared; slot/schema validation and entry selection
remain child-local. Evaluated Build state, observations, generated output, and
optimization/provider selections are not shared build facts.

Each child has the same subject, identity, diagnostics, and outcome as its
standalone exact-target invocation. Adding siblings cannot alter it; one failure
cannot suppress another child's checking. Return one outcome per canonical
target. An aggregate exit code or human summary is orchestration, not support,
audit, or completeness evidence.

Immutable target-neutral products may feed multiple children only after their
governing strong identities match. Each consumer retains its own target,
admission, provider/external bindings, and realization authority, and may reject
independently. Share additional work only when it is exactly the fact each
independent child would consume, never mutable target state or authority.
No unresolved target branch is encoded inside Terminal Psi.

An optional batch manifest binds the explicit set and child commitments/outcomes,
not completeness of a support/test/deployment matrix. A fat/universal artifact
is a separate explicit envelope over independently committed subjects. CI may
cross-compile several targets on one host or check without physical realization;
neither activity creates a language support set.

## Directional wire compatibility

The authoritative Build may request
`builder.require_wire_compatibility<Edge, Lineage, Local, Peer, ...>();`.
Only the directional facts named after the first four type arguments are required.
Evaluate them against published schema, codec, unknown-member, canonicalization,
and `FormatMigration` evidence; report every fact and reject unmet requests.
This is channel/store deployment policy, not intrinsic version metadata on the
two data types.

The first four arguments identify the edge, lineage, local schema, and peer
schema. Remaining arguments come from the closed vocabulary:

| Fact | Demand |
| --- | --- |
| `Readable` | The local decoder accepts every peer value. |
| `Writable` | The peer decoder accepts every local value. |
| `PreserveUnknown` | Unknown information is preserved for relay. |
| `Canonical` | The selected encoding produces canonical bytes. |
| `CompleteMigration` | Selected `FormatMigration<Lineage, Old, New>` conformances provide the required peer-to-local migration route. |

Omitted facts remain reported but cannot reject the build. The wire report
retains schema/codec identities, numbered and retired members, accepted
historical shapes, migration routes, unknown-member behavior, canonicalization,
and derived/admitted realization provenance. `04_wire_protocols.txt` reports
every directional fact and its explanation before unmet requested facts reject.
Strict decoding cannot satisfy preservation merely by validating known fields.
