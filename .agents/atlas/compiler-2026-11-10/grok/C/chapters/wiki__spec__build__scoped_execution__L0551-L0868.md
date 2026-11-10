# Scoped execution (lines 551–868)

Source: `wiki/spec/build/scoped_execution.md` lines 551–868 (read).

## Rules

rule id | the exact sentence, quoted | the heading it is under | what must be checked, in plain words | when it must hold | lane
--- | --- | --- | --- | --- | ---
SCOPED_EXECUTION1 | "Two consumers can prepare the same source package under different backing, provider selections or configuration. They receive distinct execution contexts;" | Execution contexts and reuse | Distinct execution contexts for consumers that differ in backing, providers, or configuration | not stated | 9
SCOPED_EXECUTION2 | "neither must adopt the other's input map," | Execution contexts and reuse | A consumer must not adopt another consumer's input map | not stated | 9
SCOPED_EXECUTION3 | "and coordination never unions their authority." | Execution contexts and reuse | Coordination must not union authority across consumers | not stated | 9
SCOPED_EXECUTION4 | "Source acquisition and compiled code may be shared only where their actual checked identities permit it." | Execution contexts and reuse | Sharing of source acquisition or compiled code is permitted only when actual checked identities permit it | not stated | 9
SCOPED_EXECUTION5 | "Build results, generated code and effects are not shared merely because package bytes or aliases match." | Execution contexts and reuse | Build results, generated code, and effects must not be shared solely because package bytes or aliases match | not stated | 9
SCOPED_EXECUTION6 | "Joining produced variants still requires compatible exact product identities;" | Execution contexts and reuse | Joining produced variants requires compatible exact product identities | not stated | 9
SCOPED_EXECUTION7 | "separate execution does not authorize merging contradictory generated declarations." | Execution contexts and reuse | Separate execution must not authorize merging contradictory generated declarations | not stated | 9
SCOPED_EXECUTION8 | "Before reusing an execution result, prove applicability to the exact code, purpose, execution profile, configuration, selected providers, actual backing grants, source inventory, filesystem observations and accepted policy." | Execution contexts and reuse | Before reuse, applicability must be proved against the listed dimensions | not stated | 9
SCOPED_EXECUTION9 | "The conservative implementation commits the complete delegated immutable view and logical metadata profile, including negative lookups and directory membership." | Execution contexts and reuse | Conservative reuse must commit the full delegated immutable view and logical metadata profile including negative lookups and directory membership | not stated | 9
SCOPED_EXECUTION10 | "Content-identical immutable snapshots may share bytes without sharing grants, writer custody or live activation identities." | Execution contexts and reuse | Shared bytes must not imply shared grants, writer custody, or live activation identities | not stated | 9
SCOPED_EXECUTION11 | "A library-supplied changeset or hash alone is not an authority or replay receipt." | Execution contexts and reuse | Changeset or hash from a library must not be treated as authority or replay receipt by itself | not stated | 9
SCOPED_EXECUTION12 | "Product reuse is not evidence that the Build ran again and cannot silently omit required observable effects." | Execution contexts and reuse | Product reuse must not count as evidence the Build reran and must not silently omit required observable effects | not stated | 9
SCOPED_EXECUTION13 | "Materializing cached files or applying retained changes still requires the destination's actual authority and the selected conflict/failure contract." | Execution contexts and reuse | Materializing cached files or applying retained changes requires destination authority and the selected conflict/failure contract | not stated | 9
SCOPED_EXECUTION14 | "Mutable/live backing is not an immutable cache input." | Execution contexts and reuse | Mutable or live backing must not be used as an immutable cache input | not stated | 9
SCOPED_EXECUTION15 | "Capture an adequate immutable observation closure under its actual authority, or decline persistent result reuse when applicability cannot be established." | Execution contexts and reuse | Either capture an adequate immutable observation closure under actual authority or decline persistent reuse when applicability cannot be established | not stated | 9
SCOPED_EXECUTION16 | "Resource or host failures cannot be relabeled as stable NotFound/EOF observations." | Execution contexts and reuse | Resource or host failures must not be relabeled as stable NotFound or EOF observations | not stated | 9
SCOPED_EXECUTION17 | "No persistent scratch cache becomes implicitly readable." | Execution contexts and reuse | Persistent scratch cache must not become implicitly readable | not stated | 9
SCOPED_EXECUTION18 | "Different contexts are ordinary supported occurrences, not a conflict that must always be detected before the package's first-ever activation." | Execution contexts and reuse | Different contexts are supported; they are not required to be detected as conflicts before first activation | not stated | 9
SCOPED_EXECUTION19 | "Each occurrence and each newly reached continuation is admitted before its own effects; earlier authorized execution has no rollback." | Execution contexts and reuse | Each occurrence and newly reached continuation is admitted before its effects; earlier authorized execution has no rollback | not stated | 9
SCOPED_EXECUTION20 | "Additional real-host access is opt-in under independently accepted consumer and executor policy." | Execution contexts and reuse | Additional real-host access requires opt-in under independently accepted consumer and executor policy | not stated | 9
SCOPED_EXECUTION21 | "Requests by downloaded code cannot grant themselves authority." | Execution contexts and reuse | Downloaded code must not grant itself authority via its own requests | not stated | 9
SCOPED_EXECUTION22 | "Install/update must expose direct and transitive restricted build requests before executing them." | Execution contexts and reuse | Install or update must expose direct and transitive restricted build requests before execution | not stated | 9
SCOPED_EXECUTION23 | "the invocation supplies actual grants separately." | Execution contexts and reuse | Actual grants must be supplied separately by the invocation | not stated | 9
SCOPED_EXECUTION24 | "No dependency's own lock, runtime acceptance, or ordinary resolution refresh substitutes for that decision." | Execution contexts and reuse | A dependency's lock, runtime acceptance, or resolution refresh must not substitute for the consuming project's restricted-build acceptance decision | not stated | 9
SCOPED_EXECUTION25 | "Specify operation, root/object scope, lifetime, and bounds; do not use an undifferentiated \"trust this package\" flag." | Execution contexts and reuse | Restricted access must specify operation, scope, lifetime, and bounds; undifferentiated trust-this-package flags are forbidden | not stated | 9
SCOPED_EXECUTION26 | "A host grant does not propagate automatically to dependency builds or sibling helpers." | Execution contexts and reuse | Host grants must not propagate automatically to dependency builds or sibling helpers | not stated | 9
SCOPED_EXECUTION27 | "Prefer capturing another input over lending live filesystem authority." | Execution contexts and reuse | Captured inputs are preferred over lending live filesystem authority | not stated | 9
SCOPED_EXECUTION28 | "Network and subprocess facilities, if later justified, need their own admitted protocols; a filesystem grant or std import provides neither." | Execution contexts and reuse | Network and subprocess require their own admitted protocols; filesystem grant or std import must not suffice for them | not stated | 9
SCOPED_EXECUTION29 | "Resolver credentials and package retrieval remain outside build code's authority." | Execution contexts and reuse | Resolver credentials and package retrieval must remain outside build code's authority | not stated | 9
SCOPED_EXECUTION30 | "The default portable profile prefers captured read-only views." | Execution contexts and reuse | Default portable profile must prefer captured read-only views | not stated | 9
SCOPED_EXECUTION31 | "Separately admitted live backing uses the same ordinary service abstraction while retaining its real observations, external effects and actual scoped authority." | Execution contexts and reuse | Live backing must use the ordinary service abstraction and retain real observations, external effects, and actual scoped authority | not stated | 9
SCOPED_EXECUTION32 | "It must not claim snapshot isolation or enter snapshot-result reuse by a mode flag." | Execution contexts and reuse | Live backing must not claim snapshot isolation or enter snapshot-result reuse via a mode flag | not stated | 9
SCOPED_EXECUTION33 | "Library provider selection cannot approve its own host grant." | Execution contexts and reuse | Library provider selection must not approve its own host grant | not stated | 9
SCOPED_EXECUTION34 | "Network and process APIs remain separately specified capabilities, not consequences of filesystem access." | Execution contexts and reuse | Network and process APIs must remain separately specified; filesystem access must not imply them | not stated | 9
SCOPED_EXECUTION35 | "Calling a library within the root build does not create a new sandbox." | Execution contexts and reuse | Library calls within the root build must not be treated as creating a new sandbox | not stated | 9
SCOPED_EXECUTION36 | "It can use capabilities passed by its caller." | Execution contexts and reuse | Called libraries may use only capabilities passed by the caller | at a call | 9
SCOPED_EXECUTION37 | "Passing the root builder may permit all of that builder's operations; use narrower values when that is not intended." | Execution contexts and reuse | Passing the root builder may expose all builder operations; narrower values must be used when that is not intended | at a call | 9
SCOPED_EXECUTION38 | "Reading permitted secrets and writing them to permitted logs/artifacts is still possible." | Execution contexts and reuse | Broad grants may still permit reading secrets and writing them to permitted logs or artifacts | not stated | 9
SCOPED_EXECUTION39 | "This contract provides confinement and explicit delegation, not information-flow security or protection after an intentionally broad grant." | Execution contexts and reuse | The contract must not be interpreted as information-flow security beyond confinement and explicit delegation | not stated | 9
SCOPED_EXECUTION40 | "included bytes join a later product checking stratum and cannot redefine the already admitted build or introduce new build helpers for that invocation." | Staged products and failure | Included generated bytes must join later product checking and must not redefine the admitted build or introduce new build helpers | not stated | 9
SCOPED_EXECUTION41 | "Dependency-generated source enters only through the corresponding role/target-bound handoff, never a mutable output path." | Staged products and failure | Dependency-generated source must enter only via role/target-bound handoff, never a mutable output path | not stated | 9
SCOPED_EXECUTION42 | "`Build::require_artifact(builder, name)` reserves one logical output name and returns its activation-bound obligation, or a checked error." | Staged products and failure | `require_artifact` must reserve one logical name and return an activation-bound obligation or checked error | at the operation | 9
SCOPED_EXECUTION43 | "Duplicate names and file/directory prefix collisions reject; no last writer wins." | Staged products and failure | Duplicate output names and file/directory prefix collisions must reject; last writer wins is forbidden | at the operation | 9
SCOPED_EXECUTION44 | "Names use the same logical path rules as staging." | Staged products and failure | Output names must follow the same logical path rules as staging | not stated | 9
SCOPED_EXECUTION45 | "Registration may depend on admitted build computation, as may atomic dependency requests." | Staged products and failure | Output registration and atomic dependency requests may depend on admitted build computation only when admission holds | not stated | 9
SCOPED_EXECUTION46 | "never infer that a conditional branch which omitted `require` nevertheless checked the intended artifact." | Staged products and failure | A conditional branch that omitted `require` must not be inferred to have checked the intended artifact | not stated | 9
SCOPED_EXECUTION47 | "A caller needing a fixed output roster supplies that roster as an invocation requirement:" | Staged products and failure | Fixed output rosters must be supplied as invocation requirements | at entry | 9
SCOPED_EXECUTION48 | "one narrow executor-owned completion record that cannot be forged, reset, or satisfied by dropping the source value:" | Staged products and failure | Completion record must be executor-owned and not forgeable, resettable, or satisfiable by dropping the source value | not stated | 2
SCOPED_EXECUTION49 | "Record `Pending` and return its unique obligation." | Staged products and failure | On `require(name)`, record Pending and return a unique obligation | at the operation | 9
SCOPED_EXECUTION50 | "Consume the obligation, bind the exact immutable file from this staging occurrence, record `Completed`, return an output receipt." | Staged products and failure | On `complete`, consume obligation, bind exact immutable file from this staging occurrence, record Completed, return receipt | at the operation | 9
SCOPED_EXECUTION51 | "Return the obligation and file custody; state remains `Pending`, so an explicit retry is possible." | Staged products and failure | On completion error, return obligation and file custody; state stays Pending for retry | at the operation | 9
SCOPED_EXECUTION52 | "Consume the obligation, record `Failed`; the activation cannot publish a successful product set." | Staged products and failure | On `fail`, consume obligation, record Failed; activation cannot publish a successful product set | at the operation | 9
SCOPED_EXECUTION53 | "Ordinary linear checking rejects source-level discard." | Staged products and failure | Undischarged obligations must be rejected on source-level discard by linear checking | not stated | 2
SCOPED_EXECUTION54 | "If interruption prevents disposition or finalization finds it pending, the activation fails; no implicit cancellation or new general linear-drop rule." | Staged products and failure | Pending obligations at interruption or finalization must fail the activation; no implicit cancellation or new linear-drop rule | not stated | 9
SCOPED_EXECUTION55 | "Succeed only if all registered and invocation-required outputs are completed and no sticky build failure exists." | Staged products and failure | Build return succeeds only if all required outputs are completed and no sticky build failure exists | at normal return | 9
SCOPED_EXECUTION56 | "Abort the occurrence and its new product set; do not synthesize completion receipts." | Staged products and failure | On trap, cancellation, exhaustion, or executor failure, abort occurrence and new product set; do not synthesize completion receipts | not stated | 9
SCOPED_EXECUTION57 | "`complete` cannot accept an open writer, a mutable host path, a file from another activation, or a previous run's receipt." | Staged products and failure | `complete` must reject open writers, mutable host paths, files from other activations, and previous run receipts | at the operation | 9
SCOPED_EXECUTION58 | "publication does not require the retired BuildOutput writer category." | Staged products and failure | Publication must not require the retired BuildOutput writer category | not stated | 9
SCOPED_EXECUTION59 | "requires a new completion under the current obligation." | Staged products and failure | A retained immutable blob backing a new staged file requires a new completion under the current obligation | at the operation | 9
SCOPED_EXECUTION60 | "Completion cannot trigger loading of artifact-defined code." | Staged products and failure | Completion must not trigger loading of artifact-defined code | at the operation | 9
SCOPED_EXECUTION61 | "Required outputs are files in this first slice;" | Staged products and failure | Required outputs in this slice must be files | not stated | 9
SCOPED_EXECUTION62 | "Names belong to the output set, separately from scratch paths." | Staged products and failure | Output names belong to the output set, separate from scratch paths | not stated | 9
SCOPED_EXECUTION63 | "Completion binds the sealed bytes to the reserved output name; it does not rename or move a host file." | Staged products and failure | Completion binds sealed bytes to the reserved name without renaming or moving a host file | at the operation | 9
SCOPED_EXECUTION64 | "A completion receipt is provisional occurrence evidence until the final result-set commit, not permission for an external consumer to observe early output." | Staged products and failure | Completion receipt is provisional until result-set commit and must not authorize early external observation | not stated | 9
SCOPED_EXECUTION65 | "No host/export executable bit is inferred from content, filename, or such a receipt;" | Staged products and failure | Executable bit must not be inferred from content, filename, or completion receipt | not stated | 9
SCOPED_EXECUTION66 | "executable admission remains a separate product/installation operation." | Staged products and failure | Executable admission must remain a separate product or installation operation | not stated | 9
SCOPED_EXECUTION67 | "Once it chooses `fail`, that failure is sticky." | Staged products and failure | After `fail` is chosen, the failure must remain sticky | at the operation | 9
SCOPED_EXECUTION68 | "An ordinary log message is not failure and an ignored error cannot complete an outstanding output." | Staged products and failure | Log messages must not count as failure; ignored errors must not complete outstanding outputs | not stated | 9
SCOPED_EXECUTION69 | "There is no arbitrary compiler validation-plugin registry or new general effect mechanism." | Staged products and failure | No arbitrary validation-plugin registry or new general effect mechanism may be introduced for this | not stated | 9
SCOPED_EXECUTION70 | "Completing an output binds exact bytes and declared dependencies; it does not cause the compiler to understand or approve a package's semantic claim." | Staged products and failure | Completion binds bytes and dependencies only; compiler must not approve package semantic claims | at the operation | 9
SCOPED_EXECUTION71 | "an independent consumer must still check the artifact's meaning and assumptions." | Staged products and failure | Independent consumers must still check artifact meaning and assumptions | not stated | 9
SCOPED_EXECUTION72 | "A malicious writer can produce arbitrary bytes, not a valid proof by marking an output complete." | Staged products and failure | Marking output complete must not constitute valid proof of artifact semantics | at the operation | 9
SCOPED_EXECUTION73 | "On unsuccessful evaluation or incomplete obligations, publish no new successful product set." | Staged products and failure | Unsuccessful evaluation or incomplete obligations must not publish a new successful product set | not stated | 9
SCOPED_EXECUTION74 | "Explicitly granted live-host effects need their own failure contract and are not magically rolled back by discarding staged products." | Staged products and failure | Live-host effects need their own failure contract and are not rolled back by discarding staged products | not stated | 9
SCOPED_EXECUTION75 | "Publish a complete immutable output-set manifest only after build execution, generated-source checking, and all requested product checks succeed." | Staged products and failure | Output-set manifest publication requires successful build execution, generated-source checking, and product checks | not stated | 9
SCOPED_EXECUTION76 | "A failed export into a user-selected directory cannot make a partial copy an admitted result; materialization reports its own failure." | Staged products and failure | Failed export must not admit partial copies; materialization must report its own failure | not stated | 9
SCOPED_EXECUTION77 | "Consumers read the committed manifest and bound blobs, not whatever files happen to exist in a previous output folder." | Staged products and failure | Consumers must use committed manifest and bound blobs, not incidental files in prior output folders | not stated | 9
SCOPED_EXECUTION78 | "failure of one child is not hidden by another child's successful set." | Staged products and failure | One child's failure must not be hidden by another child's successful set | not stated | 9
SCOPED_EXECUTION79 | "Select `Build::artifact_only(builder)` once during Build execution." | Staged products and failure | `artifact_only` may be selected at most once during Build execution | at the operation | 9
SCOPED_EXECUTION80 | "It is valid for an application and invalid for a workspace/package." | Staged products and failure | `artifact_only` is valid only for application, invalid for workspace or package | not stated | 9
SCOPED_EXECUTION81 | "No fourth project role is added." | Staged products and failure | No fourth project role may be added for artifact-only mode | not stated | 9
SCOPED_EXECUTION82 | "Executable is the application default." | Staged products and failure | Application builds default to executable mode | not stated | 9
SCOPED_EXECUTION83 | "Artifact-only mode rejects executable root/provider selections rather than silently ignoring them and must complete at least one required artifact." | Staged products and failure | Artifact-only mode must reject executable root/provider selections and complete at least one required artifact | not stated | 9
SCOPED_EXECUTION84 | "the mode does not imply cross-target equivalence or a new target profile." | Staged products and failure | Artifact-only mode must not imply cross-target equivalence or a new target profile | not stated | 9
SCOPED_EXECUTION85 | "A generator cannot inspect a selected realization target:" | Staged products and failure | Generators must not inspect a selected realization target | not stated | 9
SCOPED_EXECUTION86 | "the evaluated Build records target-keyed provider rows." | Staged products and failure | Evaluated Build must record target-keyed provider rows | not stated | 9
SCOPED_EXECUTION87 | "It emits neither target-prefixed declarations nor a different file selected per realized target." | Staged products and failure | Generated output must not use target-prefixed declarations or per-target file selection | not stated | 9
SCOPED_EXECUTION88 | "This first slice does not automatically merge outputs from different target children." | Staged products and failure | Outputs from different target children must not be automatically merged in this slice | not stated | 9
SCOPED_EXECUTION89 | "Executable builds still require the target's ProgramEntry and other mandatory roots even when they also publish artifacts." | Staged products and failure | Executable builds must retain ProgramEntry and mandatory roots even when publishing artifacts | not stated | 9
SCOPED_EXECUTION90 | "Publishing bytes is not proof that a graph models a component or that a deployment is confined." | Staged products and failure | Published bytes must not be treated as proof of topology modeling or deployment confinement | not stated | 9
SCOPED_EXECUTION91 | "No new compiler callback that recognizes a topology verifier or arbitrary native plugin is introduced." | Staged products and failure | No new compiler callback for topology verifiers or arbitrary native plugins | not stated | 9
SCOPED_EXECUTION92 | "A separately selected installer receives its own explicit runtime authority and independently validates package-defined artifacts before activation." | Staged products and failure | Installers must have explicit runtime authority and validate artifacts independently before activation | not stated | 9
SCOPED_EXECUTION93 | "Build execution may inspect explicitly supplied verified component descriptions without executing those components." | Target inspection and reflection | Build may inspect explicitly supplied verified component descriptions without executing them | not stated | 9
SCOPED_EXECUTION94 | "Reading raw artifact metadata does not make it verified." | Target inspection and reflection | Raw artifact metadata must not be treated as verified | not stated | 9
SCOPED_EXECUTION95 | "Missing completeness or unaccepted assumptions remain failure to establish the required claim, not an empty interface." | Target inspection and reflection | Missing completeness or unaccepted assumptions must fail claim establishment, not yield an empty interface | not stated | 9
SCOPED_EXECUTION96 | "The accepted assumption inputs come from the consuming side's existing acceptance policy, not from the component or dependency Build that published the description." | Target inspection and reflection | Assumption inputs must come from consumer acceptance policy, not publisher Build | not stated | 9
SCOPED_EXECUTION97 | "omission inherits it, explicit empty permits none in the restricted category, and widening rejects." | Target inspection and reflection | Build policy narrowing: omission inherits ceiling, explicit empty permits none, widening rejects | not stated | 9
SCOPED_EXECUTION98 | "Check the full per-target selected closure, including initialization, callbacks, cleanup, helpers, and retained providers." | Target inspection and reflection | Full per-target selected closure must be checked including init, callbacks, cleanup, helpers, providers | not stated | 9
SCOPED_EXECUTION99 | "A restriction cannot erase a possible runtime path or rewrite the verified description's inventory." | Target inspection and reflection | Restrictions must not erase possible runtime paths or rewrite verified description inventory | not stated | 9
SCOPED_EXECUTION100 | "The retired `Build::accept_component_assumption` call grants no consumer approval." | Target inspection and reflection | Retired `accept_component_assumption` must grant no consumer approval | not stated | 9
SCOPED_EXECUTION101 | "Type reflection may help construct schemas for explicitly authorized types, but does not reveal arbitrary binaries, private code, installed endpoint identities, or complete executable authority." | Target inspection and reflection | Reflection must not reveal binaries, private code, endpoint identities, or full executable authority beyond authorized types | not stated | 8
SCOPED_EXECUTION102 | "A build-only alias does not authorize reflection over all product dependencies." | Target inspection and reflection | Build-only alias must not authorize reflection over all product dependencies | not stated | 8
SCOPED_EXECUTION103 | "Start with explicit component descriptors rather than a new privileged \"reflect the whole application\" API." | Target inspection and reflection | Reflection must use explicit component descriptors, not a privileged whole-application API | not stated | 8
SCOPED_EXECUTION104 | "Any missing generic verified-description consumer must be scoped in its existing owner with positive and mutation tests." | Target inspection and reflection | Missing generic verified-description consumers must be scoped in existing owners with positive and mutation tests | not stated | 8
SCOPED_EXECUTION105 | "Previously completed independent components may feed this sequence as explicit immutable input artifacts." | Target inspection and reflection | Independent completed components may enter only as explicit immutable input artifacts | not stated | 9
SCOPED_EXECUTION106 | "Inspecting this invocation's final component during its own build is a cycle." | Target inspection and reflection | Inspecting this invocation's final component during its own build must be rejected as a cycle | not stated | 9
SCOPED_EXECUTION107 | "therefore uses a separate composition build after those services, not a hidden post-compilation callback in their build entry." | Target inspection and reflection | Topology inspection of freshly built services must use a separate composition build, not hidden post-compilation callbacks | not stated | 9
SCOPED_EXECUTION108 | "A build cannot rewrite its own dependency or input frontier by serializing a new manifest as output." | Target inspection and reflection | Build must not rewrite its dependency or input frontier by serializing a manifest as output | not stated | 9
SCOPED_EXECUTION109 | "Request may complete without suspending; Build return still joins and records it." | Implementation acceptance | Unused declared build dependency may complete without suspending; Build return still joins and records it | not stated | 9
SCOPED_EXECUTION110 | "Suspend at `Build::await_provider<R, Backing>`, install only the explicitly named admitted backing into the exact static slot, and resume the same continuation without replay." | Implementation acceptance | Provider readiness before dependency work: suspend at await_provider, install only named admitted backing in exact slot, resume without replay | across a pause | 9
SCOPED_EXECUTION111 | "Execute normally; only the requests actually emitted belong to this Build result." | Implementation acceptance | Conditional or helper dependency declarations: only actually emitted requests belong to this Build result | not stated | 9
SCOPED_EXECUTION112 | "Remains inert until preparation; check the dependent continuation before any use executes." | Implementation acceptance | Deferred import stays inert until preparation; dependent continuation checked before use | not stated | 9
SCOPED_EXECUTION113 | "Normally warn; acquire and execute nothing." | Implementation acceptance | Unused import without dependency request: warn only; no acquire or execute | not stated | 9
SCOPED_EXECUTION114 | "Reject without retargeting any previously checked declaration." | Implementation acceptance | Alias mutation, duplicate registration, or later request justifying earlier code must reject without retargeting checked declarations | not stated | 9
SCOPED_EXECUTION115 | "Require preparation on every incoming use path or an explicit checked preparation before the use." | Implementation acceptance | Branch-local dependency at join requires preparation on every path or explicit checked preparation before use | on every edge | 9
SCOPED_EXECUTION116 | "Checked in the build context with exact Build vocabulary and normal visibility." | Implementation acceptance | Local multi-file build helper must be checked in build context with Build vocabulary and normal visibility | not stated | 9
SCOPED_EXECUTION117 | "Exact designated product operand resolves; no target code executes on the host." | Implementation acceptance | Product-only entry without host import: designated product operand resolves; no target code on host | not stated | 9
SCOPED_EXECUTION118 | "Ordinary calls select only the host package; product operands select only the product package. No fallback." | Implementation acceptance | Same alias for host and product: calls select host package only, product operands product package only; no fallback | not stated | 9
SCOPED_EXECUTION119 | "Ordinary build resolution; no special product authority." | Implementation acceptance | User-defined product-selection lookalike uses ordinary build resolution without special product authority | not stated | 9
SCOPED_EXECUTION120 | "Reject; an explicitly delegated restricted entry reference may only be used for its permitted binding." | Implementation acceptance | Helper querying caller-private product member or using description as host callable must reject; restricted entry reference only for permitted binding | not stated | 9
SCOPED_EXECUTION121 | "Reject the phase cycle; use a separately staged build." | Implementation acceptance | Build querying own generated entry or final component must reject phase cycle | not stated | 9
SCOPED_EXECUTION122 | "Reject, even if build evaluation already loaded its source." | Implementation acceptance | Application or generated source importing build-only dependency must reject even if source was loaded during build | not stated | 9
SCOPED_EXECUTION123 | "Shared acquisition permitted; checked contexts and outputs remain distinct." | Implementation acceptance | One package for host tools and different product target: shared acquisition allowed; contexts and outputs distinct | not stated | 9
SCOPED_EXECUTION124 | "Build provenance and affected results change; no stale output reuse." | Implementation acceptance | Changed build-tool revision or input changes provenance and results; no stale output reuse | not stated | 9
SCOPED_EXECUTION125 | "Succeeds using snapshot/staging capabilities without live-host access." | Implementation acceptance | Dependency generator reading template and writing generated code must succeed via snapshot/staging without live-host access | not stated | 9
SCOPED_EXECUTION126 | "Same logical observations; unsupported metadata is not fabricated." | Implementation acceptance | Same snapshot with different host metadata must yield same logical observations; unsupported metadata must not be fabricated | not stated | 9
SCOPED_EXECUTION127 | "Accounted in the input commitment; no stale watch-list-based reuse." | Implementation acceptance | Missing input, negative lookup, or changed directory membership must be in input commitment; no stale watch-list reuse | not stated | 9
SCOPED_EXECUTION128 | "Explicit refusal before unauthorized host access." | Implementation acceptance | Reads of secrets, escaping links, or fabricated handles must refuse before unauthorized host access | not stated | 9
SCOPED_EXECUTION129 | "May read that input; no false confidentiality claim." | Implementation acceptance | Helper given wider input may read it; no false confidentiality claim | not stated | 9
SCOPED_EXECUTION130 | "Actual backing and complete closure admission remain required; no host escape or automatic grant." | Implementation acceptance | Alternative filesystem/network provider: actual backing and complete closure admission required; no host escape or automatic grant | not stated | 9
SCOPED_EXECUTION131 | "Distinct execution contexts; no grant union or reuse of an inapplicable result." | Implementation acceptance | Two consumers with different backing views: distinct contexts; no grant union or inapplicable result reuse | not stated | 9
SCOPED_EXECUTION132 | "Preserve exact base/view and destination authority; no implicit commit to parent or host." | Implementation acceptance | Cow overlay exports must preserve base/view and destination authority; no implicit commit to parent or host | not stated | 9
SCOPED_EXECUTION133 | "No new successful product publication." | Implementation acceptance | Partial output, ignored error, or unfinished required output must not publish new successful product | not stated | 9
SCOPED_EXECUTION134 | "Custody is returned on the failed attempt; one valid completion may succeed." | Implementation acceptance | Failed completion then successful retry: custody returned on failure; one valid completion may succeed | not stated | 9
SCOPED_EXECUTION135 | "Reject; cannot substitute equal bytes for activation custody." | Implementation acceptance | Duplicate completion, consumed obligation reuse, or prior activation receipt must reject; equal bytes cannot substitute custody | not stated | 9
SCOPED_EXECUTION136 | "No new committed successful product set." | Implementation acceptance | One output complete and another failed, or invalid later generated source: no new committed successful product set | not stated | 9
SCOPED_EXECUTION137 | "Final result rejects the missing requested member." | Implementation acceptance | Conditional branch never registering invocation-required output: final result rejects missing requested member | at normal return | 9
SCOPED_EXECUTION138 | "Only the explicit artifact-only intent may omit executable roots." | Implementation acceptance | Omitting executable roots is permitted only under explicit artifact-only intent | not stated | 9
SCOPED_EXECUTION139 | "Package verifier/installer rejects; publication is not semantic approval." | Implementation acceptance | Forged topology in published file: verifier/installer rejects; publication is not semantic approval | not stated | 9
SCOPED_EXECUTION140 | "Reject the mutation; the completion receipt keeps the same immutable bytes." | Implementation acceptance | Sealed or completed output with surviving attempted writer: reject mutation; receipt keeps same immutable bytes | not stated | 9
SCOPED_EXECUTION141 | "Run actual acquisition, multi-file build execution, and generated-source checks, not just dependency-parser tests." | Implementation acceptance | Validation must run acquisition, multi-file build execution, and generated-source checks | not stated | SHARED
SCOPED_EXECUTION142 | "Exercise snapshot/staging isolation and failure paths on Windows and macOS; source inspection is not a host execution pass." | Implementation acceptance | Tests must exercise snapshot/staging isolation and failure on Windows and macOS | not stated | SHARED
SCOPED_EXECUTION143 | "There is no permanent legacy mode or parallel filesystem API." | Migration and retirement | No permanent legacy mode or parallel filesystem API | not stated | 9
SCOPED_EXECUTION144 | "changing names must not remove those guarantees." | Migration and retirement | Renaming retired APIs must not remove source capture, provider installation, linear publication, accounting, or authority guarantees | not stated | 9
SCOPED_EXECUTION145 | "Library helper signatures and protocol encoding must preserve that journey." | Migration and retirement | Library helper signatures and protocol encoding must preserve the accepted public journey | not stated | 9
SCOPED_EXECUTION146 | "reject or explicitly migrate old records rather than reinterpret their bytes." | Migration and retirement | Old records with changed context or stage meaning must be rejected or explicitly migrated, not reinterpreted | not stated | 9
SCOPED_EXECUTION147 | "raise any semantic or trust amendment in [owner questions](../../../OWNER_QUESTIONS.md) before relying on it." | Migration and retirement | New semantic or trust choices exposed by examples must be raised in OWNER_QUESTIONS before reliance | not stated | SHARED
SCOPED_EXECUTION148 | "Specify logical snapshot observations and fail closed if the executor cannot provide them." | Comparison and mechanism choice | Executor must fail closed when it cannot provide logical snapshot observations | not stated | 9
SCOPED_EXECUTION149 | "No implicit sandbox fallback or fixed-output network exception is authorized here." | Comparison and mechanism choice | No implicit sandbox fallback or fixed-output network exception | not stated | 9
SCOPED_EXECUTION150 | "File transforms, graph policies, codecs, and artifact semantic verification remain package code." | Comparison and mechanism choice | File transforms, graph policies, codecs, and artifact semantic verification must remain package code | not stated | 9
SCOPED_EXECUTION151 | "Generic mechanisms should remove duplicated ownership, not hide a small compiler operation behind an unneeded plugin system." | Comparison and mechanism choice | Generic mechanisms must not hide small compiler operations behind unneeded plugin systems | not stated | 9
SCOPED_EXECUTION152 | "One Build argument with scoped operations is the baseline: two arguments do not by themselves enforce dependency or authority separation." | Comparison and mechanism choice | One Build argument with scoped operations is baseline; two arguments alone do not enforce separation | not stated | 9
SCOPED_EXECUTION153 | "No-op I/O and relying on linker stripping to enforce build-only access are rejected alternatives." | Comparison and mechanism choice | No-op I/O and linker stripping for build-only access are rejected | not stated | 9
SCOPED_EXECUTION154 | "Unsupported behavior must not silently weaken this contract." | Comparison and mechanism choice | Unsupported behavior must not silently weaken the scoped execution contract | not stated | SHARED

## Ambiguous sentences

Sentence | Reading A | Reading B
--- | --- | ---
"Registration may depend on admitted build computation, as may atomic dependency requests." | Registration and atomic dependency requests are allowed to be conditional on admitted build computation succeeding first. | Registration might be required to depend on admitted build computation; "may" does not state whether unconditional registration is forbidden. |
"A caller needing a fixed output roster supplies that roster as an invocation requirement: never infer that a conditional branch which omitted `require` nevertheless checked the intended artifact." | Invocation requirement is mandatory for a fixed roster; omission of `require` in a branch never satisfies roster membership. | Fixed roster might also be satisfiable by other means not stated here; sentence only forbids inference from omitted `require`. |
"Use ordinary ownership for delegation, with one narrow executor-owned completion record that cannot be forged, reset, or satisfied by dropping the source value:" | Delegation uses ordinary ownership everywhere except the singled completion record with the stated properties. | Ordinary ownership might apply only to delegation edges while obligations use a different ownership story not fully stated in this sentence. |
"Ordinary filesystem operations supply immutable captured bytes; publication does not require the retired BuildOutput writer category." | Any publication path must use immutable captured bytes via ordinary filesystem operations without BuildOutput writers. | Immutable bytes are one allowed source; whether mutable staging paths can feed `complete` after capture is not stated in this sentence alone. |
"Internal scratch writes may occur before failure." | Scratch writes before failure are permitted and do not constitute successful product publication. | "May occur" could mean optional behavior rather than permission for implementations that choose to write scratch. |
"Ordinary packages can publish companion files without this application-mode declaration." | Non-application packages may publish companion files without `artifact_only`. | Companion files might still require other unstated mechanisms; sentence only negates need for application-mode declaration. |
"Ordinary checked Build policy may optionally narrow the consumer-approved ceiling for the selected product; omission inherits it, explicit empty permits none in the restricted category, and widening rejects." | Only narrowing, inheriting, empty-none, and reject-on-widen are defined; other policy shapes are unstated. | "Optionally narrow" might permit Build policy to replace rather than narrow when omission does not inherit consumer ceiling. |
"The same rules govern assumptions separately from checked reach and actual authority," | Assumption checks are independent of reach and authority checks but use parallel rule structure. | "Separately" might mean assumptions are evaluated in a different phase whose ordering relative to reach checks is not stated. |
"Normally warn; acquire and execute nothing." | Unused import without dependency request must produce a warning and must not acquire or execute. | "Normally" could allow suppression or upgrade to error in configurations not described in this table row. |
"Helper is deliberately given a wider input | May read that input; no false confidentiality claim." | Wider delegated input authorizes read; compiler must not claim confidentiality that delegation does not provide. | Wider input might authorize only specific paths within the input; "may read that input" could mean the whole grant or only what the helper accesses. |
"One Build argument with scoped operations is the baseline: two arguments do not by themselves enforce dependency or authority separation." | API design must prefer one Build argument; two arguments are insufficient alone for separation guarantees. | Two arguments might still be used if additional unstated enforcement exists; sentence only denies sufficiency "by themselves". |

## Terms used without definition in this chapter

- execution contexts
- backing
- provider selections
- input map
- checked identities
- product identities
- execution profile
- delegated immutable view
- logical metadata profile
- negative lookups
- directory membership
- writer custody
- live activation identities
- library-supplied changeset
- replay receipt
- observable effects
- conflict/failure contract
- immutable observation closure
- persistent result reuse / persistent scratch cache
- continuation
- restricted build requests
- `omega.lock`
- restricted-build acceptance
- actual grants
- root/object scope
- sibling helpers
- admitted protocols
- captured read-only views
- live backing
- snapshot isolation
- snapshot-result reuse
- mode flag
- root builder
- generated-source rule / product checking stratum
- build helpers
- role/target-bound handoff
- `RequiredOutput`
- `Build::require_artifact`
- logical output name
- activation-bound obligation
- logical path rules (staging)
- invocation requirement
- output roster
- executor-owned completion record
- `Pending` / `Completed` / `Failed`
- output receipt
- sticky build failure
- open writer
- sealed file
- staging occurrence
- BuildOutput writer category
- immutable blob
- scratch paths
- output-set manifest
- result-set commit
- `Build::artifact_only`
- workspace/package (project roles)
- companion artifacts
- target/evaluation context
- realization target
- target-keyed provider rows
- ProgramEntry
- mandatory roots
- generic provenance machinery
- verified component descriptions
- Terminal evidence owners
- consumer-approved ceiling
- per-target selected closure
- `Build::accept_component_assumption`
- type reflection
- build-only alias
- verified-description consumer
- resumable activation
- atomic dependency registration
- product-selection frontier
- `Build::await_provider`
- static slot
- deferred import
- product operand
- restricted entry reference
- build-only dependency
- build-tool revision
- input commitment
- watch-list-based reuse
- Cow overlay
- activation custody
- artifact-only intent
- topology verdict
- BuildInputs / BuildSource (retired names)
- with_dependency_inputs
- global incoming-map agreement rule
- build-tool-package route
- fixed-output network exception
- action DSL
- virtual operating system

CHAPTER COMPLETE
