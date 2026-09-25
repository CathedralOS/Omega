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
test-group, behavior-exclusion, optimization, and output selections. It does not
serialize source/staging roots, handles, or admitted host-service authority. Image/application facts remain
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

## Proof-carrying output

Normalized Build retains two independent, false-by-default PCC requests: Psi
and native. The intended fields are `builder.pcc.psi` and `builder.pcc.native`.
[Proof publication](../proofs/publication.md) owns product compatibility,
`<artifact>.proof` sidecars, complete-pair publication and outcome classification.
These selections affect portable evidence production, not ordinary language
checking, optimizer correctness or receiving authority. A receiver independently
selects its versioned policy package and concrete configuration.

## Test groups

[Requirement-based tests](testing.md) register exact product machine requirements
through `builder.tests.group<Requirement>()`. Each enabled group collects its
package-local concrete satisfiers for Terminal Psi execution before successful
publication. Groups own provider configuration, not new authority. Root grants
can only be attenuated; selecting a real provider cannot escape virtual backing.
The group and global `disabled` fields default to false. No registered groups
means no automatic tests, not an implicit core/std dependency or hidden group.
Group selection belongs to evaluated build configuration; it does not change
the unconditional dependency-discovery rules.

## Behavior exclusions

The root may require the selected product to exclude exact crash causes, abstract
boundary services, or existing physical authority classes. These are additional
admission requirements, empty by default, independent of provider permission and
ordinary callable contracts. Repeated exclusions combine monotonically by union.
They are evaluated product selections, not dependency-discovery declarations.

[Behavior exclusions](behavior_exclusions.md) defines the complete execution scope,
evidence, failure outcomes, and installation obligations. A verified selected
composition can satisfy a stronger exclusion than its broad public declarations
promise without changing those declarations or making them satisfy narrower
callable requirements. Source checking still enforces every ordinary contract.
No debug/release mode, assertion primitive, or new crash cause is implied.

## Exact target requests

An absent selection realizes every deployable target profile the toolchain
closure provides. An explicit selection accepts one exact profile or a
caller-supplied nonempty set, normalized to canonical order with duplicates
removed; it narrows realization, never checking. Reject `all`, `*`, empty
explicit sets, and inference from source or dependencies. The catalog's
abstract/local modes and inactive bindings cannot define deployment intent and
realize only when selected explicitly.
Authored target-policy/activation declarations are not admitted source syntax
and do not define a support matrix.

A CLI Host convenience selects the realization set before semantic
evaluation; Host is not artifact identity. The Build does not observe a
selected target: target-dependent selections are rows keyed by target, and
dependency discovery cannot branch on a target. Target schemas and
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

### Target recognition and implementation availability

Recognizing a profile and its slot identities does not require implementing its
checking or artifact backend. An inactive binding to a recognized profile does
not require that profile's backend; unknown profile names still reject rather
than silently becoming inactive. Selected requests require the implementation
of the requested operation. An unavailable operation reports a target/product
not-implemented diagnostic, not successful checking or a fallback artifact.

This applies to `alpha_bootstrap`: the Rust reference compiler recognizes its
inactive bindings when compiling the Omega-written compiler for a native target,
but may report Alpha compilation as not implemented when selected. The bootstrap
does not require a Rust Alpha backend. Profile and slot identity remain
target-package facts; do not create a parallel bootstrap selection mechanism or
infer support from the mere presence of those declarations.

## Multi-target compilation

One compilation serves every provided target. Psi runs once: source loading,
parsing, resolution, typing, checking, proof, and Terminal Psi production do
not observe a selected target. A target-scoped machine declaration
(`windows_x86_64 machine Owner::name(...)`) is one body of a family keyed by
target under one path; every body of every family is resolved, typed, and
checked on every compilation, so a compile on a Windows host reports a macOS
body's errors. Terminal Psi carries every body with its target tag. A family
is data selected by target, never a branch removed before checking.

Build evaluation runs once for the whole compilation. It does not observe a
single selected target and cannot branch on one; target-dependent selections
(provider plans, program entry, subsystem, application intent, opaque
representation, grants, behavior exclusions) are rows keyed by target in the
one evaluated Build. A row for a target outside the realization set is checked
and retained, not realized.

Realization is per target. The realization set defaults to every deployable
target profile the toolchain closure provides; an explicit `--target` selection
narrows realization only and never narrows checking. Omega selects each
family's body and each row for the target it realizes. A realized target with
no body for a called family, or no row for a required selection, rejects that
realization; it does not invalidate the portable semantics or another target's
realization. Return one outcome per realized target; one realization's failure
cannot suppress another's. An aggregate exit code or human summary is
orchestration, not support, audit, or completeness evidence.

An optional batch manifest binds the realization set and each realization's
commitment/outcome, not completeness of a support/test/deployment matrix. A
fat/universal artifact is a separate explicit envelope over independently
committed realizations. CI may cross-compile several targets on one host or
check without physical realization; neither activity creates a language
support set.

The Rust reference compiler does not implement this contract yet. It checks
every target's bodies on every compilation (the other targets' bodies lower as
sibling declarations and are pruned once checking has committed), but it
still selects the realized target's declarations before resolution, evaluates
the Build per target, and settles providers on checked trees before Terminal
Psi. The [pipeline route items](../../../TASKS.md#pipeline-route) own the
repair.

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
