# Package source selection

Source: `wiki/spec/packages/sources.md` lines 1–158 (read).

## Rules

rule id | the exact sentence, quoted | the heading it is under | what must be checked, in plain words | when it must hold | lane
--- | --- | --- | --- | --- | ---
SOURCES1 | "The application coordinator forwards each exact `depend*` or `build_depend*` request when the package's one Build activation emits it." | Package source selection | Each `depend*` or `build_depend*` request emitted by the package's one Build activation must be forwarded by the application coordinator | when the package's one Build activation emits the request | 9
SOURCES2 | "no requested code executes before its source, review, checking, and admission complete." | Package source selection | Requested code must not run until source acquisition, review, checking, and admission have all completed | before any requested code executes | 9
SOURCES3 | "The graph becomes closed only after every activation returns and every outstanding request joins;" | Package source selection | The dependency graph must be treated as open until every Build activation has returned and every outstanding request has joined | before graph closure and product source compilation | 9
SOURCES4 | "product source compilation consumes that immutable result." | Package source selection | Product source compilation must use the immutable acquisition result established at graph closure | at product source compilation | 9
SOURCES5 | "No import scan, call-name parser, or dependency output manufactures an edge." | Package source selection | Dependency edges must not be created from import scanning, call-name parsing, or dependency-output side channels | when constructing the dependency graph | 9
SOURCES6 | "Both request forms are target-independent because Build cannot observe a realization target." | Package source selection | `depend*` and `build_depend*` requests must not depend on or vary with realization target | when registering or resolving those requests | 9
SOURCES7 | "Checked scheduling, review, generated outputs and realization retain purpose, execution profile and requested product target under [scoped build execution](../build/scoped_execution.md#two-checked-contexts)." | Package source selection | Checked scheduling, review, generated outputs, and realization must retain purpose, execution profile, and requested product target per scoped build execution | under scoped build execution | 9
SOURCES8 | "A dependency declares its own canonical snake_case package name." | Requester-local graph | Each dependency must declare a canonical snake_case package name | at dependency registration | 9
SOURCES9 | "Default aliases preserve that spelling;" | Requester-local graph | Default aliases must match the declared canonical name spelling exactly | at dependency registration | 9
SOURCES10 | "`depend_as` and `build_depend_as` supply validated aliases in their respective product and build scopes." | Requester-local graph | Product and build alias renames must use validated `depend_as` / `build_depend_as` in the correct scope | at dependency registration | 9
SOURCES11 | "Aliases are unique within a scope, not across both; there is no cross-scope fallback." | Requester-local graph | Reject duplicate aliases within a scope; reject cross-scope alias fallback between product and build scopes | at dependency registration and graph validation | 9
SOURCES12 | "an ancestor cannot rename dependencies inside another package." | Requester-local graph | Reject an ancestor renaming dependencies declared inside another package | at dependency registration | 9
SOURCES13 | "The resolver coalesces requests for the same key and exact resolution as they arrive." | Requester-local graph | Requests for the same package key and exact resolution must coalesce into one resolution as they arrive | as dependency requests arrive | 9
SOURCES14 | "Before installing a requested Build-scoped provider slot, and again at final join, the coordinator rejects conflicting resolutions and cycles." | Requester-local graph | Reject conflicting resolutions and cycles before Build-scoped provider-slot installation and again at final join | before Build-scoped provider slot install; at final join | 9
SOURCES15 | "Repeated edges must point to the same node." | Requester-local graph | Repeated dependency edges for the same relation must resolve to the same graph node | at graph validation | 9
SOURCES16 | "A diamond through two libraries to the same std therefore has one std owner, not two copied declarations." | Requester-local graph | Shared transitive dependencies (e.g. std) must have one owner node, not duplicated declarations | at final graph join | 9
SOURCES17 | "Those instances do not rename the package's declarations." | Requester-local graph | Differing checked build instances by host/product context or target must not rename the package's declarations | when build instances differ by context or target | 9
SOURCES18 | "Product compilation receives only the final closed, validated graph." | Requester-local graph | Product compilation must use only the final closed, validated requester-local graph | at product compilation | 9
SOURCES19 | "The standalone compiler handoff serializes this same canonical source-lineage structure and uses the same package-key identity algorithm." | Requester-local graph | Standalone compiler handoff must serialize the same canonical source-lineage structure and use the same package-key identity algorithm | at standalone compiler handoff | 9
SOURCES20 | "Local aliases and exact source resolutions remain separate; the request does not introduce a second origin identity." | Requester-local graph | Local aliases and exact source resolutions must stay separate; a request must not invent a second origin identity | at resolution and handoff | 9
SOURCES21 | "working dependency-first over exact source inputs without compiling product outputs or consulting a historical build catalog." | Requester-local graph | After graph closure, anonymous root establishment must proceed dependency-first over exact source inputs only, without compiling product outputs or using a historical build catalog | after final graph closure | 9
SOURCES22 | "An invocation-private Build activation identity never substitutes for that owner." | Requester-local graph | Reject using invocation-private Build activation identity in place of the content-derived owner | after final graph closure | 9
SOURCES23 | "Compilation carries final owners forward; it does not rediscover them from paths, import spelling or missing package fields." | Requester-local graph | Compilation must preserve final owners from coordination; must not re-derive owners from paths, import spelling, or missing package fields | during product compilation | 9
SOURCES24 | "Import discovery does not combine package-authored dependency requests." | Requester-local graph | Import discovery must not merge or synthesize package-authored dependency requests | during import discovery | 9
SOURCES25 | "Paths route source loading; they are not nominal identity." | Requester-local graph | Paths may route loading only; must not be used as nominal package or owner identity | during source loading and identity assignment | 9
SOURCES26 | "Orchestration re-roots each package at exactly its transitive subgraph in the applicable context and compiles product prerequisites first." | Requester-local graph | Each package must be re-rooted to its exact transitive subgraph in the applicable context; product prerequisites must compile before dependents | during orchestration | 9
SOURCES27 | "that does not merge checked host and product instances or make a helper's own build dependencies its library imports." | Requester-local graph | Shared source custody must not merge checked host and product instances or treat a helper's build dependencies as its library imports | when acquisition shares custody | 9
SOURCES28 | "package aliases require the validated package-aware entrance." | Requester-local graph | Package alias resolution must go through the validated package-aware entrance, not entrances that only support root-relative or toolchain imports | when resolving package aliases | 9
SOURCES29 | "This is an implementation limitation, not permission to drop owners or a separate language rule for roots without build files." | Requester-local graph | Focused-file compatibility entrance must not drop owners or define a separate language rule for roots without build files | when using focused-file compatibility entrance | 9
SOURCES30 | "Package selection separately normalizes to `Root` or `Named(PackageName)` and does not enter `SourceIdentity`." | Git acquisition and member selection | Package selection must normalize to `Root` or `Named(PackageName)` only; member selection must not enter `SourceIdentity` | at package/member selection | 9
SOURCES31 | "Named selection uses only members emitted by the verified root's admitted Build activation and requires exactly one matching package declaration." | Git acquisition and member selection | Named member selection must use only members from the verified root's admitted Build activation; reject unless exactly one matching package declaration exists | at named member selection | 9
SOURCES32 | "The selected member path remains navigation/replay custody and the base for relative dependencies, not package-key identity." | Git acquisition and member selection | Selected member path must not be used as package-key identity; it is navigation/replay and relative-dependency base only | at member selection and identity assignment | 9
SOURCES33 | "Retain declaration bytes outside that subtree for replay." | Git acquisition and member selection | Declaration bytes outside the published selected subtree must be retained for replay | after publishing the selected subtree | 9
SOURCES34 | "All selections in one closure share the same pinned commit/root tree, including symbolic revisions; do not observe a moving branch separately for each member." | Git acquisition and member selection | All member selections in one closure must share one pinned commit/root tree; must not resolve symbolic revisions to different moving branches per member | within one closure | 9
SOURCES35 | "Host routing does not change authored lineage." | Git acquisition and member selection | Host-routed Git/SSH transport must not alter authored canonical lineage | during acquisition | 9
SOURCES36 | "generic Git lineage retains transport, user, host, port, path-case, and suffix distinctions until equivalence is established." | Git acquisition and member selection | Generic Git lineage must keep transport, user, host, port, path-case, and suffix distinct until an adapter establishes equivalence | during lineage normalization | 9
SOURCES37 | "Locks and review consume canonical lineage, exact selected commit/tree/content, member projection, and immutable snapshot custody, not a fetching-process receipt." | Git acquisition and member selection | Locks and review must be based on canonical lineage, exact commit/tree/content, member projection, and immutable snapshot—not fetch receipts | at lock and review | 9
SOURCES38 | "Host credentials never become package authority." | Git acquisition and member selection | Host credentials must not confer package acceptance or source authority | during acquisition and review | 9
SOURCES39 | "Every `.git` member is omitted at any depth; version-control internals are never package source." | Local source capture | Mutable-root capture must omit every `.git` member at any depth from package source | during local source capture | 9
SOURCES40 | "At the root only, the build output directory and the project control files named in [locks](locks.md#source-and-control-state) are omitted as well." | Local source capture | At repository root only, omit build output directory and lock-named project control files from captured source | during local source capture | 9
SOURCES41 | "Nested directories of those names, and nested control-named files, remain ordinary source." | Local source capture | Nested directories or files with those control names (not at root policy) must be included as ordinary source | during local source capture | 9
SOURCES42 | "an excluded member appearing or vanishing during capture is drift under the quiescent-tree premise and rejects." | Local source capture | Reject capture when an omitted member appears or disappears during capture (quiescent-tree drift) | during local source capture | 9
SOURCES43 | "Symlink targets must resolve inside the root;" | Local source capture | Symlink targets must resolve to paths inside the capture root | during local source capture | 9
SOURCES44 | "under the package policy a target carrying any `.git` component, entering a root control file, or entering the root build output rejects rather than smuggling omitted state back into the source." | Local source capture | Reject symlinks whose targets include `.git`, root control files, or root build output under package policy | during local source capture | 9
SOURCES45 | "Resolver-owned materializations — published Git trees and stored local snapshots — capture exactly what was published, retaining every row including locks and build output that a mutable root would omit." | Local source capture | Published Git trees and stored local snapshots must retain full published rows, including locks and build output omitted from mutable-root policy | at resolver materialization | 9
SOURCES46 | "declaration projection, source existence, contained import links and file reads all use the captured namespace." | Local source capture | Declaration projection, source existence checks, import links, and file reads must use the captured namespace, not raw host paths alone | during ordinary local compilation | 9
SOURCES47 | "Build source metadata and scoped inventories derive from those same bytes; Build never follows captured links." | Local source capture | Build metadata and scoped inventories must derive from captured bytes; Build must not follow symlink/captured links | during Build activation on captured source | 9
SOURCES48 | "Resolution does not perform semantic-version solving." | Reconciliation and updates | Resolution must not apply semantic-version range solving | during dependency resolution | 9
SOURCES49 | "Requests for one key that select the same immutable instance deduplicate despite different authored selectors." | Reconciliation and updates | Same package key and same immutable instance must deduplicate even when authored selectors differ | during reconciliation | 9
SOURCES50 | "Different resolutions reject with all conflicting dependency paths;" | Reconciliation and updates | Reject when the same key resolves to different immutable instances; report all conflicting dependency paths | during reconciliation | 9
SOURCES51 | "there is no guessed compatibility relation." | Reconciliation and updates | Must not infer or guess compatibility between conflicting resolutions | during reconciliation | 9
SOURCES52 | "Multiple simultaneous instances per key are unsupported:" | Reconciliation and updates | Reject multiple simultaneous immutable instances for the same package key within one coordinated closure | during reconciliation | 9
SOURCES53 | "Selective updates preserve pins for unchanged Git locator/revision requests." | Reconciliation and updates | Unchanged Git locator/revision requests must keep existing pins during selective update | during selective update | 9
SOURCES54 | "Changing an alias or member selection does not refresh the repository." | Reconciliation and updates | Alias or member selection changes alone must not trigger repository refresh | during selective update | 9
SOURCES55 | "New or changed requests resolve normally and reconcile with the complete graph." | Reconciliation and updates | New or changed dependency requests must resolve and reconcile against the full graph | during update resolution | 9
SOURCES56 | "Selecting a package for update refreshes its repository lineage, not unrelated transitive repositories." | Reconciliation and updates | Package update must refresh only that package's repository lineage, not unrelated transitive repos | when selecting a package for update | 9
SOURCES57 | "Reachable workspace members and relative Path edges move together at one revision;" | Reconciliation and updates | Reachable workspace members and relative Path dependencies must advance together at one revision when updated | when moving workspace/path edges at a revision | 9
SOURCES58 | "this does not add unused members." | Reconciliation and updates | Joint revision moves must not add workspace members that were not already reachable | when workspace members and Path edges move together | 9
SOURCES59 | "Missing preserved content either fails offline or is acquired at the exact recorded commit, never replaced by a newer selector result." | Reconciliation and updates | Missing preserved content must fail under offline policy or be fetched at the exact recorded commit; must not substitute a newer selector resolution | during resume or offline reconciliation | 9
SOURCES60 | "Whole-invocation offline policy overrides that fetch permission and rejects every new/refreshed Git request, including transitive discoveries." | Reconciliation and updates | Whole-invocation offline mode must reject every new or refreshed Git request, including transitive discoveries | under whole-invocation offline policy | 9
SOURCES61 | "local-only graphs need no lock." | Reconciliation and updates | Graphs using only local roots must be allowed without a lock | for local-only graphs | 9
SOURCES62 | "Resume uses exact proposal pins." | Reconciliation and updates | Resume must apply exact proposal pins without reinterpretation | on resume | 9
SOURCES63 | "Missing old source restricts source diagnostics, not comparison with the accepted baseline." | Reconciliation and updates | Missing old source must limit source diagnostics only; must not block comparison against the accepted baseline | when old source is missing | 9
SOURCES64 | "Offline policy changes acquisition, not compilation, scoped build outputs, project decisions, or publication recovery; it is not runtime containment." | Reconciliation and updates | Offline policy may block acquisition only; must not alter compilation semantics, scoped build outputs, project decisions, publication recovery, or runtime containment | under offline policy | 9

## Ambiguous sentences

Sentence | Reading A | Reading B
--- | --- | ---
"The resolver may begin immutable acquisition immediately, but no requested code executes before its source, review, checking, and admission complete." | Acquisition I/O may start before admission while execution remains blocked until all four stages finish. | “Immediately” might still require some ordering relative to Build emission; whether partial acquisition counts as executing “requested code” is unstated. |
"A package's further requests remain unknown until its own Build activation runs." | Downstream dependency requests from a package cannot be scheduled until that package's Build activation has run at least once. | Further requests might be known from lock pins before Build runs; the sentence speaks only to what the coordinator may treat as known for graph construction. |
"Checked build instances may still differ by host/product context or target." | Host and product checked instances may diverge in context/target while sharing declarations. | Whether “differ” includes only scheduling context or also distinct compiled artifacts under the same package key is not fully specified here. |
"This is an implementation limitation, not permission to drop owners or a separate language rule for roots without build files." | Focused-file entrance must eventually match package-aware owner semantics; until then owners still apply conceptually. | Whether compilers must reject missing owners on focused-file paths today, or only must not treat the limitation as policy, is unstated. |
"Converge it on the same default-build graph preparation and compiler handoff." | Implementation requirement on tooling convergence, not a separate reject rule for programs. | “Converge” might imply interim acceptable divergence between entrances; the obligation timeline is not stated. |
"Omission narrows captured rows, not observed membership:" | Capture identity uses narrowed rows while membership observation still sees the full tree for drift checks. | Drift might be defined only on rows that would have been captured if present at start; whether transient host-only files outside rows trigger reject is unclear. |
"Multiple simultaneous instances per key are unsupported:" | Within one coordinated closure, two immutable instances for the same key always reject. | Component closures (next sentences) might permit duplicated immutable dependencies across separate closures; whether “per key” is global or per-closure root is implicit. |
"a program is one coordinated closure whose every dependency revision its owner has reviewed and accepted, so two libraries that privately pin conflicting revisions of a third are reconciled by that owner at update." | Conflicting private pins are rejected at closure until the owning root reconciles at update time. | Conflict might be deferred until update is attempted; whether compile is rejected immediately on detection is not stated in this sentence alone. |
"Missing preserved content either fails offline or is acquired at the exact recorded commit, never replaced by a newer selector result." | When online, missing content must be fetched at the recorded commit even if a newer revision would satisfy selectors. | Offline failure might apply only when policy forbids fetch; whether partial offline allows recorded-commit fetch is delegated to policy wording elsewhere. |
"Offline policy changes acquisition, not compilation, scoped build outputs, project decisions, or publication recovery;" | Offline blocks fetches only; checked compilation paths and outputs stay unchanged. | Compilation might still fail for lack of source while “not changing compilation” means not altering language rules; diagnostic vs semantic distinction is implicit. |

## Terms used without definition in this chapter

- package keys / opaque package keys
- interpreted dependency requests
- acceptance / admission (as distinct from source resolution)
- immutable source custody
- application coordinator
- `depend*` / `build_depend*` request forms
- Build activation (one per package)
- review / checking / admission (pipeline stages before execution)
- closed graph / final join
- import scan / call-name parser / dependency output
- realization target / requested product target
- purpose / execution profile
- scoped build execution / two checked contexts
- canonical snake_case package name
- `depend_as` / `build_depend_as`
- product scope vs build scope
- requester-local graph / purpose-qualified aliases
- canonical source roots
- Build-scoped provider slot
- checked build instances / host/product context
- standalone compiler handoff / sealed request protocol
- canonical source-lineage structure / package-key identity algorithm
- second origin identity
- shared package identity contract / pre-release format policy
- anonymous root identities
- historical build catalog
- invocation-private Build activation identity
- nominal identity vs routing paths
- transitive subgraph / applicable context
- exact source custody
- checked host and product instances
- Rust focused-file compatibility entrance / package-aware entrance
- default-build graph preparation
- `SourceIdentity`
- `Root` / `Named(PackageName)` member selection
- verified fetch/tree / verified-tree session
- Omega-aware planner / bounded member paths
- symbolic revisions / moving branch
- resolver security contract / locator validation / host-routed Git/SSH
- namespace equivalence adapters
- canonical lineage
- member projection / immutable snapshot custody / fetching-process receipt
- package policy / non-source reservations
- quiescent-tree premise
- root control file / build output directory (lock-named)
- Resolver-owned materializations
- captured namespace / contained import links
- Build source metadata / scoped inventories
- transaction and foreign-review snapshots
- semantic-version solving
- authored selectors / immutable instance
- instance identity (types, conformances, providers, evidence)
- coordinated closure
- components / component contract / duplicable immutable dependencies
- selective updates / proposal pins
- repository lineage
- reachable workspace members / relative Path edges
- whole-invocation offline policy
- accepted baseline
- publication recovery / runtime containment
- resolution owner

CHAPTER COMPLETE
