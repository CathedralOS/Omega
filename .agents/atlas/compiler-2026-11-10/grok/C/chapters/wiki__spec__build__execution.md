# Build execution and generated source

Source: `wiki/spec/build/execution.md` lines 1–199 (read).

## Rules

| rule id | the exact sentence, quoted | the heading it is under | what must be checked, in plain words | when it must hold | lane |
| --- | --- | --- | --- | --- | --- |
| EXECUTION1 | "Build execution uses explicitly delegated filesystem backing, with a confined snapshot/staging profile by default; effects outside that baseline require explicit opt-in." | Build execution and generated source | Build execution must use delegated backing and the default confined profile unless effects outside the baseline are explicitly opted in. | not stated | 9 |
| EXECUTION2 | "It is not the hermetic evaluator used for constants and proofs." | Build execution and generated source | Build execution must not be implemented as the hermetic evaluator used for constants and proofs. | not stated | 9 |
| EXECUTION3 | "Parse Build syntax and establish lexical structure, then check the initial executable prefix against core, local declarations and available exact dependencies." | Admission before execution | Build syntax must parse; lexical structure must be established; the initial executable prefix must be checked against core, local declarations, and available exact dependencies. | not stated | 9 |
| EXECUTION4 | "Surface and admit that stage's restricted requests before producing its prepared Terminal continuation." | Admission before execution | Restricted requests for that stage must be surfaced and admitted before its prepared Terminal continuation is produced. | not stated | 9 |
| EXECUTION5 | "Interpret the checked stage once." | Admission before execution | The checked stage must be interpreted exactly once. | not stated | 9 |
| EXECUTION6 | "Atomic dependency registration fixes each source, purpose, alias and delegated context before coordination may schedule acquisition and preparation." | Admission before execution | Each source, purpose, alias, and delegated context must be fixed by atomic dependency registration before coordination may schedule acquisition and preparation. | not stated | 9 |
| EXECUTION7 | "An import alone performs none of those actions." | Admission before execution | Import by itself must not perform acquisition, preparation, or the actions tied to atomic dependency registration. | not stated | 9 |
| EXECUTION8 | "For each request, the application coordinator asks package management to resolve and review source, recursively activates the dependency Build, prepares its exact declarations and selected implementations in that context, and asks Psi to check and admit the dependent executable closure." | Admission before execution | For each request, package management must resolve and review source; dependency Builds must be recursively activated; exact declarations and selected implementations must be prepared; Psi must check and admit the dependent executable closure. | not stated | 9 |
| EXECUTION9 | "No satisfier search, ambient import or default chooses a service implementation." | Admission before execution | Service implementation selection must not use satisfier search, ambient import, or a default chooser. | not stated | 9 |
| EXECUTION10 | "At explicit dependency preparation, suspend the same activation if necessary." | Admission before execution | The same activation must be suspended when necessary at explicit dependency preparation. | across a pause | 10 |
| EXECUTION11 | "Resolve the dependent imports, types, contracts and exact provider bindings, check and admit the selected continuation, then resume without replaying earlier work." | Admission before execution | Dependent imports, types, contracts, and exact provider bindings must be resolved; the selected continuation must be checked and admitted; resumption must not replay earlier work. | across a pause | 9 |
| EXECUTION12 | "Repeat stage preparation as ordinary control flow discovers dependencies; do not execute unchecked source or rerun the Build as a scan." | Admission before execution | Stage preparation may repeat as dependencies are discovered; unchecked source must not execute; the Build must not rerun as a scan. | not stated | 9 |
| EXECUTION13 | "When the Build entry returns, join every outstanding dependency job, reject any failed, cyclic, unresolved, or unadmitted edge, resolve inert product selections, and freeze the normalized Build output." | Admission before execution | On Build entry return, all dependency jobs must be joined; failed, cyclic, unresolved, or unadmitted edges must reject; inert product selections must be resolved; normalized Build output must be frozen. | at normal return | 9 |
| EXECUTION14 | "Incorporate generated source and finish all requested product checks, including behavior exclusions." | Admission before execution | Generated source must be incorporated and all requested product checks, including behavior exclusions, must complete. | not stated | 9 |
| EXECUTION15 | "Discover registered test roots, admit their execution authority, and run all enabled test groups." | Admission before execution | Registered test roots must be discovered; their execution authority must be admitted; all enabled test groups must run. | not stated | 9 |
| EXECUTION16 | "Check required-output completion and commit one immutable result set only after required tests pass." | Admission before execution | Required-output completion must be checked; one immutable result set must be committed only after required tests pass. | not stated | 9 |
| EXECUTION17 | "A dependency declaration may complete in the background when Build code never awaits its provider slot." | Admission before execution | Background completion of a dependency declaration is permitted only when Build code never awaits its provider slot. | not stated | 9 |
| EXECUTION18 | "Build return is still a mandatory join, so no unresolved request leaks into product compilation or publication." | Admission before execution | Build return must join all requests; no unresolved request may reach product compilation or publication. | at normal return | 9 |
| EXECUTION19 | "No current activation can inspect its own final component or select its generated entries; those customers require separately staged compilation." | Admission before execution | The current activation must not inspect its own final component or select its generated entries. | not stated | 9 |
| EXECUTION20 | "Behavior exclusions are checked against the product's complete selected execution scope before its successful publication." | Admission before execution | Behavior exclusions must be checked against the product's complete selected execution scope before successful publication. | not stated | 9 |
| EXECUTION21 | "They do not authorize a build-time query of that unfinished product or change the build entry's own admission." | Admission before execution | Behavior-exclusion checking must not authorize a build-time query of the unfinished product or change the build entry's admission. | not stated | 9 |
| EXECUTION22 | "Physical exclusions additionally require complete selected mechanism classification before realized-product admission." | Admission before execution | Physical exclusions require complete selected mechanism classification before realized-product admission. | not stated | 9 |
| EXECUTION23 | "Before each stage executes, its exact name/type resolution, semantic prechecks, operational/reach plans and effect admission are complete." | Admission before execution | Before each stage executes, name/type resolution, semantic prechecks, operational/reach plans, and effect admission must be complete. | at entry | 9 |
| EXECUTION24 | "Before a provider is installed, its selected implementation's review, checking, restricted-request admission and exact binding are complete." | Admission before execution | Before provider installation, review, checking, restricted-request admission, and exact binding of the selected implementation must be complete. | before provider install | 9 |
| EXECUTION25 | "Checkpoints retain stage code, declaration/environment identities, source and filesystem observations, live loans/resources and authority verdicts." | Admission before execution | Checkpoints must retain stage code, declaration/environment identities, source and filesystem observations, live loans/resources, and authority verdicts. | not stated | 2 |
| EXECUTION26 | "Execution uses those exact entries, not later lookup that can retarget an earlier alias." | Admission before execution | Execution must use the exact checkpoint entries, not later lookup that could retarget an earlier alias. | at the operation | 9 |
| EXECUTION27 | "Reach, authority roots, retained storage, resource bounds, failure, and termination must fit the executor's policy." | Admission before execution | Reach, authority roots, retained storage, resource bounds, failure handling, and termination must conform to the executor's policy. | not stated | 9 |
| EXECUTION28 | "A blocking operation must expose the progress/failure premise needed to satisfy the build entry's termination contract." | Admission before execution | Each blocking operation must expose the progress/failure premise required by the build entry's termination contract. | at the operation | 4 |
| EXECUTION29 | "Service reach need not be empty, but no host service is ambient." | Admission before execution | No host service may be ambient; service reach may be non-empty only under non-ambient rules stated elsewhere. | not stated | 9 |
| EXECUTION30 | "Resolver authority is separate from build authority." | Admission before execution | Resolver authority and build authority must remain separate. | not stated | 9 |
| EXECUTION31 | "Retrieval validates requested lineage, revision resolution, object/content identity, archive path containment, expansion limits, and immutable destination bytes." | Admission before execution | Retrieval must validate lineage, revision resolution, object/content identity, archive path containment, expansion limits, and immutable destination bytes. | not stated | 9 |
| EXECUTION32 | "It does not attest the ambient transport or executor." | Admission before execution | Retrieval must not attest ambient transport or the executor. | not stated | 9 |
| EXECUTION33 | "Dependency code receives neither resolver credentials nor the root's filesystem, process, secret, or acceptance authority." | Admission before execution | Dependency code must not receive resolver credentials or the root's filesystem, process, secret, or acceptance authority. | not stated | 9 |
| EXECUTION34 | "Each dependency build gets its own admitted package scope." | Admission before execution | Each dependency build must have its own admitted package scope. | not stated | 9 |
| EXECUTION35 | "Runtime provider selection is not a route around delegated backing and stage admission; additional host effects require actual supplied capabilities and accepted policy." | Admission before execution | Runtime provider selection must not bypass delegated backing or stage admission; additional host effects require supplied capabilities and accepted policy. | not stated | 9 |
| EXECUTION36 | "Registering a root assumed binding contract selects a runtime product relationship only." | Restricted build requests | Root assumed-binding registration must select only a runtime product relationship. | not stated | 9 |
| EXECUTION37 | "The build receives no callable binding or usable premise from registration." | Restricted build requests | Registration must not supply the build a callable binding or usable premise. | not stated | 9 |
| EXECUTION38 | "Execution through it, or using its assumptions to justify build computation, rejects across the complete build, generator and automatic-test closure." | Restricted build requests | Using the relationship or its assumptions for build computation must reject across the full build, generator, and automatic-test closure. | not stated | 9 |
| EXECUTION39 | "This is not a restricted request that lock acceptance or a wider host grant can enable." | Restricted build requests | Lock acceptance or a wider host grant must not enable execution through assumed-binding registration. | not stated | 9 |
| EXECUTION40 | "Independently checked calls that do not rely on the relationship retain their ordinary admission rules." | Restricted build requests | Calls checked without relying on the relationship keep ordinary admission rules. | at a call | 3 |
| EXECUTION41 | "Ordinary computation, admitted immutable inputs, private staged outputs, and bounded diagnostics use the default build protocol." | Restricted build requests | Listed ordinary build activities must use the default build protocol. | not stated | 9 |
| EXECUTION42 | "They do not require an unsafe-build approval merely because a dependency supplies the code." | Restricted build requests | Dependency-supplied code alone must not require unsafe-build approval for those baseline activities. | not stated | 9 |
| EXECUTION43 | "This baseline does not provide arbitrary live filesystem, process, network, credential, or resolver access." | Restricted build requests | The default protocol must not grant arbitrary live filesystem, process, network, credential, or resolver access. | not stated | 9 |
| EXECUTION44 | "Unsupported host operations remain unsupported; acceptance cannot create a missing execution protocol." | Restricted build requests | Unsupported host operations must stay unsupported; acceptance must not invent a missing execution protocol. | not stated | 9 |
| EXECUTION45 | "Checked build contracts expose requests outside that baseline, including requests reached through local helpers, installed dependency bindings, or prerequisite dependency Build activations." | Restricted build requests | Requests outside the default baseline exposed by checked build contracts must be identified, including those via helpers, bindings, or prerequisite Builds. | not stated | 9 |
| EXECUTION46 | "No nested call, dependency edge, or accepted runtime permission hides or authorizes a build-host action." | Restricted build requests | Nested calls, dependency edges, and runtime permissions must not hide or authorize build-host actions. | not stated | 9 |
| EXECUTION47 | "Retain the originating package, build purpose, operation, logical resource scope, bounds, and applicable profile/target context." | Restricted build requests | Originating package, build purpose, operation, logical resource scope, bounds, and profile/target context must be retained for restricted requests. | not stated | 9 |
| EXECUTION48 | "Delegation uses explicitly supplied capabilities and cannot widen their authority." | Restricted build requests | Delegation must use only explicitly supplied capabilities and must not widen their authority. | not stated | 9 |
| EXECUTION49 | "The parent supplies each child's filesystem backing and selected implementation under ordinary service contracts." | Restricted build requests | The parent must supply each child's filesystem backing and selected implementation under ordinary service contracts. | not stated | 9 |
| EXECUTION50 | "Alternative providers are legal only with explicitly supplied backing within the parent's grant;" | Restricted build requests | Alternative providers are permitted only with explicitly supplied backing within the parent's grant. | not stated | 9 |
| EXECUTION51 | "importing a real-filesystem API cannot create such a grant or bypass a virtual view." | Restricted build requests | Importing a real-filesystem API must not create a backing grant or bypass a virtual view. | not stated | 9 |
| EXECUTION52 | "Unsupported operations do not open a host route." | Restricted build requests | Unsupported operations must not open a host route. | not stated | 9 |
| EXECUTION53 | "Lock agreement and actual capability checks remain separate obligations." | Restricted build requests | Lock agreement and actual capability checks must remain separate obligations. | not stated | 9 |
| EXECUTION54 | "[Install/update review](../packages/acceptance.md#restricted-build-acceptance) surfaces those requests before restricted execution." | Restricted build requests | Install/update review must surface restricted requests before restricted execution. | not stated | 9 |
| EXECUTION55 | "Explicit project decisions are retained in `omega.lock`; the executor separately supplies actual scoped resources." | Restricted build requests | Explicit project decisions must be retained in `omega.lock`; the executor must separately supply actual scoped resources. | not stated | 9 |
| EXECUTION56 | "Neither lock acceptance nor a callee's own lock manufactures a host grant." | Restricted build requests | Lock acceptance and a callee's lock must not manufacture a host grant. | not stated | 9 |
| EXECUTION57 | "A root's grant is not automatically available to dependency builds." | Restricted build requests | A root's grant must not automatically apply to dependency builds. | not stated | 9 |
| EXECUTION58 | "Checking and audit cannot run an unaccepted restricted action merely to learn what it requests." | Restricted build requests | Checking and audit must not execute an unaccepted restricted action only to discover its requests. | not stated | 9 |
| EXECUTION59 | "Check each reached Build stage or requested implementation before its effects;" | Restricted build requests | Each reached Build stage or requested implementation must be checked before its effects run. | before effects | 9 |
| EXECUTION60 | "if further checking needs such an action, retain a pending review/incomplete audit until acceptance and actual grants permit continuation." | Restricted build requests | When checking needs a restricted action, review/audit must stay pending until acceptance and grants permit continuation. | not stated | 9 |
| EXECUTION61 | "Generated source is then checked and audited normally, not trusted because its generator was accepted." | Restricted build requests | Generated source must be checked and audited normally, not trusted solely because its generator was accepted. | not stated | 9 |
| EXECUTION62 | "Benign work needs no approval ceremony; unchanged accepted requests need no recurring approval." | Restricted build requests | Benign work must not require approval ceremony; unchanged accepted requests must not require recurring approval. | not stated | 9 |
| EXECUTION63 | "This governs existing helper delegation and dependency build scheduling; it adds no recursive build API or permission to import another activation's build entry." | Restricted build requests | Must govern helper delegation and dependency scheduling without adding a recursive build API or permission to import another activation's build entry. | not stated | 9 |
| EXECUTION64 | "The executor, not the offered program, enforces actual host confinement." | Restricted build requests | Actual host confinement must be enforced by the executor, not the offered program. | not stated | 9 |
| EXECUTION65 | "The same admission and attenuation rules govern build-time tests, including fixtures and selected providers." | Restricted build requests | Build-time tests, fixtures, and selected providers must follow the same admission and attenuation rules. | not stated | 9 |
| EXECUTION66 | "Completing the build entry does not grant its test roots ambient host authority." | Restricted build requests | Completing the build entry must not grant test roots ambient host authority. | at normal return | 9 |
| EXECUTION67 | "Restricted test requests join ordinary install/update review before test execution." | Restricted build requests | Restricted test requests must go through install/update review before test execution. | not stated | 9 |
| EXECUTION68 | "Tests may use delegated virtual services without restricted approval, but cannot replace them with live-host backing." | Restricted build requests | Tests may use delegated virtual services without restricted approval but must not replace them with live-host backing. | not stated | 9 |
| EXECUTION69 | "Runtime permissions and a child package's lock are not build-test grants." | Restricted build requests | Runtime permissions and a child package's lock must not count as build-test grants. | not stated | 9 |
| EXECUTION70 | "Path lookup, filesystem writing and compiler publication are distinct operations." | Generated-source boundary | Path lookup, filesystem writing, and compiler publication must be treated as distinct operations. | not stated | 9 |
| EXECUTION71 | "Filesystem helpers produce the bytes through their ordinary capability contracts." | Generated-source boundary | Filesystem helpers must produce bytes only through their ordinary capability contracts. | at the operation | 9 |
| EXECUTION72 | "`Build::include_source` accepts an authorized logical generated name and exact retained UTF-8 bytes under the current activation's resources; it does not reopen a mutable host pathname or execute those bytes." | Generated-source boundary | `Build::include_source` must accept only an authorized logical name and exact retained UTF-8 bytes under current activation resources; it must not reopen a mutable host pathname or execute those bytes. | at a call | 9 |
| EXECUTION73 | "The compiler captures the published snapshot and exact generating context; subsequent writes to a file or overlay cannot change it." | Generated-source boundary | The compiler must capture published snapshot and generating context; later file or overlay writes must not change that snapshot. | not stated | 9 |
| EXECUTION74 | "Source consumption retains the matching compiler-owned generated-source namespace and product purpose." | Generated-source boundary | Source consumption must retain the matching compiler-owned generated-source namespace and product purpose. | not stated | 9 |
| EXECUTION75 | "It may refer to admitted authored declarations, and generated units may resolve together." | Generated-source boundary | Generated stratum may refer only to admitted authored declarations; generated units may resolve together. | not stated | 9 |
| EXECUTION76 | "Authored occurrences, including every transitive build helper, cannot select generated declarations." | Generated-source boundary | Authored occurrences and transitive build helpers must not select generated declarations. | not stated | 9 |
| EXECUTION77 | "Later overloads, conformances, and providers cannot alter the already-admitted build meaning." | Generated-source boundary | Later overloads, conformances, and providers must not alter already-admitted build meaning. | not stated | 9 |
| EXECUTION78 | "Final checking extends the retained checkpoint." | Generated-source boundary | Final checking must extend the retained checkpoint only. | not stated | 9 |
| EXECUTION79 | "It does not rediscover dependencies, rerun build execution, or rebuild the authored frontend." | Generated-source boundary | Final checking must not rediscover dependencies, rerun build execution, or rebuild the authored frontend. | not stated | 9 |
| EXECUTION80 | "Each generated unit uses ordinary pre-resolution evaluation and its matching one-shot post-typing continuation." | Generated-source boundary | Each generated unit must use ordinary pre-resolution evaluation and a matching one-shot post-typing continuation. | not stated | 9 |
| EXECUTION81 | "Target filtering validates the complete base-plus-extension declaration roster while modifying only the extension." | Generated-source boundary | Target filtering must validate the full base-plus-extension roster while modifying only the extension. | not stated | 9 |
| EXECUTION82 | "Duplicate selected declarations and missing selected-target members reject;" | Generated-source boundary | Duplicate selected declarations and missing selected-target members must reject. | not stated | 9 |
| EXECUTION83 | "unselected generated siblings remain inert." | Generated-source boundary | Unselected generated siblings must remain inert. | not stated | 9 |
| EXECUTION84 | "The completed candidate must preserve the admitted base exactly: root graphs, symbols, names, paths, authored selections, semantic tables, evidence, proof and ranking records, and layout/placement/wire/specialization data remain prefixes." | Generated-source boundary | The completed candidate must preserve the admitted base exactly; listed data must remain prefixes. | not stated | 9 |
| EXECUTION85 | "An unsupported extension rejects without replacing the owned base." | Generated-source boundary | Unsupported extension must reject without replacing the owned base. | not stated | 9 |
| EXECUTION86 | "Final package-declaration admission covers both strata before package-subject derivation." | Generated-source boundary | Final package-declaration admission must cover both strata before package-subject derivation. | not stated | 9 |
| EXECUTION87 | "Every generated selection retains its requesting package and obeys public visibility and direct-dependency rules." | Generated-source boundary | Each generated selection must retain its requesting package and obey public visibility and direct-dependency rules. | not stated | 9 |
| EXECUTION88 | "Possessing transitive dependency source does not authorize importing its declarations." | Generated-source boundary | Possessing transitive dependency source must not authorize importing its declarations. | not stated | 9 |
| EXECUTION89 | "The earlier verdict continues to identify the frozen base admitted for build execution." | Generated-source boundary | The earlier verdict must continue to identify the frozen base admitted for build execution. | not stated | 9 |
| EXECUTION90 | "Generated code receives no authority from its generator." | Generated-source boundary | Generated code must not inherit authority from its generator. | not stated | 9 |
| EXECUTION91 | "Ordinary runtime reach, crash, work, conservation, and trust checking applies to it independently." | Generated-source boundary | Generated code must undergo ordinary runtime reach, crash, work, conservation, and trust checking independently. | not stated | SHARED |
| EXECUTION92 | "Review compiles dependency-first." | Dependency handoff | Review must compile dependency-first. | not stated | 9 |
| EXECUTION93 | "A consumer requires one compiler-issued generated-source bundle for every transitive package, including an explicit empty bundle when the producer included no source." | Dependency handoff | The consumer must have one compiler-issued generated-source bundle per transitive package, including an explicit empty bundle when the producer included no source. | not stated | 9 |
| EXECUTION94 | "The consumer loads these bytes in its initial frontend under the producer's identity and logical paths, without rerunning the producer build or reading its output tree." | Dependency handoff | The consumer must load bundle bytes in its initial frontend under producer identity and logical paths without rerunning the producer build or reading its output tree. | not stated | 9 |
| EXECUTION95 | "Its own source-consumption commitment includes the injected bytes." | Dependency handoff | The consumer's source-consumption commitment must include the injected bytes. | not stated | 9 |
| EXECUTION96 | "Missing, duplicate, foreign, root-self, purpose/profile/target/closure, or same-review custody mismatches reject." | Dependency handoff | Listed bundle mismatches must reject. | not stated | 9 |
| EXECUTION97 | "The handoff is opaque compiler-issued state, not a package instance or canonical admission evidence; it has no public constructor or decoder." | Dependency handoff | The handoff must be opaque compiler-issued state without public constructor or decoder; it must not be treated as package instance or canonical admission evidence. | not stated | 9 |
| EXECUTION98 | "A bundle binds: Producer package, dependency purpose, execution profile, and selected product target; Producer dependency closure and source-consumption commitment; Canonical generated paths, retained bytes, and their digests." | Dependency handoff | Each bundle must bind producer package, dependency purpose, execution profile, selected product target, producer dependency closure, source-consumption commitment, canonical generated paths, retained bytes, and digests. | not stated | 9 |

## Ambiguous sentences

| quoted sentence | reading A | reading B |
| --- | --- | --- |
| "Surface and admit that stage's restricted requests before producing its prepared Terminal continuation." | "Admit" means project/install review acceptance of restricted requests before continuation production. | "Admit" means Psi or stage admission of the restricted request set before Terminal continuation is prepared. |
| "Interpret the checked stage once." | The checked stage is executed exactly one time total for the activation. | The checked stage is interpreted once per distinct continuation segment, not retried on resume. |
| "resume without replaying earlier work." | Resumption continues from the suspension point without re-executing prior interpreted instructions. | Resumption reuses cached checkpoint results and does not repeat package resolution or earlier stage preparation. |
| "resolve inert product selections" | Resolve means fix or validate inactive product-target choices into a frozen form. | Resolve means eliminate or reject selections that have no executable effect. |
| "finish all requested product checks, including behavior exclusions." | All product checks requested by the build must run, and behavior exclusions are one subset of those checks. | Behavior exclusions are mandatory product checks even when not explicitly requested elsewhere in the build. |
| "admit their execution authority" | Test roots receive an explicit authority grant before tests run. | Test roots pass a check that their declared execution authority is valid for the build context. |
| "commit one immutable result set only after required tests pass." | Exactly one immutable result set is published; partial commits before tests are forbidden. | Multiple staging steps may occur, but only one final commit after required tests. |
| "A dependency declaration may complete in the background when Build code never awaits its provider slot." | Background completion is allowed only if no await on the provider slot appears anywhere in Build code for that dependency. | Background completion is allowed if the dynamic execution path that runs does not await the slot, even if unreachable code awaits it. |
| "This does not claim that all deferred branches of the whole Build were checked before its first effect;" | First build effect may occur before some deferred branches are checked; scoped_execution defines the boundary. | Some build effects are permitted before full-program checking of all branches; cross-reference defines which. |
| "Service reach need not be empty, but no host service is ambient." | Declared service reach may name services, but none may be implicitly available without admission. | Empty reach is not required; every reachable service must be explicitly admitted rather than host-default. |
| "unsupported host operations remain unsupported; acceptance cannot create a missing execution protocol." | Lock or review acceptance cannot invent a protocol for operations with no defined execution path. | Project acceptance cannot turn a forbidden operation into a supported one without a protocol definition. |
| "Benign work needs no approval ceremony; unchanged accepted requests need no recurring approval." | "Benign" is defined only by absence of restricted-request classification, not by a separate benign list. | Some operations are intrinsically benign and skip review even when they resemble restricted requests. |
| "Tests may use delegated virtual services without restricted approval, but cannot replace them with live-host backing." | Tests must use the same virtual delegation as build; swapping to live backing at test time is forbidden. | Virtual services need no extra approval, but explicit replacement with live backing is rejected even if restricted approval exists. |
| "Target filtering validates the complete base-plus-extension declaration roster while modifying only the extension." | Validation reads the union roster but only the extension subset is eligible for target-driven removal or alteration. | The base roster is validated read-only; only extension members are filtered by target selection. |
| "unselected generated siblings remain inert." | Unselected siblings are retained in the program but excluded from compilation products. | Unselected siblings are present in the AST but cannot be referenced or linked. |

## Terms used without definition in this range

- Build declarations
- activation-scoped Build operations
- explicitly delegated filesystem backing
- confined snapshot/staging profile
- explicit opt-in
- hermetic evaluator
- initial executable prefix
- exact dependencies
- prepared Terminal continuation
- checked stage
- atomic dependency registration
- source, purpose, alias
- delegated context
- acquisition and preparation
- application coordinator
- dependent executable closure
- satisfier search
- ambient import
- explicit dependency preparation
- selected continuation
- outstanding dependency job
- inert product selections
- normalized Build output
- behavior exclusions
- registered test roots
- execution authority
- test groups
- required-output completion
- immutable result set
- provider slot
- execution-profile/product-target distinction
- deterministic snapshot and staging protocol
- linear output lifecycle
- final component
- generated entries
- separately staged compilation
- selected execution scope
- successful publication
- build-time query
- realized-product admission
- mechanism classification
- semantic prechecks
- operational/reach plans
- effect admission
- restricted-request admission
- exact binding
- checkpoints
- authority verdicts
- executor's policy
- progress/failure premise
- termination contract
- host service
- resolver authority
- build authority
- lineage
- revision resolution
- object/content identity
- archive path containment
- expansion limits
- immutable destination bytes
- admitted package scope
- runtime provider selection
- root assumed binding contract
- runtime product relationship
- usable premise
- generator and automatic-test closure
- lock acceptance
- wider host grant
- unsafe-build approval
- default build protocol
- checked build contracts
- build-host action
- logical resource scope
- profile/target context
- virtual provider
- real-filesystem API
- install/update review
- `omega.lock`
- scoped resources
- host grant
- pending review
- incomplete audit
- recursive build API
- build entry
- host confinement
- build-time tests
- live-host backing
- build-test grants
- authorized logical generated name
- retained UTF-8 bytes
- current activation's resources
- mutable host pathname
- capability contracts
- published snapshot
- exact generating context
- generated-source namespace
- product purpose
- resolution stratum
- admitted authored declarations
- transitive build helper
- already-admitted build meaning
- pre-resolution evaluation
- one-shot post-typing continuation
- base-plus-extension declaration roster
- selected-target members
- admitted base
- root graphs
- semantic tables
- ranking records
- layout/placement/wire/specialization data
- package-subject derivation
- direct-dependency rules
- frozen base
- generated-source bundle
- transitive package
- explicit empty bundle
- producer package
- dependency purpose
- execution profile
- selected product target
- dependency closure
- source-consumption commitment
- canonical generated paths
- digests
- initial frontend
- output tree
- injected bytes
- root-self
- same-review custody
- opaque compiler-issued state
- canonical admission evidence
- public constructor
- decoder
- implementation entry points
- bounded continuation support

CHAPTER COMPLETE
