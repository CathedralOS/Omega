# Service permission and terminal authority

Source: `wiki/spec/build/permissions.md` lines 1–258 (read).

## Rules

| rule id | the exact sentence, quoted | the heading it is under | what must be checked, in plain words | when it must hold | lane |
| --- | --- | --- | --- | --- | --- |
| PERMISSIONS1 | "None of these axes substitutes for another." | Service permission and terminal authority | Service reach, authority values, provider trust, receiving policy, and mechanism classification must each be checked on their own merits; one must not stand in for another. | not stated | SHARED |
| PERMISSIONS2 | "A boundary requirement contributes only its explicitly declared service row, not the name of its containing provider interface." | Service permission and terminal authority | A boundary requirement's service contribution must be only its declared service row, not inferred from the provider interface name. | not stated | 9 |
| PERMISSIONS3 | "Core-critical and root-registered services are always surfaced by package audit policy." | Service permission and terminal authority | Core-critical and root-registered services must always appear in package audit surfacing. | not stated | 9 |
| PERMISSIONS4 | "That policy neither replaces physical-mechanism classification nor grants receiver permission." | Service permission and terminal authority | Package audit policy must not replace mechanism classification or count as receiver permission grant. | not stated | 9 |
| PERMISSIONS5 | "An empty abstract row cannot hide a selected mechanism's filesystem or hardware authority; classification and admitted trust remain independently visible." | Service permission and terminal authority | An empty abstract service row must not conceal filesystem or hardware authority; classification and admitted trust stay visible separately. | not stated | 9 |
| PERMISSIONS6 | "This authority-class policy does not replace memory-safety guarantees or confine which objects may be touched." | Service permission and terminal authority | Authority-class receiving policy must not be treated as memory-safety or object-confinement. | not stated | SHARED |
| PERMISSIONS7 | "Ordinary compilation and retained-Terminal lowering produce artifacts, not permission to install or execute them in another environment." | Artifact production versus receiver admission | Ordinary compile and retained-Terminal lowering must output artifacts without implying install or execute permission elsewhere. | not stated | 9 |
| PERMISSIONS8 | "They require no receiving policy." | Artifact production versus receiver admission | Ordinary production must not require a receiving policy input. | not stated | 9 |
| PERMISSIONS9 | "Absence of that input means no receiver-admission claim, not an empty deny-all policy and not implicit acceptance." | Artifact production versus receiver admission | Missing receiving policy must mean no admission claim, not deny-all default and not implicit accept. | not stated | 9 |
| PERMISSIONS10 | "Do not add a mandatory policy file, build declaration, or CLI switch merely to emit a binary." | Artifact production versus receiver admission | The toolchain must not require policy file, build declaration, or CLI switch only to emit a binary. | not stated | 9 |
| PERMISSIONS11 | "The compiler still checks source contracts, project acceptance, selected-provider custody, lowering, and all requested guarantees." | Artifact production versus receiver admission | Production must still verify source contracts, project acceptance, provider custody, lowering, and requested guarantees. | not stated | SHARED |
| PERMISSIONS12 | "It retains the mechanism and authority evidence needed for those claims." | Artifact production versus receiver admission | Production must retain mechanism and authority evidence for its claims. | not stated | 9 |
| PERMISSIONS13 | "An explicitly requested deployment compatibility check applies the supplied policy and rejects incompatibility;" | Artifact production versus receiver admission | When deployment compatibility is explicitly requested, supplied policy must be applied and incompatibility rejected. | not stated | 9 |
| PERMISSIONS14 | "ordinary production must not silently invent such a request." | Artifact production versus receiver admission | Ordinary production must not implicitly trigger deployment compatibility checking. | not stated | 9 |
| PERMISSIONS15 | "Neither a binary nor its build script can authorize itself, select the receiver's policy, or enforce rules on its host." | Artifact production versus receiver admission | Binaries and build scripts must not be treated as self-authorizing or as selecting or enforcing receiver policy on the host. | not stated | 9 |
| PERMISSIONS16 | "production grants no host permission." | Artifact production versus receiver admission | Combining production with host execution must not grant host permission from production alone. | not stated | 9 |
| PERMISSIONS17 | "Physical exclusions consume the classified selected-mechanism closure below; missing classification is failure, not evidence of absence." | Artifact production versus receiver admission | Physical exclusions must use classified selected-mechanism closure; missing classification must fail, not prove absence. | not stated | 9 |
| PERMISSIONS18 | "A class exclusion is a producer-requested guarantee, not receiver permission, and grants no authority." | Artifact production versus receiver admission | Physical class exclusions must be producer guarantees only, not receiver permission or authority grants. | not stated | 9 |
| PERMISSIONS19 | "It uses the same classification semantics without requiring a receiving-policy acceptance table." | Artifact production versus receiver admission | Physical exclusions must use shared classification semantics without requiring a receiving-policy acceptance table. | not stated | 9 |
| PERMISSIONS20 | "An empty physical class set cannot satisfy an abstract service exclusion if that service is invoked." | Artifact production versus receiver admission | Empty exercised physical classes must not satisfy an abstract service exclusion when that service is invoked. | not stated | 9 |
| PERMISSIONS21 | "For receiver admission, every demanded terminal leaf must satisfy `exercised classes subset-of permitted classes`." | Two-axis containment | Receiver admission must require every demanded terminal leaf's exercised classes ⊆ permitted classes. | not stated | 9 |
| PERMISSIONS22 | "Every admitted leaf needs exactly one explicit classification, including an explicit empty set." | Two-axis containment | Each admitted leaf must have exactly one explicit classification row, including explicit empty. | not stated | 9 |
| PERMISSIONS23 | "The policy may be partial over the operating system's coordinate universe, but it must be complete for the admitted demand." | Two-axis containment | Receiving policy must cover every leaf in the admitted demand even if partial elsewhere. | not stated | 9 |
| PERMISSIONS24 | "Unknown, missing, duplicate, cyclic, string-only, or substituted leaves reject." | Two-axis containment | Those leaf defects must cause rejection. | not stated | 9 |
| PERMISSIONS25 | "Names, filenames, aliases, package roles, reviewer judgments, and risk labels are not authority identities." | Two-axis containment | Those strings must not be used as authority identities. | not stated | 9 |
| PERMISSIONS26 | "Provider context/schema cannot narrow physical behavior." | Two-axis containment | Provider context or schema must not narrow classified physical behavior. | not stated | 9 |
| PERMISSIONS27 | "Only meaningful coordinates belong to each role; its discriminant participates in identity." | Two-axis containment | Mechanism identity must include only meaningful coordinates per role and the role discriminant. | not stated | 9 |
| PERMISSIONS28 | "A foreign contract uses its strong canonical boundary-entry-plan commitment, not a provider report fingerprint." | Two-axis containment | Foreign mechanism identity must use boundary-entry-plan commitment, not provider report fingerprint. | not stated | 9 |
| PERMISSIONS29 | "A syscall's checked argument contract is distinct from that calling-plan commitment: it accounts for every runtime value permitted by the carriers, or the exact retained constants, ranges, handle provenance, and other constraints justifying a narrower union." | Two-axis containment | Syscall mechanism keys must use checked argument contract separate from calling-plan commitment with the stated accounting. | not stated | 9 |
| PERMISSIONS30 | "Syscall number zero is an ordinary valid coordinate when the target permits it." | Two-axis containment | Syscall zero must be accepted as a valid coordinate when the target allows it. | not stated | 9 |
| PERMISSIONS31 | "Module-local IDs and readable service names cannot stand in for that evidence." | Two-axis containment | Module-local IDs and readable service names must not replace mechanism identity evidence. | not stated | 9 |
| PERMISSIONS32 | "Each row classifies the union of authority reachable through all permitted argument values." | Two-axis containment | Classification rows must cover authority union over all permitted argument values. | not stated | 9 |
| PERMISSIONS33 | "Narrowing requires exact checked constraint evidence in the mechanism key." | Two-axis containment | Narrower classification must require exact checked constraint evidence in the mechanism key. | not stated | 9 |
| PERMISSIONS34 | "An empty row means no class in this vocabulary is exercised;" | Two-axis containment | Explicit empty classification must mean no terminal class in the closed vocabulary is exercised. | not stated | 9 |
| PERMISSIONS35 | "it does not mean pure, nonblocking, interference-free, trusted, custody-safe, or object-confined." | Two-axis containment | Empty classification must not be read as pure, nonblocking, interference-free, trusted, custody-safe, or object-confined. | not stated | 9 |
| PERMISSIONS36 | "Historical broad `Filesystem`/`Process` risk summaries grant no terminal permission." | Two-axis containment | Broad historical risk summaries must not grant terminal permission. | not stated | 9 |
| PERMISSIONS37 | "`Linux + Windows` would mean reach union, not provider choice." | Two-axis containment | Combined platform realization must be classified as reach union, not as choosing one provider. | not stated | 9 |
| PERMISSIONS38 | "Permissions are explicit consumer input, not findings that approve themselves." | Consumer acceptance and replay | Permission tables must come from explicit consumer input, not self-approving findings. | not stated | 9 |
| PERMISSIONS39 | "A supplied service row joins its exact schema and requirement to one method." | Consumer acceptance and replay | Each supplied service permission row must bind exact schema and requirement to one method. | not stated | 9 |
| PERMISSIONS40 | "final selected closure admission owns demand-completeness." | Consumer acceptance and replay | Final selected-closure admission must enforce demand-completeness for permissions. | not stated | 9 |
| PERMISSIONS41 | "A candidate carries no permission merely because its service is familiar." | Consumer acceptance and replay | Familiar service names must not imply permission without explicit rows. | not stated | 9 |
| PERMISSIONS42 | "Broad risk acceptance cannot authorize a specific terminal class." | Consumer acceptance and replay | Broad risk acceptance must not authorize a named terminal class. | not stated | 9 |
| PERMISSIONS43 | "they do not additionally require an ecosystem's receiving permission set." | Consumer acceptance and replay | Direct compile and Terminal re-entry must not require ecosystem receiving permission set beyond project acceptance comparison. | not stated | 9 |
| PERMISSIONS44 | "Missing, changed, or duplicate required package rows reject." | Consumer acceptance and replay | Missing, changed, or duplicate required package permission rows must reject. | not stated | 9 |
| PERMISSIONS45 | "Explicit receiver admission separately checks the offered artifact against its independently selected policy;" | Consumer acceptance and replay | Receiver admission must check the artifact against independently selected policy apart from package rows. | not stated | 9 |
| PERMISSIONS46 | "Project acceptance must never be mirrored into that policy to make admission pass." | Consumer acceptance and replay | Project acceptance must not be copied into receiving policy to force admission pass. | not stated | 9 |
| PERMISSIONS47 | "Accepted-permission transport derives solely from obligations accepted by fresh root-policy replay." | Consumer acceptance and replay | Transported accepted permissions must come only from obligations accepted in fresh root-policy replay. | not stated | 9 |
| PERMISSIONS48 | "A constructed policy, reconstructed proposal, or Terminal module cannot grant package admission." | Consumer acceptance and replay | Constructed policy, reconstructed proposal, or Terminal module must not grant package admission. | not stated | 9 |
| PERMISSIONS49 | "When receiver admission is requested, its exact policy version and strong commitment survive admission replay against the native artifact." | Consumer acceptance and replay | Requested receiver admission must retain exact policy version and strong commitment through admission replay on the native artifact. | not stated | 9 |
| PERMISSIONS50 | "Ordinary artifact evidence does not fabricate that verdict." | Consumer acceptance and replay | Ordinary artifact evidence must not substitute for admission replay verdict. | not stated | 9 |
| PERMISSIONS51 | "Classification alone grants no execution, installation, or invocation authority." | Consumer acceptance and replay | Mechanism classification alone must not grant execution, installation, or invocation authority. | not stated | 9 |
| PERMISSIONS52 | "The six filesystem facets distinguish operations, not confined objects." | Portable filesystem control and lifecycle authority | Filesystem facet classification must distinguish operations, not object confinement. | not stated | 9 |
| PERMISSIONS53 | "Raw integer descriptors establish no object-confinement guarantee." | Portable filesystem control and lifecycle authority | Raw integer file descriptors must not be treated as object-confinement guarantees. | not stated | 9 |
| PERMISSIONS54 | "requirement names identify examples, not classification keys." | Portable filesystem control and lifecycle authority | Requirement names in examples must not be used as mechanism classification keys. | not stated | 9 |
| PERMISSIONS55 | "Metadata query of the stored link target, not content read or directory enumeration merely because it returns a path." | Portable filesystem control and lifecycle authority | `read_link` must classify as metadata query of link target, not content read or directory enumeration solely from returning a path. | not stated | 9 |
| PERMISSIONS56 | "Explicit empty for durability-only completion of existing changes." | Portable filesystem control and lifecycle authority | `sync` and `sync_data` under stated contracts must get explicit empty classification. | not stated | 9 |
| PERMISSIONS57 | "Explicit empty for ordinary locking/unlocking; there is no coordination class." | Portable filesystem control and lifecycle authority | Ordinary file lock/unlock mechanisms must get explicit empty; no coordination class without new policy work. | not stated | 9 |
| PERMISSIONS58 | "Explicit empty for accepted position, handle-representation, and alias operations." | Portable filesystem control and lifecycle authority | Listed seek/handle/alias operations under accepted contracts must get explicit empty classification. | not stated | 9 |
| PERMISSIONS59 | "Explicit empty for recognized error-state observation, independent of declaring service." | Portable filesystem control and lifecycle authority | Error-state observation mechanisms must get explicit empty classification. | not stated | 9 |
| PERMISSIONS60 | "Explicit empty only with an established ordinary-release contract, not automatically for generic raw-handle close." | Portable filesystem control and lifecycle authority | Close-family operations get explicit empty only with established ordinary-release contract, not automatically for generic raw-handle close. | not stated | 9 |
| PERMISSIONS61 | "Adding a coordination class requires a concrete policy customer, a new accepted version/commitment, and revalidation." | Portable filesystem control and lifecycle authority | New coordination terminal class must require policy customer, new version/commitment, and revalidation. | not stated | 9 |
| PERMISSIONS62 | "source placement inside a raw filesystem service cannot force a broader mechanism into filesystem-only classes." | Portable filesystem control and lifecycle authority | Filesystem service source placement must not force broader mechanisms into filesystem-only classes. | not stated | 9 |
| PERMISSIONS63 | "Requesting or arming namespace mutation exercises namespace-mutation authority." | Requested mutation versus incidental completion | Requesting or arming namespace mutation must classify as exercising namespace-mutation authority. | at the operation | 9 |
| PERMISSIONS64 | "Ordinary release does not additionally exercise it merely because release completes another actor's already-requested deletion." | Requested mutation versus incidental completion | Ordinary release must not additionally count as namespace-mutation authority when completing another actor's pending deletion. | at the operation | 9 |
| PERMISSIONS65 | "Likewise, durability-only completion is not a new logical write request." | Requested mutation versus incidental completion | Durability-only completion must not be classified as a new logical content write request. | at the operation | 9 |
| PERMISSIONS66 | "Preserve Windows shared-delete behavior; removing `FILE_SHARE_DELETE` to obtain an empty row changes the protocol." | Requested mutation versus incidental completion | Classification must not remove `FILE_SHARE_DELETE` merely to obtain an empty row. | not stated | 9 |
| PERMISSIONS67 | "Do not invent a portable POSIX delete-on-close open flag as its counterpart." | Requested mutation versus incidental completion | Must not invent a portable POSIX delete-on-close open flag as counterpart behavior change. | not stated | 9 |
| PERMISSIONS68 | "An ordinary-empty release must exclude deletion attached to the released handle itself, whether at acquisition or by a later disposition-changing operation, and all other behavior outside ordinary release." | Requested mutation versus incidental completion | Ordinary-empty release classification requires excluding handle-attached deletion and non-ordinary release behavior. | not stated | 9 |
| PERMISSIONS69 | "filesystem placement does not narrow that mechanism." | Requested mutation versus incidental completion | Placing a generic Win32 closer in filesystem services must not narrow that mechanism's classification. | not stated | 9 |
| PERMISSIONS70 | "A completely classified broad mechanism can fail filesystem-only containment." | Requested mutation versus incidental completion | Fully classified broad mechanisms may fail filesystem-only containment checks. | not stated | 9 |
| PERMISSIONS71 | "An unknown/incompletely classified mechanism instead fails classification." | Requested mutation versus incidental completion | Unknown or incomplete mechanisms must fail classification, not containment-only. | not stated | 9 |
| PERMISSIONS72 | "Both refuse realization, not source requirement formation." | Requested mutation versus incidental completion | Containment and classification failures must block realization, not source requirement formation. | not stated | 9 |
| PERMISSIONS73 | "Demand-completeness never permits fabricating a broad union for an unsupported leaf." | Requested mutation versus incidental completion | Demand-completeness must not allow fabricating a broad union for an unsupported leaf. | not stated | 9 |
| PERMISSIONS74 | "Use one retained occurrence, one exact mechanism-plus-checked-contract key, and one classification." | Bounded occurrence-specific release proof | Bounded release proof must use one occurrence, one mechanism-plus-contract key, and one classification. | not stated | 9 |
| PERMISSIONS75 | "Constrained and unconstrained uses have distinct keys, not competing rows." | Bounded occurrence-specific release proof | Constrained versus unconstrained uses must use distinct mechanism keys, not competing rows. | not stated | 9 |
| PERMISSIONS76 | "An unsupported unconstrained operation does not block an independently proved constrained use." | Bounded occurrence-specific release proof | Failed unconstrained proof must not block a separately proved constrained use. | not stated | 9 |
| PERMISSIONS77 | "Successful acquisition under the exact object/argument contract, excluding attached deferred deletion." | Bounded occurrence-specific release proof | Release proof must establish successful acquisition under exact contract without attached deferred deletion. | not stated | 8 |
| PERMISSIONS78 | "Handle identity through aliases and control flow, excluding unproved escape, storage, duplication, substitution, or foreign intervention." | Bounded occurrence-specific release proof | Proof must establish handle identity through aliases and control flow without unproved escape paths. | not stated | 8 |
| PERMISSIONS79 | "Preservation by every intervening handle-taking call." | Bounded occurrence-specific release proof | Every intervening handle-taking call must preserve handle identity per proof. | at a call | 8 |
| PERMISSIONS80 | "An empty or query-only authority set is not a preservation contract." | Bounded occurrence-specific release proof | Empty or query-only authority classification must not count as handle preservation contract. | not stated | 9 |
| PERMISSIONS81 | "One release on applicable returning paths, no subsequent use, and allowed release behavior under explicit external-environment assumptions." | Bounded occurrence-specific release proof | Proof must show single release on returning paths, no subsequent use, and allowed release under stated external assumptions. | not stated | 8 |
| PERMISSIONS82 | "Constants, a digest of an asserted proof, and method-name exceptions do not satisfy these obligations." | Bounded occurrence-specific release proof | Those shortcuts must not satisfy bounded release proof obligations. | not stated | 8 |
| PERMISSIONS83 | "Bind the constraint identity to the derivation and exact occurrence." | Bounded occurrence-specific release proof | Constraint identity must bind to derivation and exact occurrence. | not stated | 8 |
| PERMISSIONS84 | "Changed arguments, flow, selected calls, or contracts require re-establishment." | Bounded occurrence-specific release proof | Changes to arguments, flow, calls, or contracts must require re-proving narrowed classification. | not stated | 8 |
| PERMISSIONS85 | "Failure falls back only to a justified unconstrained classification; otherwise reject, never retain an unsupported narrowed row." | Bounded occurrence-specific release proof | Failed narrow proof must fall back to justified unconstrained classification or reject; unsupported narrow rows must not be kept. | not stated | 9 |
| PERMISSIONS86 | "access zero, `OPEN_EXISTING`, and `BACKUP_SEMANTICS` without `DELETE_ON_CLOSE` are useful inputs, not proof of object kind or complete lifecycle." | Bounded occurrence-specific release proof | Those open flags/constants must not alone prove object kind or complete lifecycle. | not stated | 8 |
| PERMISSIONS87 | "Check successful acquisition before query or close and preserve read/write/delete sharing." | Bounded occurrence-specific release proof | Open/query/close proofs must verify acquisition before query or close and preserve sharing modes. | not stated | 8 |
| PERMISSIONS88 | "An `i64 in FileHandle` qualification alone cannot prove lifecycle: copies may survive release and refer to recycled handles." | Bounded occurrence-specific release proof | `i64 in FileHandle` alone must not prove lifecycle for narrowing. | not stated | 8 |
| PERMISSIONS89 | "Failed acquisition, escape, alias reuse, second close, handle-changing calls, and stale/substituted proofs must prevent narrowing." | Bounded occurrence-specific release proof | Listed failure patterns must block narrowed classification. | not stated | 9 |
| PERMISSIONS90 | "deferred deletion attached to that handle does not." | Bounded occurrence-specific release proof | Deferred deletion attached to the handle must not be compatible with empty release classification. | not stated | 9 |
| PERMISSIONS91 | "Checked instructions retain separate service identities, not a catch-all `Privileged` or lowercase effect category:" | Privileged services | Checked instructions must use separate service identities, not a catch-all Privileged effect category. | not stated | 9 |
| PERMISSIONS92 | "These two are the whole vocabulary: `source/library/core/assembly.omg` declares `MachineControl` and `PortIo` and no third identity." | Privileged services | Privileged service vocabulary must be exactly MachineControl and PortIo per assembly.omg. | not stated | 9 |
| PERMISSIONS93 | "Interrupt-table publication is not a service of its own -- it is an authority requirement (`AsmAuthorityRequirement::IdtControl`) discharged by the `Build.privileged_services.interrupt_table` flag described below." | Privileged services | IDT publication must be IdtControl authority discharged by `interrupt_table` build flag, not a third service identity. | not stated | 9 |
| PERMISSIONS94 | "The evaluated `Build.freestanding` selection is machine-owner admission and covers every defined class, including the mediated ones." | Privileged services | Evaluated `Build.freestanding` must admit all defined privileged classes including mediated ones. | not stated | 9 |
| PERMISSIONS95 | "A hosted build begins with no class and widens per class through the authored `Build.privileged_services` flags:" | Privileged services | Hosted builds must start with no privileged class and widen only via authored `Build.privileged_services` flags. | not stated | 9 |
| PERMISSIONS96 | "`port_io` admits port I/O without machine control," | Privileged services | `port_io` flag must admit port I/O only, without machine control. | not stated | 9 |
| PERMISSIONS97 | "`interrupt_table` admits interrupt-table publication without port I/O." | Privileged services | `interrupt_table` flag must admit interrupt-table publication without port I/O. | not stated | 9 |
| PERMISSIONS98 | "machine-owner authority has no granular grant — it stays `freestanding`-only." | Privileged services | Machine-owner authority must be only via `freestanding`, not granular flags. | not stated | 9 |
| PERMISSIONS99 | "A flag widens admission only for its own class; it never claims machine ownership," | Privileged services | Each privileged_services flag must widen only its class and must not claim machine ownership. | not stated | 9 |
| PERMISSIONS100 | "listing its service does not establish ownership of the machine." | Privileged services | Listing MachineControl service must not establish machine ownership. | not stated | 9 |
| PERMISSIONS101 | "Each exact instruction contract independently requires its machine or consumer-defined publication authority, or no authority." | Privileged services | Each instruction contract must independently require matching publication authority or none. | not stated | 9 |
| PERMISSIONS102 | "Direct emission and checked helpers retain the same recursive reach rules." | Privileged services | Direct emission and checked helpers must use the same recursive reach rules as other privileged paths. | not stated | 9 |
| PERMISSIONS103 | "Locally interpreting or lowering portable Psi does not admit supplied native bytes." | Privileged services | Local Psi interpret or lower must not admit supplied native bytes. | not stated | 9 |
| PERMISSIONS104 | "Executable installation requires exact admitted artifact and placed-byte provenance." | Privileged services | Executable installation checks must require exact admitted artifact and placed-byte provenance. | not stated | 9 |
| PERMISSIONS105 | "Admitted call/state exits do not make opaque in-process code memory-safe or its executable inventory complete." | Privileged services | Admitted call or state exits must not imply memory-safety or complete executable inventory for opaque in-process code. | not stated | SHARED |
| PERMISSIONS106 | "Exact service identity, normalized schema commitment, and requirement identity map to permitted terminal classes." | Two-axis containment | Service permission table keys must map service identity, schema commitment, and requirement identity to permitted terminal classes. | not stated | 9 |
| PERMISSIONS107 | "Exact role-tagged normalized mechanism and checked/admitted contract map to exercised terminal classes." | Two-axis containment | Mechanism classification table must map role-tagged mechanism and contract to exercised terminal classes. | not stated | 9 |
| PERMISSIONS108 | "Package review retains each permission as its own blocking obligation with exact service/requirement source custody." | Consumer acceptance and replay | Package review must retain each permission as a blocking obligation with exact service/requirement source custody. | not stated | 9 |
| PERMISSIONS109 | "Direct compilation and retained-Terminal re-entry compare checked package rows with project acceptance;" | Consumer acceptance and replay | Direct compile and Terminal re-entry must compare checked package permission rows to project acceptance. | not stated | 9 |
| PERMISSIONS110 | "Re-entry binds the retained production report to the accepted root's dependency closure, consumed sources, selected build machine, invocation-local evaluation usage, build observations, and target." | Consumer acceptance and replay | Terminal re-entry must bind production report to listed accepted-root subjects. | not stated | 9 |
| PERMISSIONS111 | "The closed classes are filesystem content read/write, metadata query/mutation, directory enumeration, namespace mutation, process input, process output, process termination, machine control, port I/O, interrupt control, interrupt entry, and root-memory access." | Two-axis containment | Terminal authority vocabulary must be exactly the listed closed classes. | not stated | 9 |
| PERMISSIONS112 | "At an ecosystem admission boundary, receiving policy separately checks the dangerous authority exercised by selected physical mechanisms." | Service permission and terminal authority | Ecosystem receiver admission must separately check dangerous authority of selected physical mechanisms against receiving policy. | not stated | 9 |
| PERMISSIONS113 | "Exact service reach and demanded operation remain visible." | Two-axis containment | Empty or narrowed classification must still leave exact service reach and demanded operation visible in evidence. | not stated | 9 |
| PERMISSIONS114 | "unrelated receiving-policy rows are allowed." | Consumer acceptance and replay | Receiving-policy rows unrelated to the artifact must not by themselves fail admission. | not stated | 9 |
| PERMISSIONS115 | "Locks can deny others access and aliases can share state without becoming content or metadata mutation." | Portable filesystem control and lifecycle authority | Lock and alias mechanisms must not be classified as content or metadata mutation solely from denying access or sharing state. | not stated | 9 |
| PERMISSIONS116 | "The six facets do not require six core traits." | Portable filesystem control and lifecycle authority | Six filesystem facets must not force six separate core traits in the library surface. | not stated | 9 |
| PERMISSIONS117 | "Separate cross-platform realizations satisfy one portable requirement;" | Two-axis containment | Multiple platform realizations must be allowed to satisfy one portable requirement without conflating provider choice. | not stated | 9 |
| PERMISSIONS118 | "Aggregate sponsor/session peaks remain orchestration custody, not invocation identity." | Consumer acceptance and replay | Sponsor or session peak aggregates must not be treated as invocation identity for permission replay binding. | not stated | 9 |
| PERMISSIONS119 | "The mechanism identity is a closed role-tagged sum:" | Two-axis containment | Every classified mechanism must fall into one of the closed role-tagged sum variants listed in this section. | not stated | 9 |
| PERMISSIONS120 | "consumer-defined publication authority stays receiver-side." | Privileged services | Consumer-defined publication authority must not be granted by build flags; it remains receiver-side. | not stated | 9 |

## Ambiguous sentences

| Sentence | Reading A | Reading B |
| --- | --- | --- |
| "The receiver selects the policy's versioned package, exact dependency closure and concrete configuration independently of the offered program." | The receiver's policy selection is outside the compiler's artifact production path; the compiler only records what the receiver supplies at admission. | The compiler or admission toolchain must verify that policy selection is independent of the offered program's contents. |
| "A partial table may be retained during binding construction; final selected closure admission owns demand-completeness." | Partial permission tables are allowed mid-build but must be complete before final admission closes. | Partial tables may remain in the artifact if final admission never runs; demand-completeness applies only when admission runs. |
| "Exact service reach and demanded operation remain visible." | Empty classification still requires reporting reach and demanded operation in evidence. | Reach and operation stay visible to humans but need not appear in machine-checkable admission tables. |
| "This is attribution policy, not an assumption excluding concurrent deletion." | The compiler attributes authority only to explicit request/arm, without assuming no concurrent deletion. | Concurrent deletion may still be modeled elsewhere; this sentence only limits attribution rules, not concurrency semantics. |
| "Bounded flow proof may reason about the integer without making it linear; general owned opaque handles are not a prerequisite." | Narrowing proofs may use integer reasoning without linear typing of the handle integer. | Any proof that does not use linear handles is automatically invalid unless integer reasoning is present. |
| "External pending deletion completed by ordinary close remains compatible with empty classification;" | Empty release classification is allowed when external deletion completes on ordinary close. | External pending deletion must be proven compatible only when ordinary close completes it; otherwise narrowing fails. |
| "A build supplies authority two ways." | Only `freestanding` and hosted `privileged_services` flags are valid authority supply mechanisms for builds. | Other build configuration fields might also supply authority; this sentence names two without claiming exclusivity. |
| "consumer-defined publication authority stays receiver-side." | Build flags never establish consumer-defined publication authority; only the receiver can. | Build may declare needs for consumer authority but cannot substitute receiver acceptance for those needs. |
| "Source-derived contracts rely on the trusted compiler." | Untrusted manifests cannot replace compiler-checked source contracts for authority. | Source contracts are only as strong as compiler trust; untrusted compilers are out of scope but not ruled here. |

## Terms used without definition in this chapter

- Service reach
- Authority values
- Provider trust
- ecosystem admission boundary
- receiving policy
- physical mechanisms
- boundary requirement
- service row
- provider interface
- core-critical services
- root-registered services
- package audit policy
- physical-mechanism classification
- receiver permission
- admitted trust
- versioned package
- exact dependency closure
- concrete configuration
- receiver-admission evidence
- authority-class policy
- memory-safety guarantees
- retained-Terminal lowering
- receiving policy input
- deployment compatibility check
- selected-provider custody
- mechanism evidence
- authority evidence
- build script
- host execution request
- Build-time authority
- Build-level exclusions
- abstract services
- physical terminal classes
- selected-mechanism closure
- producer-requested guarantee
- receiving-policy acceptance table
- nominal reach
- installed authority review
- selected-provider closure
- Service permission table
- Mechanism classification table
- normalized schema commitment
- requirement identity
- permitted terminal classes
- exercised terminal classes
- demanded terminal leaf
- admitted leaf
- explicit classification
- explicit empty set
- coordinate universe
- admitted demand
- substituted leaves
- authority identities
- provider context/schema
- mechanism identity
- role-tagged sum
- Structural compiler intrinsic
- compiler-checked argument contract
- normalized foreign locator
- admitted contract
- firmware/table identity
- receiver contract
- checked physical-operation catalog entry
- meaningful coordinates
- discriminant
- strong canonical boundary-entry-plan commitment
- provider report fingerprint
- calling-plan commitment
- handle provenance
- terminal permission
- cross-platform realizations
- portable requirement
- consumer input
- binding construction
- demand-completeness
- package review
- blocking obligation
- source custody
- broad risk acceptance
- project acceptance
- receiving permission set
- root-policy replay
- production report
- invocation-local evaluation usage
- build observations
- aggregate sponsor/session peaks
- orchestration custody
- invocation identity
- constructed policy
- reconstructed proposal
- Terminal module
- strong commitment
- admission replay
- native artifact
- filesystem facets
- object-confinement guarantee
- classification keys
- coordination class
- core traits
- raw filesystem service
- filesystem-only classes
- namespace-mutation authority
- durability-only completion
- logical write request
- attribution policy
- FILE_SHARE_DELETE
- delete-on-close open flag
- ordinary-empty release
- ordinary-release contract
- kill-on-close job handle
- filesystem-only containment
- demand-completeness
- unsupported leaf
- retained occurrence
- mechanism-plus-checked-contract key
- constrained use
- unconstrained use
- deferred deletion
- preservation contract
- external-environment assumptions
- unsupported narrowed row
- OPEN_EXISTING
- BACKUP_SEMANTICS
- DELETE_ON_CLOSE
- read/write/delete sharing
- FileHandle
- bounded flow proof
- linear handles
- owned opaque handles
- checked instructions
- Privileged effect category
- MachineControl
- PortIo
- interrupt-table publication
- AsmAuthorityRequirement::IdtControl
- Build.privileged_services
- Build.freestanding
- machine-owner admission
- hosted build
- port_io flag
- interrupt_table flag
- publication authority
- recursive reach rules
- untrusted binary manifest
- portable Psi
- supplied native bytes
- executable installation
- placed-byte provenance
- lower-rooted bootstrap assurance
- hardware control-flow hardening
- admitted call/state exits
- opaque in-process code
- executable inventory

CHAPTER COMPLETE
