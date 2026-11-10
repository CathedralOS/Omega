# Build configuration and target selection

Source: `wiki/spec/build/configuration.md` lines 1–308 (read).

## Rules

| rule id | the exact sentence, quoted | the heading it is under | what must be checked, in plain words | when it must hold | lane |
| --- | --- | --- | --- | --- | --- |
| CONFIGURATION1 | "Roots, handles, and rooted paths expire with the activation and cannot enter runtime data or the durable build result." | Activation filesystem scope | Rooted activation paths and handles must not be persisted into runtime data or normalized durable Build output. | not stated | 9 |
| CONFIGURATION2 | "Sponsored operations use compiler-owned facets, not the runtime filesystem services." | Activation filesystem scope | Build-sponsored filesystem access must use compiler-owned facets, not runtime filesystem services. | not stated | 9 |
| CONFIGURATION3 | "require an explicit captured inventory, deterministic logical observations, and append-and-seal files." | Activation filesystem scope | Snapshot/staging protocol must use explicit captured inventory, deterministic logical observations, and append-and-seal files. | not stated | 9 |
| CONFIGURATION4 | "Extra captured inputs have separately scoped roots; no ambient consumer tree or live-host authority follows from a dependency." | Activation filesystem scope | Dependency inputs must use separate scoped roots; consumer tree and live-host authority must not be inferred from a dependency alone. | not stated | 9 |
| CONFIGURATION5 | "Existing live-filesystem modes must be explicitly admitted and identified as transitional, not represented as snapshot-isolated execution or silently cached as such." | Activation filesystem scope | Live-filesystem modes must be explicitly admitted and marked transitional; must not be treated as snapshot-isolated or silently cached as snapshot-isolated. | not stated | 9 |
| CONFIGURATION6 | "An erased qualification over bare bytes cannot replace that identity." | Activation filesystem scope | Filesystem identity must use facet-bound root occurrence and relative bytes; bare erased bytes must not substitute. | not stated | 9 |
| CONFIGURATION7 | "Reject absolute input, traversal outside the root, ambiguous membership, and symlink escape before host access." | Activation filesystem scope | Reject absolute paths, out-of-root traversal, ambiguous membership, and symlink escape before host filesystem access. | not stated | 9 |
| CONFIGURATION8 | "Authorized path-returning operations, including canonicalization, preserve the same root or reject." | Activation filesystem scope | Path-returning authorized operations must stay within the same root or reject. | not stated | 9 |
| CONFIGURATION9 | "`read_link` returns inert target bytes; following them requires new checked resolution." | Activation filesystem scope | `read_link` must return inert target bytes only; following a link requires a new checked resolution step. | at the operation | 9 |
| CONFIGURATION10 | "An outside link may be inspected but not traversed by the grant." | Activation filesystem scope | Links outside the granted root may be inspected but must not be traversed under the grant. | not stated | 9 |
| CONFIGURATION11 | "Staging a write does not include it in compilation." | Activation filesystem scope | Staged writes must not be treated as compilation inputs until explicit handoff. | not stated | 9 |
| CONFIGURATION12 | "failure discards the staging occurrence." | Activation filesystem scope | Failed build evaluation must discard the staging occurrence. | not stated | 9 |
| CONFIGURATION13 | "Evidence paths such as `/source/assets/font.bin` and `/output/font.bin` are serialization spellings, not prefixes that can mint authority." | Activation filesystem scope | Serialized evidence path spellings must not be used to mint filesystem or build authority. | not stated | 9 |
| CONFIGURATION14 | "Identity retains root role/occurrence and relative bytes, never a host path or working directory." | Activation filesystem scope | Filesystem identity in build evidence must use root role/occurrence and relative bytes, not host paths or working directories. | not stated | 9 |
| CONFIGURATION15 | "It does not serialize source/staging roots, handles, or admitted host-service authority." | Durable result | Normalized durable Build must omit source/staging roots, handles, and admitted host-service authority. | not stated | 9 |
| CONFIGURATION16 | "Optimizations form an exact, empty-by-default set of individually named, semantics-preserving families." | Durable result | Optimization selection must be an exact named set defaulting empty. | not stated | 9 |
| CONFIGURATION17 | "During the experimental phase only the root's authoritative `build.omg` enables them, not dependency metadata or embedding defaults." | Durable result | In experimental phase, only root authoritative `build.omg` may enable optimizations; dependency metadata and embedding defaults must not. | not stated | 9 |
| CONFIGURATION18 | "There are no debug/release or O1/O2/O3 optimization categories." | Durable result | Must not admit debug/release or O1/O2/O3 optimization category selections. | not stated | 9 |
| CONFIGURATION19 | "Hosted/freestanding environment, default providers, calling policies, fault and resource supply belong to the selected target profile, not duplicated mutable Build flags." | Durable result | Target-profile environment, default providers, calling policies, and fault/resource supply must not be duplicated as mutable Build flags. | not stated | 9 |
| CONFIGURATION20 | "Target-qualified bindings select facts; they do not redefine target policy." | Durable result | Target-qualified bindings may only select facts; must not redefine target policy. | not stated | 9 |
| CONFIGURATION21 | "`builder.proof.index_expansion_terms`, a positive `u64` with default `64`." | Proof-work policy | Configured `index_expansion_terms` must be positive `u64`; effective default must be `64` when unset. | not stated | 8 |
| CONFIGURATION22 | "Zero is rejected rather than treated as unlimited." | Proof-work policy | Zero `index_expansion_terms` must reject, not be treated as unlimited. | not stated | 8 |
| CONFIGURATION23 | "Only the root may select or raise the effective allowance." | Proof-work policy | Only root `build.omg` may set or raise effective `index_expansion_terms` allowance. | not stated | 9 |
| CONFIGURATION24 | "Dependencies may publish expected discovery cost and mathematical claims, but cannot increase the root's grant." | Proof-work policy | Dependencies must not increase the root's proof index-expansion grant. | not stated | 9 |
| CONFIGURATION25 | "preflight or Build-stage checking uses its already granted policy, never a later setting to justify its own admission." | Proof-work policy | Preflight and Build-stage proof checking must use policy already granted, not a later setting to justify admission. | not stated | 8 |
| CONFIGURATION26 | "Record the effective policy at each sponsored proof-discovery request under the [staged execution rules](execution.md#admission-before-execution)." | Proof-work policy | Each sponsored proof-discovery request must record the effective policy per staged execution rules. | at a call | 8 |
| CONFIGURATION27 | "Retain the applicable usage receipt or certified resource bound under [proof-work accounting](../resources/logical_work.md#proof-discovery-and-evidence-checking)." | Proof-work policy | Proof-work policy usage must retain applicable usage receipt or certified resource bound. | not stated | 8 |
| CONFIGURATION28 | "Host speed, scheduling and elapsed time cannot choose semantic outcomes." | Proof-work policy | Wall-clock or scheduling must not determine proof or compatibility semantic outcomes. | not stated | 8 |
| CONFIGURATION29 | "Exhaustion leaves an unproved obligation and a resource diagnostic; it grants no equality, inequality, implementation selection or runtime fallback." | Proof-work policy | Proof-work exhaustion must leave obligation unproved with resource diagnostic and grant no semantic fallback. | not stated | 8 |
| CONFIGURATION30 | "Increasing this allowance under the same source, operations, assumptions and otherwise sufficient resource policy must preserve smaller-allowance successes." | Proof-work policy | Raising index-expansion allowance must not invalidate successes that succeeded under a smaller allowance. | not stated | 8 |
| CONFIGURATION31 | "Larger discovery can establish more compatibility; it cannot rename types, merge declarations, alter canonical matching, or change specialization keys." | Proof-work policy | Increased proof discovery must not change types, declarations, canonical matching, or specialization keys. | not stated | 8 |
| CONFIGURATION32 | "A smaller discovery allowance cannot invalidate that reuse." | Proof-work policy | Smaller discovery allowance must not invalidate reuse of published compatibility judgments from trusted artifacts. | not stated | 8 |
| CONFIGURATION33 | "Loading and preparation have separate resource limits; exhaustion there reports resource refusal rather than failed theorem discovery." | Proof-work policy | Loading/preparation resource exhaustion must report resource refusal, not failed theorem discovery. | not stated | 8 |
| CONFIGURATION34 | "The backend validates a nonempty absolute target pathname without embedded NUL bytes and emits it in `PT_INTERP`." | ELF executable loader selection | For selected ELF interpreters, backend must validate nonempty absolute path without embedded NUL and emit `PT_INTERP`. | not stated | 9 |
| CONFIGURATION35 | "Do not probe the build host, require that path to exist on the build host, or silently choose its loader during cross-compilation." | ELF executable loader selection | Must not probe build host, require interpreter path on build host, or silently pick host loader when cross-compiling. | not stated | 9 |
| CONFIGURATION36 | "If a dynamic executable requires an interpreter and neither an explicit selection nor a declared default supplies one, reject with a missing-output-configuration diagnostic." | ELF executable loader selection | Dynamic ELF needing interpreter without explicit selection or declared default must reject with missing-output-configuration. | not stated | 9 |
| CONFIGURATION37 | "A statically linked executable needs no interpreter pathname and emits no `PT_INTERP`;" | ELF executable loader selection | Statically linked executables must not require interpreter path or emit `PT_INTERP`. | not stated | 9 |
| CONFIGURATION38 | "A loader path alone neither requests dynamic linking nor makes foreign imports compatible." | ELF executable loader selection | Interpreter path selection alone must not imply dynamic linking or foreign import compatibility. | not stated | 9 |
| CONFIGURATION39 | "The selected runtime and imported libraries must still match the target deployment." | ELF executable loader selection | Runtime and imported libraries must match target deployment regardless of interpreter path. | not stated | 9 |
| CONFIGURATION40 | "Root grants can only be attenuated; selecting a real provider cannot escape virtual backing." | Test groups | Test-group provider selection may only attenuate root grants; real provider cannot escape required virtual backing. | not stated | 9 |
| CONFIGURATION41 | "No registered groups means no automatic tests, not an implicit core/std dependency or hidden group." | Test groups | Absence of registered test groups must mean no automatic tests and no implicit core/std or hidden group. | not stated | 9 |
| CONFIGURATION42 | "Group selection belongs to evaluated build configuration; it does not change the target-independent dependency-request and final-join rules." | Test groups | Test group selection must not alter target-independent dependency-request or final-join rules. | not stated | 9 |
| CONFIGURATION43 | "Repeated exclusions combine monotonically by union." | Behavior exclusions | Repeated behavior exclusions must combine by set union without removing prior restrictions. | not stated | 9 |
| CONFIGURATION44 | "They are evaluated product selections, not dependency requests." | Behavior exclusions | Behavior exclusions must be evaluated as product Build selections, not dependency requests. | not stated | 9 |
| CONFIGURATION45 | "Source checking still enforces every ordinary contract." | Behavior exclusions | Ordinary callable/source contracts remain fully enforced when behavior exclusions apply. | not stated | SHARED |
| CONFIGURATION46 | "An absent selection realizes every deployable target profile the toolchain closure provides." | Exact target requests | When no explicit target selection is given, realization must include every deployable profile in the toolchain closure. | not stated | 9 |
| CONFIGURATION47 | "An explicit selection accepts one exact profile or a caller-supplied nonempty set, normalized to canonical order with duplicates removed; it narrows realization, never checking." | Exact target requests | Explicit target selection must be one profile or a normalized nonempty set; it may only narrow realization, not checking scope. | not stated | 9 |
| CONFIGURATION48 | "Reject `all`, `*`, empty explicit sets, and inference from source or dependencies." | Exact target requests | Must reject target tokens `all`, `*`, empty explicit sets, and target inference from source or dependencies. | not stated | 9 |
| CONFIGURATION49 | "The catalog's abstract/local modes and inactive bindings cannot define deployment intent and realize only when selected explicitly." | Exact target requests | Abstract/local catalog modes and inactive bindings must not define deployment intent; realize those modes only when explicitly selected. | not stated | 9 |
| CONFIGURATION50 | "Authored target-policy/activation declarations are not admitted source syntax and do not define a support matrix." | Exact target requests | Must reject or ignore authored target-policy/activation declarations as source syntax and must not treat them as a support matrix. | not stated | 9 |
| CONFIGURATION51 | "The Build does not observe a selected target: target-dependent selections are rows keyed by target, and dependency requests cannot branch on a target." | Exact target requests | Build evaluation must not observe a single selected target; dependency requests must not branch on target. | not stated | 9 |
| CONFIGURATION52 | "Use exact canonical spellings, not extra aliases such as `windows_x64` for `windows_x86_64`." | Exact target requests | Target profile names must use exact canonical spellings; aliases such as `windows_x64` must reject. | not stated | 9 |
| CONFIGURATION53 | "an executable application needs its selected ProgramEntry;" | Exact target requests | Executable applications must have selected ProgramEntry for admissibility. | not stated | 9 |
| CONFIGURATION54 | "an explicitly artifact-only application requires completed artifacts and rejects executable root/provider selections." | Exact target requests | Artifact-only applications must require completed artifacts and reject executable root/provider selections. | not stated | 9 |
| CONFIGURATION55 | "a library or target-neutral artifact does not acquire a native entry merely to pass checking." | Exact target requests | Libraries and target-neutral artifacts must not gain native entry solely to satisfy checking. | not stated | 9 |
| CONFIGURATION56 | "Artifact-only mode still binds an explicit target and does not imply target-neutral identity." | Exact target requests | Artifact-only mode must bind an explicit target and must not imply target-neutral identity. | not stated | 9 |
| CONFIGURATION57 | "Target semantics, ABI/layout, resources, and reach must validate." | Exact target requests | Selected target semantics, ABI/layout, resources, and reach must be validated mechanically. | not stated | 9 |
| CONFIGURATION58 | "Target assumptions must be checked facts." | Exact target requests | Target assumptions used in closure must be checked facts, not unstated assumptions. | not stated | SHARED |
| CONFIGURATION59 | "unknown profile names still reject rather than silently becoming inactive." | Target recognition and implementation availability | Unknown target profile names must reject, not silently become inactive bindings. | not stated | 9 |
| CONFIGURATION60 | "Selected requests require the implementation of the requested operation." | Target recognition and implementation availability | When a target profile is selected for an operation, that operation's implementation must be required. | not stated | 9 |
| CONFIGURATION61 | "An unavailable operation reports a target/product not-implemented diagnostic, not successful checking or a fallback artifact." | Target recognition and implementation availability | Unavailable target operations must emit not-implemented diagnostic, not succeed or emit fallback artifacts. | not stated | 9 |
| CONFIGURATION62 | "do not create a parallel bootstrap selection mechanism or infer support from the mere presence of those declarations." | Target recognition and implementation availability | Must not add parallel bootstrap target selection or infer support from declaration presence alone. | not stated | 9 |
| CONFIGURATION63 | "Psi runs once: source loading, parsing, resolution, typing, checking, proof, and Terminal Psi production do not observe a selected target." | Multi-target compilation | Single Psi pass must not branch on selected realization target through Terminal Psi production. | not stated | SHARED |
| CONFIGURATION64 | "Every supplied named implementation is resolved, typed, checked, and published under its ordinary declaration identity, including implementations not selected for realization." | Multi-target compilation | All supplied named implementations must be fully checked and published regardless of realization set. | not stated | SHARED |
| CONFIGURATION65 | "Target-prefixed machine declarations and implicit same-path target-body families are retired; targets are Build/toolchain data, not source declaration modifiers." | Multi-target compilation | Must not admit target-prefixed machine declarations or implicit same-path target-body families as source modifiers. | not stated | 9 |
| CONFIGURATION66 | "A declared row for a target outside the realization set still demands its applicable layouts;" | Multi-target compilation | Build rows for non-realized targets must still demand applicable layouts. | not stated | 9 |
| CONFIGURATION67 | "a body reached by no row or entry does not acquire every supplied target merely because it was source-checked." | Multi-target compilation | Source-checked bodies not reached by any row or entry must not gain layout obligations for every supplied target. | not stated | 9 |
| CONFIGURATION68 | "Any later consumer must validate the exact application before use." | Multi-target compilation | Consumers of layouts or applications added later must validate the exact application before use. | not stated | SHARED |
| CONFIGURATION69 | "Build evaluation runs once for the whole compilation." | Multi-target compilation | Build must be evaluated once per compilation, not per realization target. | not stated | 9 |
| CONFIGURATION70 | "It does not observe a single selected target and cannot branch on one;" | Multi-target compilation | Build evaluation must not observe or branch on a single selected target. | not stated | 9 |
| CONFIGURATION71 | "A row for a target outside the realization set is checked and retained, not realized." | Multi-target compilation | Non-realized target rows must still be checked and retained in evaluated Build. | not stated | 9 |
| CONFIGURATION72 | "its value cannot vary by target." | Multi-target compilation | Const provider-folded values must be identical across targets in the one checked program. | not stated | SHARED |
| CONFIGURATION73 | "Every realized target's row must select the same provider for such an application; Psi folds it once through that provider." | Multi-target compilation | For const provider applications, every realized target's Build row must select the same provider. | not stated | 9 |
| CONFIGURATION74 | "Rows that disagree reject the application and name the diverging targets." | Multi-target compilation | Disagreeing provider rows for const provider applications must reject and identify diverging targets. | not stated | 9 |
| CONFIGURATION75 | "an explicit `--target` selection narrows realization only and never narrows checking." | Multi-target compilation | CLI `--target` may only narrow realization set, not universal checking scope. | not stated | 9 |
| CONFIGURATION76 | "Omega selects each required provider from the exact Build row for the target it realizes." | Multi-target compilation | Per-target realization must select providers from that target's exact Build row. | not stated | 9 |
| CONFIGURATION77 | "Its selected transitive closure must satisfy the requirement and the target's ABI, import, layout, and ISA constraints." | Multi-target compilation | Realized provider transitive closure must satisfy requirement and target ABI/import/layout/ISA constraints. | not stated | 9 |
| CONFIGURATION78 | "A missing, ambiguous, or incompatible row rejects that realization; it does not invalidate the portable semantics or another target's realization." | Multi-target compilation | Per-target missing/ambiguous/incompatible Build row rejects only that realization, not portable semantics or other targets. | not stated | 9 |
| CONFIGURATION79 | "Return one outcome per realized target; one realization's failure cannot suppress another's." | Multi-target compilation | Multi-target builds must report per-target outcomes; one failure must not suppress others. | not stated | 9 |
| CONFIGURATION80 | "Callers must use the requirement's public contract, never a selected implementation's private guarantees or a synthesized intersection of alternate bodies." | Multi-target compilation | Call-site checking must use requirement public contract only, not private implementation guarantees or synthesized alternate-body intersections. | not stated | 3 |
| CONFIGURATION81 | "The explicit argument must borrow that activation's original Build." | Directional wire compatibility | `require_wire_compatibility` builder argument must borrow the activation's original Build value. | at a call | 9 |
| CONFIGURATION82 | "Requests are collected when executed, including through ordinary helper machines; an unexecuted call contributes no demand." | Directional wire compatibility | Wire-compatibility demands accrue only from executed calls; uncalled helpers impose none. | at a call | 9 |
| CONFIGURATION83 | "The receiver-call spelling is retired." | Directional wire compatibility | Receiver-style `require_wire_compatibility` spelling must reject or not admit. | at a call | 9 |
| CONFIGURATION84 | "Evaluate them against published schema, codec, unknown-member, canonicalization, and `FormatMigration` evidence; report every fact and reject unmet requests." | Directional wire compatibility | Requested wire-compatibility facts must be evaluated against published evidence; unmet requested facts must reject after reporting. | not stated | 9 |
| CONFIGURATION85 | "Omitted facts remain reported but cannot reject the build." | Directional wire compatibility | Wire facts omitted from a request must appear in reporting but must not by themselves reject the build. | not stated | 9 |
| CONFIGURATION86 | "`04_wire_protocols.txt` reports every directional fact and its explanation before unmet requested facts reject." | Directional wire compatibility | Wire protocol report must list every directional fact and explanation before rejection for unmet requested facts. | not stated | 9 |
| CONFIGURATION87 | "Strict decoding cannot satisfy preservation merely by validating known fields." | Directional wire compatibility | `PreserveUnknown` wire fact cannot be satisfied by strict decoding that only validates known fields. | not stated | 9 |
| CONFIGURATION88 | "The local decoder accepts every peer value." | Directional wire compatibility | When `Readable` is requested, local decoder must accept every peer value. | not stated | 9 |
| CONFIGURATION89 | "The peer decoder accepts every local value." | Directional wire compatibility | When `Writable` is requested, peer decoder must accept every local value. | not stated | 9 |
| CONFIGURATION90 | "Unknown information is preserved for relay." | Directional wire compatibility | When `PreserveUnknown` is requested, unknown information must be preserved for relay. | not stated | 9 |
| CONFIGURATION91 | "The selected encoding produces canonical bytes." | Directional wire compatibility | When `Canonical` is requested, selected encoding must produce canonical bytes. | not stated | 9 |
| CONFIGURATION92 | "Selected `FormatMigration<Lineage, Old, New>` conformances provide the required peer-to-local migration route." | Directional wire compatibility | When `CompleteMigration` is requested, selected `FormatMigration` conformances must supply required peer-to-local migration route. | not stated | 9 |
| CONFIGURATION93 | "Normalize and retain the effective value even when defaulted." | Proof-work policy | Effective proof index-expansion value must be normalized and retained in durable Build even when taken from default. | not stated | 8 |
| CONFIGURATION94 | "Product discovery uses the admitted policy;" | Proof-work policy | Product proof discovery must use the admitted index-expansion policy for that discovery context. | not stated | 8 |
| CONFIGURATION95 | "Retain the effective path and target in normalized output configuration and the corresponding artifact/review inputs, including when a declared default supplies the path." | ELF executable loader selection | Effective ELF interpreter path and target must appear in normalized output and artifact/review inputs, including default-supplied paths. | not stated | 9 |
| CONFIGURATION96 | "An explicit selection overrides a default declared by the selected target/toolchain environment." | ELF executable loader selection | Explicit `builder.elf_interpreter` selection must override target/toolchain default interpreter. | not stated | 9 |
| CONFIGURATION97 | "An architecture alone does not imply glibc, musl, or a loader pathname." | ELF executable loader selection | Target architecture must not be used to infer C library flavor or default loader path without explicit/default configuration. | not stated | 9 |
| CONFIGURATION98 | "Each enabled group collects its package-local concrete satisfiers for Psi execution before successful publication." | Test groups | Enabled test groups must collect package-local satisfiers and run Psi execution before publication succeeds. | not stated | 9 |
| CONFIGURATION99 | "A component closes declared slots;" | Exact target requests | Component products must close all declared slots for admissibility. | not stated | 9 |
| CONFIGURATION100 | "No debug/release mode, assertion primitive, or new crash cause is implied." | Behavior exclusions | Configuring behavior exclusions must not imply debug/release modes, assertion primitives, or new crash causes. | not stated | 9 |
| CONFIGURATION101 | "Only the directional facts named after the first four type arguments are required." | Directional wire compatibility | Wire-compatibility obligations apply only to directional facts named in the generic argument list after the first four schema identifiers. | not stated | 9 |
| CONFIGURATION102 | "A facet binds canonical relative bytes to an exact root occurrence." | Activation filesystem scope | Filesystem facet identity must bind canonical relative bytes to an exact root occurrence. | not stated | 9 |
| CONFIGURATION103 | "This is ordinary Build data, not a generic argument, source annotation, or language keyword." | Proof-work policy | `index_expansion_terms` must be interpreted as ordinary Build field data, not as generic/source/keyword syntax. | not stated | 9 |

## Ambiguous sentences (both readings)

| quoted sentence | reading A | reading B |
| --- | --- | --- |
| "This contract separates activation authority from durable selections and exact-target results." | Activation-time authority is a distinct artifact from what is stored in normalized Build output. | The specification divides responsibilities between activation filesystem scope and durable configuration without prescribing when each is enforced. |
| "Staging a write does not include it in compilation." | Staged bytes are never compilation inputs even after successful handoff. | Staged bytes are excluded from compilation until explicit handoff after successful evaluation completes. |
| "Explicit handoff follows successful evaluation and evidence custody; failure discards the staging occurrence." | Handoff is mandatory after success; staging is always discarded on failure. | Handoff occurs only when evidence custody succeeds; partial custody failures might leave staging in an unspecified state. |
| "During the experimental phase only the root's authoritative `build.omg` enables them, not dependency metadata or embedding defaults." | Only root `build.omg` may enable optimizations until the experimental phase ends by a rule stated elsewhere. | Dependency and embedding sources can never enable optimizations even outside the experimental phase unless another document overrides. |
| "Product discovery uses the admitted policy; preflight or Build-stage checking uses its already granted policy, never a later setting to justify its own admission." | Product discovery and preflight always use the same policy snapshot taken at admission time. | Product discovery may use policy admitted for discovery while preflight uses an earlier grant frozen before later Build edits. |
| "Increasing this allowance under the same source, operations, assumptions and otherwise sufficient resource policy must preserve smaller-allowance successes." | Any proof that succeeded under a smaller limit must still succeed when the limit is raised. | Only successes under identical resource policies besides the index-expansion limit must be preserved when that limit alone increases. |
| "A smaller discovery allowance cannot invalidate that reuse." | Reuse of producer judgments is valid regardless of consumer's smaller allowance. | Smaller allowance on the consumer cannot force rediscovery but might still block admission for other reasons. |
| "empty row keeps the declared default." | Empty `builder.elf_interpreter.<target>` means use target/toolchain default without recording an explicit override. | Empty row means omit `PT_INTERP` even when a default exists in the environment metadata. |
| "A loader path alone neither requests dynamic linking nor makes foreign imports compatible." | Setting interpreter path does not toggle dynamic linking flags or validate imports. | Interpreter path is ignored for link-mode decisions; dynamic linking is determined solely by link settings elsewhere. |
| "Root grants can only be attenuated; selecting a real provider cannot escape virtual backing." | Test groups may weaken root virtual requirements but never strengthen beyond root grant. | Real providers must still satisfy virtual backing obligations implied by the root even when a concrete provider is named. |
| "A verified selected composition can satisfy a stronger exclusion than its broad public declarations promise without changing those declarations or making them satisfy narrower callable requirements." | Product admission may use stronger exclusion evidence than public callable contracts state, without editing declarations. | Stronger exclusion satisfaction affects admission only and never narrows how ordinary callers type-check against declarations. |
| "An absent selection realizes every deployable target profile the toolchain closure provides." | Default realization includes all deployable profiles known to the toolchain closure at build time. | Absent selection realizes every profile listed in a fixed catalog regardless of host capability to build them. |
| "it narrows realization, never checking." | Explicit target selection affects only artifact emission targets, never which source is type-checked. | Explicit target selection narrows runtime deployment but Build rows for other targets remain checked. |
| "A CLI Host convenience selects the realization set before semantic evaluation; Host is not artifact identity." | Host-only flag adjusts realization set prior to Build/Psi evaluation without becoming part of artifact identity. | Host selection is a pre-parse CLI step that can differ from Build-recorded realization metadata. |
| "Recognizing a profile and its slot identities does not require implementing its checking or artifact backend." | Name recognition and backend implementation are separate; recognition alone does not imply backend presence. | Compiler may list profiles in catalogs without implementing them, but admission still requires backends when selected. |
| "An inactive binding to a recognized profile does not require that profile's backend;" | Inactive bindings skip backend requirements until the profile is actively selected for an operation. | Inactive bindings never require backends even when the profile name appears in source or Build data. |
| "Universal source checking does not instantiate every unused target layout." | Psi checking must not materialize layouts for targets with no demanding row or entry. | Layout formation is demand-driven, so unused targets might still be checked at source level without full layout instantiation. |
| "A compilation realizing one target, or a Build that selects one provider for all targets, satisfies the rule trivially." | Single-target realization exempts const-provider agreement checks because only one row matters. | Trivial satisfaction applies only when Build explicitly selects one provider for all targets, not merely when CLI realizes one target. |
| "A fat/universal artifact is a separate explicit envelope over independently committed realizations." | Universal binaries require an additional explicit packaging step beyond per-target realizations. | Fat artifacts are never implicit products of multi-target compilation unless an envelope is explicitly requested. |
| "Only the directional facts named after the first four type arguments are required." | Type arguments beyond the fourth list optional facts; only named facts become obligations. | Facts are required only when spelled in the generic argument list, not from defaults in the closed vocabulary table. |
| "This is channel/store deployment policy, not intrinsic version metadata on the two data types." | Wire compatibility checks deployment configuration only; schema types carry no embedded version policy. | Wire requests do not add version fields to Local/Peer types in the language model. |

## Terms used without definition in this chapter

- Declarations
- execution
- activation authority
- durable selections
- exact-target results
- immutable source root
- exact package occurrence
- fresh writable staging root
- build occurrence
- sponsored operations
- compiler-owned facets
- runtime filesystem services
- roots
- handles
- rooted paths
- runtime data
- durable build result
- Snapshot and staging semantics
- captured inventory
- deterministic logical observations
- append-and-seal files
- Extra captured inputs
- separately scoped roots
- ambient consumer tree
- live-host authority
- dependency
- live-filesystem modes
- transitional
- snapshot-isolated execution
- facet
- canonical relative bytes
- exact root occurrence
- erased qualification
- bare bytes
- absolute input
- traversal outside the root
- ambiguous membership
- symlink escape
- host access
- path-returning operations
- canonicalization
- read_link
- inert target bytes
- checked resolution
- outside link
- grant
- Staging
- Explicit handoff
- successful evaluation
- evidence custody
- staging occurrence
- Evidence paths
- serialization spellings
- prefixes
- authority
- Identity
- root role/occurrence
- relative bytes
- host path
- working directory
- Normalized Build
- pipeline-consumed
- target
- dependency
- root
- provider
- test-group
- behavior-exclusion
- optimization
- proof-work
- output selections
- source/staging roots
- admitted host-service authority
- Image/application facts
- build-owned
- signing-affecting
- publication-only metadata
- application publication
- Optimizations
- semantics-preserving families
- experimental phase
- authoritative `build.omg`
- dependency metadata
- embedding defaults
- debug data
- diagnostics
- assertions
- packaging
- debug/release
- O1/O2/O3 optimization categories
- Optimization phases
- Hosted/freestanding environment
- default providers
- calling policies
- fault and resource supply
- selected target profile
- mutable Build flags
- Project role
- roots
- dependencies
- outputs
- image intent
- provider selection
- build facts
- Target-qualified bindings
- target policy
- stable index identity and compatibility
- authoritative root `build.omg`
- `builder.proof.index_expansion_terms`
- positive `u64`
- Build entry
- ordinary Build data
- generic argument
- source annotation
- language keyword
- resulting summands
- automatic compatibility-proof product expansion
- normalization steps
- integer magnitude
- total proof cost
- sponsor-controlled work/storage ceilings
- effective allowance
- expected discovery cost
- mathematical claims
- root's grant
- Product discovery
- admitted policy
- preflight
- Build-stage checking
- sponsored proof-discovery request
- staged execution rules
- Policy provenance
- root/scope
- limit
- search algorithm/accounting version
- attempted measure
- consumption
- outcome
- usage receipt
- certified resource bound
- proof-work accounting
- semantic outcomes
- unproved obligation
- resource diagnostic
- equality
- inequality
- implementation selection
- runtime fallback
- extensible search schedule
- established derivations
- budget-dependent strategy
- Larger discovery
- compatibility
- rename types
- merge declarations
- canonical matching
- specialization keys
- proof record
- artifact bytes
- Published compatibility judgments
- exact subjects
- Psi admission
- host-authorized consumer
- trusted producer's exact artifact
- discovery allowance
- Loading and preparation
- resource limits
- resource refusal
- failed theorem discovery
- Local cached search
- usage-receipt accounting
- `elf-interpreter-declaration-surface` decision
- build-owned output configuration
- deployment framework
- dynamically linked Linux ELF executable
- exact interpreter pathname
- selected target/toolchain environment
- architecture
- glibc
- musl
- loader pathname
- `builder.elf_interpreter.<target>`
- byte-string row
- canonical ELF target name
- `linux_arm64`
- `linux_x86_64`
- declared default
- normalized output configuration
- artifact/review inputs
- backend
- nonempty absolute target pathname
- embedded NUL bytes
- `PT_INTERP`
- build host
- cross-compilation
- dynamic executable
- missing-output-configuration diagnostic
- statically linked executable
- dynamic linking
- foreign imports
- target deployment
- runtime environment
- authenticate
- installation
- execution authority
- receiver admission
- Requirement-based tests
- exact product machine requirements
- `builder.tests.group<Requirement>()`
- enabled group
- package-local concrete satisfiers
- Psi execution
- successful publication
- provider configuration
- new authority
- Root grants
- attenuated
- real provider
- virtual backing
- group and global `disabled` fields
- registered groups
- automatic tests
- implicit core/std dependency
- hidden group
- evaluated build configuration
- target-independent dependency-request
- final-join rules
- selected product
- crash causes
- abstract boundary services
- physical authority classes
- additional admission requirements
- provider permission
- ordinary callable contracts
- Behavior exclusions
- verified selected composition
- broad public declarations
- narrower callable requirements
- debug/release mode
- assertion primitive
- new crash cause
- deployable target profile
- toolchain closure
- explicit selection
- caller-supplied nonempty set
- canonical order
- duplicates
- realization
- checking
- `all`
- `*`
- empty explicit sets
- inference
- catalog's abstract/local modes
- inactive bindings
- deployment intent
- Authored target-policy/activation declarations
- admitted source syntax
- support matrix
- CLI Host convenience
- realization set
- semantic evaluation
- Host
- artifact identity
- selected target
- target-dependent selections
- rows keyed by target
- dependency requests
- Target schemas
- observation vocabulary
- closed
- toolchain-owned
- concrete profiles
- validated target packages
- authored reinterpretation
- forever-fixed compiler enum
- exact canonical spellings
- extra aliases
- `windows_x64`
- `windows_x86_64`
- Admissibility
- product role
- output mode
- executable application
- ProgramEntry
- artifact-only application
- completed artifacts
- executable root/provider selections
- component
- declared slots
- library
- target-neutral artifact
- native entry
- Artifact-only mode
- target-neutral identity
- Target semantics
- ABI/layout
- resources
- reach
- mechanical closure
- human testing
- Target assumptions
- checked facts
- profile
- slot identities
- checking
- artifact backend
- inactive binding
- recognized profile
- unknown profile names
- Selected requests
- requested operation
- unavailable operation
- target/product not-implemented diagnostic
- fallback artifact
- `alpha_bootstrap`
- Rust reference compiler
- Omega-written compiler
- native target
- Alpha compilation
- bootstrap
- Rust Alpha backend
- Profile and slot identity
- target-package facts
- parallel bootstrap selection mechanism
- One compilation
- provided target
- Psi
- source loading
- parsing
- resolution
- typing
- checking
- proof
- Terminal Psi production
- supplied named implementation
- ordinary declaration identity
- realization
- Windows realization
- macOS implementation
- Target-prefixed machine declarations
- implicit same-path target-body families
- Build/toolchain data
- source declaration modifiers
- Universal source checking
- unused target layout
- Demand-driven layout formation
- concrete geometry
- actual consumers
- entries
- selected Build-row closures
- declared row
- realization set
- applicable layouts
- body
- row or entry
- source-checked
- later consumer
- exact application
- Build evaluation
- whole compilation
- single selected target
- provider plans
- program entry
- subsystem
- application intent
- opaque representation
- grants
- behavior exclusions
- one evaluated Build
- checked and retained
- realized
- constant
- folds through a provider
- const application
- boundary operator
- requirement
- selected realization
- checked body
- one checked program
- realized target's row
- Psi folds
- diverging targets
- `--target`
- Omega
- required provider
- exact Build row
- selected transitive closure
- ABI
- import
- layout
- ISA constraints
- missing, ambiguous, or incompatible row
- portable semantics
- another target's realization
- one outcome per realized target
- aggregate exit code
- human summary
- orchestration
- support
- audit
- completeness evidence
- fat/universal artifact
- separate explicit envelope
- independently committed realizations
- CI
- cross-compile
- physical realization
- language support set
- deployment-tagged family members
- synthesized portable family heads
- migration machinery
- second supported selection model
- `BUILD-OWNED-TARGET-SELECTION`
- pipeline route items
- explicit requirements
- named implementations
- requirement's public contract
- selected implementation's private guarantees
- synthesized intersection
- alternate bodies
- compiler
- `CompileRequest`
- source typing
- per-target route
- provider settlement
- single Psi artifact
- source-free per-target settlement
- authoritative Build
- `Build::require_wire_compatibility<Edge, Lineage, Local, Peer, ...>(builder);`
- explicit argument
- activation's original Build
- helper machines
- unexecuted call
- demand
- receiver-call spelling
- directional facts
- first four type arguments
- published schema
- codec
- unknown-member
- canonicalization
- `FormatMigration` evidence
- channel/store deployment policy
- intrinsic version metadata
- edge
- lineage
- local schema
- peer schema
- closed vocabulary
- `Readable`
- `Writable`
- `PreserveUnknown`
- `Canonical`
- `CompleteMigration`
- local decoder
- peer value
- peer decoder
- local value
- Unknown information
- relay
- selected encoding
- canonical bytes
- `FormatMigration<Lineage, Old, New>` conformances
- peer-to-local migration route
- Omitted facts
- wire report
- schema/codec identities
- numbered and retired members
- accepted historical shapes
- migration routes
- unknown-member behavior
- derived/admitted realization provenance
- `04_wire_protocols.txt`
- directional fact
- explanation
- unmet requested facts
- Strict decoding
- preservation
- known fields

CHAPTER COMPLETE
