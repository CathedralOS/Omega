# Target slots and program entry

Source: `wiki/spec/build/entry_roots.md` lines 1–212 (read).

## Rules

| rule id | the exact sentence, quoted | the heading it is under | what must be checked, in plain words | when it must hold | lane |
| --- | --- | --- | --- | --- | --- |
| ENTRY_ROOTS1 | "[Target selection](configuration.md) fixes one profile per child activation." | Target slots and program entry | Each child activation must have exactly one target profile fixed by target selection | at entry | 9 |
| ENTRY_ROOTS2 | "Its closed typed slots define what an installable artifact must bind;" | Target slots and program entry | Installable artifacts must bind exactly what the closed typed slots require | not stated | 9 |
| ENTRY_ROOTS3 | "source does not discover an entry by name." | Target slots and program entry | Source must not discover program entry by ambient or lexical name lookup | not stated | 9 |
| ENTRY_ROOTS4 | "Direction, lifecycle, cardinality, and indexing are independent axes." | Slots | Slot validation must treat direction, lifecycle, cardinality, and indexing as independent constraints | not stated | 9 |
| ENTRY_ROOTS5 | "Program entry, reset/interrupt vectors, callbacks, and providers share the binding model without becoming one undifferentiated kind." | Slots | Distinct root kinds must share one binding model while remaining differentiated kinds | not stated | 9 |
| ENTRY_ROOTS6 | "Binding shape comes from the slot, not the trait's current requirement count." | Slots | Binding shape must come from the slot definition, not from how many requirements a trait currently has | not stated | 9 |
| ENTRY_ROOTS7 | "An exact-requirement consumer may use that requirement's contract, not conformance identity or unrelated trait laws." | Slots | Exact-requirement consumers must use the requirement's own contract, not conformance identity or unrelated trait laws | at a call | 3 |
| ENTRY_ROOTS8 | "Every required build-bound slot must have exactly one selection." | Slots | Each required `BuildBound` slot must have exactly one binding selection | not stated | 9 |
| ENTRY_ROOTS9 | "Only rows owned by the selected profile enter the durable child projection." | Slots | Durable child projection must include only rows owned by the selected profile | not stated | 9 |
| ENTRY_ROOTS10 | "Inactive rows must resolve their profile and slot ownership, but do not require checking or emission support for that profile." | Slots | Inactive rows must resolve profile and slot ownership; checking and emission for their profile are not required | not stated | 9 |
| ENTRY_ROOTS11 | "Target-specific entry and calling-plan validation belongs to the selected child;" | Slots | Entry and calling-plan validation must run only for the selected child profile | not stated | 9 |
| ENTRY_ROOTS12 | "[implementation availability](configuration.md#target-recognition-and-implementation-availability) is distinct from recognizing a binding." | Slots | Recognizing a binding must not be conflated with implementation-availability checks | not stated | 9 |
| ENTRY_ROOTS13 | "Unknown profiles or slots, a row misattributing its slot to another profile, duplicate active bindings, or a missing required slot reject with the relevant identity." | Slots | Unknown profile/slot, wrong slot attribution, duplicate active bindings, or missing required slot must reject citing relevant identity | not stated | 9 |
| ENTRY_ROOTS14 | "Package/library products bind no roots." | Slots | Package and library products must bind no entry roots | not stated | 9 |
| ENTRY_ROOTS15 | "An explicitly artifact-only application also binds no executable roots;" | Slots | Explicit artifact-only applications must bind no executable roots | not stated | 9 |
| ENTRY_ROOTS16 | "it must instead satisfy [required artifact completion](scoped_execution.md#staged-products-and-failure)." | Slots | Artifact-only applications must satisfy required artifact completion instead of binding executable roots | not stated | 9 |
| ENTRY_ROOTS17 | "Runtime-installed slots may remain open until installation validates the same shape, portable demands, target supply, authority, and lifecycle." | Slots | `RuntimeInstalled` slots may stay unbound until installation validates shape, portable demands, target supply, authority, and lifecycle | not stated | 9 |
| ENTRY_ROOTS18 | "Resolve the target-owned schema, or the exact consumer-accepted package/schema on a package-aware route." | Slots | Schema resolution must use target-owned schema or exact consumer-accepted package/schema on package-aware routes | not stated | 9 |
| ENTRY_ROOTS19 | "An unrelated same-leaf declaration neither shadows that selection nor creates a second candidate." | Slots | Unrelated same-leaf declarations must neither shadow resolved schema nor become alternate candidates | not stated | 9 |
| ENTRY_ROOTS20 | "The caller's package and a declaration's leaf spelling cannot choose the schema." | Slots | Caller package and declaration leaf spelling must not determine schema selection | not stated | 9 |
| ENTRY_ROOTS21 | "Selecting an alternate package contract requires the existing explicit accepted binding;" | Slots | Alternate package contract selection requires an existing explicit accepted binding | not stated | 9 |
| ENTRY_ROOTS22 | "loading it is not selection." | Slots | Loading a package or contract must not count as slot/schema selection | not stated | 9 |
| ENTRY_ROOTS23 | "Neither selection invokes application code." | Source selection is not invocation | Build slot/entry/provider selections must not invoke application code | not stated | 9 |
| ENTRY_ROOTS24 | "The Build operations themselves follow the ordinary static-machine argument rules;" | Source selection is not invocation | Build bind/select operations must follow ordinary static-machine argument rules | at a call | 10 |
| ENTRY_ROOTS25 | "they have no receiver-syntax exemption." | Source selection is not invocation | Build operations must not get receiver-syntax exemptions | at a call | 10 |
| ENTRY_ROOTS26 | "The first argument explicitly supplies the current activation's mutable Build value;" | Source selection is not invocation | Root `Build::bind` must take explicit mutable Build value as first argument from current activation | at a call | 9 |
| ENTRY_ROOTS27 | "The remaining operands select the slot and entry rather than invoking either." | Source selection is not invocation | Slot and entry operands must perform selection only, not invocation | at a call | 9 |
| ENTRY_ROOTS28 | "There is no `roots` receiver facet or implicit Build argument." | Source selection is not invocation | No `roots` receiver facet or implicit Build argument may be used for root binding | not stated | 9 |
| ENTRY_ROOTS29 | "The entry selection identifies an exact static machine." | Source selection is not invocation | Entry selection must identify one exact static machine declaration | not stated | 10 |
| ENTRY_ROOTS30 | "Build selects that declaration, not a data instance or an allocation request." | Source selection is not invocation | Entry selection must target the static machine declaration, not instances or allocation requests | not stated | 9 |
| ENTRY_ROOTS31 | "An explicitly installed entry contract determines its invocation arguments and the arrival bridge must establish each one from actual backing." | Source selection is not invocation | Installed entry contracts fix invocation arguments; the arrival bridge must establish each from actual backing | at entry | 3 |
| ENTRY_ROOTS32 | "A mutable parameter alone does not ask the bridge to construct its referent." | Source selection is not invocation | Mutable parameter types must not imply bridge construction of referents | at entry | 2 |
| ENTRY_ROOTS33 | "The provider selection names a provider implementation; its ordinary satisfiers determine the derived plan." | Source selection is not invocation | Provider selection names an implementation; derived plan must come from ordinary satisfiers | not stated | 9 |
| ENTRY_ROOTS34 | "Irreducible native backing uses the separate `Build::bind<Requirement>(builder, target, locator)` overload under [foreign bindings](foreign_bindings.md#build-owned-native-binding), not a source `via` clause or a per-symbol forwarding declaration." | Source selection is not invocation | Irreducible native backing must use `Build::bind<Requirement>` overload, not `via` or per-symbol forwarding | not stated | 9 |
| ENTRY_ROOTS35 | "Both forms record product intent rather than invoking selected code or granting authority." | Source selection is not invocation | Root binding forms must record product intent only, not invoke code or grant authority | not stated | 9 |
| ENTRY_ROOTS36 | "Only these designated toolchain operands resolve in the product context." | Source selection is not invocation | Only designated toolchain operands may resolve in product build context | not stated | 9 |
| ENTRY_ROOTS37 | "They do not require a host import of the selected implementation, and host `use` exposure cannot redirect them." | Source selection is not invocation | Product-context toolchain operands must not require host import; host `use` must not redirect them | not stated | 9 |
| ENTRY_ROOTS38 | "Ordinary same-named operations receive no exception." | Source selection is not invocation | Ordinary same-named operations must not gain special resolution exceptions | not stated | 9 |
| ENTRY_ROOTS39 | "There is no special `main`, data-owned entry machine, uniquely visible export, entry field, or ambient static discovery." | Source selection is not invocation | Compiler must not admit special `main`, data-owned entry machines, unique exports, entry fields, or ambient static entry discovery | not stated | 9 |
| ENTRY_ROOTS40 | "A test harness name override carries no slot, signature, calling-plan, or storage authority." | Source selection is not invocation | Test harness name overrides must not confer slot, signature, calling-plan, or storage authority | not stated | 9 |
| ENTRY_ROOTS41 | "Their runner roots are separate from application target slots;" | Source selection is not invocation | Build-time test runner roots must be separate from application target slots | not stated | 9 |
| ENTRY_ROOTS42 | "registration does not install them in the application or make their checked bodies trusted boundary declarations." | Source selection is not invocation | Test root registration must not install roots in the application or promote test bodies to trusted boundary declarations | not stated | 9 |
| ENTRY_ROOTS43 | "A native test harness instead binds an ordinary executable entry normally." | Source selection is not invocation | Native test harnesses must bind ordinary executable entries like other applications | not stated | 9 |
| ENTRY_ROOTS44 | "An entry has no special storage or automatic context-construction mode." | Entry shape and arrival bridge | Entries must not use special storage or automatic context-construction modes | not stated | 9 |
| ENTRY_ROOTS45 | "`NoneOrProvisionedZii` and signature-scanned mutable-context provisioning are retired, not an optional compatibility path." | Entry shape and arrival bridge | Retired entry storage modes must be rejected, not offered as compatibility | not stated | 9 |
| ENTRY_ROOTS46 | "The installed contract identifies exact argument occurrences separately from physical arrival arguments;" | Entry shape and arrival bridge | Installed contracts must separate exact argument occurrences from physical arrival arguments | not stated | 3 |
| ENTRY_ROOTS47 | "no receiver flag, machine name or data attachment participates." | Entry shape and arrival bridge | Receiver flags, machine names, and data attachments must not participate in installed-argument identity | not stated | 10 |
| ENTRY_ROOTS48 | "no physical arrival parameters are exposed to source unless its explicit installed contract declares and establishes them." | Entry shape and arrival bridge | Physical arrival parameters must not appear in source unless the installed contract declares and establishes them | not stated | 3 |
| ENTRY_ROOTS49 | "The platform calling plan validates the generated physical shell; the target's entry shape validates the selected source signature." | Entry shape and arrival bridge | Physical shell must pass platform calling plan; selected source signature must pass entry shape validation | not stated | 3 |
| ENTRY_ROOTS50 | "Resolve the slot first, then validate source shape, then the applicable physical/semantic and storage calling plans." | Entry shape and arrival bridge | Validation order must be slot resolution, then source shape, then applicable calling plans | not stated | 9 |
| ENTRY_ROOTS51 | "Keep those facts joined through component-progress/provider projection and their backend/storage consumers." | Entry shape and arrival bridge | Slot, shape, and plan facts must stay joined through projection and backend/storage consumers | not stated | SHARED |
| ENTRY_ROOTS52 | "The exact target adapter validates them, installs scoped providers, establishes semantic arrival, and supplies only schema-declared source arguments." | Entry shape and arrival bridge | Target adapter must validate physical values, install scoped providers, establish semantic arrival, and supply only schema-declared source arguments | at entry | 9 |
| ENTRY_ROOTS53 | "Its result map defines both pre-handoff rejection and normal-return physical outcomes." | Entry shape and arrival bridge | Result map must define pre-handoff rejection and normal-return physical outcomes | at normal return | 3 |
| ENTRY_ROOTS54 | "The compiler does not invent meanings for platform handles." | Entry shape and arrival bridge | Compiler must not assign ad hoc semantics to platform handles | not stated | SHARED |
| ENTRY_ROOTS55 | "The combined bridge derives crash, reach, write, work, stack/state, introduction, provisioning, and provenance contracts and composes them with the application closure." | Entry shape and arrival bridge | Combined bridge must derive listed contracts and compose them with the application closure | not stated | 3 |
| ENTRY_ROOTS56 | "Ordinary stack locals use ordinary construction, lifetime and cleanup, with their complete activation demand checked by WCSU against actual stack supply." | Entry shape and arrival bridge | Stack locals at entry use ordinary construction/lifetime/cleanup; WCSU must check full activation demand against stack supply | at entry | 6 |
| ENTRY_ROOTS57 | "ZII is an initialization rule, not a special entry allocation mechanism." | Entry shape and arrival bridge | ZII must apply as ordinary initialization, not as entry-specific allocation | not stated | 0 |
| ENTRY_ROOTS58 | "Static backing does not make mutable state ambient or freely shareable; an exclusive owner may lend it explicitly." | Entry shape and arrival bridge | Static backing must not imply ambient mutable state; sharing requires explicit lend from exclusive owner | not stated | 2 |
| ENTRY_ROOTS59 | "the contract must establish backing, occurrence identity, qualifications and authority before invocation." | Entry shape and arrival bridge | Installed-entry provisioning contracts must establish backing, occurrence identity, qualifications, and authority before invocation | at a call | 8 |
| ENTRY_ROOTS60 | "Build selection does not execute the call or manufacture runtime capabilities." | Entry shape and arrival bridge | Build selection must not execute entry calls or create runtime capabilities | not stated | 9 |
| ENTRY_ROOTS61 | "Heap-backed state remains ordinary allocation/library use, not another entry mechanism." | Entry shape and arrival bridge | Heap state at entry must use ordinary allocation/library rules | not stated | 6 |
| ENTRY_ROOTS62 | "A parameter not provided by the installed contract rejects;" | Entry shape and arrival bridge | Parameters absent from the installed contract must cause rejection | not stated | 3 |
| ENTRY_ROOTS63 | "its type or mutable-reference shape supplies no default object." | Entry shape and arrival bridge | Parameter type or mutable-reference shape must not synthesize default objects | not stated | 2 |
| ENTRY_ROOTS64 | "Non-ZII state needs ordinary explicit construction." | Entry shape and arrival bridge | Non-ZII state at entry must be explicitly constructed under ordinary rules | at the operation | 0 |
| ENTRY_ROOTS65 | "Erased values obey the normal [binding rules](../proofs/contracts.md#explicit-erased-bindings), not an entry-only field-synthesis path." | Entry shape and arrival bridge | Erased entry values must follow explicit erased binding rules, not entry-only synthesis | not stated | 8 |
| ENTRY_ROOTS66 | "Boundary services remain exact provider slots demanded by calls, never provisioned fields on an application record." | Entry shape and arrival bridge | Boundary services must be provider slots from calls, not fields provisioned on an application record | not stated | 3 |
| ENTRY_ROOTS67 | "Storage used for application state or the active stack cannot also be forwarded whole; retain the partition and pass only the exact disjoint residual." | Entry shape and arrival bridge | Application state or active stack storage must not be forwarded whole; only exact disjoint residuals may pass | at a call | 2 |
| ENTRY_ROOTS68 | "Generated bridge code remains subject to portable demand checking." | Entry shape and arrival bridge | Generated arrival bridge code must satisfy portable demand checking | not stated | 3 |
| ENTRY_ROOTS69 | "A freestanding schema may explicitly expose image and initial-storage roots; hosted source sees neither by default." | Entry shape and arrival bridge | Freestanding schemas may expose image/initial-storage roots explicitly; hosted source must not see them by default | not stated | 9 |
| ENTRY_ROOTS70 | "Removing automatic context allocation does not remove the target ABI startup bridge or weaken its custody obligations." | Entry shape and arrival bridge | Absence of automatic context allocation must not remove ABI startup bridge or weaken custody obligations | not stated | 9 |
| ENTRY_ROOTS71 | "Ordinary owners perform cleanup on normal return;" | Entry shape and arrival bridge | Ordinary owners must perform cleanup on normal return from entry | at normal return | 2 |
| ENTRY_ROOTS72 | "explicitly installed arguments retain the caller/bridge's declared handback obligations." | Entry shape and arrival bridge | Installed arguments must retain declared caller/bridge handback obligations | at normal return | 2 |
| ENTRY_ROOTS73 | "A crash retains its crash frontier;" | Entry shape and arrival bridge | Crashes must retain the crash frontier contract | not stated | 3 |
| ENTRY_ROOTS74 | "a canonical [process exit](../language/process_exit.md) instead retains the distinct external terminal outcome and abandonment contract." | Entry shape and arrival bridge | Canonical process exit must retain external terminal outcome and abandonment contract distinct from crash | not stated | 3 |
| ENTRY_ROOTS75 | "Returning from the entry does not establish that other activations were settled:" | Entry shape and arrival bridge | Entry return must not prove other activations were settled | at normal return | 3 |
| ENTRY_ROOTS76 | "the root/runtime contract must settle or legally transfer outstanding task and resource custody before physical completion." | Entry shape and arrival bridge | Root/runtime contract must settle or legally transfer task and resource custody before physical completion | at normal return | 2 |
| ENTRY_ROOTS77 | "The result map cannot conceal abandonment behind the same exit syscall." | Entry shape and arrival bridge | Result map must not hide abandonment behind a normal exit syscall | at normal return | 3 |
| ENTRY_ROOTS78 | "Other roots, tasks, and handlers obtain access only through explicit ordinary capability transfer, borrowing, or synchronized sharing." | Entry shape and arrival bridge | Access for other roots, tasks, and handlers requires explicit transfer, borrow, or synchronized sharing | not stated | 2 |
| ENTRY_ROOTS79 | "Hosted writable-image placement and freestanding storage partitioning must preserve the same occurrence, root lineage, backing, and initial exclusive borrow." | Entry shape and arrival bridge | Writable-image placement and storage partitioning must preserve occurrence, lineage, backing, and initial exclusive borrow | not stated | 2 |
| ENTRY_ROOTS80 | "Knowing an argument's size never creates a new physical root." | Entry shape and arrival bridge | Argument size knowledge must not mint new physical roots | not stated | 2 |
| ENTRY_ROOTS81 | "Each demanded static provider slot retains the exact source call occurrence, closed requirement application, schema, target, and selected plan digest." | Entry shape and arrival bridge | Static provider slots must retain exact call occurrence, requirement application, schema, target, and plan digest | not stated | 3 |
| ENTRY_ROOTS82 | "Missing, incompatible, ambiguous, or unauthorized supply rejects compilation;" | Entry shape and arrival bridge | Missing, incompatible, ambiguous, or unauthorized provider supply must reject compilation | not stated | 3 |
| ENTRY_ROOTS83 | "no erased-field fallback or late source lookup is permitted." | Entry shape and arrival bridge | Erased-field fallback and late source lookup for provider supply must be forbidden | not stated | 3 |
| ENTRY_ROOTS84 | "For selected Fused slots, Terminal replay independently joins the call demand and plan before erasing dispatch." | Entry shape and arrival bridge | Fused slots require Terminal replay to join call demand and plan before erasing dispatch | not stated | 3 |
| ENTRY_ROOTS85 | "Independent slots retain internal era identity without adding a source context field." | Entry shape and arrival bridge | Independent slots must retain internal era identity without a source context field | not stated | 9 |
| ENTRY_ROOTS86 | "Calling the same machine ordinarily supplies every argument explicitly and gains no provisioning authority." | Entry shape and arrival bridge | Ordinary calls to the same machine must supply all arguments explicitly and grant no provisioning authority | at a call | 3 |
| ENTRY_ROOTS87 | "Provider demands follow its checked call closure, not a constructor attached to a data type." | Entry shape and arrival bridge | Provider demands must follow the checked call closure, not data-type constructors | not stated | 3 |
| ENTRY_ROOTS88 | "Automatic entry-context code is not supported source semantics." | Entry shape and arrival bridge | Automatic entry-context code must be rejected as unsupported source semantics | not stated | 9 |
| ENTRY_ROOTS89 | "Selection is distinct from runtime permission and from an optional machine annotation." | Authority, identity, and resources | Build selection must not be equated with runtime permission or optional machine annotations | not stated | 9 |
| ENTRY_ROOTS90 | "For installation or an explicitly requested deployment-compatibility check, the root's application/deployment manifest or installed contract supplies the authoritative complete permission ceiling, and admission proves:" | Authority, identity, and resources | Installation or deployment-compatibility checks must use manifest or installed contract as permission ceiling and prove the stated subset relation | not stated | 8 |
| ENTRY_ROOTS91 | "transitive runtime demand ⊆ manifest/installation permission ceiling ⊆ target supply" | Authority, identity, and resources | Admission must prove transitive runtime demand within manifest/installation ceiling within target supply | not stated | 8 |
| ENTRY_ROOTS92 | "Omitting a checked machine's `reaches` requests inference; it neither supplies nor removes manifest permission." | Authority, identity, and resources | Omitted `reaches` on a checked machine requests inference and must not add or remove manifest permission by itself | not stated | 3 |
| ENTRY_ROOTS93 | "A declared service ceiling grants no runtime authority." | Authority, identity, and resources | Declared service ceilings must not confer runtime authority | not stated | 3 |
| ENTRY_ROOTS94 | "Omission from the manifest's complete set denies permission;" | Authority, identity, and resources | Manifest complete-set omission must deny permission | not stated | 8 |
| ENTRY_ROOTS95 | "absence of a provider selection alone does not." | Authority, identity, and resources | Missing provider selection alone must not deny manifest permission | not stated | 9 |
| ENTRY_ROOTS96 | "Rejection preserves provenance to the dependency/declaration/selection introducing excess demand, including when only artifact evidence is available." | Authority, identity, and resources | Permission rejections must cite provenance of excess demand to dependency, declaration, or selection | not stated | SHARED |
| ENTRY_ROOTS97 | "Physical contracts bind exact target-contract source bytes with domain-separated SHA-256 and retain the target role." | Authority, identity, and resources | Physical contracts must bind exact source bytes with domain-separated SHA-256 and retain target role | not stated | 8 |
| ENTRY_ROOTS98 | "Compact source/report fingerprints are not identity." | Authority, identity, and resources | Compact fingerprints must not be used as contract identity | not stated | SHARED |
| ENTRY_ROOTS99 | "Source custody, accepted package/schema, contract identities, and calling plans remain separate checks." | Authority, identity, and resources | Source custody, package/schema acceptance, contract identity, and calling plans must be checked separately | not stated | 8 |
| ENTRY_ROOTS100 | "Standalone UEFI compatibility requires the byte-exact bundled contract;" | Authority, identity, and resources | Standalone UEFI compatibility must require byte-exact bundled contract | not stated | 9 |
| ENTRY_ROOTS101 | "package-aware UEFI requires an exact consumer-accepted ordinary-package binding, not origin or location inference." | Authority, identity, and resources | Package-aware UEFI must use exact consumer-accepted ordinary-package binding, not origin/location inference | not stated | 9 |
| ENTRY_ROOTS102 | "only statically enumerable installed environment-to-program parameter occurrences may introduce domain-authorized fresh roots." | Authority, identity, and resources | Only statically enumerable installed `EnvironmentToProgram` parameter occurrences may introduce domain-authorized fresh roots | at entry | 2 |
| ENTRY_ROOTS103 | "Ordinary calls require existing lineage." | Authority, identity, and resources | Ordinary calls must require existing root lineage for introduced resources | at a call | 2 |
| ENTRY_ROOTS104 | "Installation evidence is not a second source authorization mechanism." | Authority, identity, and resources | Installation evidence must not serve as a parallel source authorization path | not stated | 8 |
| ENTRY_ROOTS105 | "[WCSU](../resources/storage.md#stack-demand-and-backing) is checked against target StackPlan supply." | Authority, identity, and resources | WCSU stack demand must be checked against target `StackPlan` supply | not stated | 6 |
| ENTRY_ROOTS106 | "Ordinary build files do not choose stack size;" | Authority, identity, and resources | Build files must not select stack size | not stated | 9 |
| ENTRY_ROOTS107 | "a target-supply override is deployment policy." | Authority, identity, and resources | Stack size override must be treated as deployment policy via target supply, not build source | not stated | 9 |
| ENTRY_ROOTS108 | "Insufficient fixed supply reports the responsible call path and requires a program change, not a nonexistent build knob." | Authority, identity, and resources | Insufficient fixed stack supply must report responsible call path and require program change, not a build-file knob | not stated | 6 |
| ENTRY_ROOTS109 | "Identity and schema | Exact target-owned declaration and normalized schema." | Slots | Each slot's identity and schema binding must be exact target-owned declaration with normalized schema | not stated | 9 |
| ENTRY_ROOTS110 | "Direction | `EnvironmentToProgram` external root or `ProgramToProvider` outbound service." | Slots | Slot direction must be one of the two listed kinds | not stated | 9 |
| ENTRY_ROOTS111 | "Binding shape | `ExactRequirement(requirement)`, `CompleteConformance(trait)`, or `EntryMachine(entry_shape)`." | Slots | Binding shape must be one of the three listed forms from the slot | not stated | 9 |
| ENTRY_ROOTS112 | "Lifecycle | `BuildBound` or `RuntimeInstalled`." | Slots | Slot lifecycle must be `BuildBound` or `RuntimeInstalled` | not stated | 9 |
| ENTRY_ROOTS113 | "Occurrences | Cardinality and required, optional, and reserved indices." | Slots | Slot occurrences must respect declared cardinality and required, optional, and reserved indices | not stated | 9 |
| ENTRY_ROOTS114 | "Installation authority | Authority needed to publish the bound occurrences." | Slots | Publishing bound occurrences must meet slot installation authority | not stated | 8 |
| ENTRY_ROOTS115 | "The root-binding form is `Build::bind(builder, Target::Slot, Product::entry)`." | Source selection is not invocation | Root binding must use the stated `Build::bind` form | at a call | 9 |
| ENTRY_ROOTS116 | "ordinary helpers may forward that value." | Source selection is not invocation | Helpers may forward the explicit Build value per ordinary forwarding rules | at a call | 9 |
| ENTRY_ROOTS117 | "Templates may write ordinary bindings." | Source selection is not invocation | Templates may emit only ordinary bindings, not special entry discovery | not stated | 9 |
| ENTRY_ROOTS118 | "[Build-time test roots](testing.md#entry-contracts-and-ordinary-calls) reuse the ordinary storage and explicit installed-argument rules below." | Source selection is not invocation | Build-time test roots must obey ordinary storage and explicit installed-argument rules from this chapter | not stated | 9 |
| ENTRY_ROOTS119 | "The environment supplies physical values under its requirement." | Entry shape and arrival bridge | Environment must supply physical values only under its stated requirement | at entry | 9 |
| ENTRY_ROOTS120 | "Application state has the same storage choices as other state:" | Entry shape and arrival bridge | Application state at entry must use the same storage choices as non-entry state | not stated | SHARED |
| ENTRY_ROOTS121 | "Ordinary static data uses its existing placement, lifetime and access rules." | Entry shape and arrival bridge | Static application data must follow existing placement, lifetime, and access rules | not stated | 2 |
| ENTRY_ROOTS122 | "An explicitly installed entry call may receive values or borrows, as with UEFI handoff." | Entry shape and arrival bridge | Explicitly installed entry calls may receive values or borrows only under installed-contract provisioning rules | at a call | 2 |
| ENTRY_ROOTS123 | "This is provisioning rather than a third storage class:" | Entry shape and arrival bridge | Installed-argument delivery must be modeled as provisioning, not a third storage class | not stated | 2 |

## Ambiguous sentences

| Sentence | Reading A | Reading B |
| --- | --- | --- |
| "Its closed typed slots define what an installable artifact must bind;" | Every installable artifact must bind all and only what slots require before install succeeds. | Slots define minimum bindings; other bindings may exist outside this chapter's slot table. |
| "Inactive rows must resolve their profile and slot ownership, but do not require checking or emission support for that profile." | Inactive rows still need ownership resolution errors when misconfigured. | Inactive rows may be ignored entirely after ownership resolution with no further diagnostics. |
| "Runtime-installed slots may remain open until installation validates the same shape, portable demands, target supply, authority, and lifecycle." | Build may succeed with open runtime-installed slots; installation must close them later. | Open runtime-installed slots are allowed only through an explicit installation phase named elsewhere. |
| "loading it is not selection." | Loading a package never satisfies or changes slot binding selection. | Loading may be required after selection but cannot substitute for an explicit bind operation. |
| "ordinary helpers may forward that value." | Any helper in the forwarding chain may pass the Build reference if types match. | Only helpers enumerated under product-reference selection rules may forward Build. |
| "The provider selection names a provider implementation; its ordinary satisfiers determine the derived plan." | Plan is fixed entirely at build time from satisfiers of the named implementation. | Named implementation selects a candidate; plan may still depend on later target-specific validation. |
| "Only these designated toolchain operands resolve in the product context." | Only operands listed or cross-referenced in this chapter get product-context resolution. | "Designated" means operands syntactically marked in `Build::bind`; others never resolve in product context. |
| "[Product-reference selection] defines lexical visibility, restricted helper delegation, exact source/target identity, and rejection of generated-source or unfinished-layout cycles." | Those checks apply to all product-reference selections including entry binds. | Product-reference rules apply only to non-root selections unless the linked section says otherwise. |
| "The platform calling plan validates the generated physical shell; the target's entry shape validates the selected source signature." | Two separate validations must both pass; order is fixed by ENTRY_ROOTS50. | Physical shell validation may be deferred until after source signature when slot is runtime-installed. |
| "Keep those facts joined through component-progress/provider projection and their backend/storage consumers." | Compiler must carry joined facts through named pipeline stages without dropping links. | "Joined" is a consistency requirement on emitted artifacts, not a source-check ordering constraint. |
| "The combined bridge derives crash, reach, write, work, stack/state, introduction, provisioning, and provenance contracts and composes them with the application closure." | Bridge lowering must materialize all listed contract kinds before composing with closure. | Some listed contracts may be derived only at realization if they are target-neutral in Checked Psi. |
| "ZII is an initialization rule, not a special entry allocation mechanism." | Entry paths must not use ZII to justify extra stack or heap allocation. | ZII at entry obeys the same rules as elsewhere; "not special" forbids only entry-specific ZII syntax. |
| "Build selection does not execute the call or manufacture runtime capabilities." | No runtime capabilities may be implied by bind rows alone. | Bind rows may still emit static data or symbols that become capabilities after installation. |
| "A freestanding schema may explicitly expose image and initial-storage roots; hosted source sees neither by default." | Hosted programs cannot reference image/initial-storage roots unless a schema exposes them. | Hosted source may reference roots only through generated bridge code, not direct source fields. |
| "Returning from the entry does not establish that other activations were settled:" | Compiler must not infer activation settlement from entry return type or control flow alone. | Runtime contract must still be checked; static proof of settlement may be required separately. |
| "the root/runtime contract must settle or legally transfer outstanding task and resource custody before physical completion." | Static checking must prove settlement or legal transfer before modeling physical completion. | Obligation applies at runtime; compiler only checks that contract declares settlement or transfer paths. |
| "For selected Fused slots, Terminal replay independently joins the call demand and plan before erasing dispatch." | "Selected Fused" is a build-time slot property; replay must run for all such slots. | Only slots explicitly marked Fused in target configuration trigger independent Terminal replay. |
| "Independent slots retain internal era identity without adding a source context field." | Era identity is internal to compiler artifacts; source must not declare context fields for it. | Era identity may surface in diagnostics but must not become a user-visible struct field. |
| "Omitting a checked machine's `reaches` requests inference; it neither supplies nor removes manifest permission." | Inference may add reaches facts for checking but cannot change manifest permission set. | Omission triggers inference that affects only compile-time reach graphs, not installation admission. |
| "absence of a provider selection alone does not." | Missing provider bind does not by itself fail manifest permission check. | Permission denial requires manifest omission, not missing optional provider slot selection. |
| "Installation evidence is not a second source authorization mechanism." | Installation proofs cannot authorize source operations that lack lineage in the program. | Installation evidence may satisfy runtime admission without duplicating compile-time `Build` authorization. |
| "a target-supply override is deployment policy." | Stack size changes only via target/deployment configuration, never source literals. | Build may read deployment policy files that are not ordinary `build.omg` but still count as target-supply. |
| "Insufficient fixed supply reports the responsible call path and requires a program change, not a nonexistent build knob." | Diagnostic must point to call path; no suggestion to change build file stack settings. | Program change is required; whether reducing activation depth or changing types counts as program change is not split here. |

## Terms used without definition in this chapter

- target selection / child activation / profile
- closed typed slots
- installable artifact
- external-root admission
- `EnvironmentToProgram` / `ProgramToProvider`
- `ExactRequirement` / `CompleteConformance` / `EntryMachine`
- `BuildBound` / `RuntimeInstalled`
- cardinality / reserved indices
- installation authority
- durable child projection
- inactive rows
- implementation availability
- package/library products
- artifact-only application
- required artifact completion
- portable demands / target supply
- entry-slot-schema-resolved-by-bare-name
- target-owned schema / normalized schema
- package-aware route
- consumer-accepted package/schema
- explicit accepted binding
- `Build::bind` / mutable Build value
- `Target::Slot` / `Product::entry`
- roots receiver facet
- static machine / data instance
- installed entry contract
- arrival bridge
- provider selection / derived plan
- irreducible native backing / locator
- foreign bindings / `via` clause
- product intent
- designated toolchain operands
- product context
- host import / host `use`
- product-reference selection
- restricted helper delegation
- unfinished-layout cycles
- data-owned entry machine
- uniquely visible export
- test harness name override
- Build-time test roots / runner roots
- trusted boundary declarations
- native test harness
- entry shape
- physical and semantic arrival requirements
- boundary schema / physical contract package
- generated arrival bridge / physical result map
- `ordinary-entry-storage`
- `NoneOrProvisionedZii`
- signature-scanned mutable-context provisioning
- physical arrival arguments / argument occurrences
- `ProgramEntry` / `WindowsProcessEntry` / `ProgramStorageEntry`
- `WindowsX86_64CallingPolicy` / `WindowsX86_64Application`
- platform calling plan
- component-progress/provider projection
- backend/storage consumers
- target adapter / scoped providers
- semantic arrival
- platform handles
- application closure
- WCSU / activation demand / StackPlan
- ZII
- UEFI handoff
- provisioning
- explicit erased bindings
- boundary services
- portable demand checking
- image and initial-storage roots
- target ABI startup bridge
- custody obligations
- handback obligations
- crash frontier
- process exit / abandonment contract
- physical completion
- capability transfer
- writable-image placement
- root lineage
- initial exclusive borrow
- physical root
- static provider slot
- closed requirement application
- plan digest
- Fused slots / Terminal replay
- Independent slots / era identity
- source context field
- provisioning authority
- deployment-compatibility check
- application/deployment manifest
- permission ceiling / transitive runtime demand
- `reaches` / service ceiling
- physical contracts / domain-separated SHA-256
- compact source/report fingerprints
- source custody
- standalone UEFI / package-aware UEFI
- bundled contract / ordinary-package binding
- installed content / installed introduction schemas
- domain-authorized fresh roots
- existing lineage
- installation evidence
- deployment policy
- build knob

CHAPTER COMPLETE
