# Build declarations and package identity

Source: `wiki/spec/build/declarations.md` lines 1–276 (read).

## Rules

rule id | the exact sentence, quoted | the heading it is under | what must be checked, in plain words | when it must hold | lane
--- | --- | --- | --- | --- | ---
DECLARATIONS1 | "Host effects need admitted build authority;" | Build declarations and package identity | Host effects during build must have admitted build authority | not stated | 9
DECLARATIONS2 | "there is no second configuration grammar or ambient runtime service." | Build declarations and package identity | Must not use a second configuration grammar or ambient runtime service for build | not stated | 9
DECLARATIONS3 | "Checked declarations and transitive build contracts expose restricted host requests for [project acceptance](../packages/acceptance.md#restricted-build-acceptance) in `omega.lock`; they cannot approve those requests themselves." | Build declarations and package identity | Build declarations must not approve restricted host requests; only expose them in lock | not stated | 9
DECLARATIONS4 | "Build selections also cannot choose an external ecosystem's receiving policy." | Build declarations and package identity | Build selections must not select an external ecosystem receiving policy | not stated | 9
DECLARATIONS5 | "Producing a binary requires no such policy declaration." | Build declarations and package identity | Binary production must not require that policy declaration | not stated | 9
DECLARATIONS6 | "Project description is one logical interpreted Omega activation, not a privileged syntax subset or replayed discovery program." | Build declarations and package identity | Project description must be one logical interpreted activation, not privileged subset or replayed discovery | not stated | 9
DECLARATIONS7 | "Psi checks and admits the initial prefix against core, local Build declarations and available exact dependencies, then resumes subsequent checked continuations as dependencies are explicitly prepared." | Build declarations and package identity | Initial prefix admitted only against core, local Build decls and available exact deps; continuations resume after explicit prep | not stated | 9
DECLARATIONS8 | "No source-text call scanner decides the meaning of arbitrary same-spelled user code." | Build declarations and package identity | Meaning of same-spelled user code must not be decided by a source-text call scanner | not stated | 9
DECLARATIONS9 | "`Build::dependency` and `Build::build_dependency` atomically register configured product and interpreted-build dependencies respectively." | Build declarations and package identity | Dependency registrations must be atomic per kind | at a call | 9
DECLARATIONS10 | "A description's source, alias, purpose and delegated context are fixed before it may be scheduled." | Build declarations and package identity | Source, alias, purpose, delegated context fixed before scheduling | not stated | 9
DECLARATIONS11 | "an alias cannot be renamed, reused ambiguously or retargeted afterward." | Build declarations and package identity | Reject alias rename, ambiguous reuse, or retarget after registration | not stated | 9
DECLARATIONS12 | "Product dependencies supply no callable Build authority." | Build declarations and package identity | Product dependencies must not supply callable Build authority | not stated | 9
DECLARATIONS13 | "`use dependency;` is inert logical name selection and can remain deferred until a dependent checked stage; it never acquires source or executes an imported package." | Build declarations and package identity | `use dependency` must not acquire source or execute imported package | at a call | 9
DECLARATIONS14 | "The dependent continuation resolves names, types and contracts and is admitted before executing them." | Build declarations and package identity | Dependent continuation admitted only after names, types, contracts resolve | before execution | 9
DECLARATIONS15 | "Exact provider-slot installation still checks the selected requirement and implementation under [Build provider readiness](provider_selection.md#build-provider-readiness)." | Build declarations and package identity | Provider-slot install checks selected requirement and implementation per provider readiness | not stated | 9
DECLARATIONS16 | "Neither preparation nor an ordinary import chooses a default implementation, grants host access or fabricates declaration identity." | Build declarations and package identity | Preparation and ordinary import must not pick default impl, grant host access, or fabricate declaration identity | not stated | 9
DECLARATIONS17 | "When `build` returns, package coordination joins every outstanding dependency request, rejects failed, cyclic, unresolved, or unadmitted dependency edges, and freezes the normalized Build intent." | Build declarations and package identity | On build return reject failed/cyclic/unresolved/unadmitted edges and freeze normalized intent | at normal return | 9
DECLARATIONS18 | "product discovery and checking must resolve and validate them before a completed product or executable plan is admitted." | Build declarations and package identity | Resolve and validate inert product references before admitting product or executable plan | not stated | 9
DECLARATIONS19 | "Product bodies never execute during Build; product compilation starts only after the join." | Build declarations and package identity | Product bodies must not execute during Build; compilation after join | not stated | 9
DECLARATIONS20 | "The exact requirement is an inert designated product-selection reference until product discovery and checking resolve its complete owned declaration/application." | Native binding intent | Native bind requirement stays inert product reference until discovery/checking resolve decl/application | at a call | 9
DECLARATIONS21 | "The target is an ordinary exact toolchain profile reference." | Native binding intent | Target must be exact toolchain profile reference | at a call | 9
DECLARATIONS22 | "fixed-byte literal lengths are inferred without explicit `ForeignBinding<...>` counts or mandatory helper machines." | Native binding intent | Infer fixed-byte literal lengths without mandatory ForeignBinding counts or helper machines | at a call | 9
DECLARATIONS23 | "The operation borrows this activation's original Build, records target-keyed intent, and never invokes the product operation or installs its backing into the running Build." | Native binding intent | Bind borrows activation Build, records intent; must not invoke product op or install backing into running Build | at a call | 9
DECLARATIONS24 | "It grants no ordinary import of unresolved product code and does not replace explicit Build-host readiness." | Native binding intent | Bind must not grant import of unresolved product or replace Build-host readiness | at a call | 9
DECLARATIONS25 | "ambiguous same-precedence rows reject." | Native binding intent | Reject ambiguous same-precedence binding rows | not stated | 9
DECLARATIONS104 | "Exact contract, lifetime, memory-access, calling-policy, native applicability, foreign admission, provenance, and runtime-authority obligations remain." | Native binding intent | Native bind retains listed contract and authority obligations | not stated | 9
DECLARATIONS26 | "There is no source `via` clause or per-symbol forwarding realization, and no author API for manufacturing trusted plan rows." | Native binding intent | Must not use source via clause, per-symbol forwarding, or author API to manufacture trusted plan rows | not stated | 9
DECLARATIONS27 | "The entry-binding overload of `Build::bind` is a separate operation." | Native binding intent | Entry-binding overload classified as separate operation from direct bind | at a call | 9
DECLARATIONS28 | "A selected `build.omg` has exactly one free build entry with the explicit mutable Build parameter and any declared execution-backing parameters selected and admitted by its invocation." | Project role | Selected build.omg has exactly one free build entry with admitted Build and backing parameters | at entry | 9
DECLARATIONS29 | "These are explicit source arguments, not a synthesized receiver or ambient provider." | Project role | Build and filesystem parameters are explicit arguments, not synthesized receiver or ambient provider | at entry | 9
DECLARATIONS30 | "The one interpreted activation must produce exactly one package/application declaration or the workspace member set." | Project role | One activation yields exactly one package/app declaration or workspace member set | at normal return | 9
DECLARATIONS31 | "Mixed, missing, duplicate, and noncanonical results reject." | Project role | Reject mixed, missing, duplicate, noncanonical role results | at normal return | 9
DECLARATIONS32 | "`Owner::build` is not a project root and cannot acquire a synthesized receiver." | Project role | `Owner::build` must not be project root or acquire synthesized receiver | not stated | 9
DECLARATIONS33 | "With no selected `build.omg`, the compiler supplies ordinary default Build source and runs it through the same one-entry checking, interpretation, join, and freeze route." | Project role | Default build uses same one-entry check, interpret, join, freeze route | not stated | 9
DECLARATIONS34 | "A standalone loose root defaults to application role; an already resolved package retains its supplied role and owner." | Project role | Loose root defaults to application; resolved package keeps supplied role and owner | not stated | 9
DECLARATIONS35 | "Checking source does not require an executable entry." | Project role | Source checking must not require executable entry | not stated | 9
DECLARATIONS103 | "Executable output still needs ordinary target-slot selections and provider checks;" | Project role | Executable output requires target-slot selections and provider checks | not stated | 9
DECLARATIONS36 | "the default does not discover a machine named `main` or bypass [entry admission](entry_roots.md)." | Project role | Default build must not discover `main` or bypass entry admission | not stated | 9
DECLARATIONS37 | "`package`, `application`, `member`, `dependency` and `build_dependency` are compiler-owned operations on the activation's Build value." | Project role | Listed operations are compiler-owned on activation Build | at a call | 9
DECLARATIONS38 | "Product and build dependency sets remain separate; aliases are unique within each scope, with no cross-scope fallback." | Project role | Keep product and build dep sets separate; aliases unique per scope; no cross-scope fallback | not stated | 9
DECLARATIONS39 | "The interpreted result, not source spelling, determines the requested graph." | Project role | Dependency graph from interpreted result, not source spelling | not stated | 9
DECLARATIONS40 | "Default configuration does not discover undeclared dependency edges from imports." | Project role | Must not discover undeclared dependency edges from imports in default config | not stated | 9
DECLARATIONS41 | "Product registrations authorize product imports; Build registrations authorize checked interpreted dependency preparation." | Project role | Product reg authorizes product imports; Build reg authorizes interpreted dep preparation only | not stated | 9
DECLARATIONS42 | "Unprepared external uses cannot execute, and a later registration cannot justify earlier code." | Project role | Reject executing unprepared external uses; later registration cannot justify earlier code | at a call | 9
DECLARATIONS43 | "Using a package in both contexts requires both edges, including std." | Project role | Host and product use of same package requires both dependency edges including std | not stated | 9
DECLARATIONS44 | "Legacy locks cannot imply a missing edge." | Project role | Legacy locks must not imply undeclared dependency edges | not stated | 9
DECLARATIONS45 | "Applications default to executable output." | Project role | Application role defaults to executable output | not stated | 9
DECLARATIONS46 | "The Build activation may request `artifact_only` once for an application, never a package/workspace." | Project role | `artifact_only` at most once for application only, never package/workspace | at a call | 9
DECLARATIONS47 | "It requires at least one completed required artifact and rejects executable root/provider selections." | Project role | artifact_only requires completed required artifact; reject executable root/provider selections | not stated | 9
DECLARATIONS48 | "It is not a new role or target profile." | Project role | `artifact_only` must not be treated as new role or target profile | not stated | 9
DECLARATIONS49 | "Names begin with an ASCII lowercase letter, followed by lowercase letters, digits, and single underscore-separated segments." | Names, lineage, and resolution | Package/dependency names match stated ASCII pattern | not stated | 9
DECLARATIONS50 | "Leading, trailing and repeated underscores reject." | Names, lineage, and resolution | Reject leading, trailing, repeated underscores in names | not stated | 9
DECLARATIONS51 | "Default import aliases preserve the declared name exactly;" | Names, lineage, and resolution | Default import alias equals declared name exactly | not stated | 9
DECLARATIONS52 | "the dependency description provides explicit renaming before atomic registration." | Names, lineage, and resolution | Renaming via dependency description before atomic registration only | not stated | 9
DECLARATIONS53 | "For Git, lineage is the canonical repository namespace, excluding requested revision, resolved commit, tree, and content." | Names, lineage, and resolution | Git lineage uses canonical repo namespace excluding revision/commit/tree/content | not stated | 9
DECLARATIONS54 | "Same-named packages from different lineages remain distinct even with equal content or policy." | Names, lineage, and resolution | Distinct lineage keeps packages distinct regardless of equal content | not stated | 9
DECLARATIONS55 | "a short spelling cannot replace the selected owner." | Names, lineage, and resolution | Short spelling must not replace selected owner in provenance | not stated | 9
DECLARATIONS56 | "every dependency edge must resolve to a package." | Names, lineage, and resolution | Each dependency edge resolves to a package | not stated | 9
DECLARATIONS57 | "Workspace role is not a compilation root role." | Names, lineage, and resolution | Workspace role must not be used as compilation root role | not stated | 9
DECLARATIONS58 | "Do not infer application kind from ProgramEntry." | Names, lineage, and resolution | Application kind must not be inferred from ProgramEntry | not stated | 9
DECLARATIONS59 | "Root role survives closure evidence, lock, review, compiler handoff, diagnostics, and audit." | Names, lineage, and resolution | Root role retained through listed artifacts and stages | not stated | 9
DECLARATIONS60 | "Package-to-application loses dependency compatibility; application-to-package loses executable-activation compatibility." | Names, lineage, and resolution | Role change loses stated compatibilities | not stated | 9
DECLARATIONS61 | "Report the directional broken contract, not a new package identity or a symmetric change." | Names, lineage, and resolution | Diagnostics report directional broken contract, not new identity or symmetric change | not stated | 9
DECLARATIONS62 | "A missing build file is not missing provenance. Preserve any exact owner already supplied by the resolved source graph or an imported artifact." | Default builds and anonymous owner identity | Missing build file still preserves exact owner from graph or artifact | not stated | 9
DECLARATIONS63 | "Neither shares an absent-owner key with the root, and repeated imports never mint fresh owners." | Default builds and anonymous owner identity | Core/std must not share absent-owner key with root; repeated imports must not mint new owners | not stated | 9
DECLARATIONS64 | "That identity scopes mutable state, continuations, diagnostics, and dependency requests but never becomes a declaration owner or portable artifact identity." | Default builds and anonymous owner identity | Invocation-private activation identity must not become declaration owner or portable artifact identity | not stated | 9
DECLARATIONS65 | "After Build returns and every dependency request joins, the root receives a content-derived owner before product source compilation." | Default builds and anonymous owner identity | Content-derived owner assigned after build return and join, before product compilation | at normal return | 9
DECLARATIONS105 | "The complete captured source snapshot, with root-relative paths and exact bytes under [source capture](../packages/sources.md#local-source-capture)." | Default builds and anonymous owner identity | Anonymous owner record must include complete captured source snapshot per source capture | not stated | 9
DECLARATIONS106 | "Its resolved dependency edges, including purpose, local aliases, exact owner identities and immutable resolutions; include the applicable core identity." | Default builds and anonymous owner identity | Owner record must include resolved dependency edges with listed fields and core identity | not stated | 9
DECLARATIONS107 | "The language/default-build semantics identity and captured semantic inputs that affect source meaning." | Default builds and anonymous owner identity | Owner record must include language/default-build semantics identity and semantic inputs affecting meaning | not stated | 9
DECLARATIONS66 | "Output paths, debug information and backend optimization selections are not such inputs." | Default builds and anonymous owner identity | Exclude output paths, debug info, backend optimization from anonymous owner digest inputs | not stated | 9
DECLARATIONS67 | "Use canonical ordering and unambiguous field framing, not discovery order or private object serialization." | Default builds and anonymous owner identity | Owner digest record uses canonical ordering and unambiguous framing | not stated | 9
DECLARATIONS68 | "Host locations used to capture or load those inputs, timestamps, random IDs, cache state, unrelated consumers and previous compilations are not added to the record." | Default builds and anonymous owner identity | Exclude listed host/cache/timestamp fields from owner record | not stated | 9
DECLARATIONS69 | "changing a dependency's lineage is a changed input, not merely relocating its retained snapshot." | Default builds and anonymous owner identity | Lineage change changes owner input; relocation alone does not | not stated | 9
DECLARATIONS70 | "do not hash a root's own future compiled output to establish its input owner." | Default builds and anonymous owner identity | Must not hash root's future compiled output for input owner | not stated | 9
DECLARATIONS71 | "Equal anonymous records have equal owner identity, including after relocation or cache deletion." | Default builds and anonymous owner identity | Equal canonical records yield equal owner identity | not stated | 9
DECLARATIONS72 | "Changing the snapshot, dependency resolution or semantic inputs changes that identity." | Default builds and anonymous owner identity | Snapshot/dep/semantic input changes change owner identity | not stated | 9
DECLARATIONS73 | "Distinct named lineages remain distinct even with equal content." | Default builds and anonymous owner identity | Named lineages stay distinct despite equal content | not stated | 9
DECLARATIONS74 | "Declarations retain their owner plus owner-local declaration identity through checking, specialization and publication." | Default builds and anonymous owner identity | Preserve owner and owner-local declaration identity through check/specialize/publish | not stated | 9
DECLARATIONS75 | "Artifact consumers preserve these identities rather than rebasing them onto the importing root." | Default builds and anonymous owner identity | Consumers keep identities; must not rebase onto importing root | not stated | 9
DECLARATIONS76 | "Importable products must carry the necessary identities and provenance; a cache may locate them but is never their authority." | Default builds and anonymous owner identity | Importable products carry identities/provenance; cache is not authority | not stated | 9
DECLARATIONS77 | "Existing products without that information cannot acquire portable nominal provenance from a guessed path or registry entry." | Default builds and anonymous owner identity | Must not guess portable provenance for products missing identity info | not stated | 9
DECLARATIONS78 | "Identity alone authenticates no producer and grants neither package acceptance nor execution authority." | Default builds and anonymous owner identity | Identity must not alone grant acceptance or execution authority | not stated | 9
DECLARATIONS79 | "Build-dependency helpers call ordinary prepared declarations or an exact static requirement after its admitted implementation is installed for that activation." | Evaluated build work | Build-dep helpers call prepared decls or static requirement only after admitted impl installed | at a call | 9
DECLARATIONS80 | "Authority follows ordinary operation arguments and the checked call graph, not provider-slot selection." | Evaluated build work | Authority from args and call graph, not provider-slot selection alone | at a call | 9
DECLARATIONS81 | "Transitive reach, invocation, blocking/suspension, termination, observations, and authority demands compose into the root's published ceiling." | Evaluated build work | Compose listed operational demands into root published ceiling | not stated | 3
DECLARATIONS82 | "`Build::assume_binding_contract<Required::operation, Supplied::operation>(builder)` is an evaluated root-authorized product selection under the [assumed-binding contract](assumed_binding_contracts.md), not a statically projected project/dependency declaration." | Evaluated build work | Classify assume_binding_contract as root-authorized product selection, not static project declaration | at a call | 9
DECLARATIONS83 | "Its exact product references do not execute either requirement or grant any assumption to the build-host context." | Evaluated build work | Product refs in assume_binding_contract must not execute requirements or grant assumptions to build host | at a call | 9
DECLARATIONS84 | "Ordinary applications should not use this selection." | Evaluated build work | Ordinary applications should not use assume_binding_contract selection | not stated | 9
DECLARATIONS85 | "Build helpers and libraries must not insert it as a workaround for a failed provider contract or checker diagnostic;" | Evaluated build work | Reject helpers/libraries inserting assume_binding_contract as workaround for failures | not stated | 9
DECLARATIONS86 | "The old BuildSource/BuildOutput filesystem facets are retired under `STAGED-BUILD-CAPABILITIES`, not the public dependency-helper interface." | Evaluated build work | BuildSource/BuildOutput facets must not be used as public dependency-helper interface | not stated | 9
DECLARATIONS87 | "Compiler publication and diagnostic records remain activation-scoped and do not become filesystem authority." | Evaluated build work | Publication/diagnostics activation-scoped; must not become filesystem authority | not stated | 9
DECLARATIONS88 | "Selecting a runtime provider does not route it into build execution." | Evaluated build work | Runtime provider selection must not route into build execution | not stated | 9
DECLARATIONS89 | "Additional host effects require explicit protocol operations and policy; package code gets no resolver credentials." | Evaluated build work | Extra host effects need explicit protocol/policy; package code must not get resolver credentials | not stated | 9
DECLARATIONS90 | "A workspace is a path-declared catalog of locatable members, not a combined dependency graph or a node with its own package key." | Workspaces | Workspace is member catalog only; not combined graph or package key node | not stated | 9
DECLARATIONS91 | "Each application/package remains an independently selected closure root; building one does not include unrelated members." | Workspaces | Each member is independent closure root; build excludes unrelated members | not stated | 9
DECLARATIONS92 | "A check that names no member takes every declared member as a [validation subject](validation_subjects.md);" | Workspaces | Check without named member validates all declared members | not stated | 9
DECLARATIONS93 | "members may only narrow them." | Workspaces | Members may only narrow shared pins and ceilings passed in | not stated | 9
DECLARATIONS94 | "Source code never searches parent directories for imports; only tooling discovers the enclosing build/workspace." | Workspaces | Source import resolution must not search parent directories | not stated | 9
DECLARATIONS95 | "Remote selection uses the member's declared name, not its path." | Workspaces | Remote selection by declared name, not path | not stated | 9
DECLARATIONS96 | "relocation does not rename the package." | Workspaces | Path relocation must not change package name | not stated | 9
DECLARATIONS97 | "Reject undeclared/escaping members, absent/duplicate names, and recursive search for undeclared build files." | Workspaces | Reject undeclared/escaping members, absent/duplicate names, recursive search for undeclared build files | not stated | 9
DECLARATIONS98 | "There is no independent package version field: the exact resolved source supplies instance identity." | Workspaces | Must not use independent version field; instance identity from resolved source | not stated | 9
DECLARATIONS99 | "The workspace owns its lock; dependency locks do not pin the consumer's graph." | Workspaces | Workspace lock separate; dependency locks must not pin consumer graph | not stated | 9
DECLARATIONS100 | "Membership does not merge acceptance across roots." | Workspaces | Workspace membership must not merge acceptance across roots | not stated | 9
DECLARATIONS101 | "A multi-root workspace lock must retain each root's source closure, target sections and acceptance separately;" | Workspaces | Multi-root lock retains each root's closure, targets, acceptance separately | not stated | 9
DECLARATIONS102 | "neither catalog membership nor another root's acceptance can fill missing state." | Workspaces | Catalog membership or another root's acceptance must not fill missing lock state | not stated | 9

## Ambiguous sentences

Sentence | Reading A | Reading B
--- | --- | ---
"Host effects need admitted build authority;" | Every host effect during build must fail or block without prior admission of build authority. | Host effects are permitted only when authority exists somewhere in the activation; per-effect admission timing is not stated. |
"Initial Build checking need not already know a newly admitted package's shared non-core interfaces." | Build checking may defer knowledge of shared non-core interfaces until after admission. | Build checking may skip ever learning those interfaces if nothing in the continuation references them. |
"Product selections may retain inert exact designated references;" | Inert references may remain unresolved in the frozen intent until later product checking. | Inert references may remain in the graph even if never resolved before executable admission. |
"Existing ownership, defaults/override precedence, and complete-coverage checks remain; ambiguous same-precedence rows reject." | Native binding rows participate in the same precedence model as other bindings; ambiguity at equal precedence rejects. | Only native-binding rows with equal precedence are covered; other binding domains keep separate precedence rules unstated here. |
"Checking source does not require an executable entry." | Source-type checking of a package may complete without a machine selected as executable entry. | The build entry machine need not be marked executable even when the project will later emit an executable. |
"Ordinary applications should not use this selection." | Compiler or acceptance should reject ordinary applications that register assume_binding_contract. | Policy guidance for authors; compilation may still succeed when other chapters' authorization checks pass. |
"A genuinely anonymous root uses an invocation-private activation identity while its Build runs." | Only roots with no named owner use invocation-private identity. | Any root during Build uses invocation-private identity until content-derived owner is assigned. |
"This is not inferred project continuity across edits: named source lineage supplies that when required." | Anonymous owner identity never implies continuity across edits. | Continuity across edits requires named lineage; whether anonymous roots can be compared across edits for other purposes is unstated. |
"A check that names no member takes every declared member as a [validation subject](validation_subjects.md);" | Workspace-wide check without member filter validates all members. | Any check operation that omits an explicit member parameter implicitly validates all members, including non-workspace checks if applicable. |
"Shared pins and ceilings may be passed into member Build values, and members may only narrow them." | Members cannot widen pins/ceilings relative to workspace-passed values. | Narrowing is the only permitted mutation; whether members must inherit workspace pins when none are passed is unstated. |
"Source kinds are extensible; Git is not the semantic definition of a source." | Non-Git sources use the same semantic resolution rules defined elsewhere. | Git-specific lineage rules in this chapter do not apply to other source kinds; their rules are elsewhere without stating equivalence. |

## Terms used without definition in this chapter

- build-host context / Build-host readiness
- hermetic semantic evaluator
- build authority / admitted build authority
- confined filesystem backing / snapshot/staging library profile
- restricted host requests / project acceptance / receiving policy
- `omega.lock`
- staged-build-capability-attenuation
- logical interpreted Omega activation
- Psi / Psi interpreter / checked continuations
- mutable Build / live storage
- stage checking / publication
- source-text call scanner
- `Build::dependency` / `Build::build_dependency` / delegated context
- product vs interpreted-build dependencies
- `use dependency`
- `suspend Build::prepare_dependency`
- provider-slot installation / Build provider readiness
- package coordination / normalized Build intent
- inert exact designated references
- `Build::bind` / entry-binding overload
- toolchain profile reference / locator / ForeignBinding
- portable provider / complete-coverage checks
- trusted plan rows
- free build entry / execution-backing parameters
- `FilesystemBacking` / `Files satisfies Filesystem`
- `Build::package` / `Build::application` / `Build::member`
- `Owner::build`
- default Build source / loose root
- executable entry / target-slot selections
- compiler-owned operations
- legacy locks
- purpose-specific scheduling / two checked contexts
- `artifact_only` / required artifact
- PackageKey / PackageInstance / root role
- canonical source lineage / tagged content identity
- Git lineage / canonical repository namespace
- ProgramEntry
- root-consuming commitments
- anonymous root / invocation-private activation identity
- content-derived owner / domain-separated strong digest
- source capture / semantic inputs
- named and toolchain owners
- declaration owner / owner-local declaration identity
- portable nominal provenance
- published ceiling
- `Build::assume_binding_contract`
- BuildSource/BuildOutput filesystem facets / STAGED-BUILD-CAPABILITIES
- sponsor / authorized roots / containment / custody
- validation subject / `Build::validate`
- shared pins and ceilings
- multi-root workspace lock / source closure

CHAPTER COMPLETE
