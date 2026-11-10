# Area 6: Terminal runtime and packages

Source: records `unit_032.md` to `unit_037.md` (six first-pass records; no second-pass gap record names
any file of this area). Checkout `worker-23` at `457adf4a11f`. Nothing was built or run.

Path prefixes, all under `omega-rust/omega/`:

| Prefix | Directory | Crate lines (crates.json) | Records |
| --- | --- | --- | --- |
| TR | `interpreter/terminal-runtime/src/execution/host_imports` | 14,328 (11,780 in this area) | 032 |
| BD | `packages/build-declarations/src` | 2,118 | 032 |
| BO | `packages/build-output/src` | 2,189 | 032 |
| PC | `packages/package-compilation/src` | 17,433 | 032, 033 |
| PM | `packages/package-manager/src` | 26,065 | 033, 034 |
| RX | `packages/resolver-execution/src` | 464 | 034 |
| RE | `packages/review-evidence/src` | 47,300 | 034, 035, 036 |
| SA | `packages/source-acquisition/src` | 21,951 | 036, 037 |

A citation such as `PC/package_compilation.rs:370` means that file under the prefix. "(032)" names the
record a statement comes from. Words such as "open", "suspected", "inferred" and "not traced" are the
reader's own and are kept.

## What this area does

This area is everything around the compiler proper that deals with the outside world. One part runs an
already-compiled program inside the reference interpreter and stands in for the operating system when
the program calls out (clock, files, standard streams, a few C library functions). The rest is the
package system: reading `build.omg`, running it under a fenced filesystem, fetching dependency sources
from Git or local folders into tamper-checked caches, working out the dependency graph and the lock
file, compiling each package with the right inputs, and producing a "review" record of what each
package is allowed to do so a person can accept or reject changes. Most of this code is checking: it
re-derives a fact, compares it with a stored copy, and returns an error on any difference.

**terminal-runtime (host imports part).** In: a bound foreign-call declaration, its native ABI shape, and
interpreter storage loans. Out: a result value or guest crash, plus writes back into the loans. Each
adapter picks an OS entry by syscall number or normalised library symbol, checks the declaration against
a fixed table of shapes, then calls `admit_exact_foreign_call` (032, `TR/clock/admission.rs:19-72`,
`TR/filesystem/linux.rs:154-172`, `TR/filesystem/windows/contracts.rs:32-446`).
`TR/borrowed_storage.rs` with `borrowed_storage/geometry.rs` is the one place that turns a storage loan
into bytes and back (032). A separate "private native worker" process hosts shared-memory homes for
generic native imports (`TR/native_provider.rs:143-851`, `TR/native_provider/original_homes.rs`).

**build-declarations.** In: `build.omg` syntax trees, or the list of requests produced by running it.
Out: a validated project role (package, application, workspace), dependency rows, and the `Build` API
itself as Omega source text with target and optimization catalogs spliced in (032, `BD/lib.rs:5-18`,
`BD/prelude.rs:20-421`).

**build-output.** In: a private output directory on disk, or decoded persistence rows. Out: a
`BuildStagedOutputTree` with a domain-separated commitment, subsets of it, and a routine that writes a
tree into an empty directory (032, `BO/staged_output_tree.rs:85-466`, `BO/capture/mod.rs:45`,
`BO/materialization/mod.rs:28`).

**package-compilation.** Three groups. `build_execution/**`: admits the `build` machine, fences its
filesystem (source root, private output root, named inputs), records every filesystem attempt, and
settles required outputs (032, `PC/build_execution.rs:67-168`, `PC/build_execution/filesystem_scope.rs`,
`filesystem_scope/runtime.rs`). `package_compilation/**`: the closed input contract
`PackageCompilationInputs`, split into a shared source half and a per-target half, plus build handoffs
and wire codecs (033, `PC/package_compilation.rs:66-140`). `source/**` and `reuse/**`: two-phase source
assembly (Build then Product), `use` resolution, source snapshots, an on-disk metadata cache and
digests (033, `PC/source/source_assembly.rs:55-65`, `PC/reuse/checked_source_cache.rs:131`).

**package-manager.** In: `build.omg`, `omega.lock`, command-line requests, acquired sources. Out: a
resolved source closure, a canonical "subject" in binary and text form, the lock file, a review
document with decisions, and compile inputs per (package, purpose). Source resolution is a
suspend/resume protocol (034, `PM/resolution/graph/reconcile/resolution.rs:84`); review runs two passes
and carries identical compiles forward from memory or a disk cache (034,
`PM/review/candidate/passes.rs:53-119`, `passes/occurrences.rs:499-618`); lock and `build.omg` are
published through a forward-completing journal (033, `PM/operations/publication/transaction.rs:124-213`).

**resolver-execution.** In: a Git executable path and package-controlled roots. Out: a frozen
executable and per-phase bounded `Command`s (034, `RX/resolver_execution.rs:32-96`).

**review-evidence.** `capture/**`: in, a checked compilation (candidate trees plus immutable custody);
out, a `PackagePolicyBaseline` (035, `RE/capture/package_policy.rs:45`, `RE/capture/package_review.rs:30`).
`encoding/encode/**` and `encoding/recovery/**`: canonical binary, named text, sorted review rows, and a
decoder that accepts input only when re-encoding reproduces it (036, `RE/encoding/recovery/policy.rs:33-53`).
`record/**`: the record types and structural validators (036, `RE/record/baseline/validation.rs:23`).

**source-acquisition.** In: a Git locator and revision, or a local directory. Out: an immutable snapshot
in a per-user cache with a content identity. All Git launches go through `run_git_output` after
`sealed_git_command` (037, `SA/git/git_command.rs:44`, `SA/git/git_command/policy.rs:20`); object bytes
are hashed in-process (`SA/git/objects/authentication.rs`); caches are guarded by lock files and
ownership checks (036, `SA/custody/lock.rs`, `SA/custody/platform.rs`).

## How it decides

**Keyed on identity** (digests, symbols, file identity).
- Package identity digests, `DependencyPurpose`, and alias strings select compile inputs (033, `PC/package_compilation.rs:185-238`).
- Storage loans match by exact structural type identity and canonical path vectors (032, `TR/borrowed_storage.rs:178-222`).
- Native homes are keyed by `(root.namespace(), root.allocation())` and `Rc::ptr_eq` backing identity (032, `TR/native_provider/original_homes.rs:211,337-358`).
- Review projection keys on nominal identity records and symbols, "exactly one row else error" (034 general shape; 035 `RE/capture/semantics/facts.rs:6`).
- Cache custody keys on device/inode (Windows: volume serial + file index), owner uid and mode bits (036, `SA/custody/lock.rs:289-301,381-405`, `SA/custody/platform.rs:60-74`).
- Source closure keys on `PackageKey`, and acquisition reuse on `(lineage, navigation)` (034, `PM/resolution/graph/reconcile/resolution.rs:192-196,355`).
- Git object algorithm is keyed on id string length, 40 or 64 (037, `SA/git/objects/identity.rs:70-79`).

**Keyed on value** (byte equality, digests, counts).
- Canonical recovery: decode, re-encode, compare bytes (036, `RE/encoding/recovery/policy.rs:42-53`; 034 `PM/resolution/graph/subject/text/record.rs:189-192`).
- Source admission by path plus byte equality (033, `PC/package_compilation/provider_selections.rs:137-204`, `provider_defaults.rs:559`).
- Review-cache key is SHA-256 over package, purpose, target, profile, entry path, compiler identity, inputs and snapshot digests (034, `PM/review/candidate/review_cache/key.rs:58-73`).
- Budgets everywhere: entries, bytes, depth, launches, wall time (037, `SA/limits.rs:39-58`; 036 `RE/encoding/recovery/policy/policy_recovery_accounting.rs:72-81`).

**Keyed on position.**
- Native parameters by telescope ordinal (032, `TR/native_provider.rs:412-436`); binders by ordinal (035, `RE/capture/calling/callbacks.rs:31-39`).
- First-found wins: BFS path ties (034, `PM/resolution/graph/reconcile/dependency_paths.rs:36`), first matching capture (032, `TR/native_provider/reference_outputs.rs:314`), first lock target `targets()[0]` (034, `PM/package_manager/source_review.rs:26`), first matching Git edge (034, `PM/package_manager/source_review/old_sources.rs:55`).
- `review_positions` indexed by a two-slot purpose array (034, `PM/review/candidate/passes/occurrences.rs:281`).

**Keyed on place** (path prefix).
- Package of a source file by longest `source_root` prefix (033, `PC/package_compilation.rs:496`, `PC/source/source_imports/frontier.rs:543-591`).
- Write-root overlap by `overlap_key` and `starts_with` (032, `PC/build_execution/filesystem_scope.rs:85-127,458-470`).
- Executable outside package roots by `starts_with`, lexical and canonical (034, `RX/resolver_execution/path_custody.rs:68-93`; 037 `SA/git/executable/selection.rs:186-194`).

**Keyed on source spelling: 80 distinct sites cited by the records** (my count; a "site" is one cited
location or one closely adjacent group; wire-format token tables are not counted).

- terminal-runtime, 2: six `toolchain::Filesystem...`/`DirectoryEnumeration` service identity strings (`TR/filesystem.rs:446-456`); owner is the text before the first `::` and must start with `package:` (`TR/native_provider/service_reach.rs:9-14`). Test-only: source-text substring tests (`terminal-runtime/src/runtime_boundary.rs:6-26`), exact error string (`terminal-runtime/tests/retained_runtime.rs:71-73`).
- build-declarations, 6: call names `depend`, `depend_as`, `build_depend`, `build_depend_as` (`BD/dependencies/mod.rs:62-69`); four `*_when` names (`:93`); `Build` and `builder` (`BD/direct_build_call.rs:22,38`); machine leaf name `build` (`BD/syntax_projection/mod.rs:101,390`); parameter `builder` of type `&mut Build` (`:152-173`); role calls `package|application|member|artifact_only` (`:310-316`).
- build-output, 2: `.omg` suffix (`BO/staged_output_tree.rs:377-387`); names `build.omg`/`main.omg` (`:395-432`).
- package-compilation, 18: protocol name `"BuildFilesystemOperations"` (`PC/build_execution/build_admission.rs:89-96`); machine named `build` (`build_value/vocabulary.rs:13-28`); `Build` field names as strings (`build_value/configuration.rs:145-151`); enum case by last `::` segment, `Console`, `Gui`, `EfiApplication`, `Baseline`, `AvxFma3` (`configuration.rs:164-198`); facet names `BuildLog`, `BuildProduct`, ... (`build_value/initial.rs:51-121`); data `Build` with field `identifier` (`vocabulary.rs:52-71`); `Build.optimizations` (`build_value/optimization/vocabulary.rs:18-82`); substring `f32|f64|Float|F32|F64` over raw file text (`PC/source/source_assembly.rs:1038-1045`); path `<build-prelude>` (`source_assembly/build_source.rs:54`); `"build"` and `"Build"` (`source_imports/build_prelude.rs:30,35`); directory name `core` (`source_imports/frontier.rs:555-561`); file name `build.omg` (`frontier.rs:657-658`); first path segment against alias (`package_selection.rs:268,278`); literals `omega`, `language`, `core` (`paths.rs:96,99,109`); product-query first segment and `build.omg` (`product_queries.rs:310-313,334,394`); `main.omg`/`build.omg` companions (`project.rs:77-83`); identity string equality (`provider_defaults.rs:552`); extension `omg` (`toolchain_library.rs:126`).
- package-manager, 14: constructor names `Source`, `PackageSelection` by `rsplit_once("::")` (`PM/declarations/dependencies/read/source_literal.rs:18`); field names `location`, `repository`, `revision`, `selection`, `package` (`:46-93`); exact token run `machine build ( builder : & mut Build ) {` (`dependencies/edit/layout.rs:108`); `builder . <op> (` tokens (`:223`); `"local" | "git"` (`operations/inspect_source/request.rs:13`); `://`, `file://`, `user@host:` substrings (`package_manager/planning.rs:146-149`); `"build.omg"` (`operations/stage_build_edit.rs:19`); Git pin chosen by locator and revision strings (`resolution/graph/git_pins.rs:93-120`); default alias from package name with `-` to `_` (`subject/validation/closure.rs:99-108`); `main.omg` else `build.omg` (`resolution/source/mod.rs:59`, and `review/candidate/passes/occurrences.rs:476-481`); member path prefix and declared name (`resolution/source/git/workspace.rs:315-323,354`); member by normalised path string then name (`workspace_acquisition.rs:360-383`); `"build/package-manager"` (`package_manager/state.rs:64`, `source_review.rs:105-108`).
- resolver-execution, 0.
- review-evidence, 30: `"Carry::Portable"` and `CarryPermission::from_name` (`RE/capture/api/domains/aliases.rs:62-65`); core root file name `core` plus 14 `(file, declaration)` pairs (`capture/authority.rs:25-40,57-60,74-78`); `parameter:{index}`/`service:{name}` strings (`behavior/invocations.rs:10-44`); `"FilesystemHost"`, `"TimeHost"` by `display_path` (`capture/semantic_bindings.rs:92-98`); `"Console"`, `"ProcessExit"` (`:354-363`); method names `exit_process`, `write_byte`, `read_byte` (`:51-59,372`); term name equals binding (`contracts/evidence.rs:122`); `ByteSequencePredicate::from_name` of the callee (`contracts/expressions/calls.rs:230-238`); `ProofView` whose identity is the callee name (`calls.rs:254-287`); field by name (`calls.rs:164`); `"len" | "capacity"` (`members.rs:153`); parameter by name (`members.rs:299`); parameter by name, reserved `result`, field by name (`names.rs:108,128,18`); term and requirement by name (`propositions/binders.rs:100,122`); carry permission from joined domain path (`contracts/facts.rs:171-179`); target-name compare (`providers/application_realizations.rs:241`); `display_path == nominal.path` (`semantics/conformances/policy_callables.rs:39-44`); virtual path spelling `<...>` (`semantics/declarations/ownership.rs:104`); `"OmegaLayout"` (`semantics/types/validation.rs:84-90`); binder by spelling, two sites (`validation.rs:221-226,241-246`); integer type-name list (`static_arguments/const_values.rs:78-83`); lifetimes compared as strings, two sites (`conformances/policy.rs:174`, `conformances/application.rs:283-294`); generated plan-name compare (`providers/policy.rs:216-242`); membership check re-parses rendered identity text by first byte and frame label (`encoding/encode/membership/framing.rs:54-120,140-145`); operator coordinate by rendered spelling (`record/contracts/expressions/operator_policy.rs:27-83`).
- source-acquisition, 8: Windows file name `git.exe` (`SA/git/executable/validation.rs:44-49`); `.gitmodules` component (`git/objects/object_rows.rs:75-90`, and `git/objects/tree.rs:109-118`); `.git` component, case-insensitive here (`tree.rs:237,325`) but exact-case in `tree/capture/traversal.rs:290,322`; host `github.com`/`gitlab.com` (`identity/git.rs:50-55`); `.git`, root `build`, root `omega.lock` (`tree/capture/traversal.rs:284-293`); `build.omg` (`git/resolution/workspace_member/declarations.rs:21,75`); schemes `https`/`ssh` (`identity/locator.rs:17-33`).

Separately, review-evidence compares compiler-built identity **text** by string equality at 13 cited sites
(035/034): `callables/boundary_operators.rs:141-143`, `callables/boundary_requirements.rs:92-94`,
`callables/external_supply.rs:205`, `calling/callbacks/binders.rs:29,46`, `calling/callbacks/layouts.rs:71`,
`package_policy/assumed_bindings.rs:79`, `package_policy/native_bindings.rs:36-42`,
`providers/installation.rs:100`, `providers/policy/bindings.rs:118`, `providers/boundary_policy/realizations.rs:61-81`,
`runtime_permissions.rs:188-192`, `terminal_authority_permissions/declarations.rs:87-98`, `semantics/services.rs:86`.

## The same job in more than one place

Cross-record pairs I opened are marked "opened".

1. **snake_case name rule, 3 copies.** `BD/syntax_projection/mod.rs:440`, `PC/package_compilation/construction.rs:381`, `PM/declarations/identity.rs:88` (033 grep). Opened: same rule in all three (first byte lowercase, no trailing `_`, no `__`, `[a-z0-9_]`); the BD copy is a loop, the other two use `contains("__")`.
2. **Portable path component and Windows reserved device names, 2 identical copies plus other rules.** `BO/portable_paths/mod.rs:91-119` and `SA/git/objects/tree.rs:252-287` (037 "suspected", other side not read by 037). Opened: the reserved-device expression is the same text in both; both reject control bytes, `: * ? " < > |` and trailing dot or space. A different, weaker rule is `WorkspaceMemberPath::is_valid` (`BD/build_declaration.rs:60-71`): no reserved-name or trailing-dot check (032, both read). Further path validators: `SA/identity/local.rs:47` (accepts `.git` and `CON`, 037), `SA/identity/locator.rs:165`, `PM/resolution/source/workspace_path.rs:8-39`, `PC/source/source_snapshot.rs:85-123`.
3. **`BuildDeclarationKind` to byte, 3 copies.** `PC/reuse/review_record_encoding.rs:48` (Package 0, Application 1, Workspace 2), `PC/build_execution/observation_identity.rs:244` (None 0, Package 1, Application 2, Workspace 3), `PC/source/source_consumption.rs:126` (`unreachable!` for Workspace) (032 "suspected", 033). Opened the first two: two tables for one enum with different numbers.
4. **`DependencyPurpose` to 0/1, 3 copies.** `PC/reuse/review_record_encoding.rs:35`, `PM/declarations/mod.rs:30`, `PM/resolution/graph/subject/encoding/selection.rs:104` (033; grep re-run, 3 hits).
5. **Resolution stratum tags 0/1/2, 3 files.** `PC/package_compilation/provider_selection_codec.rs:339,455`, `native_output_codec.rs:40,171`, and `compilation/products/src/package_build/retention_identity.rs:667` outside this area (033).
6. **Target profile encodings.** `PC/target_profile_tags.rs` (9 entries); raw `target_name()` bytes at `PC/build_execution/observation_identity.rs:59` (032); profile tags 0 to 7 at `RE/encoding/encode/declarations/dependency_trust/geometry.rs:19-31` (035); identity-string match at `RE/encoding/recovery/policy/selected_providers.rs:43` and `external.rs:106` (036).
7. **Entry file choice, `main.omg` else `build.omg`.** Helper `PM/resolution/source/mod.rs:59` (3 callers) and a hand copy at `PM/review/candidate/passes/occurrences.rs:476-481` (034). Opened: the helper asks the captured tree when there is one; the copy always asks the physical snapshot root. 034 says the entry "may be chosen from the filesystem while the source bytes come from the capture ... if they ever diverge (inferred; not traced)". Related name lists, different jobs: `PC/source/source_imports/project.rs:19,77-83`, `BO/staged_output_tree.rs:395-432`, `SA/git/resolution/workspace_member/declarations.rs:21,75`.
8. **Reachability and cycle checks over a dependency graph, 6 sites.** `PC/package_compilation/dependency_graph.rs:216` (reachable), `:233` (DFS) and `:287` (Kahn count); `PM/resolution/graph/mod.rs:302,333-394`; `PM/resolution/graph/subject/validation/walk.rs:11-45`; `PM/resolution/compiler_input.rs:186-209` (purpose-filtered); `PM/build_session/order.rs:11` (033, 034; 034 grepped the PC site by name without reading it, 033 read it).
9. **Shortest root-to-package path, 2 copies plus an all-paths walk.** `PM/resolution/graph/reconcile/dependency_paths.rs:17-50`, `PM/review/compare/paths.rs:13-142`, `reconcile/resolution.rs:402-512` (034).
10. **Alias rules.** Uniqueness at `PM/resolution/graph/mod.rs:185-194` and `reconcile/resolution.rs:310-327`; default alias `-` to `_` at `subject/validation/closure.rs:101-107` and `subject/catalog.rs:135` (034).
11. **Source subject in two serializations plus a validator.** Binary `PM/resolution/graph/subject/encoding/source.rs:36-125`, text `subject/text/source.rs:98-252`, validator `subject/validation/source.rs:107-132`; a new lineage variant must be added to all three (034).
12. **Bounded text framing with Writer/Reader, 4 places.** `PM/package_manager/proposal/framing.rs:6-121` (length prefix), `PM/lock/text/framing.rs:6,66` (length-delimited; I opened its header only), `PM/resolution/graph/subject/text/framing.rs:8,118` (quoted bytes), `RE/encoding/encode/encoder/text/writer.rs:14` with `RE/encoding/recovery/policy/text/{framing,tokens}.rs` (named scopes) (034 "suspected, not read" for RE; 036 read RE).
13. **Directory tree walkers with entry/byte ceilings, 8 sites.** `PC/source/source_snapshot.rs:370` and `:409`, `PC/reuse/checked_source_cache.rs:287` (033); `BO/capture/mod.rs:45` (032); `PM/review/audit/snapshot.rs:123-179` (034); `SA/custody/tree.rs:334-448`, `SA/git/cache/custody.rs:72-148` (bans symlinks, checks link count), `SA/tree/capture/traversal.rs:51-185` (037).
14. **"File did not change while I read it", 4 sites.** `BO/capture/mod.rs:178-189,227-235`, `PC/build_execution/completed_outputs/verification.rs:179-215` (032); `PC/source/source_snapshot/file_read.rs:108-154` (033); `SA/tree/filesystem.rs:175-214` (037).
15. **Classify directory, open without following links, compare identity, 10 sites in SA.** `custody/tree.rs:189-227,456-487`, `git/cache/custody.rs:28-49`, `git/cache/configuration.rs:23-41`, `git/cache/invalidation.rs:52-70`, `snapshot/publication.rs:62-74`, `git/cache/snapshots.rs:37-54`, `git/snapshot.rs:227-245`, `local/snapshot.rs:170-180`, `local/recovery.rs:84-94` (037; grep of `is_symlink() || !`: 16 hits in 11 files).
16. **Cache lock acquire loop 4 times; owner/mode check twice.** `SA/custody/lock.rs:94,126,167,210`; `SA/custody/platform.rs:53` and `:88` have identical bodies (036).
17. **Snapshot metadata and verification, Git and local.** Parsers `SA/git/snapshot_metadata.rs:30-66` and `SA/snapshot/metadata.rs:36-69`; verifiers `SA/git/snapshot.rs:334-376` and `SA/snapshot/metadata.rs:71-118` (037).
18. **Two Git workspace flows.** Planner-driven `SA/git/resolution/workspace_member.rs:134-191` and session-driven `SA/git/resolution/tree_session.rs` with `tree_session/inventory.rs`; both end in `materialize_member` (`workspace_member.rs:193`) (037).
19. **Git batch request-file sequence twice; exit-status mapping three times.** `SA/git/objects/batch/transfer.rs:49-67` and `batch/availability.rs:51-79` ("suspected from reading both"); `git/git_command.rs:118`, `transfer.rs:146-152`, `availability/protocol.rs:20-26` (037).
20. **Repeated authentication work.** Whole graph at `SA/git/objects/inspection.rs:90` and `SA/git/snapshot.rs:251`; member payloads at `objects/projection.rs:165,190` and `snapshot.rs:251`; repository subtree walked twice per `verify_current` (`git/cache/repository.rs:167,186`); three full captures per new publication (`git/snapshot.rs:192,211,224`) (037).
21. **Length framing and hex helpers.** LE `SA/identity/digest.rs:4`, BE `SA/identity/mod.rs:73`, and `DomainDigestEncoder::bytes`; `fn hash_field` has 4 definitions (`BO/staged_output_tree.rs:515`, `PM/declarations/identity.rs:97`, `SA/identity/mod.rs:73`, `SA/local/observation.rs:60`; grep re-run); `encode_hex` at `SA/identity/mod.rs:110` and `PM/resolution/graph/subject/encoding/framing.rs:15` (037 "suspected"; grep re-run confirms both definitions, bodies not compared).
22. **Two byte cursors in one crate.** `PC/cursor.rs:5` (documented as shared, one user) and `ObservationCursor` in `PC/build_execution/build_record_codec.rs:11` (032).
23. **Encode and recover tag tables written by hand on both sides.** All of `RE/encoding/encode/**` against `RE/encoding/recovery/policy/**`; a third copy of the capability-flow table at `RE/record/package/behavior_policy.rs:29-37`; primitive types decoded by tag and by name (`recovery/policy/structural_expressions/tags.rs:8,28`) (036). Two tables have drifted; see Disagreements and What I checked.
24. **Every public declaration projected twice.** `PackageReview*Shape` in `RE/capture/api/*`, then `PackagePolicy*Shape` in `RE/capture/api/policy/*` (`policy/declarations.rs:33-139`); trait requirements at `api/policy/traits.rs:75-179` and `api/traits.rs:160-268`; termination at `behavior/policy/termination.rs:81-100` and `behavior/termination.rs:42-65` (034). Validators likewise: `RE/record/package/callable_policy/validation/behavior.rs:95-133,161-190` and `RE/record/public_policy/validation/behavior.rs:36-54,58-87` (036).
25. **Boundary singleton checks 3 times.** `RE/capture/callables/boundary_operators.rs:14-56,126-184,186-234` and `callables/boundary_requirements.rs:127-174`; the alignment check at `boundary_operators.rs:70-74` and `boundary_requirements.rs:87-91` (034).
26. **Accepted-binding owner check and permission matching twice.** `RE/capture/terminal_authority_permissions/declarations.rs:67-98` and `RE/capture/runtime_permissions.rs:165-207` ("near-identical text", 035).
27. **Nested generic static-argument walk 3 times.** `RE/capture/contracts/expressions/static_arguments.rs:251-325`, `semantics/conformances/application.rs:63-109`, `semantics/conformances/policy_arguments.rs:205-235` (035).
28. **"Exactly one fact row for this machine" re-fetch, 5 sites.** `RE/capture/callables/surface.rs:133-185`, `source/invocations.rs:29`, `source/service_reach.rs:64-81`, `source/suspension.rs:86-105`, `callables/reach_dependency.rs:25-42` (035).
29. **Parent-trait traversal 3 times.** `RE/capture/calling/application/signature/inheritance.rs:23`, `contracts/propositions/evidence.rs:88`, `semantics/declarations/provider_schema.rs:40-75` (035).
30. **Two scratch-projection carriers and two functions of one name.** `SignatureProjection` (`RE/capture/semantics/signatures/projection.rs:27`) and `validation::CheckingProjection` (not read, "suspected"); `project_type_parameters` at `signatures/parameters.rs:112` and `signatures/policy.rs:29` (035).
31. **Service method authority encoded twice in one record.** Nominal form `RE/encoding/encode/selected_providers/authority.rs:12-56`, string form `selected_providers/service.rs:88-138` (036).
32. **Measure, emit, compare.** `RE/encoding/encode/policy_rows/builder.rs:116-160` and `policy_rows.rs:72-93`; filter-map-sort at `RE/permission_acceptance.rs:70-86` and `:102-116` (036).
33. **OS filesystem entry tables, one per OS.** Darwin `TR/filesystem.rs:201-358`, Linux `TR/filesystem/linux.rs:154-237`, Windows `TR/filesystem/windows/contracts.rs:32-446`; guest descriptor table and guest handle table are the same scheme twice (`TR/filesystem.rs:40,672-738`, `TR/filesystem/windows.rs:18-51`) (032).
34. **Adapter admission shape repeated.** select binding, validate declaration, `admit_exact_foreign_call`: `TR/clock/admission.rs`, `TR/descriptor_io.rs:186-317`, `TR/descriptor_mode.rs:30-77`, `TR/integer_absolute.rs:26-119` (032).
35. **Record leaf enumeration, 4 forms.** `TR/native_provider/record_fields.rs:258`, `record_shape.rs:409-762`, `borrowed_storage/geometry.rs:65-163`, `original_homes.rs:603-659` (032). Also pointer-output selection `TR/integer_parsing.rs:148,247-261` against `native_provider/reference_outputs.rs:249-339`; stdin reads `TR/standard_input.rs` against `TR/descriptor_io.rs:534-576`; ms-to-ns at `TR/clock.rs:427-432` and `clock/windows.rs:161`.
36. **Build execution duplicates.** Required-output collision at `PC/build_execution/filesystem_scope.rs:343-358` and `filesystem_scope/runtime.rs:586-592`; two limit constants (`runtime.rs:583`, `filesystem_scope.rs:83`); literals 4096 and 512 MiB at `snapshot_request.rs:104,108` against 4096 and 256 MiB at `BO/staged_output_tree.rs:48-50`; automatic snapshot choice at `execution_inputs.rs:36` and `filesystem_scope/preparation.rs:90-92`; anonymous root choice at `PC/lib.rs:89-96` and `preparation.rs:50-59` (032).
37. **Package compilation duplicates.** Three `use` resolvers: `PC/source/source_imports.rs:72`, `:186` with `package_selection.rs:204`, and the hand-written branch `product_queries.rs:288-339`; `.omega/generated` path built 4 times (`generated.rs:61,229`, `build_handoffs.rs:428,441`); source custody admission at `provider_defaults.rs:559` and `provider_selections.rs:137`; TerminalPsi measurement check at `native_bindings.rs:69-76` and `native_evaluation.rs:24-29`; handoff construction at `source_consumption/checked_sources.rs:37-58` and `:171-192`; metadata capture at `bindings.rs:55,64`; row capture at `source_snapshot.rs:679` and `:723`; required-member check at `:344-359` and `:288` (033).
38. **Package manager duplicates.** Custody equality at `PM/resolution/graph/accepted.rs:95-116,199-210`, `subject.rs:65-84`, `package_manager/source_review/old_sources.rs:102-107`; role choice at `old_sources.rs:37-41,78-82` and `resolution/compiler_input.rs:122-126`; `MAXIMUM_OUTPUT_BYTES` 8 MiB (`source_review.rs:16`) and 32 MiB (`review/audit/mod.rs:40`); member-subtree compare at `resolution/source/git/workspace.rs:316-327` and `workspace_acquisition.rs:145-186`; `entry_path` at `build_resolution.rs:130` and `acquisition.rs:426-431`; limits 1024 and 16384 at `reconcile/mod.rs:90-104` and `subject.rs:327-331`; record byte limit at `review_cache.rs:44` and `record_codec.rs:10,849`; four conversions between the two request enums (`subject.rs:216`, `text/requests.rs:174`, `request_view.rs:22,47`) (034).

## Fixed lists of shapes

**terminal-runtime (032).**
- Darwin filesystem entries: fixed candidate table, `TR/filesystem.rs:201-358` (27 rows by my count; the record says 26 and lists 27 names). Path formal is exactly `UnsignedArray(4096, 8, SharedBorrow)` (`:199`).
- Windows entries: fixed table `TR/filesystem/windows/contracts.rs:32-446` (27 rows, 22 symbols by my count; the record says 28 and 24); `_ => false` for unknown symbols (`windows.rs:309`); `NtQueryDirectoryFile` only classes 12 and 1 (`contracts.rs:350-353,389-392`); `GetFileInformationByHandleEx` only classes 0, 1, 9 (`:171-214`).
- Linux: eight operations (`TR/filesystem/linux.rs:109,154-237`).
- Clock selectors: `(_,0)`, Darwin 4|5|6|8|9, Linux 1|4|7, Windows 1 (`TR/clock.rs:75-78`); Poll is `[u64 pointer, u32|u64 count, i32 timeout]` (`TR/clock/admission.rs:134-137`); timespec is 16 bytes at offset 0 (`TR/clock/timespec.rs:47-48`).
- Three zero-argument provider symbols only (`TR/reference_state.rs:85-98`); two output kinds, line and byte (`TR/buffered_output.rs`); `strtol` with 8-byte pointers only, no Windows entry (`TR/integer_parsing.rs:148-153,294`); records of N 64-bit words (`TR/scalar_record.rs:230-239`); float-to-integer truncation up to 64 bits (`TR/numeric.rs:273-282`); owned records are scalar, finite record or fixed array of literal length (`TR/native_provider/record_shape.rs:579-583,614-618`); reference slots 8 bytes aligned 8 (`original_homes.rs:674-676`); discriminator tags 4 bytes (`borrowed_storage/geometry.rs:32`); native traps: signals 4 and 5, or Windows statuses `0xc000001d`/`0x80000003` (`TR/native_provider.rs:857-867`).

**build-declarations and build-output (032).** "direct unconditional" dependency statements only (`BD/dependencies/mod.rs:163-169`); exactly one role kind, `artifact_only` only with an application (`BD/syntax_projection/mod.rs:263-274`). Staged tree ceilings 4096 entries, 256 MiB, 16 MiB of paths (`BO/staged_output_tree.rs:34-52`); on a host that is neither Unix nor Windows, capture refuses every file (`BO/capture/mod.rs:252-255,294-297`).

**package-compilation (032, 033).** Access mode by what is bound: `CapturedScoped`, `RealScopedSponsored`, `RealScoped` (`PC/build_execution/filesystem_scope.rs:412-425`); numeric errno literals 13, 45, 21, 40, 2, 5 (`filesystem_scope/runtime.rs:293-325`); grant roots `SourceInventory|StagedOutput|Other(u32)` (`build_record_codec.rs:582-587`); locator tags 0 to 4 (`package_compilation/native_output_codec.rs:58-94`); symbol-kind tags 0 to 5 with `unreachable!` for others (`provider_selection_codec.rs:417-437`); 10 accepted semantic-binding roles (`PC/semantic_bindings.rs:39`); import candidates `<p>.omg`, `<p>/mod.omg`, `<p>.omega`, `<p>/mod.omega` (`source/source_imports/paths.rs:139-155`); entry contract source is Bundled, Package or Unavailable (`entry_contract_seed.rs:107`).

**package-manager (033, 034).** `Source::Path{location}` with exactly 1 field, `Source::Git` with 2 or 3, `PackageSelection::Root|Named` (`PM/declarations/dependencies/read/source_literal.rs:41,57,129-141`); automatic edits need the exact signature tokens (`dependencies/edit/layout.rs:108`); lock header `omega_lock 4`, with `omega_lock 3` readable as review baseline only (`lock/text.rs:10`, `lock/text/reader.rs:34-38`); request match handles Path/Path and Git/Git, `_ => false` (`resolution/graph/accepted.rs:246`); subject binary versions 7, 8, 9 by catalog shape (`subject/encoding/selection.rs:62-72`) and text versions 1 to 4 (`subject/text/record.rs:26-29`); catalog providers "must be a flat closed package source graph at the same target" (`subject/catalog.rs:97-103`); exactly two review passes, no fixed-point loop (`review/candidate/passes.rs:93-95`); two purposes hard-wired as `[None; 2]` (`passes/occurrences.rs:281`); only `Application` roots get target-entry discovery (`:390`); root-role change flagged only for `Package<->Application` (`review/compare/mod.rs:224-231`); cached record decodes authority flow `"accepts"` only and termination subject tag 1 only (`review/candidate/review_cache/record_codec.rs:401-404,440-443`).

**resolver-execution (034).** Four phases that differ only in the error name; all get the same limits (`RX/resolver_execution.rs:24-29,70-76,86-96`).

**review-evidence (034, 035, 036).**
- 14 fixed dangerous-service rows, anything else `Ok(None)` (`RE/capture/authority.rs:25-40,111`).
- Fixed-token boundary operators: two operands among `Add, Subtract, Multiply, Divide, Modulo, Equal, NotEqual, Less, LessEqual, Greater, GreaterEqual, Index` (`callables/boundary_operators.rs:99-114`); `"must contain exactly one schema method"` (`:28`); `"external leaf has {} realization edges; exactly one is required"` (`callables/external_policy.rs:27-31`).
- Contract expressions refuse `TypeExpression`, `SavedSubject`, `UndeterminedNaN`, destructuring `Match` (`contracts/expressions/projection/mod.rs:77-91`); atomic loads only `NoOrdering|Receive|GlobalOrder` (`expressions/atomic_loads.rs:9-30`); index expressions from a closed list, 17 kinds rejected (`semantics/types/validation.rs:252-428`); conformance applications only in the "lifetime-free cohort" (`static_arguments/conformance.rs:132-162`); const integers only for `i8..u64, addr` (`const_values.rs:78-83`).
- Two host services and two hosted-exit roles (`capture/semantic_bindings.rs:1-18`); only `FilesystemHostService` and `TimeHostService` get permission rows (`PM/review/candidate/semantic_bindings.rs:132-169`).
- 20-variant intrinsic execution mapping (`providers/intrinsics.rs:63`); families seeded only from `BuildOverride`/`TargetDefault` (`providers/policy/families.rs:39-44`); row count hard-codes `3` non-collection rows (`encoding/encode/policy_rows/assembly.rs:20`); 26 row kinds (`record/policy_rows.rs:48-77`); capacity 256 for calling shape graph, parameters, callback and opaque catalogs (`record/calling/application/validation/shapes.rs:21,99`); evaluated syscalls on Linux only (`record/package/provider_policy/binding_validation.rs:24,60`); membership frame labels and topology words are closed lists (`encoding/encode/membership/framing.rs:78-197`).

**source-acquisition (037).** Git tree entries: `(160000,commit,-)`, `(040000,tree,-)`, `(100644|100755|120000,blob,size)`, else "unsupported Git tree entry" (`SA/git/objects/tree.rs:119-143`); two object formats (`git/cache/creation.rs:227-240`); schemes `https` and `ssh` (`identity/locator.rs:17-33`); `may_preserve_cache` lists 6 error variants, everything else invalidates (`git/cache/retry.rs:65-75`); listing allowance "+4" for the three root exclusions (`tree/capture/traversal.rs:72-77`); file kinds symlink, directory, regular file, else `UnsupportedFileType` (`traversal.rs:169`).

## Accepts when it does not know

### Classed "open" by a reader (no reason at the site, not traced)

terminal-runtime (032):
- `TR/clock.rs:405-409,422`: Poll ignores its pointer argument; "a poll with a nonzero pointer and zero count is treated as a pure sleep". Not traced whether the pointer is validated elsewhere.
- `TR/filesystem.rs:437-439`: `admit_services` checks only additional services and skips the owning service, while `supplied` is true for any `HostNamespace` delegation. No wrong program named.

build-declarations and build-output (032):
- `BD/dependencies/mod.rs:162`: dependency calls nested in other states or helpers are not seen by `project_dependency_rows`; "open for this crate". See Disagreements 6: the package-manager caller rejects them.
- `BO/captured_source/mod.rs:68-75,159`: `matches_row_kind` ignores `digest` and the enum fields are public, so an entry whose digest does not match its bytes would be accepted; not traced whether any caller builds variants directly.
- `BO/staged_output_tree.rs:136-176`: `from_retained_parts` accepts a tree with a missing parent directory or a symlink target that escapes the root, if the commitment matches; the only non-test caller is `PC/build_execution/build_record_codec.rs:233`.

package-compilation (032, 033):
- `PC/build_execution/build_admission.rs:78-81`: no reach-plan row for the build machine means empty reach, so it is admitted as `Pure` with no restrictions; plan construction not read.
- `PC/build_execution/build_record_codec.rs` (observation at end of its entry): decoded `RestrictedBuildRequest` and `BuildObservationSummary` are rebuilt without re-running admission; whether the cache key binds them is "open".
- `PC/build_execution/filesystem_scope.rs:91,99-101`: `overlap_key` falls back to a lexical or raw path, so "an odd path then compares by spelling only".
- `PC/build_execution/filesystem_scope.rs:630-632`: `staged_output_tree` returns `Ok(None)` with a reachable filesystem and no sponsor; traced unreachable on the Terminal runtime path (`runtime.rs:75-81`), other paths not traced.
- `PC/package_compilation.rs:370`: `from_parts` skips the missing-handoff check when the handoff map is empty; callers not traced.
- `PC/package_compilation/provider_selections.rs:39-41`: a native declaration's source seed is kept whenever it has a dependency alias; no comment.
- `PC/source/source_imports/entry_contract_seed.rs:77`: `Unavailable => continue` skips seeding that target's `ProgramEntry` contract with no diagnostic.
- `PC/source/source_snapshot.rs:352`: a required member outside the root is skipped; callers in `filesystem_scope/preparation.rs` "not read" by 033.
- `PC/reuse/review_cache_digests.rs:142`: a missing retained file contributes nothing to the digest.
- `PC/source/toolchain_library.rs:126`: the library digest hashes only `.omg` files while `.omega` files are importable (`source_imports/paths.rs:151-153`).

package-manager (034):
- `PM/resolution/compiler_input.rs:196-198`: a reachable key with no graph node is kept and contributes no edges; "I infer it cannot occur".
- `PM/resolution/graph/accepted.rs:60-66`: catalog search takes the first match before checking purpose, so "two catalog entries differing only by purpose would see only the first".
- `PM/resolution/graph/mod.rs:60-66` and `PM/resolution/source/custody.rs:101`: lineage check is `debug_assert!` only; in release "a key/resolution pair with mismatched lineage" gets through if a caller passes unvalidated parts.
- `PM/resolution/graph/subject/catalog.rs:31-35`: `zip` silently drops extra entries if lengths differ; "I infer it matches".
- `PM/review/candidate/review_cache.rs:197-200`: the trailer is an unkeyed SHA-256; "a hand-built record with a correct trailer and matching lineage/resolution digests would be accepted", and the build handoff is not recomputed; open for the handoff and policy content.
- Panics, not accepts, also classed open: `PM/review/candidate/error.rs:184` (byte slice of a name), `PM/review/compare/assembly.rs:73-75` (slice indexing if lock occurrences are misaligned).

resolver-execution (034):
- `RX/resolver_execution/path_custody.rs:77`: when a root cannot be canonicalized only the lexical test runs; "an executable whose canonical path lies under a root that cannot be canonicalized at open time" passes. Not traced to callers.

review-evidence (034, 035, 036):
- `RE/capture/authority.rs:111`: "a new core service not listed in the table is reviewed as ordinary"; also `TimeHostService` is not in the aggregate-binding list.
- `RE/capture/api/data/evidence.rs:154-155,181-182`: a context or symbol set with a missing span that is not about a data symbol is skipped.
- `RE/capture/api/slot_metadata.rs:51-56`: no slot plan means empty metadata; not traced whether a requirement with a returned reference and no plan can exist.
- `RE/capture/behavior/policy/receivers.rs:20-22`: a call whose authored expression is not a call node is skipped.
- `RE/capture/callables/boundary_operators.rs:203-205` and `callables/boundary_requirements.rs:108-110`: `Ok(())` when no selected plan lists the machine; "an external leaf that is not a provider-selected realization is not joined to a plan at all".
- Missing identity text becomes `""` and is then compared or stored: `callables/boundary_operators.rs:141-143`, `callables/boundary_requirements.rs:92-94` ("an empty identity would equal an empty plan identity text"); `callables/external_supply.rs:164,169`; `contracts/expressions/static_arguments.rs:319-320,400-401`; `contracts/propositions/binders.rs:68-69`; `semantics/types/identity.rs:60,130` (a type "recorded with an empty identity rather than rejected").
- `RE/capture/callables/surface.rs:199-200,264`: Build role with inferred ceilings yields no declared values; "open whether Build reaches are otherwise validated".
- Pass-through arms that leave a type reference unsubstituted: `calling/application/signature/instantiation.rs:227`, `semantics/conformances/policy_arguments.rs:190`; not traced which node kinds reach them.
- `RE/capture/contracts/expressions/calls.rs:99-109`: only Borrow arguments are type-compared.
- `zip` truncation: `package_policy/native_bindings.rs:21-26`, `runtime_permissions.rs:185-187`, `terminal_authority_permissions/declarations.rs:87-92`; lengths not shown equal.
- `RE/capture/package_policy/semantic_dependencies.rs:143-145`: an invalid root symbol is skipped.
- `RE/capture/package_review.rs:53-63`: the selection-observation check is skipped when the handoff has none.
- `RE/capture/providers/application_realizations.rs:339-346`: an expression with an empty span and no authored selection returns `Ok(false)` and the use is dropped from review.
- `RE/capture/providers/policy/rows.rs:111-113` with `providers/intrinsics.rs:264-265`: an unsupported compiler-intrinsic row is accepted into the policy with no execution identity, unlike the application path (`application_realizations/compiler_intrinsics.rs:277`) which rejects it; later consumer not traced.
- `RE/capture/semantic_bindings.rs:120-122`: a `ProgramEntry` requirement without exactly one boundary definition is skipped (omits a candidate, does not accept one).
- `RE/capture/semantics/declarations/ownership.rs:145`: a symbol with no source file counts as Product scope.
- `RE/encoding/encode/membership/framing.rs:73-75`: names not starting with a digit are accepted without scanning; the external visitor was not read.
- `RE/record/package/provider_policy/binding_validation.rs:180`: `(ToolchainSource(_), None) => true` accepts any toolchain digest.
- `RE/record/package/callable_policy/validation/signature_expressions.rs:125-148`: calls to `BuiltinFunction`, `ByteSequencePredicate`, `ProofView` get no target check.
- `RE/record/package/provider_policy/validation.rs:30`: an empty `plan.target` string is accepted.
- `RE/record/package/callable_policy/validation/structural.rs:106-112`: float and integer literal text is checked only for non-emptiness.
- Unclassed by 036 but recorded as read: `RE/record/baseline/validation.rs` does not inspect `assumed_bindings` at all and checks only the target of `restricted_build_requests` (`:127-131`).

source-acquisition (037):
- `SA/git/cache/custody.rs:187-193` (F2): no link-count check off Unix; open unless `custody/platform.rs` covers it.
- `SA/git/cache/retry.rs:41` (F5): a symlink or directory named `source.identity` counts as "complete"; depends on `RecordFileRoot` (not read).
- `SA/identity/resolution.rs:77-87` (F11): `matches_lineage` compares only the family, so "a `Git` resolution paired with a different Git lineage passes this predicate".

### Classed "stated" (a comment at the site says why)

- 032: Windows CRT write to other descriptors returns -1 (`TR/descriptor_io.rs:467-471`); `_setmode` stores any mode, "returns the previous mode verbatim" (`TR/descriptor_mode.rs:4,126,133`); modeled std streams bypass the service check, stated by test `modeled_stream_supply_does_not_authorize_native_descriptor_namespace` (`TR/native_provider.rs:606-616`, `TR/filesystem.rs:770`); accepted role call is "the unique occurrence" (`BD/syntax_projection/mod.rs:9-15,433`); `AvxFma3` admits nothing off x86_64 (`PC/build_execution/build_value/configuration.rs:110-113`); "legacy empty-selection behavior" (`build_value/optimization.rs:3-7`); captured input adopts metadata when none bound (`filesystem_scope.rs:232-234`); plain standalone builds use a live source (`filesystem_scope/preparation.rs:7-8`); private snapshot builds report no restricted request (`restricted_requests.rs:183-185`, also traced).
- 033: provider defaults `(None, None) => continue` (`PC/package_compilation/provider_defaults.rs:330,380`); warm cache hit on stat fingerprint (`PC/reuse/checked_source_cache.rs:12-14,142-148`); no lock means no acceptance check (`PM/operations/accepted_compilation.rs:6-8,383`); inspection forces `verify` (`:13-14,450-454`); old-schema rows give no compile inputs (`PM/lock/acceptance.rs:20,49-55`).
- 034: baseline unavailable, diff rendered standalone (`PM/package_manager/source_review.rs:51-57,96`); fresh resolve when no pin and not offline (`PM/resolution/graph/git_pins.rs:47`); lock check opt-in per operation (`PM/resolution/source/acquisition.rs:77,109-112`; which operations call it not traced); `Root` and `HistoricalGit` evidence always revalidates (`PM/resolution/source/selection.rs:26-28`); in-memory captures trusted (`PM/review/audit/snapshot.rs:189-191`); `Live` carried compile skips re-derivation (`PM/review/candidate/passes/occurrences.rs:126`); legacy v1 text is product-only (`subject/text/record.rs:4-6,228-232`); non-owned operators skipped (`RE/capture/api/operators.rs:73-79`); absent inferred crash summary gives `Unknown` (`RE/capture/behavior/policy/crash.rs:112`); calls with no concrete machine body pass (`RE/capture/behavior/policy/flow.rs:6-9,85-104`).
- 035: role-based skips and dedup (`RE/capture/callables/policy.rs:6-8,85,90-91`); no authored target means `TargetIndependent` (`callables/target_context.rs:1-4`); `ProofView` accepts any name that resolves to nothing (`contracts/expressions/calls.rs:1-5,254-287`); the assumed-binding audit never rejects, "without vetoing accepted semantic links" (`package_policy/assumed_bindings.rs:6-8`); three `continue`s for compiler-intrinsic applications (`providers/application_realizations/compiler_intrinsics.rs:7-9,14-17,55-81`); boundary operators carry empty authority (`semantics/services/authority.rs:5-7,84-90`); absent producer row yields `""` (`semantics/declarations/identity.rs:5-8,52`); representation demands omitted with no selected availability (`representation/policy.rs:5-7,65-67,76-78`; "not traced for a missed demand").
- 036: uncatalogued target tags are "valid inert disclosure" (`RE/record/target_context.rs:5-6`); toolchain owners skip demand validation (`RE/record/baseline/boundary_owners.rs:4-5,30`); off Unix owner/mode checks return `Ok(())` and off macOS ACL checks return `Ok(())` (`SA/custody/platform.rs:5-12,78-85,113-120`); toolchain-source owner labels never looked up (`RE/encoding/encode/membership/framing.rs:143`; "inferred" from the module header).
- 037: `NotFound` proves absence (`SA/git/cache/custody.rs:209,232`, F4, pinned by a test); a non-id revision accepts whatever commit Git resolves (`SA/git/objects/authentication.rs:48`, F6; `git/mod.rs:13-24`); `SelectedOnly` listing keeps gitlinks and `.gitmodules` inert (`git/objects/tree.rs:100-118`, F7); off Unix directory mode is not checked (`snapshot/permissions.rs:113-153`, F13, "stated-ish").

### Classed "traced" (the reader followed callers)

- 032: `TR/borrowed_storage.rs:352` bounded by host count; `TR/filesystem.rs:687` unknown descriptor becomes -1; `TR/filesystem/linux.rs:430` covered by `:384-388`; `TR/filesystem/windows.rs:34` passes NULL only; `TR/native_provider/original_homes.rs:86-88`; `TR/native_provider/reference_outputs.rs:61-65` bad layout rejects later; `TR/reference_state.rs:159-167` application checks are skipped when no provisions are supplied, while four sibling adapters return an error; `BO/materialization/mod.rs:71`; `PC/build_execution.rs:122`; `PC/build_execution/build_record_codec.rs:209-682` (`Vec::with_capacity(count)` on corrupt input, a robustness hazard, not an accept path); `PC/build_execution/build_value/configuration.rs:179` ("inferred, not traced to the checker").
- 033: `PC/source/source_imports/frontier.rs:587-591` default-package attribution (validation precedes for package routing; virtual paths inferred); non-Unix cache fingerprint gap ("inferred").
- 034: `PM/resolution/graph/accepted.rs:56-75`; `graph/mod.rs:318,365`; `reconcile/resolution.rs:192-196`; `subject/text/record.rs:336`; `subject/validation/dependency.rs:83,92-105` and `validation/source.rs:29-44`; `resolution/source/custody.rs:60`; `resolution/source/local/recovery.rs:29`; `review/candidate/passes.rs:158`; `passes/occurrences.rs:557-572`; `review_cache.rs:95-129`; `review/compare/mod.rs:221-223`; `review/decision/mod.rs:62-66`; `review/restricted_build_grants.rs:137` (fail-closed); `RE/capture/behavior/crash/expressions.rs` unsupported shapes become errors at `behavior/policy/crash.rs:105-110`.
- 035: `RE/capture/calling/callbacks/layouts.rs:114-129`; `providers/policy/bindings.rs:149-150`; `source/family_interfaces.rs:35-37`; `terminal_authority_permissions/policy.rs:46`; `semantics/types/validation.rs:221-226,241-246` (two same-named binders reject).
- 036: discarded encoder results are sticky (`RE/encoding/encode/encoder.rs:241`, `values/providers/carry.rs:5-33`); acceptance-only row skipping is count-checked (`policy_rows/builder.rs:81,175`); `RE/record/public_policy/validation/declarations.rs:199-205,235-246`: "foreign-package traits are therefore accepted unverified; no comment at the site".
- 037: F1 `SA/custody/tree.rs:293-296`; F3 `git/executable/selection.rs:179`; F8 `git/request.rs:248-261` (abbreviated hex is treated as a selector); F9 `git/resolution/workspace_member.rs:84-93` (limits not clamped; no external caller); F10 `tree_session/inventory.rs:103,121`; F12 `local/recovery.rs:149-151` (returns a miss); F14 `tree/filesystem.rs:223-226` ("inferred").

## Silent drops

Places where failing to plan or resolve produces no row and no diagnostic. Counts are distinct cited
sites (my count): terminal-runtime 3, build-declarations 2, build-output 0, package-compilation 10,
package-manager 11, resolver-execution 0, review-evidence about 50, source-acquisition about 11.

- terminal-runtime: layouts whose codec fails are dropped by `filter_map(.ok())` (`TR/native_provider/reference_outputs.rs:61-65`); Poll pointer ignored (`TR/clock.rs:407`); unknown guest descriptor mapped to -1 (`TR/filesystem.rs:687`).
- build-declarations: statements that are not direct calls, and nested dependency calls, are not rows (`BD/dependencies/mod.rs:162-169`); the target-slot `replace` does not count matches, unlike the optimization slots which assert exactly one (`BD/prelude/targets.rs:36`, `BD/prelude/optimizations.rs:17-23`).
- package-compilation: `PC/package_compilation/provider_defaults.rs:330,380`; `source_imports/entry_contract_seed.rs:77` (and `:72-74` by my reading: package root not found also `continue`s); `source/source_snapshot.rs:352`; `reuse/review_cache_digests.rs:142`; cache store returns `Ok(false)` when full (`reuse/checked_source_cache.rs:199-224`) and a bad record is deleted and treated as cold (`:184-187`); corrupt codec input decodes to `None`, a miss (`build_execution/build_record_codec.rs:1-6`); optional `Build` fields default when missing (`build_value/configuration.rs:217,250,262`); `LegacyWithoutField` gives default selections and a suppressed report (`build_value/optimization/selection.rs:46-51`).
- package-manager: custody not in `reachable` dropped (`PM/resolution/compiler_input.rs:65`); cache-directory open failure falls through (`:176-183`); node missing `continue` (`:196-198`); cache coordinates vanish through `.ok()` (`resolution/source/custody.rs:47-61`); `zip` truncation (`subject/catalog.rs:31-35`); non-dependency requests skipped (`declarations/dependencies/read`, `from_executed_requests` `:90-97`); review-cache read failures become fresh compiles (`review/candidate/passes/occurrences.rs:557-572`, `review_cache.rs:160,174`); no `build.omg` and no lock returns `Ok(None)`, standalone (`operations/prepare_project.rs:235-237`); no matching accepted occurrence gives empty bindings (`operations/accepted_compilation.rs:537-547`); missing occurrence gives an empty grant set (`review/restricted_build_grants.rs:137`); local cache miss `Ok(None)` (`resolution/source/local/recovery.rs:29`).
- review-evidence, the concentrated ones (035's list plus 034 and 036): non-owned declarations omitted by `owned_identity(...)? else continue` (e.g. `RE/capture/api/constants.rs:34-36`, `api/operators.rs:73-79`); compiler-intrinsic applications (`providers/application_realizations/compiler_intrinsics.rs:55,58,74-81,218-224,256-263`); package-ownership filters (`providers/application_realizations.rs:55-64,152-160,339-346`); representation demands (`representation/policy.rs:65-67,76-78`, `representation/policy/selections.rs:74-76`); semantic-binding nomination (`semantic_bindings.rs:120-122,133-135`); callables skipped by role or owner (`callables/policy.rs:85,90-91`); `TopLevelRequirement` supplies dropped from external projection (`callables/surface.rs:124`); non-boundary parents skipped (`calling/application/signature/inheritance.rs:98-100`); un-substituted pass-through (`signature/instantiation.rs:227`, `signature/parameters.rs:124`, `semantics/conformances/policy_arguments.rs:190`); `Crashes` contracts skipped here, handled elsewhere "suspected" (`contracts/facts.rs:152`); demands for non-public or non-owned machines (`providers/boundary_policy/demands.rs:26-31`); operators and non-installation-bound requirements give `Ok(None)` (`providers/installation.rs:90,92`); toolchain-settled methods with no row (`providers/policy.rs:159-161`); services with no permissions (`terminal_authority_permissions/policy.rs:46-48`); `ExactMachine` routes dropped (`semantics/declarations/establishment_routes.rs:28`); non-`Terminates` guarantees give no rows (`semantics/services/authority/progress.rs:18`); acceptance-only builder skips non-initial rows (`encoding/encode/policy_rows/builder.rs:81-83`); toolchain owners skipped (`record/baseline/boundary_owners.rs:30`); foreign traits not checked (`record/public_policy/validation/declarations.rs:199-205,235-246`); plus the `zip` and skip entries listed under "open".
- source-acquisition: unreadable legacy candidates skipped (`SA/local/recovery.rs:149-151`); missing collection `Ok(None)` (`:72-76`); `PATH` candidates skipped (`git/executable/selection.rs:93-103`); gitlinks filtered from the catalog (`git/objects/inspection.rs:43-47`); trees and gitlinks skipped in payload hashing (`git/objects/authentication.rs:143`); excluded names not captured (`tree/capture/traversal.rs:284-293`); cleanup errors ignored with `let _ =` (`snapshot/permissions.rs:327,347,366,439`); `NotFound` ignored on invalidation (`git/cache/invalidation.rs:46-77`).

## Unused or unwired

With the readers' grep counts.

- **Baseline named-text recovery, 0 callers, about 353 lines.** `PackagePolicyBaseline::recover_text_with_usage` (`RE/encoding/recovery/policy/text.rs:35`), `PackagePolicyTextRecoveryLimits` (`:14`), `text/framing.rs`, `text/tokens.rs`. Grep "finds no caller of the baseline text recovery outside its own file (only README.md:250,294 mention it)"; the limits type cannot be named outside the crate (036). Grep re-run: confirmed.
- **Planner-driven workspace API, 0 callers outside the crate.** `GitWorkspaceProjectionPlanner`, `GitWorkspaceSelection`, `GitWorkspaceProjectionResult`, `resolve_git_workspace_member_from_pin_in_lanes`, `resolve_git_workspace_member_at_revision_in_lanes` (`SA/git/resolution/workspace_member.rs:68-132`, `SA/git/workspace/workspace_declaration.rs:132-146`); package-manager uses `GitWorkspaceSession` (037). Grep re-run: 29 occurrences in 12 files, all in source-acquisition.
- **`test-fixtures` feature**: defined at `packages/source-acquisition/Cargo.toml:9`, 1 hit across `**/Cargo.toml`, so `GitExecutionTransport::File` exists only for this crate's tests (037).
- source-acquisition, other: `PrimaryGitSelection::capture` 0 hits outside the crate (`SA/git/executable/selection.rs:23-29`); `Present.size` never read in production and `ExactGitObjectKind::Tag` never compared (`git/objects/batch/availability.rs`); `PendingCacheEntry::create` ignores `_cache_identity` (`SA/custody/publication.rs:197`); test-only lock openers `open_retained`, `open_git`, `acquire`, `acquire_within`, `acquire_local` (036); `format_sha256` and `format_hex` are two names for `encode_hex` (`SA/identity/digest.rs:9-15`).
- package-compilation: the "shared bounded byte cursor" has 1 user outside its file (`PC/cursor.rs:5`; `reuse/checked_source_cache.rs`); `push_u64`/`push_bytes` used by one outside file (`PC/codec_bytes.rs:8-15`) (032). `BuildHandoffCustodyMismatch` (`package_compilation/errors.rs:101`) is not raised in this crate; 3 total matches, constructed in `PM/review/candidate/passes/occurrences.rs` (033). `default_alias_spelling` returns its input (`source_imports/package_selection.rs:386`). `review_record_encoding.rs:4-6` says "Each tag table exists once"; 8 files use it, and three of its tables have other copies (items 3 to 6 above).
- package-manager: `NestedBuildActivationRequired` (`PM/operations/prepare_project.rs:116`) "constructor not found in this unit", not grepped (033); four acceptance row methods return constant `true` (`PM/lock/acceptance.rs:170-181`); empty inherent impls at `resolution/graph/reconcile/mod.rs:79,87,147` and `resolution/source/mod.rs:128`; unused parameter `_requester_navigation` (`subject/validation/dependency.rs:18`); `BuildDeclarationEvidence.repository_path`, `.byte_count` and `GitWorkspaceMemberPlan.role` used only through derived equality ("suspected", `resolution/source/git/workspace.rs:137-156`); `PackageSourceVerificationPhase` has one variant (`review/candidate/review_set.rs:112-114`); `compiled_review_key_with_identity` has 1 production caller outside its file.
- resolver-execution: `phase` is not used for any check (`RX/resolver_execution.rs:70-76`); `ResolverExecutionBackend::open` has 1 production caller and 6 test callers.
- review-evidence: `encode/values/crashes/vocabulary.rs:2` says the rows are shared with recovery; grep finds no recovery user (036). `recovery/mod.rs:1-8` says it owns "both directions of the public recovered-row envelope"; it declares only `mod policy` (036). Text framing accepts `u128`/`i128` tokens the writer never emits (`recovery/policy/text/framing.rs:148-155`). `PackageReviewEvaluatedBindingUsage`: all 3 occurrences in its own file. One-caller functions: `project_signature_crash_routes`, `review_domain_lifetime_label`, `policy_preallocated`, `canonical_text_with_element_count`, `encode_installation_reach`, `encode_boundary_application`, `encode_supply`, `encode_physical`. Unused re-exports kept on purpose (`encoding/encode/declarations.rs:15-41`). Termination subject tag 1 is unused (`encode/callable_policy/behavior.rs:124,128`). Empty `impl X {}` blocks remain across `RE/record` (list in 036, e.g. `record/propositions.rs:63,93,103,113,153,161`; my grep of single-line `impl ... {}` under `record/` counts 32).
- build-output: `validate_retained_tree`, `canonical_symlink_target`, `tree_from_entries.rs` and the mode and symlink validators are `#[cfg(test)]` only; production constructors do not run the parent-directory, strict-order or symlink-target checks (`BO/materialization/mod.rs:74-161,233,261-313`) (032).
- terminal-runtime: `PointerOutputPlan` has one user, `strtol`; `admit_c_word_record` has 1 non-test call site (`TR/integer_division.rs:37`); `runtime_boundary` is compiled for tests only (032).

## Stated limits

- "CPU accounting, signals, and descriptor readiness remain unsupported" (`TR/clock.rs:17`).
- "The host explicitly assumes these implementations are equivalent" (`TR/descriptor_io.rs:14-19`).
- "Failed close may have retired the OS number" (`TR/filesystem.rs:42`).
- "The private worker supplies no additional service authority on its own" (`TR/native_provider.rs:9`).
- "input accesses unmodeled permission or byte-descriptor storage" (`TR/borrowed_storage/geometry.rs:114-118`).
- "default cleanly when omitted by builds that predate those surfaces" (`PC/build_execution/build_value/configuration.rs:3-10`).
- "retained custody never falls back to a real-host grant" (`PC/build_execution/filesystem_scope/preparation.rs:7-8`).
- "Only the compilation root may hold build-scope edges." (`PC/package_compilation.rs:147`).
- "Corruption, truncation, or capacity limits degrade cleanly to cold captures" (`PC/reuse/checked_source_cache.rs` header).
- "Absent on version 3 records, which remain usable as review baselines but not compile inputs." (`PM/lock/acceptance.rs:20`).
- "this direct compiler route does not execute dependency builds" (`PM/operations/prepare_project.rs:140`).
- "without claiming to bound host CPU time or process memory" (`PM/build_session.rs` header).
- "offline resolution forbids new or refreshed Git selection; an unchanged recorded pin is required" (`PM/resolution/graph/git_pins.rs:44`).
- "Legacy `omega-source-closure 1` records remain readable as the versioned lock migration." (`PM/resolution/graph/subject/text/mod.rs` header).
- "sealed compiler program storage is not serializable" (`PM/review/candidate/review_cache.rs:5-16`).
- "Nothing in this crate produces canonical package evidence; git command policy belongs to `source-acquisition`." (`RX/lib.rs` header).
- "not yet represented by package review" (`RE/capture/callables/conformances.rs:172-177`; also `contracts/expressions/calls.rs:288-290`, `static_arguments.rs:241-250,426`).
- "no canonical structural-pattern record; refusing to erase payload bindings" (`RE/capture/contracts/expressions/projection/mod.rs:91`).
- "indexed domain membership clauses require a canonical package-review application record" (`RE/capture/contracts/memberships.rs:22`).
- "outside the lifetime-free cohort with type parameters and exact literal, caller-binder, or named canonical const parameters" (`RE/capture/contracts/expressions/static_arguments/conformance.rs:160`).
- "version 8 rows must be re-reviewed rather than decoded" (`RE/record/policy_rows.rs:6-7`).
- "Closed-vocabulary tag retired without reuse" (`RE/encoding/recovery/policy/policy_recovery_accounting.rs:94`).
- "`UnscopedFilesystemExecution` remains in the enum so review surfaces it if an evaluator ever carries it" (`RE/record/restricted_build.rs:9-11`).
- "Non-Unix targets enforce portable kind, handle identity, and bounded topology checks without promoting host ACLs into resolver isolation evidence" (`SA/custody/platform.rs:6-8`).
- "submodule edges that must become explicit package edges before they are supported" (`SA/error.rs:243`).
- "Omega's own workspace needs 163,219 graph entries / 23 MiB of listing" (`SA/limits.rs:7-9`).
- Inferred by 037, not run: on Windows a Git tree with a `100755` blob would mismatch at `SA/git/snapshot.rs:193-200`, because the executable bit is constant off Unix (`SA/tree/filesystem.rs:223-226`).

## Disagreements

No record contradicts another on what a line of code does. There are no second-pass records for this
area. The items below are open questions one record left that another record or the code settles, and
counts inside a record that the code does not bear out.

1. **Discarded encoder results (035 "open" against 036 "traced").** 035 could not say whether `let _ =` at `RE/encoding/encode/declarations/data.rs:15-49,111,125` and `encode/calling/callbacks.rs:68-76` loses errors. 036 says errors are sticky. Code: `append` latches `exceeded` (`encoder.rs:129-150`), `scope` marks the text writer failed and re-runs `check()` (`encoder/text.rs:79-97`), and the closures at those sites only write tags and scalars. 036 is right for these sites. One caveat from the code: depth and element-ceiling errors from `nested`/`policy_elements` are returned without setting a flag (`encoder.rs:91-110`); none of the discard sites I opened calls them.
2. **`CacheEntryLock::acquire_local` (036 unsure, 037 implies test-only).** The definition is `#[cfg(test)]` (`SA/custody/lock.rs:166-167`) and the caller at `SA/local/snapshot.rs:66` is inside `#[cfg(test)] fn publish_local_snapshot` (`:46-47`). Test-only; 037 is right.
3. **Table sizes in 032.** The record says a "26-row Darwin candidate table" and lists 27 names; the code has 27 rows (`TR/filesystem.rs:201-358`). It says "28 Windows entry contracts" in one place and "24 Windows symbols" in another; the code has 27 rows over 22 distinct symbols (`TR/filesystem/windows/contracts.rs:32-446`; three symbols have 3, 3 and 2 rows).
4. **Callers of `PackageCompilationInputs::from_parts` (033).** The record names `source_assembly.rs:386-387`. Grep finds three call sites: `PC/source/source_assembly.rs:387` (default target), `PC/package_compilation.rs:387` (`with_compilation_purpose`), and `compilation/products/src/compile_request.rs:448` (outside this area, passes real per-target inputs). The empty-handoff skip at `:370` is as described; whether a required handoff can be absent at the third site is still not traced.
5. **Line count in 034.** The record's heading says resolver-execution is "6 files, 534 lines"; the unit table and crates.json give 464. Coverage is unaffected.
6. **Hidden dependency calls (032 "open for this crate", 033 describes a guard).** `project_dependency_rows` has one production caller (`PM/declarations/dependencies/read/projection/calls.rs:31`), and the same projection then runs `reject_unprojected_dependency_syntax` (`PM/.../read/projection/mod.rs:49,55`, `read/policy.rs:54-154`), which rejects any dependency-named call whose operands are not among the accepted rows. So on the source-syntax route hidden calls are rejected. The executed-requests route (`from_executed_requests`) is a different path and was not part of this question.
7. **"Each tag table exists once" (033 quotes the comment; 032 suspects a second table).** Code has two tables for `BuildDeclarationKind` with different numbers (item 3 of the duplicates list). The comment at `PC/reuse/review_record_encoding.rs:4-6` does not hold across the crate.

## What I checked myself

Each line: the claim, where I looked, verdict.

1. Recovery has no arm for intrinsic tags 8, 11, 16, 17 that the encoder emits (036). `RE/encoding/recovery/policy/intrinsic.rs:15-50` has arms 0-7, 9, 10, 12-15; `RE/encoding/encode/values/providers/intrinsics.rs:14,17,69,101` emits 16, 17, 11, 8. **Matches.** Recovery rejects such a record; it does not accept a wrong one.
2. Recovery primitive types stop at `Addr` 11 while the encoder has `Selector` 12 (036). `recovery/policy/structural_expressions/tags.rs:12-46`, `encode/values/crashes/vocabulary.rs:26`. **Matches.**
3. Baseline text recovery has no caller (036). Grep of `recover_text_with_usage|PackagePolicyTextRecoveryLimits` over `omega-rust/**/*.rs`: the limits type appears only in `text.rs`; the other `recover_text_with_usage` hits are methods on package-manager types. **Matches.**
4. Planner workspace API has no caller outside source-acquisition (037). Grep count: 29 occurrences, 12 files, all under `packages/source-acquisition`. **Matches.**
5. Dangerous-service classification returns `Ok(None)` for anything not in a fixed table (034, open). `RE/capture/authority.rs:25-40` (14 rows), `:84-93` (two roles, no `TimeHostService`), `:111`. **Matches.**
6. Build machine with no reach-plan row is admitted with empty reach (032, open). `PC/build_execution/build_admission.rs:78-81,137-138`. **Matches.**
7. `from_parts` skips handoff completeness on an empty map (033, open). `PC/package_compilation.rs:370-374`. **Partly**: the skip matches; the record's caller list is incomplete (Disagreements 4).
8. Lineage check is `debug_assert!` only (034, open). `PM/resolution/graph/mod.rs:60-66`. **Matches.**
9. Executable-outside-roots uses `canonicalize().ok()` (034, open). `RX/resolver_execution/path_custody.rs:77-82`. **Matches.**
10. Three `is_snake_case` definitions (033). Grep finds exactly three; bodies opened. **Matches**, same rule.
11. Windows reserved device list duplicated (037 suspected). `BO/portable_paths/mod.rs:107-118`, `SA/git/objects/tree.rs:271-282`. **Matches**, same expression.
12. Two tag tables for `BuildDeclarationKind` (032 suspected). `PC/build_execution/observation_identity.rs:244-251`, `PC/reuse/review_record_encoding.rs:48-54`. **Matches**; numbers differ.
13. Entry-file hand copy reads the physical root (034). `PM/review/candidate/passes/occurrences.rs:476-481`, `PM/resolution/source/mod.rs:59-65`. **Matches.**
14. `reference_state` tolerates a missing calling application (032, traced). `TR/reference_state.rs:158-167`. **Matches.**
15. Filesystem service permission by six identity strings; package owner by `package:` prefix (032). `TR/filesystem.rs:437-456`, `TR/native_provider/service_reach.rs:9-14`. **Matches.**
16. Foreign traits are not checked against a declaration (036, traced). `RE/record/public_policy/validation/declarations.rs:199-205`. **Matches.**
17. `(ToolchainSource(_), None) => true` (036, open). `RE/record/package/provider_policy/binding_validation.rs:180`. **Matches.**
18. Unsupported compiler-intrinsic row stored with no execution identity (035, open). `RE/capture/providers/intrinsics.rs:264-265`, `providers/policy/rows.rs:111-113`. **Matches.**
19. Missing type identity text becomes `""` (035, open). `RE/capture/semantics/types/identity.rs:55-61`. **Matches.**
20. Membership check skips plain names and toolchain-source owners (036). `RE/encoding/encode/membership/framing.rs:73-75,143`. **Matches.**
21. Float helper file loaded by substring search (033). `PC/source/source_assembly.rs:1038-1044`. **Matches.**
22. `Unavailable => continue` (033, open). `PC/source/source_imports/entry_contract_seed.rs:77`. **Matches**; `:72-74` is a second silent `continue` the record does not mention.
23. Seed kept for any declaration with a dependency alias (033, open). `PC/package_compilation/provider_selections.rs:39-41`. **Matches.**
24. Library digest covers `.omg` only while `.omega` is importable (033, open). `PC/source/toolchain_library.rs:123-126`, `source_imports/paths.rs:146-154`. **Matches.**
25. Table sizes in 032 (26 Darwin rows; 28 Windows contracts; 24 Windows symbols). Counted literals in `TR/filesystem.rs:201-358` and `TR/filesystem/windows/contracts.rs:32-446`. **Does not match**: 27, 27 and 22.
26. Encoder discard sites and `acquire_local`, hidden dependency calls: see Disagreements 1, 2, 6.
27. Smaller greps re-run: purpose tag 3 hits, stratum tag 3 files, `fn hash_field` 4 definitions, `PC/cursor.rs` one user. **Match.**

## Not covered

- **Records.** All six first-pass records were read in full (`unit_032.md` 528 lines, `unit_033.md` 178, `unit_034.md` 511, `unit_035.md` 388, `unit_036.md` 347, `unit_037.md` 423). No `unit_03[2-7]_gap_*.md` file exists, and no file under `gap_units/` names a path in this area.
- **Source ranges.** Seven package crates are covered line for line: the unit tables sum to the crates.json totals (2,118; 2,189; 17,433; 26,065; 464; 47,300; 21,951). terminal-runtime has 2,548 lines in 9 files that belong to unit 031, outside this area: `terminal-runtime/src/execution.rs`, `execution/components.rs`, `execution/entry.rs`, `execution/host_imports.rs`, `host_imports/absolute_value.rs`, `host_imports/affine_transform.rs`, `host_imports/allocation.rs`, `allocation/admission.rs`, `allocation/backing.rs`. This digest says nothing about them except what 032 looked up (`host_imports.rs:185-247,490-585`).
- **Not read by any record in this area, and relied on:** `optimization_core` (cursor, digest encoder), `bounded_process`, `platform_custody::record_file`, `filesystem_authority`, `language_semantics::type_identity` (the membership visitor), `validation::CheckingProjection`, and the callers in `compilation/products` and `build-system`.
- **Counts.** Spelling-keyed and silent-drop counts are my tallies of sites the records cite; grouping adjacent lines into one site is my judgment. I did not re-count against the code.
