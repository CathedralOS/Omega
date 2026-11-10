# Machines and contract refinement

Source: `wiki/spec/language/machines.md` lines 1–332 (read).

## Rules

| rule id | the exact sentence, quoted | the heading it is under | what must be checked, in plain words | when it must hold | lane |
| --- | --- | --- | --- | --- | --- |
| MACHINES1 | "Anonymous machine expressions and implicit capture-environment construction are not part of the source language." | Machines and contract refinement | Anonymous machine expressions and implicit capture-environment construction must be rejected in source | not stated | 10 |
| MACHINES2 | "All machines are static declarations independent of data." | Static declarations and explicit state | Machine declarations must be static and independent of data values at runtime | not stated | 10 |
| MACHINES3 | "data types only occur as ordinary signature types or declared trait arguments." | Static declarations and explicit state | Data types may appear only as signature types or declared trait arguments in machine declarations | not stated | 7 |
| MACHINES4 | "There is no conformance data subject or `Self` type binding" | Static declarations and explicit state | Machine trait satisfaction must not use a conformance data subject or `Self` type binding | not stated | 7 |
| MACHINES5 | "There is no special `self` parameter, implicit receiver, bound-method value, or receiver-inserting call syntax." | Static declarations and explicit state | Programs must not use special `self`, implicit receivers, bound-method values, or receiver-inserting call syntax for machines | not stated | 10 |
| MACHINES6 | "Predicate-only `self` in data/default-domain and named-domain predicates ... is not a machine parameter and does not change this rule." | Static declarations and explicit state | Domain predicate `self` must not be treated as a machine parameter | not stated | 5 |
| MACHINES7 | "A data declaration is not a machine owner or namespace." | Static declarations and explicit state | Data declarations must not own or namespace nested machines | not stated | 1 |
| MACHINES8 | "Declaring a machine under a data carrier, such as `machine Counter::increment`, is retired even with an explicit parameter." | Static declarations and explicit state | Data-carrier-qualified machine declarations must be rejected | not stated | 10 |
| MACHINES9 | "Machine qualification identifies an ordinary module, trait requirement, named implementation or lexically visible nested declaration; it never binds code to a data type." | Static declarations and explicit state | Machine qualification must resolve only to module, trait requirement, named implementation, or nested declaration; never to a data type | not stated | 10 |
| MACHINES10 | "Package visibility and exact declaration identity remain independent of the types of explicit arguments." | Static declarations and explicit state | Declaration identity and package visibility must not depend on argument types | not stated | 10 |
| MACHINES11 | "Every argument, including context and capabilities, is ordinary and explicit." | Static declarations and explicit state | All machine arguments including context and capabilities must be ordinary explicit parameters | not stated | 10 |
| MACHINES12 | "Owned, shared, mutable and write-only access retain their normal contracts;" | Static declarations and explicit state | Access modes on machine arguments must use the same ownership, invariant, effect, and proof checks as non-machine code | not stated | 2 |
| MACHINES13 | "a satisfier declares no parameter its requirement lacks." | Static declarations and explicit state | Trait satisfier signatures must not declare parameters absent from the requirement | not stated | 7 |
| MACHINES14 | "A boundary provider is therefore its satisfier machines, with no provider value or private state;" | Static declarations and explicit state | Boundary supply must be satisfier machines only; no separate provider value or private provider state | not stated | 9 |
| MACHINES15 | "state that outlives a call is a value the service returns and the caller passes back." | Static declarations and explicit state | Persistent state across calls must be returned values passed explicitly by the caller, not hidden provider state | across a pause | 2 |
| MACHINES16 | "Nested machines likewise receive explicit arguments on every invocation." | Static declarations and explicit state | Nested machine calls must pass every needed value as explicit arguments on each invocation | at a call | 10 |
| MACHINES17 | "Invocation scratch belongs in ordinary local bindings or local data instances." | Static declarations and explicit state | Temporary invocation data must use ordinary locals or local data instances, not machine-scoped storage | at the operation | 0 |
| MACHINES18 | "Pass values needed by another machine as arguments;" | Static declarations and explicit state | Values needed by another machine must be passed as arguments | at a call | 10 |
| MACHINES19 | "retain data in a record only when that record actually owns the required state or resource lifetime." | Static declarations and explicit state | Context records must be used only when they own the required state or resource lifetime | not stated | 2 |
| MACHINES20 | "This introduces no machine-scoped temporary storage, implicit capture, or new local lifetime rule." | Static declarations and explicit state | Compiler must not introduce machine-scoped temporaries, implicit capture, or new lifetime rules for machines | not stated | 10 |
| MACHINES21 | "Implementations must support the ordinary local-storage and borrow path instead of requiring authors to put temporary buffers in a persistent context record." | Static declarations and explicit state | Compiler must allow ordinary locals and borrows for temporaries; must not require persistent context records for scratch | not stated | 2 |
| MACHINES22 | "Entry state uses ordinary stack or static storage." | Static declarations and explicit state | Entry state must use ordinary stack or static storage unless entry contract supplies arguments | at entry | 9 |
| MACHINES23 | "declaring a mutable context parameter requests no automatic allocation." | Static declarations and explicit state | A mutable context parameter must not trigger automatic allocation | not stated | 2 |
| MACHINES24 | "Retired receiver-based or carrier-qualified machine forms are not accepted." | Static declarations and explicit state | Receiver-based and carrier-qualified machine forms must be rejected | not stated | 10 |
| MACHINES25 | "An implementation must reject retired `self` declarations and expressions rather than silently translate them or enable a compatibility mode." | Static declarations and explicit state | Retired `self` declarations and expressions must be rejected; silent translation and compatibility modes are forbidden | not stated | 10 |
| MACHINES26 | "Header clauses are delimited by the next clause or body, never by semicolons." | Declaration forms | Machine header clause boundaries must follow next clause or body, not semicolons | not stated | 10 |
| MACHINES27 | "Only a bodyless declaration or requirement has a final declaration semicolon." | Declaration forms | Final declaration semicolon is allowed only on bodyless declarations or requirements | not stated | 10 |
| MACHINES28 | "A fixed operator token after `machine`, as in `machine + Vec2::add(...)`, binds expression syntax to the same named machine." | Declaration forms | Operator-token machine declarations must bind expression syntax to the named machine | not stated | 1 |
| MACHINES29 | "This is not a separate executable species or an alternate contract system." | Declaration forms | Operator-bound machines must not be a separate executable species or alternate contract system | not stated | 10 |
| MACHINES30 | "Eligibility comes from its complete contract and context, not separate `async`, `proof`, or `const` machine species." | Declaration forms | Call and use eligibility must come from complete contract and context, not from separate async/proof/const machine species | not stated | 10 |
| MACHINES31 | "Evaluation may erase execution but cannot change meaning." | Declaration forms | Compile-time evaluation may erase execution only without changing semantic meaning | not stated | 8 |
| MACHINES32 | "Logical conditions, mathematical abstractions, and compiler-owned proof term formers are not additional executable supply modes." | Declaration forms | Logical conditions, mathematical abstractions, and compiler proof formers must not be treated as extra executable supply modes | not stated | 8 |
| MACHINES33 | "Every machine application establishes the selected contract's `requires` under the exact argument substitution." | Call preconditions | Each machine application must establish the selected contract's `requires` under exact argument substitution | at a call | 3 |
| MACHINES34 | "This applies equally to named and token-bound calls, runtime execution, compile-time evaluation, and mathematical use." | Call preconditions | Requires establishment applies to named calls, token-bound calls, runtime, compile-time evaluation, and mathematical use | at a call | 3 |
| MACHINES35 | "A mathematical application must establish those premises before forming its result term, not only when importing `ensures`." | Call preconditions | Mathematical applications must establish `requires` before forming the result term, not only when using `ensures` | at a call | 8 |
| MACHINES36 | "An erased or discarded result does not waive the preconditions." | Call preconditions | Discarding or erasing a call result must not waive `requires` establishment | at a call | 3 |
| MACHINES37 | "A total implementation behind a restricted contract does not make that selected contract total over all inputs." | Call preconditions | Call-site contract selection must not become total merely because some implementation is total | at a call | 3 |
| MACHINES38 | "The caller establishes those premises." | Call preconditions | Caller must establish callee `requires` premises | at a call | 3 |
| MACHINES39 | "The implementation establishes its result's validity and guarantees assuming its declared premises;" | Call preconditions | Callee checking may assume established premises when proving result validity and `ensures` | at normal return | 3 |
| MACHINES40 | "unavailable evidence prevents acceptance, but failed search does not establish falsity." | Call preconditions | Missing proof evidence must prevent acceptance; prover search failure must not be treated as falsity | not stated | 8 |
| MACHINES41 | "Recursive proof applications must both establish the callee's substituted premises and satisfy the exact well-founded descent obligation." | Call preconditions | Recursive proof calls must establish substituted premises and well-founded descent | at a call | 8 |
| MACHINES42 | "Neither condition substitutes for the other." | Call preconditions | Premise establishment and descent obligation must both be checked; neither discharges the other | at a call | 8 |
| MACHINES43 | "The settled decision `boundary-operations-are-trait-requirements` retires the standalone carrier-owned boundary-requirement declaration." | Supply | Standalone carrier-owned boundary-requirement declarations must be rejected | not stated | 9 |
| MACHINES44 | "Boundary operations are requirements in boundary traits;" | Supply | Boundary operations must be declared as requirements in boundary traits | not stated | 9 |
| MACHINES45 | "An ordinary machine with a checked `{ ... }` body, including generated/default bodies after generation." | Supply | Checked-body supply must be an ordinary machine with a checked braced body, including generated or default bodies | not stated | 10 |
| MACHINES46 | "A bodyless trait/component machine requirement; a boundary operation belongs to a boundary trait, including a one-member trait when independent selection is required." | Supply | Required-body supply must be bodyless trait or component requirements; boundary operations must sit in boundary traits | not stated | 7 |
| MACHINES47 | "A package-local `Build::bind<Requirement>(builder, target, locator)` row backing an exact boundary requirement under its pinned contract, calling policy, and foreign admission." | Supply | Native provider supply must be package-local `Build::bind` backing an exact boundary requirement with pinned contract, calling policy, and admission | not stated | 9 |
| MACHINES48 | "No forwarding machine or `via` clause is declared." | Supply | Native provider rows must not declare forwarding machines or `via` clauses | not stated | 9 |
| MACHINES49 | "A bodyless `boundary machine` publishing an unproved theorem or external-behavior guarantee for separate owner acceptance." | Supply | Admission-bearing supply must be bodyless `boundary machine` with unproved theorem or external-behavior guarantee for owner acceptance | not stated | 9 |
| MACHINES50 | "A bodyless exact toolchain declaration with an authorized closed-catalog implementation and fixed semantic contract; not selectable by naming or user registration." | Supply | Compiler-supplied primitive supply must be bodyless sealed catalog declaration not selectable by naming or registration | not stated | 9 |
| MACHINES51 | "Checked artifacts distinguish required, checked, compiler-supplied, externally supplied, and admission-bearing behavior explicitly;" | Supply | Artifacts must distinguish supply kinds explicitly | not stated | SHARED |
| MACHINES52 | "a `boundary` Boolean cannot represent those distinctions." | Supply | Supply kind must not be represented by a single `boundary` Boolean | not stated | 9 |
| MACHINES53 | "A claim-free bodyless free machine outside the authorized compiler catalog has no supply mode and rejects." | Supply | Bodyless machines outside the authorized compiler catalog with no claim must be rejected | not stated | 10 |
| MACHINES54 | "The catalog exception is automatic, not a Build provider selection." | Supply | Compiler catalog admission must be automatic, not selected via Build provider rows | not stated | 9 |
| MACHINES55 | "Executable bodies always use braces, never expression-body syntax." | Supply | Machine executable bodies must use braces; expression-body syntax must be rejected | not stated | 10 |
| MACHINES56 | "A bodyless `ensures` is a claim, not a theorem by virtue of its name." | Supply | Bodyless `ensures` must be treated as a claim, not as an established theorem by naming alone | not stated | 8 |
| MACHINES57 | "Accepted theorem claims use the existing boundary surface, not a separate fact declaration." | Supply | Accepted theorem claims must use the boundary surface, not a separate fact declaration form | not stated | 9 |
| MACHINES58 | "The standalone carrier-owned `boundary requirement Carrier::operation(...);` spelling is retired:" | Supply | Carrier-qualified standalone `boundary requirement` spelling must be rejected | not stated | 9 |
| MACHINES59 | "`satisfies` does not attach an implementation to an otherwise bodyless ordinary direct declaration." | Supply | `satisfies` must not supply an implementation for a bodyless ordinary direct declaration | not stated | 7 |
| MACHINES60 | "`satisfies` names an exact trait requirement and inherits its contract for a checked implementation." | Supply | `satisfies` must name an exact trait requirement and inherit its contract for checked implementations | not stated | 7 |
| MACHINES61 | "An irreducible native implementation is supplied by a package-local Build binding from that exact requirement and target to a typed locator." | Supply | Irreducible native implementations must be supplied only by package-local Build binding from exact requirement and target to typed locator | not stated | 9 |
| MACHINES62 | "Source-level `via` clauses and per-symbol bodyless forwarding realizers are retired;" | Supply | `via` clauses and per-symbol bodyless forwarding realizers must be rejected | not stated | 9 |
| MACHINES63 | "Literal locator lengths are inferred, not manually counted in a return type or Build generic argument." | Supply | Locator byte lengths must be inferred; manual length counting in return types or Build generics must not be required or accepted as author obligation | not stated | 9 |
| MACHINES64 | "Native binding selects backing, not source conformance evidence or execution authority." | Supply | Native binding must select backing only; must not substitute for conformance evidence or execution authority | not stated | 9 |
| MACHINES65 | "Compiler intrinsics use the sealed declaration/signature/target catalog without a binding payload." | Supply | Compiler intrinsics must come from sealed catalog without a Build binding payload | not stated | 9 |
| MACHINES66 | "Composite providers remain ordinary checked bodies; only irreducible foreign leaves use Build-owned native supply." | Supply | Composite providers must be checked bodies; Build native supply applies only to irreducible foreign leaves | not stated | 10 |
| MACHINES67 | "Programs cannot assemble provider-plan rows or reauthor inherited ceilings in a binding." | Supply | Source programs must not assemble provider-plan rows or reauthor inherited ceilings in a binding | not stated | 9 |
| MACHINES68 | "A source `via` clause is not an accepted binding form." | Supply | Source `via` clauses must be rejected as binding forms | not stated | 9 |
| MACHINES69 | "Admission identifies an authorized declaration by sealed source/package custody and its normalized generic signature and complete contract." | Generic sealed primitives | Primitive admission must identify authorized declaration by sealed custody, normalized generic signature, and complete contract | not stated | 9 |
| MACHINES70 | "Binder renaming does not create another identity." | Generic sealed primitives | Renaming binders must not create a distinct primitive identity | not stated | 7 |
| MACHINES71 | "Binder kinds, bounds, lifetime relationships, domains, access, operational clauses and result obligations remain part of the admitted schema; matching a name or parameter count is insufficient." | Generic sealed primitives | Primitive admission must compare full schema including kinds, bounds, lifetimes, domains, access, operational clauses, and results; name or arity match alone must fail | not stated | 7 |
| MACHINES72 | "Each application substitutes its exact arguments through the admitted schema using ordinary generic-machine rules." | Generic sealed primitives | Each primitive application must substitute exact arguments through the admitted schema with ordinary generic rules | at a call | 7 |
| MACHINES73 | "The compiler supplies only the operation authorized for that instantiation and target." | Generic sealed primitives | Compiler may supply only the operation authorized for the instantiation and target | at a call | 9 |
| MACHINES74 | "A user declaration with copied spelling or signature, or an altered contract in an unauthorized source, cannot acquire primitive supply." | Generic sealed primitives | User declarations mimicking primitives or with altered unauthorized contracts must not receive primitive supply | not stated | 9 |
| MACHINES75 | "Closed-catalog admission is not user registration or Build provider selection." | Generic sealed primitives | Closed-catalog admission must not be implemented as user registration or Build provider selection | not stated | 9 |
| MACHINES76 | "Its full contract is checked at each use before intrinsic realization," | Generic sealed primitives | Token-bearing primitive full contract must be checked at each use before intrinsic realization | at the operation | 3 |
| MACHINES77 | "without requiring a checked forwarding wrapper or relying on inlining for ordinary access cost." | Generic sealed primitives | Primitive supply must not require a forwarding wrapper or depend on inlining for correctness of access checks | at the operation | 10 |
| MACHINES78 | "Target-prefixed machine declarations and implicit same-path target-body families are retired." | Target-scoped families | Target-prefixed declarations and implicit target-body families must be rejected | not stated | 9 |
| MACHINES79 | "A spelling such as `windows_x86_64 machine play(...)` is not accepted language syntax." | Target-scoped families | Target-prefixed machine declaration syntax must be rejected | not stated | 9 |
| MACHINES80 | "Target names are Build/toolchain data, not declaration modifiers or names that become keywords when a target package is available." | Target-scoped families | Target names must not act as declaration modifiers or contextual keywords in source | not stated | 9 |
| MACHINES81 | "Different operating-system or architecture implementations are ordinary named implementations of an explicit trait requirement." | Target-scoped families | Per-target implementations must be ordinary named implementations of an explicit trait requirement | not stated | 9 |
| MACHINES82 | "selection retains explicit target-keyed rows; source declarations acquire no target tag or alternate-body identity." | Target-scoped families | Build selection must use target-keyed rows; source declarations must not carry target tags or alternate-body identity | not stated | 9 |
| MACHINES83 | "Psi resolves, types, checks, and publishes every supplied implementation once, independently of the realization set." | Target-scoped families | Psi must resolve, typecheck, check, and publish each supplied implementation once regardless of realization target set | not stated | SHARED |
| MACHINES84 | "Omega selects the Build row for the target it realizes and validates the selected closure's ABI, imports, layout, and ISA applicability." | Target-scoped families | Omega must select Build row for the realized target and validate closure ABI, imports, layout, and ISA applicability | not stated | 9 |
| MACHINES85 | "Missing or incompatible selected supply rejects that realization;" | Target-scoped families | Missing or incompatible selected supply for a target must reject that realization | not stated | 9 |
| MACHINES86 | "selection never hides an unchecked body or grants authority." | Target-scoped families | Provider selection must not hide unchecked bodies or grant execution authority | not stated | 9 |
| MACHINES87 | "A provider refines a requirement when every provider trace, under the permitted observation projection, is allowed by the requirement and asks no more from its caller:" | Substitution | Provider refinement requires every observable provider trace under the projection to be allowed by the requirement and impose no extra caller demands | not stated | 3 |
| MACHINES88 | "Requirement premises imply provider premises." | Substitution | For refinement, requirement `requires` must imply provider `requires` | not stated | 3 |
| MACHINES89 | "Provider guarantees imply requirement guarantees." | Substitution | Provider `ensures` must imply requirement `ensures` | not stated | 3 |
| MACHINES90 | "Provider row is a subset of the requirement ceiling." | Substitution | Provider service reach must be a subset of the requirement ceiling | not stated | 3 |
| MACHINES91 | "Each possibility is permitted independently by the corresponding ceiling." | Substitution | Each suspension/blocking possibility must be independently permitted by the requirement ceiling | not stated | 3 |
| MACHINES92 | "Visible outcomes are a subset of permitted outcomes." | Substitution | Provider failure, crash, and cancellation outcomes must be a subset of permitted outcomes | not stated | 3 |
| MACHINES93 | "Every positive guarantee is preserved under no stronger premises." | Substitution | Provider termination/progress guarantees must preserve requirement guarantees under no stronger premises | not stated | 4 |
| MACHINES94 | "Demand fits the promised ceiling." | Substitution | Provider context-visible resource demand must fit the requirement ceiling | not stated | 3 |
| MACHINES95 | "Behavior is compatible with or stronger than the pinned promise." | Substitution | Provider atomicity, reentrancy, and calling-plan behavior must be compatible with or stronger than the pinned promise | not stated | 3 |
| MACHINES96 | "For a selected generic application, substitute its published static binder contract dependencies before applying the suspension/blocking, crash and termination refinement conditions." | Substitution | Generic refinement must substitute static binder contract dependencies before checking suspension, blocking, crash, and termination axes | not stated | 7 |
| MACHINES97 | "The generic body must satisfy its declared ceiling for every admitted callback." | Substitution | Generic machine bodies must satisfy declared ceilings for every admitted callback | not stated | 3 |
| MACHINES98 | "A fixed opaque ceiling cannot be narrowed by inspecting a private implementation." | Substitution | Opaque requirement ceilings must not be narrowed using private implementation inspection | not stated | 3 |
| MACHINES99 | "Admission conjoins the independently checked axes; matching signatures or one successful analysis is insufficient." | Substitution | Requirement admission must conjoin all substitution axes; signature match or single-axis success must not suffice | not stated | 3 |
| MACHINES100 | "A stronger implementation does not acquire the requirement's identity." | Substitution | Stronger implementations must not inherit the requirement's identity | not stated | 3 |
| MACHINES101 | "Remote execution, handoff, or scheduling that changes an observable item needs a different declared contract." | Substitution | Changed observables from remote execution, handoff, or scheduling require a different declared contract | not stated | 3 |
| MACHINES102 | "Internal steps may stutter only under the declared observation projection and the calling context's observation floor." | Substitution | Stuttering internal steps must respect observation projection and caller observation floor | not stated | 3 |
| MACHINES103 | "Worker occupation cannot become invisible to evade a no-block context;" | Substitution | Worker occupation must remain observable; must not be hidden to evade no-block contexts | not stated | 3 |
| MACHINES104 | "neither can authority use, resource cost, failure, cancellation, or required temporal behavior." | Substitution | Authority use, resource cost, failure, cancellation, and required temporal behavior must not be hidden to evade ceilings | not stated | 3 |
| MACHINES105 | "Direct-call audit markers follow operational ceilings without changing machine identity or invocation mode." | Substitution | Direct-call audit markers must follow operational ceilings without changing machine identity or invocation mode | at a call | 3 |
| MACHINES106 | "There is one machine declaration kind: the source `state` declaration is retired, not an alias or a second kind of machine." | Nested machines and identity | Source `state` declarations must be rejected; only one machine declaration kind is allowed | not stated | 10 |
| MACHINES107 | "Nesting restricts name visibility; it does not allocate data, capture runtime bindings, retain an enclosing activation, or impose a stack layout." | Nested machines and identity | Nested machines must not allocate implicit data, capture bindings, retain activations, or impose stack layout | not stated | 10 |
| MACHINES108 | "Every nested declaration has its own signature, contract and identity." | Nested machines and identity | Each nested machine must have its own signature, contract, and identity | not stated | 10 |
| MACHINES109 | "A body consists of its executable portion followed by zero or more nested machine declarations." | Nested machines and identity | Machine bodies must place executable code before nested machine declarations | not stated | 10 |
| MACHINES110 | "Once declarations begin, executable statements cannot resume." | Nested machines and identity | Executable statements must not appear after nested machine declarations begin | not stated | 10 |
| MACHINES111 | "This layout rule applies recursively." | Nested machines and identity | Executable-then-nested layout must hold recursively in nested machines | not stated | 10 |
| MACHINES112 | "Nested declarations are visible throughout their enclosing machine body, including before their textual declaration," | Nested machines and identity | Nested declarations must be in scope throughout the enclosing body including forward positions | not stated | 10 |
| MACHINES113 | "Duplicate declarations follow ordinary name/overload rules; nesting is not a new overload mechanism." | Nested machines and identity | Duplicate nested names must follow ordinary overload rules; nesting must not create a new overload mechanism | not stated | 10 |
| MACHINES114 | "A helper is unavailable outside the machine whose scope declares it; qualification cannot bypass this restriction." | Nested machines and identity | Nested helpers must not be callable outside their declaring machine scope; qualification must not bypass | at a call | 10 |
| MACHINES115 | "A sibling can name another visible sibling, but cannot reach into that sibling's private descendants." | Nested machines and identity | Sibling nested machines may call visible siblings but must not access private descendants of siblings | at a call | 10 |
| MACHINES116 | "An ancestor's visible declaration may be called recursively; this invokes its entry, not a saved activation or a private point inside its body." | Nested machines and identity | Recursive ancestor calls must invoke entry, not resumed activations or interior points | at a call | 10 |
| MACHINES117 | "Nested declarations are not separately package-exportable; package export of an enclosing machine does not export its helpers." | Nested machines and identity | Nested helpers must not be separately exported; exporting enclosing machine must not export nested helpers | not stated | 10 |
| MACHINES118 | "Runtime parameters and locals of enclosing invocations are **not captured**." | Nested machines and identity | Nested machines must not capture enclosing invocation parameters or locals | not stated | 10 |
| MACHINES119 | "Pass them explicitly, including references, capabilities and lifetime-bearing values." | Nested machines and identity | Enclosing values needed by nested machines must be passed explicitly | at a call | 10 |
| MACHINES120 | "Enclosing generic/static declaration bindings remain lexical context;" | Nested machines and identity | Enclosing generic and static bindings remain lexical context for nested declarations | not stated | 7 |
| MACHINES121 | "resolution and specialization retain their exact identities without creating an implicit runtime environment." | Nested machines and identity | Specialization must retain exact identities without implicit runtime environment | not stated | 7 |
| MACHINES122 | "Nested contracts can use that static context and their own parameters, not another invocation's local values or live facts." | Nested machines and identity | Nested contracts may use static context and own parameters only; not other invocations' locals or live facts | not stated | 10 |
| MACHINES123 | "An ordinary call resumes its continuation on normal return." | Nested machines and identity | Ordinary nested calls must resume caller continuation on normal return | at normal return | 3 |
| MACHINES124 | "A machine call in a terminal `transition` arm forwards its normal result as the current invocation's result, with no continuation in that invocation." | Nested machines and identity | Calls in terminal transition arms must forward callee result as caller result without local continuation | at a call | 10 |
| MACHINES125 | "A nested machine's return is local to that invocation: it does not return from arbitrary lexical ancestors." | Nested machines and identity | Nested returns must be local to the nested invocation; must not return from outer machines | at normal return | 10 |
| MACHINES126 | "Both forms use the same contract, required call acknowledgements and explicit arguments." | Nested machines and identity | Transition-arm and ordinary nested calls must use the same contract, acknowledgements, and explicit arguments | at a call | 3 |
| MACHINES127 | "The distinction is call position, never nesting depth." | Nested machines and identity | Transition versus ordinary call semantics must depend on call position only, not nesting depth | at a call | 10 |
| MACHINES128 | "Resolution checks visibility and elaborates declaration context before erasing lexical nesting into flat machine identities and explicit calls/transfers." | Nested machines and identity | Name resolution must check visibility and elaborate context before flattening nested machines to identities and calls | not stated | 10 |
| MACHINES129 | "Source ancestry remains available for identity, diagnostics and debug mapping, not as an executable parent stack." | Nested machines and identity | Source nesting ancestry must be diagnostic metadata only, not an executable parent stack | not stated | 10 |
| MACHINES130 | "Downstream representations may have ordinary basic blocks and jumps; these are not a revived source `state` construct." | Nested machines and identity | Lowered control flow must not revive source `state` as a construct | not stated | 10 |
| MACHINES131 | "Inlining, tail transfer, frame reuse and fusion must preserve observable contracts, loans, cleanup and composed storage bounds." | Nested machines and identity | Optimizations must preserve observable contracts, loans, cleanup, and storage bounds | not stated | 2 |
| MACHINES132 | "Runtime recursive cycles retain the constant-stack requirement" | Nested machines and identity | Runtime recursive cycles must satisfy constant-stack requirement from termination rules | not stated | 4 |
| MACHINES133 | "Replacing old state loops with frame-growing recursion is not an acceptable intermediate behavior or an optimization left for later." | Nested machines and identity | Frame-growing recursion replacing state loops must not be accepted as intermediate behavior or deferred optimization | not stated | 4 |
| MACHINES134 | "There is no separate state-only calling, contract, ownership or lowering path." | Nested machines and identity | Compiler must not use a separate state-only calling, contract, ownership, or lowering path | not stated | 10 |
| MACHINES135 | "The retired `state` construct is not an alternative source form." | Nested machines and identity | `state` must not be accepted as an alternative source form | not stated | 10 |
| MACHINES136 | "The normalized semantic contract is independent of syntax and lowering." | Published identity | Published contract identity must be independent of syntax and lowering | not stated | 3 |
| MACHINES137 | "Published identity includes the substitutable contract, supply, and observable boundary calling plan." | Published identity | Published identity must include substitutable contract, supply, and observable boundary calling plan | not stated | 3 |
| MACHINES138 | "Component manifests, proof artifacts, admission, and replacement checks retain it when runtime lowering erases proof-only material." | Published identity | Manifests, artifacts, admission, and replacement checks must retain published identity when proof-only material is erased | not stated | SHARED |
| MACHINES139 | "The normalized compiler-derived reach dependency is part of this contract" | Published identity | Normalized compiler-derived reach dependency must be part of the published contract | not stated | 3 |
| MACHINES140 | "The operational contract also retains derived conditional suspension-carry obligations with their exact subjects, provenance and normalized conditions." | Published identity | Operational contract must retain conditional suspension-carry obligations with exact subjects, provenance, and normalized conditions | not stated | 3 |
| MACHINES141 | "An implementation body hash, source keyword, or opportunistic narrowing of an opaque requirement cannot replace that identity." | Published identity | Identity must not be replaced by body hash, source keyword, or opportunistic narrowing of opaque requirements | not stated | 3 |
| MACHINES142 | "`05_machine_contracts.json` exposes this split as sibling `contract` and `implementation` objects." | Published identity | Contract artifacts must expose sibling `contract` and `implementation` objects in `05_machine_contracts.json` | not stated | SHARED |
| MACHINES143 | "Binding and component tools pin the contract rather than incorporating proof-local implementation fields." | Published identity | Binding and component tools must pin contract identity, not proof-local implementation fields | not stated | 9 |

## Ambiguous sentences

| Sentence | Reading A | Reading B |
| --- | --- | --- |
| "Static callbacks select those declarations and pass ordinary explicit context data." | Only static callback forms may invoke machines; dynamic indirect selection is forbidden outside `machine_values.md`. | "Static callbacks" names a checking model for reach/effects, not a restriction on which call forms exist in source. |
| "A productive machine may run forever; a function returning a value is a special case, not the definition." | Non-terminating machines are first-class; compilers must accept productive loops without values. | Termination checking for value-returning machines still applies; productivity is descriptive of the general model. |
| "Predicate-only `self` in data/default-domain and named-domain predicates ... does not change this rule." | Domain `self` is syntactically allowed but must never affect machine parameter checking. | Domain `self` may participate in domain membership only; whether it is visible in machine signatures is defined elsewhere. |
| "An explicitly installed entry contract may instead supply arguments, as with UEFI handoff;" | Entry contracts may replace stack/static entry state entirely. | Entry contracts may supply only some arguments; remaining entry state still uses stack/static rules. |
| "Eligibility comes from its complete contract and context, not separate `async`, `proof`, or `const` machine species." | Source must not annotate machines as async/proof/const species for eligibility. | Eligibility is computed from contracts; legacy species keywords might still parse but are ignored. |
| "Evaluation may erase execution but cannot change meaning." | Any constexpr evaluation that skips runtime must preserve exact semantic meaning. | Erasure applies only to approved evaluation modes; partial evaluation that changes observable traces is out of scope here. |
| "A total implementation behind a restricted contract does not make that selected contract total over all inputs." | Callers must still prove restricted `requires` even if some implementation is total on all inputs. | Type inference might widen contracts if a total impl exists; this sentence forbids that widening at the selected call site only. |
| "unavailable evidence prevents acceptance, but failed search does not establish falsity." | Programs with unproved but satisfiable obligations are rejected until evidence is supplied. | Programs may be rejected on proof timeout even when obligations are false; failed search is inconclusive, not acceptance. |
| "A claim-free bodyless free machine outside the authorized compiler catalog has no supply mode and rejects." | Any bodyless machine without ensures/theorem claim and outside catalog rejects. | "Claim-free" might allow other header claims besides `ensures`; only machines with no supply-relevant claims reject. |
| "Accepted theorem claims use the existing boundary surface, not a separate fact declaration." | Theorem acceptance must go through `boundary machine`, not a new fact syntax. | Theorem claims may use boundary surface or linked proof artifacts; "separate fact declaration" bans only one alternate syntax. |
| "`boundary` marks a crossing, not its direction." | `boundary` annotation does not encode import vs export direction in the declaration. | Import/export direction is artifact-relative only; `boundary` might still affect admission direction checks elsewhere. |
| "Imports and exports are artifact-relative; requirements and composition determine provision or demand." | Compiler resolves boundary direction from build graph, not from `boundary` keyword alone. | Source-level import/export keywords might still exist outside this chapter for modules vs machines. |
| "a helper returning binding data is optional ordinary computation." | Build bindings need not be wrapped in helper machines. | Helpers may return binding data but cannot substitute for `Build::bind` rows. |
| "This supply extension is ratified; generic catalog admission and its indexing consumers are pending **CORE-INDEXING-CONTRACTS**" | Token primitives are ratified now; full indexing catalog admission may be incomplete until that task lands. | Until CORE-INDEXING-CONTRACTS completes, indexing primitives might reject even though supply extension is ratified. |
| "Even when several targets share one implementation, selection retains explicit target-keyed rows;" | Build must record one row per target even if the same source impl is selected. | Target-keyed rows are required in Build metadata; Psi may still deduplicate checked bodies internally. |
| "Psi resolves, types, checks, and publishes every supplied implementation once, independently of the realization set." | All target implementations are checked before any single-target realization. | Psi checks once per unique implementation identity, which may still skip unreferenced target variants until selected. |
| "A stronger implementation does not acquire the requirement's identity." | Refinement/admission does not rename the requirement edge to the provider's declaration id. | Stronger behavior may still satisfy the same requirement slot without changing public requirement naming in artifacts. |
| "Internal steps may stutter only under the declared observation projection and the calling context's observation floor." | Stutter is allowed only when observably equivalent under both projection and floor. | Stutter may be invisible internally even when floor differs; only listed observables are constrained. |
| "A transition is not mandatory merely because declarations follow." | Executable portion may end without transition when nested declarations follow. | Transition may still be required by separate state-contract rules not repeated in this paragraph. |
| "Nested declarations are visible throughout their enclosing machine body, including before their textual declaration," | Forward references to nested helpers are allowed within the enclosing body. | Visibility before textual declaration applies only to names, not to contracts that reference nested machines' bodies. |
| "An ordinary call resumes its continuation on normal return." | Nested calls in non-transition positions always resume after callee returns. | "Ordinary call" excludes calls with explicit tail semantics; tail calls might not resume a continuation. |
| "A machine invocation does not prescribe a physical call frame." | Lowering may inline or tail-call without preserving frames. | Contract preservation still constrains lowering even though source does not prescribe frames. |

## Terms used without definition in this chapter

- contracted transition system
- declaration identity
- observable trace
- terminal outcome
- productive machine
- named declarations
- static callbacks
- explicit context data
- runtime machine values
- complete callable contract
- explicit state and callable fields
- static declarations independent of data
- named implementations
- ordinary signature types
- declared trait arguments
- conformance data subject
- `Self` type binding
- implicit receiver
- bound-method value
- receiver-inserting call syntax
- predicate-only `self`
- data/default-domain and named-domain predicates
- data carrier
- machine qualification
- lexically visible nested declaration
- context and capabilities
- toolchain-authored build API
- boundary provider
- provider value
- private state
- invocation scratch
- machine-scoped temporary storage
- implicit capture
- persistent context record
- entry state
- installed entry contract
- UEFI handoff
- mutable context parameter
- automatic allocation
- receiver-based machine forms
- carrier-qualified machine forms
- compatibility mode
- header clauses
- bodyless declaration or requirement
- declaration semicolon
- nested `requires machine` contracts
- fixed operator token
- executable species
- alternate contract system
- semantic evaluation
- proof citation
- task activation
- trait satisfaction
- boundary import/export
- complete contract and context
- `async` / `proof` / `const` machine species
- logical conditions
- mathematical abstractions
- compiler-owned proof term formers
- executable supply modes
- exact argument substitution
- token-bound calls
- mathematical application
- result term
- total implementation
- restricted contract
- automatic prover
- unavailable evidence
- failed search
- well-founded descent obligation
- `boundary-operations-are-trait-requirements`
- standalone carrier-owned boundary-requirement declaration
- boundary traits
- one-member traits
- checked body
- required body
- native provider
- `Build::bind<Requirement>(builder, target, locator)`
- pinned contract
- calling policy
- foreign admission
- forwarding machine
- `via` clause
- admission-bearing declaration
- `boundary machine`
- unproved theorem
- external-behavior guarantee
- owner acceptance
- compiler-supplied primitive
- closed-catalog implementation
- fixed semantic contract
- user registration
- claim-free bodyless free machine
- authorized compiler catalog
- catalog exception
- expression-body syntax
- bodyless `ensures`
- theorem claims
- boundary surface
- fact declaration
- token-bearing machines
- boundary requirement spelling
- operation slot
- carrier and authority values
- artifact-relative imports and exports
- provision or demand
- irreducible native implementation
- typed locator
- per-symbol bodyless forwarding realizers
- literal locator lengths
- source conformance evidence
- execution authority
- compiler intrinsics
- sealed declaration/signature/target catalog
- binding payload
- composite providers
- irreducible foreign leaves
- selected conformance closure
- provider-plan rows
- reauthor inherited ceilings
- generic sealed primitives
- const parameters (generic)
- generic and borrowed-result families
- core indexing
- sealed source/package custody
- normalized generic signature
- complete contract (admitted schema)
- binder renaming
- binder kinds, bounds, lifetime relationships
- operational clauses
- result obligations
- admitted schema
- ordinary generic-machine rules
- unauthorized source
- closed-catalog admission
- token-bearing primitive
- checked forwarding wrapper
- inlining (for access cost)
- CORE-INDEXING-CONTRACTS
- target-prefixed machine declarations
- implicit same-path target-body families
- target names as keywords
- explicit trait requirement
- provider selection
- target-keyed rows
- target tag
- alternate-body identity
- realization set
- selected closure
- ABI, imports, layout, ISA applicability
- substitution / refinement
- permitted observation projection
- service reach ceiling
- suspension/blocking ceiling
- failure, crash, cancellation outcomes
- termination/progress guarantees
- context-visible resources
- atomicity/reentrancy
- pinned promise (calling plan)
- static binder contract dependencies
- admitted callback
- fixed opaque ceiling
- requirement's identity
- remote execution / handoff / scheduling
- observation projection
- observation floor
- no-block context
- worker occupation
- direct-call audit markers
- operational ceilings
- machine declaration kind
- source `state` declaration
- enclosing activation
- executable portion
- transition
- permitted Unit completion
- nested declaration scopes
- name/overload rules
- qualification bypass
- private descendants
- saved activation
- package-exportable
- runtime parameters and locals (not captured)
- capabilities
- lifetime-bearing values
- generic/static declaration bindings
- implicit runtime environment
- live facts
- continuation
- terminal `transition` arm
- required call acknowledgements
- call position vs nesting depth
- flat machine identities
- explicit calls/transfers
- source ancestry
- executable parent stack
- physical call frame
- tail transfer
- frame reuse
- fusion
- composed storage bounds
- constant-stack requirement
- state-only calling path
- normalized semantic contract
- substitutable contract
- observable boundary calling plan
- proof-only material
- normalized compiler-derived reach dependency
- automatic reach propagation
- conditional suspension-carry obligations
- exact subjects, provenance, normalized conditions
- implementation body hash
- opportunistic narrowing
- opaque requirement
- private ranking evidence
- `05_machine_contracts.json`
- checked summary
- private witness
- proof-local implementation fields

CHAPTER COMPLETE
