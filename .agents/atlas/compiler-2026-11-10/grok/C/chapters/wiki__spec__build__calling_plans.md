# Boundary calling plans

Source: `wiki/spec/build/calling_plans.md` lines 1–168 (read).

## Rules

| rule id | the exact sentence, quoted | the heading it is under | what must be checked, in plain words | when it must hold | lane |
| --- | --- | --- | --- | --- | --- |
| CALLING_PLANS1 | "A boundary-trait requirement selects its calling policy." | Boundary calling plans | Each boundary-trait requirement must have its calling policy determined by that selection mechanism | not stated | 3 |
| CALLING_PLANS2 | "Omega's internal convention is compiler-owned and is not selected through this surface." | Boundary calling plans | Compiler-owned internal convention must not be selectable via the boundary-trait calling-policy surface | not stated | 9 |
| CALLING_PLANS3 | "Terminal Psi retains target-neutral demand;" | Boundary calling plans | Terminal Psi must keep calling demand target-neutral | not stated | SHARED |
| CALLING_PLANS4 | "evaluated target plans travel as realization inputs, not target policy inside the portable module." | Boundary calling plans | Evaluated target plans must be realization inputs only, not embedded target policy in the portable module | not stated | 9 |
| CALLING_PLANS5 | "`Calling<Policy>` takes an explicit named implementation parameter declared `Policy satisfies CallingPolicy`." | Authorship and evaluation | Named policy implementations must be explicit parameters satisfying `CallingPolicy` | at a call | 7 |
| CALLING_PLANS6 | "There is no convention data subject." | Authorship and evaluation | No convention-data subject may participate in policy selection | not stated | 9 |
| CALLING_PLANS7 | "Its compile-time `plan` machine receives a normalized `BoundarySignature` and returns `Accepted(BoundaryEntryPlan)` or `Rejected(CallingPolicyRejection)`." | Authorship and evaluation | Policy `plan` must take normalized `BoundarySignature` and return only accepted entry plan or structured rejection | not stated | 3 |
| CALLING_PLANS8 | "Evaluation is deterministic, terminating, and build-time-admissible." | Authorship and evaluation | Policy evaluation must be deterministic, must terminate, and must be build-time-admissible | not stated | 9 |
| CALLING_PLANS9 | "A rejection identifies the incompatible signature feature at the relationship's source span and creates no accepted contract identity." | Authorship and evaluation | Rejections must cite incompatible signature features at source span and must not mint accepted contract identity | not stated | 3 |
| CALLING_PLANS10 | "An accepted value is still subject to compiler validation and canonicalization." | Authorship and evaluation | Accepted policy output must pass compiler validation and canonicalization | not stated | 3 |
| CALLING_PLANS11 | "The policy implementation declares its machine inside its conformance block:" | Authorship and evaluation | The policy `plan` machine must be declared inside the policy conformance block | not stated | 7 |
| CALLING_PLANS12 | "Neither form uses a reference-row assignment;" | Authorship and evaluation | Policy plan implementations must not use reference-row assignment | not stated | 7 |
| CALLING_PLANS13 | "both follow [inline conformance members](../language/conformances.md#declaration-and-selection)." | Authorship and evaluation | Policy plan members must follow inline conformance member rules | not stated | 7 |
| CALLING_PLANS14 | "Accepting a plan validates the calling description, not the truth of an opaque provider's claims or its authority to execute." | Authorship and evaluation | Acceptance must validate the calling description only, not provider claim truth or execution authority | not stated | 3 |
| CALLING_PLANS15 | "Packages may author policies under the same rules as platform packages." | Authorship and evaluation | User-authored policies must meet the same rules as platform packages | not stated | 9 |
| CALLING_PLANS16 | "Its quantities use `u64`, not `addr`;" | Authorship and evaluation | `std::calling` quantities must use `u64`, not `addr` | not stated | 5 |
| CALLING_PLANS17 | "narrowing to a compiler field requires a checked range conversion." | Authorship and evaluation | Narrowing `u64` to compiler fields must use checked range conversion | at a call | 5 |
| CALLING_PLANS18 | "Policies choose from closed primitives;" | Authorship and evaluation | Policies may only select from the closed primitive vocabulary | not stated | 9 |
| CALLING_PLANS19 | "they cannot emit instructions, supply relocation bytes, inspect private carriers, or bypass validation." | Authorship and evaluation | Policies must not emit instructions, supply relocation bytes, inspect private carriers, or bypass validation | not stated | 9 |
| CALLING_PLANS20 | "Extending the primitive register, placement, control, or machine-state vocabulary requires compiler support, not another policy declaration." | Authorship and evaluation | New primitives in those vocabularies require compiler support; another policy declaration alone must not extend them | not stated | 9 |
| CALLING_PLANS21 | "Boundary service parents contribute service reach." | Authorship and evaluation | Boundary service parent kinds must contribute service reach to plans | not stated | 3 |
| CALLING_PLANS22 | "Ordinary policy parents contribute contract identity without service reach." | Authorship and evaluation | Ordinary policy parents must supply contract identity and must not add service reach | not stated | 3 |
| CALLING_PLANS23 | "each closed application has its own contract." | Authorship and evaluation | Each closed policy application must have distinct contract identity | not stated | 3 |
| CALLING_PLANS24 | "Providers cannot independently choose conventions for the same requirement." | Authorship and evaluation | Providers must not pick calling conventions independently for one requirement | not stated | 9 |
| CALLING_PLANS25 | "Mixed conventions require distinct applications or composed facets." | Authorship and evaluation | Mixed conventions must use separate applications or composed facets | not stated | 3 |
| CALLING_PLANS26 | "Static machine selection and direct invocation do not require runtime function values." | Authorship and evaluation | Static selection and direct invocation must not depend on runtime function values | not stated | 3 |
| CALLING_PLANS27 | "Calling-policy evaluation does not depend on general function-value, environment-capture, or code-address facilities." | Authorship and evaluation | Calling-policy evaluation must not rely on function values, environment capture, or code addresses | not stated | 3 |
| CALLING_PLANS28 | "A boundary machine retains its ordinary declaration and `satisfies` relationship;" | Authorship and evaluation | Boundary machines keep ordinary declaration and `satisfies` edges | not stated | 10 |
| CALLING_PLANS29 | "policy is on the requirement, not an extra modifier on the machine." | Authorship and evaluation | Calling policy attaches to the requirement, not as an extra machine modifier | not stated | 3 |
| CALLING_PLANS30 | "An ordinary call's register/stack ABI does not describe hardware entering an already-live activation." | Plan structure | Register/stack ABI for ordinary calls must not be interpreted as hardware entry into an already-live activation | not stated | 3 |
| CALLING_PLANS31 | "Agreement between the two facets for ordinary calls does not merge their meanings." | Plan structure | Facet agreement for ordinary calls must not collapse separate call vs state meanings | not stated | 3 |
| CALLING_PLANS32 | "One native-parameter identity space contains semantic formals and private callback entries." | Plan structure | Semantic formals and private callback entries share one native-parameter identity space | not stated | 3 |
| CALLING_PLANS33 | "Declaration order fixes native argument position;" | Plan structure | Native argument positions must follow declaration order | not stated | 3 |
| CALLING_PLANS34 | "declared names establish nominal identity under the owning requirement." | Plan structure | Declared names must define nominal identity for parameters under the requirement | not stated | 1 |
| CALLING_PLANS35 | "A multi-register aggregate remains one parameter with several physical locations." | Plan structure | Multi-register aggregates count as one logical parameter with multiple physical sites | not stated | 3 |
| CALLING_PLANS36 | "Whole-parameter and private-field destinations refer to the same identity space." | Plan structure | Whole-parameter and private-field destinations must use the same identity space | not stated | 3 |
| CALLING_PLANS37 | "[Boundary shapes](boundary_shapes.md) defines what the policy may classify." | Shape and placement | Policy classification must stay within boundary shapes the compiler admits | not stated | 3 |
| CALLING_PLANS38 | "every borrowed parameter to preserve its referent;" | Shape and placement | Every borrowed parameter must preserve its referent per structural access | at a call | 2 |
| CALLING_PLANS39 | "an owned value's indirect ABI placement does not create a borrow." | Shape and placement | Owned values with indirect ABI placement must not be treated as borrows | not stated | 2 |
| CALLING_PLANS40 | "[representation selection](opaque_representations.md) closes its exact shape, movement, and lifecycle application before calling-policy evaluation." | Shape and placement | By-value opaque representation must be fixed before calling-policy evaluation runs | not stated | 3 |
| CALLING_PLANS41 | "A policy classifies that application;" | Shape and placement | Policy must classify the closed opaque application, not author it | not stated | 3 |
| CALLING_PLANS42 | "it neither chooses nor reconstructs a private representation." | Shape and placement | Policy must not select or rebuild private opaque representations | not stated | 3 |
| CALLING_PLANS43 | "Reference-only pointees need no by-value application." | Shape and placement | Reference-only pointees must not require by-value representation applications | not stated | 2 |
| CALLING_PLANS44 | "The same validated plan governs outbound argument/result handling and inbound entry/exit machinery." | Validation | Outbound and inbound machinery must share one validated plan | not stated | 3 |
| CALLING_PLANS45 | "every native parameter and result is placed exactly once and compatibly;" | Validation | Each native parameter and result must have exactly one compatible placement | not stated | 3 |
| CALLING_PLANS46 | "stack ranges, alignment, shadow space, and register classes agree;" | Validation | Stack ranges, alignment, shadow space, and register classes must agree | not stated | 3 |
| CALLING_PLANS47 | "ordinary clobbers respect the ABI regime;" | Validation | Ordinary clobbers must respect the active ABI regime | not stated | 3 |
| CALLING_PLANS48 | "saved/restored state covers the state commitment;" | Validation | Saved and restored state must cover the committed machine state | not stated | 10 |
| CALLING_PLANS49 | "entry/exit control is valid for the initial regime;" | Validation | Entry and exit control must be valid for the initial machine regime | not stated | 10 |
| CALLING_PLANS50 | "target and provider applicability match the requirement." | Validation | Target and provider applicability must match the requirement | not stated | 9 |
| CALLING_PLANS51 | "Unordered register/clobber sets normalize before fingerprinting." | Validation | Register and clobber sets must normalize before fingerprinting | not stated | 3 |
| CALLING_PLANS52 | "Equivalent encodings produce the same normalized plan." | Validation | Equivalent encodings must yield identical normalized plans | not stated | 3 |
| CALLING_PLANS53 | "Instructions changing regime require their starting regime and establish the next;" | Validation | Regime-changing instructions must record starting regime and establish the next regime | not stated | 10 |
| CALLING_PLANS54 | "regions on either side retain separate plans rather than one mixed-mode plan." | Validation | Regions before and after regime change must keep separate plans, not a mixed-mode plan | not stated | 3 |
| CALLING_PLANS55 | "An `asm machine` may implement an ordinary machine requirement." | Checked assembly implementations | `asm machine` may satisfy an ordinary boundary machine requirement when checked | not stated | 10 |
| CALLING_PLANS56 | "A selected boundary implementation consumes the requirement's same validated entry plan;" | Checked assembly implementations | Selected assembly boundary implementation must use the requirement's same validated entry plan | not stated | 10 |
| CALLING_PLANS57 | "assembly introduces neither a second calling-policy selection nor an `outputs` interface." | Checked assembly implementations | Assembly must not trigger second policy selection or an `outputs` interface | not stated | 10 |
| CALLING_PLANS58 | "Every result location must be initialized and valid at the checked return, with stack and preservation obligations satisfied." | Checked assembly implementations | All result locations must be initialized and valid at checked return with stack and preservation met | at normal return | 2 |
| CALLING_PLANS59 | "Authorized parameter mutations are independent effects." | Checked assembly implementations | Authorized parameter mutations must be modeled as independent effects | not stated | 2 |
| CALLING_PLANS60 | "Entry register bindings initialize working state; they do not redefine native argument positions." | Checked assembly implementations | Entry register bindings must init working state and must not redefine native argument slots | at entry | 10 |
| CALLING_PLANS61 | "overlapping input and destination locations must preserve all incoming values needed by setup." | Checked assembly implementations | Overlapping input/destination sites must keep incoming values required for setup | at the operation | 10 |
| CALLING_PLANS62 | "Nested assembly CFG edges do not repeat it or create another calling boundary." | Checked assembly implementations | Nested assembly CFG must not duplicate entry contract handling or add another calling boundary | on every edge | 10 |
| CALLING_PLANS63 | "The entry contract relates native pointer lanes to the authorized argument or result storage." | Checked assembly implementations | Entry contract must tie native pointer lanes to authorized argument/result storage | at entry | 2 |
| CALLING_PLANS64 | "Checked copies, offsets and joins retain only justified provenance, extents and permissions." | Checked assembly implementations | Copies, offsets, and joins must keep only justified provenance, extents, and permissions | at the operation | 2 |
| CALLING_PLANS65 | "A policy places a hidden result pointer;" | Checked assembly implementations | Policies may place hidden result pointers in plans | not stated | 3 |
| CALLING_PLANS66 | "it does not create access authority merely by naming its register." | Checked assembly implementations | Naming a register for a hidden result pointer must not grant access authority by itself | not stated | 2 |
| CALLING_PLANS67 | "Explicit clobbers describe writes, while saved-state restoration proves preservation." | Checked assembly implementations | Writes need explicit clobbers; preservation needs saved-state restoration proof | not stated | 10 |
| CALLING_PLANS68 | "The canonical evaluated promise, not policy source, construction order, or a friendly convention name, defines ABI identity." | Identity and selection | ABI identity must come from canonical evaluated promise, not source text, order, or naming | not stated | 3 |
| CALLING_PLANS69 | "The complete public contract has a domain-separated SHA-256 commitment;" | Identity and selection | Full public contract must carry domain-separated SHA-256 commitment | not stated | 8 |
| CALLING_PLANS70 | "compact fingerprints are report coordinates." | Identity and selection | Compact fingerprints serve only as report coordinates | not stated | SHARED |
| CALLING_PLANS71 | "Refactoring policy code while preserving its normalized result preserves the ABI promise." | Identity and selection | Normalized-result-preserving policy refactors must preserve ABI promise | not stated | 3 |
| CALLING_PLANS72 | "Observable placement or state changes do not." | Identity and selection | Observable placement or state changes must change ABI promise | not stated | 3 |
| CALLING_PLANS73 | "The reusable physical-plan identity is weaker than its application identity." | Identity and selection | Physical-plan identity must be weaker than application identity | not stated | 3 |
| CALLING_PLANS74 | "Application identity additionally binds the exact requirement, complete ordered nominal parameter mapping, callback materializations, and any opaque representation applications." | Identity and selection | Application identity must include requirement, parameter mapping, callbacks, and opaque apps | not stated | 3 |
| CALLING_PLANS75 | "Swapping equally shaped parameters cannot preserve application identity merely because the register recipe is unchanged." | Identity and selection | Parameter swaps must change application identity even if register recipe is unchanged | not stated | 3 |
| CALLING_PLANS76 | "A `Build::bind<Requirement>(builder, target, locator)` row retains the requirement's complete signature and calling-policy application;" | Identity and selection | `Build::bind` rows must retain full signature and calling-policy application | not stated | 9 |
| CALLING_PLANS77 | "it needs no source `via` clause or synthetic forwarding machine." | Identity and selection | Bind rows must not require `via` clause or synthetic forwarding machine | not stated | 9 |
| CALLING_PLANS78 | "Literal byte lengths are inferred." | Identity and selection | Literal byte lengths in bind rows must be inferred by the compiler | not stated | 9 |
| CALLING_PLANS79 | "A locator, syscall number, or target label cannot select calling policy." | Identity and selection | Locators, syscall numbers, and target labels must not select calling policy | not stated | 9 |
| CALLING_PLANS80 | "A descendant boundary trait may own a selected inherited requirement's schema while the row retains the declaring requirement's identity." | Identity and selection | Descendant traits may own inherited schema but bind row must keep declaring requirement identity | not stated | 7 |
| CALLING_PLANS81 | "All selected import, vtable, and service-table consumers retain the evaluated plan through layout and emission." | Identity and selection | Import, vtable, and service-table consumers must keep evaluated plan through layout/emission | not stated | 9 |
| CALLING_PLANS82 | "They cannot reselect it from the output target or duplicate its arity in a dispatch recipe." | Identity and selection | Consumers must not reselect policy from output target or fake arity in dispatch recipes | not stated | 9 |
| CALLING_PLANS83 | "Dispatch-only operands are added only where the selected topology requires them." | Identity and selection | Extra dispatch operands may be added only when topology requires them | not stated | 9 |
| CALLING_PLANS84 | "Missing or unrealizable plans reject." | Identity and selection | Missing or unrealizable plans must cause rejection | not stated | 3 |
| CALLING_PLANS85 | "A wider compiler scratch slot cannot redefine a retained foreign scalar type;" | Identity and selection | Scratch slots must not redefine retained foreign scalar types | not stated | 5 |
| CALLING_PLANS86 | "insufficient storage rejects." | Identity and selection | Insufficient storage for retained types must reject | not stated | 3 |
| CALLING_PLANS87 | "The plan is the published promise." | Identity and selection | Evaluated plan is the published ABI promise | not stated | 3 |
| CALLING_PLANS88 | "The final artifact's machine-state evidence establishes that a particular implementation honors it without becoming part of that promise." | Identity and selection | Machine-state evidence must prove honor without joining the published promise | not stated | 8 |
| CALLING_PLANS89 | "A passing plan evaluator does not imply a replayed final state footprint;" | Identity and selection | Successful policy evaluation must not imply final state footprint replay | not stated | 10 |
| CALLING_PLANS90 | "machine-state evidence requires that separately." | Identity and selection | Final state footprint must be required and checked via machine-state evidence separately | not stated | 8 |
| CALLING_PLANS91 | "Selected consumers retain the evaluated plan through realization rather than reselecting it." | Identity and selection | Realization consumers must keep the evaluated plan and must not reselect policy | not stated | 9 |

## Ambiguous sentences

| Sentence | Reading A | Reading B |
| --- | --- | --- |
| "A boundary-trait requirement selects its calling policy." | The named policy on the requirement is the only calling policy for that requirement. | Selection is automatic from the requirement type with no explicit policy parameter when the trait fixes policy. |
| "Omega's internal convention is compiler-owned and is not selected through this surface." | Programs cannot nominate Omega's internal convention via boundary-trait policy parameters. | Internal convention exists but this document does not describe how the compiler applies it elsewhere. |
| "Terminal Psi retains target-neutral demand;" | Terminal Psi artifacts must not embed target-specific calling policy. | Terminal Psi may carry demand that is later specialized; "retains" does not forbid downstream specialization. |
| "Evaluation is deterministic, terminating, and build-time-admissible." | Policy `plan` must finish in build contexts and cannot use runtime-only facilities. | "Build-time-admissible" is the same constraint set as other Build operations named elsewhere; this chapter does not enumerate forbidden facilities. |
| "Accepting a plan validates the calling description, not the truth of an opaque provider's claims or its authority to execute." | Compiler acceptance never checks provider honesty or authority. | Acceptance checks structural validity only; separate mechanisms may still check authority outside calling plans. |
| "Boundary service parents contribute service reach." | Plans from service parents must record service reach facts used later in emission. | Parent kind affects plan metadata; whether reach is enforced at policy evaluation or only at provider selection is not stated here. |
| "Providers cannot independently choose conventions for the same requirement." | Two providers for one requirement must share evaluated calling policy. | Providers cannot override policy but may supply different bodies under the same evaluated plan. |
| "Mixed conventions require distinct applications or composed facets." | One requirement application cannot combine incompatible conventions without facet composition. | "Composed facets" is a required compiler mechanism; this chapter does not state whether facets are syntactic or build-time only. |
| "An ordinary call's register/stack ABI does not describe hardware entering an already-live activation." | State plan facets cover live activation entry; call plan ABI must not subsume that case. | Hardware entry scenarios are out of scope for ordinary call ABI; whether they are rejected or use a different unspecified path is not stated. |
| "every borrowed parameter to preserve its referent;" | Borrowed parameters in boundary calls must meet structural-access referent preservation before policy runs. | Preservation is required only after a plan places borrows; ordering relative to placement is not stated in this sentence. |
| "[representation selection] closes its exact shape, movement, and lifecycle application before calling-policy evaluation." | Compiler must run representation selection strictly before policy evaluation on by-value opaques. | "Before" is logical dependency; same build stage may compute both if order is invisible to policy. |
| "Equivalent encodings produce the same normalized plan." | Normalization must erase encoding differences that do not change semantics. | "Equivalent" is defined only by normalization outcome; whether non-equivalent encodings may normalize to the same plan is not stated. |
| "regions on either side retain separate plans rather than one mixed-mode plan." | Regime boundaries must emit two plans, never a single mixed-mode plan. | Separate plans may be stored in one aggregate structure if each region's semantics stay distinct; "retain separate plans" does not forbid packaging. |
| "Nested assembly CFG edges do not repeat it or create another calling boundary." | Inner assembly CFG must not re-apply the outer entry contract or add a new boundary. | Nested edges skip re-running entry setup only; whether inner exits need separate validation is not stated. |
| "A policy places a hidden result pointer;" | Policies may designate hidden result pointer locations in the validated plan. | Hidden result pointers may exist only in assembly lowering, not in policy output; "places" could mean emission stage only. |
| "Refactoring policy code while preserving its normalized result preserves the ABI promise." | ABI identity keys on normalized plan only. | Source-level refactor is allowed only if normalized output is unchanged; whether observably identical binaries are required is not stated. |
| "The reusable physical-plan identity is weaker than its application identity." | Fingerprints may match at physical-plan level while application identity differs. | "Weaker" is informational for tooling; whether compilers may reuse physical plans across applications is not stated. |
| "A descendant boundary trait may own a selected inherited requirement's schema while the row retains the declaring requirement's identity." | Bind row identity stays on declaring requirement; schema ownership may move to descendant for layout. | "Owns schema" may affect only type checking, not runtime identity retained on the row. |
| "Dispatch-only operands are added only where the selected topology requires them." | Extra dispatch operands are emitted solely when topology demands them. | "Dispatch-only" distinguishes operand kind from ordinary parameters; whether the plan must predeclare them is not stated. |
| "A wider compiler scratch slot cannot redefine a retained foreign scalar type;" | Layout must reject scratch widening that retypes foreign scalars. | Scratch may be wider for storage if semantic type identity is preserved; "redefine" vs wider storage with same logical type is not split here. |
| "machine-state evidence establishes that a particular implementation honors it without becoming part of that promise." | Evidence is separate verification, not part of ABI commitment hash. | Evidence may be required for linking but omitted from portable contract artifacts; which artifacts include evidence is not stated. |
| "A passing plan evaluator does not imply a replayed final state footprint;" | Plan acceptance does not certify final machine-state footprint. | Evaluator success might still require footprint checks elsewhere without implying replay equivalence. |

## Terms used without definition in this chapter

- boundary-trait requirement
- calling policy / `CallingPolicy`
- Omega's internal convention
- Terminal Psi
- target-neutral demand
- realization inputs
- portable module
- `Calling<Policy>`
- convention data subject
- `BoundarySignature`
- `BoundaryEntryPlan` / `CallingPolicyRejection`
- build-time-admissible
- accepted contract identity
- canonicalization (compiler validation)
- inline conformance members
- opaque provider
- platform packages
- `std::calling`
- narrowed compiler field
- closed primitives
- private carriers
- service reach
- contract identity
- closed application
- composed facets
- static machine selection
- direct invocation
- runtime function values
- function-value / environment-capture / code-address facilities
- `CallPlan` / `StatePlan` / `BoundaryEntryPlan` / `BoundaryPlanResult`
- `BoundaryPlanApplication`
- private callback materialization
- shadow space
- already-live activation
- native-parameter identity space
- semantic formals
- multi-register aggregate
- whole-parameter / private-field destinations
- boundary shapes
- structural access
- runtime opaque value
- representation selection
- by-value application
- reference-only pointees
- outbound argument/result handling
- inbound entry/exit machinery
- register classes
- ABI regime
- state commitment
- initial regime
- fingerprinting
- mixed-mode plan
- `asm machine`
- `outputs` interface
- entry register bindings
- nested assembly CFG
- calling boundary
- native pointer lanes
- hidden result pointer
- canonical evaluated promise
- domain-separated SHA-256
- compact fingerprints
- physical-plan identity
- application identity
- register recipe
- `Build::bind`
- locator
- syscall number
- target label
- descendant boundary trait
- import / vtable / service-table consumers
- dispatch recipe
- dispatch-only operands
- published promise
- machine-state evidence
- replayed final state footprint
- realization
- provider selection (linked chapter)
- normalized `BoundarySignature`
- policy source / friendly convention name
- callback destinations / callback materializations
- nominal identity
- ordinary clobbers
- target and provider applicability

CHAPTER COMPLETE
