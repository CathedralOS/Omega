# Build-level behavior exclusions

Source: `wiki/spec/build/behavior_exclusions.md` lines 1–259 (read).

## Rules

| rule id | the exact sentence, quoted | the heading it is under | what must be checked, in plain words | when it must hold | lane |
| --- | --- | --- | --- | --- | --- |
| BEHAVIOR_EXCLUSIONS1 | "1. Every implementation satisfies its authored callable/requirement contract." | Two independent checks | Each selected implementation must meet its declared callable and requirement contracts. | not stated | 3 |
| BEHAVIOR_EXCLUSIONS2 | "2. The selected composition satisfies the build's additional exclusions." | Two independent checks | The executable composition selected by the build must satisfy every build-selected exclusion. | not stated | 9 |
| BEHAVIOR_EXCLUSIONS3 | "A build without a Trap exclusion cannot authorize a Trap forbidden by an intermediate interface." | Two independent checks | If the build did not select a Trap exclusion, the build must not authorize Trap where an intermediate interface forbids it. | not stated | 9 |
| BEHAVIOR_EXCLUSIONS4 | "A declaration permitting Trap cannot override the build's exclusion." | Two independent checks | A callable declaration that permits Trap must not cause Trap to be allowed when the build excludes Trap. | not stated | 9 |
| BEHAVIOR_EXCLUSIONS5 | "That fact belongs to this composition; it does not rewrite a callable's signature, make it satisfy a narrower requirement, or supply facts to ordinary callers that only possess its contract." | Two independent checks | A stronger composition-only fact must not change signatures, narrow requirement satisfaction for callables, or give extra facts to callers limited to the contract alone. | not stated | SHARED |
| BEHAVIOR_EXCLUSIONS6 | "No general crash-effect polymorphism or automatic API refinement follows." | Two independent checks | The compiler must not treat composition facts as general crash polymorphism or automatic API refinement. | not stated | 9 |
| BEHAVIOR_EXCLUSIONS7 | "No possible semantic outcome of that cause, initially `Trap` or `Abort`." | Restriction vocabulary | For a Crash cause exclusion, no possible semantic outcome of that cause (initially Trap or Abort) may remain in the selected composition. | not stated | 9 |
| BEHAVIOR_EXCLUSIONS8 | "No possible service contribution naming that exact identity in a reached boundary requirement or primitive operation, including boundary-parent closure." | Restriction vocabulary | For an Abstract service exclusion, no reached boundary requirement or primitive operation may contribute that exact service identity (including boundary-parent closure). | not stated | 9 |
| BEHAVIOR_EXCLUSIONS9 | "No selected mechanism exercises the named terminal class under the shared mechanism-classification semantics." | Restriction vocabulary | For a Physical authority class exclusion, no selected mechanism may exercise that terminal class per shared mechanism-classification semantics. | not stated | 9 |
| BEHAVIOR_EXCLUSIONS10 | "An operation explicitly reaching Console still counts when its selected provider is silent." | Restriction vocabulary | Console-reaching must be judged from explicit reach, not from provider silence alone. | not stated | 9 |
| BEHAVIOR_EXCLUSIONS11 | "Merely calling a trait named Console does not manufacture that row." | Restriction vocabulary | A call to a trait merely named Console must not by itself count as a Console service contribution row. | not stated | 9 |
| BEHAVIOR_EXCLUSIONS12 | "An ordinary no-op logger that never invokes a Console-reaching operation can satisfy a Console exclusion, even when its public contract conservatively permits Console." | Restriction vocabulary | Console exclusion satisfaction may use verified absence of Console-reaching invocations despite a broad public Console allowance. | not stated | 9 |
| BEHAVIOR_EXCLUSIONS13 | "Likewise, removing physical filesystem access does not erase a Readable service invocation." | Restriction vocabulary | Physical filesystem removal must not be treated as removing a Readable service invocation from account. | not stated | 9 |
| BEHAVIOR_EXCLUSIONS14 | "Service names must resolve to exact authorized declarations, not string labels." | Restriction vocabulary | Service operands in exclusions must resolve to exact authorized declarations; string labels alone must not suffice. | not stated | 9 |
| BEHAVIOR_EXCLUSIONS15 | "Unrecognized crash causes, unknown classifications, and ambiguous identities reject." | Restriction vocabulary | Unrecognized crash causes, unknown classifications, and ambiguous identities must cause rejection. | not stated | 9 |
| BEHAVIOR_EXCLUSIONS16 | "Arbitrary authored crash categories, user-defined predicates, Boolean effect-row syntax, and a general policy-plugin language are not part of this contract." | Restriction vocabulary | Exclusion authoring must not admit those extension mechanisms as part of this contract. | not stated | 9 |
| BEHAVIOR_EXCLUSIONS17 | "an available Build method does not establish portable wire or replacement support." | Restriction vocabulary | Presence of an available Build method must not be treated as establishing portable wire or replacement support. | not stated | 9 |
| BEHAVIOR_EXCLUSIONS18 | "Its one explicit value argument borrows the activation's original Build value; authorized helpers may forward that borrow." | Service exclusion authoring | The `builder` argument must borrow the activation's original Build value; forwarding by authorized helpers is permitted. | at a call | 9 |
| BEHAVIOR_EXCLUSIONS19 | "The single static operand identifies an exact boundary trait, not an executable dependency or a string label." | Service exclusion authoring | The service type operand must identify an exact boundary trait, not an executable dependency or string label. | at a call | 9 |
| BEHAVIOR_EXCLUSIONS20 | "A user-authored machine with the same spelling remains an ordinary machine." | Service exclusion authoring | User machines spelled like the compiler-owned operation remain ordinary machines, not this operation. | not stated | 9 |
| BEHAVIOR_EXCLUSIONS21 | "Missing activation arguments and receiver-style calls reject; no implicit builder argument is inserted." | Service exclusion authoring | Missing activation arguments and receiver-style calls must reject; the compiler must not insert an implicit builder. | at a call | 9 |
| BEHAVIOR_EXCLUSIONS22 | "Only executing the call selects the exclusion." | Service exclusion authoring | Exclusion selection occurs only when the exclude_service call executes. | at a call | 9 |
| BEHAVIOR_EXCLUSIONS23 | "An uncalled helper imposes none." | Service exclusion authoring | Helpers that are never executed must not impose exclusions. | not stated | 9 |
| BEHAVIOR_EXCLUSIONS24 | "The selected product must then satisfy that exclusion under the checks below." | Service exclusion authoring | After an executed exclusion call, the selected product must satisfy that exclusion per verification rules. | not stated | 9 |
| BEHAVIOR_EXCLUSIONS25 | "The explicit `builder` argument must be the activation's original Build value." | Physical exclusion authoring | `exclude_physical_authority`'s builder argument must be the activation's original Build value. | at a call | 9 |
| BEHAVIOR_EXCLUSIONS26 | "Only executed calls select exclusions." | Physical exclusion authoring | Physical exclusions are selected only by executed `exclude_physical_authority` calls. | at a call | 9 |
| BEHAVIOR_EXCLUSIONS27 | "Admission retains the exact toolchain case identity, selecting machine, and authored source span before forming the canonical union; a same-spelled user declaration is not equivalent." | Physical exclusion authoring | Admission must retain toolchain case identity, selecting machine, and source span before canonical union; user same-spelled declarations are not equivalent. | not stated | 9 |
| BEHAVIOR_EXCLUSIONS28 | "Exclusions are evaluated Build selections, like other product configuration, not dependency requests." | Selection and scope | Exclusions must be treated as Build selections, not as dependency requests. | not stated | 9 |
| BEHAVIOR_EXCLUSIONS29 | "Repeated selections combine by set union; duplicates are idempotent, order cannot change the result, and later selections cannot remove earlier restrictions." | Selection and scope | Combined exclusions must be set union; duplicates idempotent; order irrelevant; later selections cannot remove earlier restrictions. | not stated | 9 |
| BEHAVIOR_EXCLUSIONS30 | "No added exclusion is implied by omission." | Selection and scope | Omission of a selection must not imply that exclusion was added. | not stated | 9 |
| BEHAVIOR_EXCLUSIONS31 | "Existing target, source, and authority restrictions still apply." | Selection and scope | Other target, source, and authority restrictions remain in force alongside exclusions. | not stated | SHARED |
| BEHAVIOR_EXCLUSIONS32 | "Receiver-policy restrictions apply at explicit admission, not by default to artifact production." | Selection and scope | Receiver-policy restrictions apply at explicit admission only, not by default to artifact production. | at entry | 9 |
| BEHAVIOR_EXCLUSIONS33 | "Physical exclusion checking reuses classification, not the receiver's permission grant table." | Selection and scope | Physical exclusion checking must use classification semantics, not the receiver permission grant table. | not stated | 9 |
| BEHAVIOR_EXCLUSIONS34 | "There is no intrinsic debug/release switch." | Selection and scope | The language/build must not treat debug/release as an intrinsic exclusion switch. | not stated | 9 |
| BEHAVIOR_EXCLUSIONS35 | "Selection uses existing checked static applications and provider mechanisms; it does not make every ordinary function a replaceable Build provider slot." | Selection and scope | Selection must use checked static applications and provider mechanisms; ordinary functions must not become replaceable Build provider slots by default. | not stated | 9 |
| BEHAVIOR_EXCLUSIONS36 | "Selection itself grants neither authority nor permission to change a requirement's meaning." | Selection and scope | Build selection must not by itself grant authority or permission to change a requirement's meaning. | not stated | 9 |
| BEHAVIOR_EXCLUSIONS37 | "The checked scope is the exact target child and selected executable root or component closure, not every package acquired by the build and not every process on the host." | Selection and scope | Exclusion checking scope is the target child plus selected executable root or component closure only. | not stated | 9 |
| BEHAVIOR_EXCLUSIONS38 | "It includes every possible admitted entry and all code, state and selected dependencies reachable from those entries: startup, generated adapters, argument evaluation, callbacks, timers, workers, cleanup, and retained providers." | Selection and scope | Scope must include all admitted entries and reachable code, state, and selected dependencies listed. | not stated | 9 |
| BEHAVIOR_EXCLUSIONS39 | "An unused unselected implementation in the source inventory need not satisfy a product exclusion, but remains subject to ordinary source checks." | Selection and scope | Unused unselected implementations need not meet product exclusions but remain subject to ordinary source checks. | not stated | SHARED |
| BEHAVIOR_EXCLUSIONS40 | "Consumer exclusions cover dependency contributions throughout that product's execution closure." | Selection and scope | Consumer-selected exclusions must apply to dependency contributions across the product execution closure. | not stated | 9 |
| BEHAVIOR_EXCLUSIONS41 | "A dependency cannot opt out or weaken the consumer's ban." | Selection and scope | Dependencies must not opt out of or weaken the consumer's exclusion ban. | not stated | 9 |
| BEHAVIOR_EXCLUSIONS42 | "Independently published components retain their own restrictions and must also satisfy the receiving composition's requirements." | Selection and scope | Published components keep their restrictions and must additionally satisfy the receiver composition's requirements. | not stated | 9 |
| BEHAVIOR_EXCLUSIONS43 | "Exclusions do not automatically apply to resolver activity, build-helper execution, or unrelated target children; those retain their own admission contracts." | Selection and scope | Exclusions must not automatically apply to resolver, build-helper, or unrelated target children. | not stated | 9 |
| BEHAVIOR_EXCLUSIONS44 | "An artifact-only output of arbitrary bytes cannot be advertised as a checked executable satisfying exclusions." | Selection and scope | Arbitrary-byte artifact-only outputs must not be advertised as checked executables satisfying exclusions. | not stated | 9 |
| BEHAVIOR_EXCLUSIONS45 | "Check exclusions after generated source and the relevant root/provider selections are fixed, and before admitting or publishing the executable product." | Verification and publication | Exclusion checks must run after generated source and root/provider selections are fixed and before admit/publish. | not stated | 9 |
| BEHAVIOR_EXCLUSIONS46 | "This is a compiler admission step after authored build execution, not a build callback that inspects its own unfinished product." | Verification and publication | Exclusion checking must be compiler admission after authored build execution, not a self-inspecting build callback. | not stated | 9 |
| BEHAVIOR_EXCLUSIONS47 | "Declarations remain the contract-checking baseline; absence checking may use stronger facts only when independently justified for the exact selected composition." | Verification and publication | Contract checking uses declarations as baseline; stronger absence facts only with independent justification for the exact composition. | not stated | SHARED |
| BEHAVIOR_EXCLUSIONS48 | "A direct abstract boundary invocation retains its explicitly declared service row even when native lowering erases the call." | Verification and publication | Abstract boundary invocations keep declared service rows even if native lowering erases the call. | not stated | 9 |
| BEHAVIOR_EXCLUSIONS49 | "Native instruction scanning, a producer-written empty row, and a test run cannot establish semantic absence." | Verification and publication | Those evidence forms must not suffice to establish semantic absence. | not stated | 9 |
| BEHAVIOR_EXCLUSIONS50 | "Sound guard proofs may exclude impossible paths under the admitted root premises." | Verification and publication | Guard proofs may exclude paths impossible under admitted root premises. | not stated | 8 |
| BEHAVIOR_EXCLUSIONS51 | "The checker need not decide arbitrary semantic reachability: unsupported or incomplete evidence is failure to establish the exclusion." | Verification and publication | Unsupported or incomplete evidence must fail to establish the exclusion; arbitrary reachability need not be decided. | not stated | 9 |
| BEHAVIOR_EXCLUSIONS52 | "Optional optimization selection must not determine admissibility." | Verification and publication | Admissibility must not depend on optional optimization selection. | not stated | 9 |
| BEHAVIOR_EXCLUSIONS53 | "Selection and ordinary semantic checking must suffice for the no-op assertion customer; the check cannot depend on a particular inlining or dead-code-elimination pass having run." | Verification and publication | No-op assertion exclusion checking must not require specific inlining or DCE. | not stated | 9 |
| BEHAVIOR_EXCLUSIONS54 | "Later native realization must preserve the semantic evidence and satisfy applicable physical class exclusions through its accounted mechanisms." | Verification and publication | Native realization must preserve semantic evidence and satisfy physical class exclusions via accounted mechanisms. | not stated | 9 |
| BEHAVIOR_EXCLUSIONS55 | "For a static selected call, use its verified implementation and exact application." | Verification and publication | Static selected calls must use verified implementation and exact application for exclusion evidence. | at a call | 9 |
| BEHAVIOR_EXCLUSIONS56 | "For dynamic or installation-bound calls, cover every admitted target or retain the fixed contract conservatively." | Verification and publication | Dynamic or installation-bound calls must cover every admitted target or stay conservative to the fixed contract. | at a call | 9 |
| BEHAVIOR_EXCLUSIONS57 | "A provider currently selected behind a mutable slot is insufficient unless replacement is constrained to preserve the exclusion." | Verification and publication | Mutable-slot current provider alone is insufficient unless replacement preserves the exclusion. | not stated | 9 |
| BEHAVIOR_EXCLUSIONS58 | "An opaque or precompiled dependency with only a broad may-Trap or may-Console contract cannot establish absence of that behavior." | Verification and publication | Opaque/precompiled broad may-Trap or may-Console contracts cannot establish absence. | not stated | 9 |
| BEHAVIOR_EXCLUSIONS59 | "Obtain stronger checked evidence, choose another artifact, or reject." | Verification and publication | When absence cannot be established from allowed evidence, must obtain stronger evidence, choose another artifact, or reject. | not stated | 9 |
| BEHAVIOR_EXCLUSIONS60 | "No private source need be exposed to author lookup, and no consumer may merely strip a dependency's declared allowance." | Verification and publication | Consumers must not establish absence by stripping declared dependency allowances without required evidence; private source need not be exposed for lookup. | not stated | 9 |
| BEHAVIOR_EXCLUSIONS61 | "Retained evidence binds the exclusion sets, execution scope and entry roster, source/generated and semantic subjects, selected applications/providers, target/realization identities, checker schema, and checked or explicitly accepted assumptions." | Verification and publication | Published/retained evidence must bind all listed subjects to the exclusion check. | not stated | 9 |
| BEHAVIOR_EXCLUSIONS62 | "Changed inputs require rechecking; cached success and equal native bytes alone are insufficient." | Verification and publication | Input changes require recheck; cache or native-byte equality alone must not skip recheck. | not stated | 9 |
| BEHAVIOR_EXCLUSIONS63 | "Assumptions remain visible and governed by ordinary admission rules; a build ban is not evidence that an unverified host contract is true." | Verification and publication | Build bans must not count as evidence that an unverified host contract holds. | not stated | 9 |
| BEHAVIOR_EXCLUSIONS64 | "Report a satisfied exclusion, a prohibited possible behavior with attributable call/entry/provider context, or inability to establish absence with the missing evidence." | Verification and publication | Outcomes must be one of: satisfied exclusion, prohibited possible behavior with context, or unable to establish absence with missing evidence. | not stated | 9 |
| BEHAVIOR_EXCLUSIONS65 | "The latter two both fail required product admission but are distinct diagnostics." | Verification and publication | Prohibited behavior and inability to establish absence must both fail admission with distinct diagnostics. | not stated | 9 |
| BEHAVIOR_EXCLUSIONS66 | "Do not label a conservative possible path as a witnessed runtime execution." | Verification and publication | Diagnostics must not label conservative possible paths as witnessed runtime execution. | not stated | 9 |
| BEHAVIOR_EXCLUSIONS67 | "No partial successful product is published when an exclusion fails." | Verification and publication | Failed exclusion must prevent publishing a partial successful product. | not stated | 9 |
| BEHAVIOR_EXCLUSIONS68 | "A receiving composition with stricter bans must establish them for the joined closure; an old receipt for a weaker policy is not enough." | Installation and replacement | Stricter receiving composition must establish bans for joined closure; weaker-policy receipts insufficient. | not stated | 9 |
| BEHAVIOR_EXCLUSIONS69 | "Runtime rebinding, new entries or replacement must preserve the restrictions through checked admission or require a new owner-authorized composition." | Installation and replacement | Rebinding, new entries, or replacement must preserve restrictions via checked admission or new owner-authorized composition. | not stated | 9 |
| BEHAVIOR_EXCLUSIONS70 | "Candidate code cannot widen its own envelope." | Installation and replacement | Candidate code must not widen its own installation/replacement envelope. | not stated | 9 |
| BEHAVIOR_EXCLUSIONS71 | "Crash exclusions cover their named semantic outcomes, not hardware failures, termination, cleanup completion, availability, confidentiality or recovery in general." | Installation and replacement | Crash exclusions apply only to named semantic outcomes, not the listed broader properties. | not stated | 9 |
| BEHAVIOR_EXCLUSIONS72 | "A Trap exclusion does not exclude Abort or ProcessExit." | Installation and replacement | Trap exclusion must not be treated as excluding Abort or ProcessExit. | not stated | 9 |
| BEHAVIOR_EXCLUSIONS73 | "Existing [component custody](component_publication.md) and external survivor obligations remain required regardless of which exclusions were requested." | Installation and replacement | Component custody and external survivor obligations remain required irrespective of exclusions. | not stated | 9 |
| BEHAVIOR_EXCLUSIONS74 | "An assertion is ordinary checked code." | Assertions and optional logging | Assertions must be checked like ordinary code. | not stated | SHARED |
| BEHAVIOR_EXCLUSIONS75 | "Public wrappers retain the same explicit allowances across configurations." | Assertions and optional logging | Public wrappers must keep the same explicit allowances across configurations. | not stated | 9 |
| BEHAVIOR_EXCLUSIONS76 | "The build can then reject the checking composition and accept the verified no-op composition under a Trap exclusion, without altering those source declarations." | Assertions and optional logging | Build may reject checking composition and accept verified no-op under Trap exclusion without changing source declarations. | not stated | 9 |
| BEHAVIOR_EXCLUSIONS77 | "It still rejects any unrelated possible Trap in the selected closure." | Assertions and optional logging | Trap exclusion must still reject unrelated possible Trap in the selected closure. | not stated | 9 |
| BEHAVIOR_EXCLUSIONS78 | "A shared checking/no-op requirement cannot promise that normal return establishes the asserted condition." | Assertions and optional logging | Shared checking/no-op requirement must not promise normal return establishes the asserted condition. | at normal return | 3 |
| BEHAVIOR_EXCLUSIONS79 | "Validation needed to justify subsequent safe operations is required program behavior, not optional diagnostics." | Assertions and optional logging | Validation for subsequent safe operations is required behavior, not optional diagnostics. | not stated | SHARED |
| BEHAVIOR_EXCLUSIONS80 | "Ordinary arguments are eager: selecting a no-op does not erase their effects, crashes, divergence, borrow validity, or ownership obligations." | Assertions and optional logging | No-op selection must not erase eager argument effects, crashes, divergence, borrow validity, or ownership obligations. | not stated | SHARED |
| BEHAVIOR_EXCLUSIONS81 | "Optimizations may discard safely removable computation under their usual rules, not because the consumer is called assert." | Assertions and optional logging | Optimizations must not discard computation merely because the consumer is named assert. | not stated | 9 |
| BEHAVIOR_EXCLUSIONS82 | "Explicit delayed computation may use ordinary named machine arguments; work outside that delayed call still occurs." | Assertions and optional logging | Delayed computation via named machine arguments is allowed; work outside the delayed call still runs. | not stated | 1 |
| BEHAVIOR_EXCLUSIONS83 | "Assertions are not generated from arbitrary preconditions, postconditions or admissions." | Assertions and optional logging | Assertions must not be generated from arbitrary pre/postconditions or admissions. | not stated | 8 |
| BEHAVIOR_EXCLUSIONS84 | "An assertion repeating an admitted fact may disappear under ordinary optimization; preserving a check against assumptions requires a separate design." | Assertions and optional logging | Assertions repeating admitted facts may be optimized away; preserving checks against assumptions needs separate design. | not stated | 9 |
| BEHAVIOR_EXCLUSIONS85 | "Failure introduces no implicit unwind, flush, recovery, or authority." | Assertions and optional logging | Assertion failure must not imply implicit unwind, flush, recovery, or authority. | not stated | SHARED |
| BEHAVIOR_EXCLUSIONS86 | "admissions require no runtime checking." | Assertions and optional logging | Admissions require no runtime checking per linked contract rules. | not stated | 8 |
| BEHAVIOR_EXCLUSIONS87 | "[Build-time tests](testing.md) use ordinary mandatory checks for their verdict, not an optional checking/no-op diagnostic contract." | Assertions and optional logging | Build-time tests use ordinary mandatory checks for verdicts, not optional checking/no-op diagnostic contract. | not stated | 9 |
| BEHAVIOR_EXCLUSIONS88 | "A registered requirement may permit Trap and its satisfiers inherit that allowance; no attribute or global build switch widens their contracts." | Assertions and optional logging | Registered requirements may permit Trap with inheriting satisfiers; no attribute or global build switch may widen contracts. | not stated | 9 |
| BEHAVIOR_EXCLUSIONS89 | "Test and application roots are separate: an application Trap exclusion does not forbid a failing check in the separately admitted test closure, and test execution cannot relax the application's exclusion." | Assertions and optional logging | Application Trap exclusion must not forbid failing checks in separate test closure; tests must not relax application exclusion. | not stated | 9 |
| BEHAVIOR_EXCLUSIONS90 | "A Trap-excluding build rejects a possible failing assertion." | Implementation acceptance | Same source with checking implementation and Trap-excluding build must reject possible failing assertion. | not stated | 9 |
| BEHAVIOR_EXCLUSIONS91 | "Pass if the entire selected closure excludes Trap, without optional optimizations." | Implementation acceptance | No-op selected: pass only if entire selected closure excludes Trap without relying on optional optimizations. | not stated | 9 |
| BEHAVIOR_EXCLUSIONS92 | "Reject; no assertion-specific omission." | Implementation acceptance | No-op selected but predicate/helper can Trap: reject; no assertion-specific omission. | not stated | 9 |
| BEHAVIOR_EXCLUSIONS93 | "Ordinary contract checking rejects." | Implementation acceptance | Trap not excluded by build but violates intermediate API: ordinary contract checking rejects. | not stated | 3 |
| BEHAVIOR_EXCLUSIONS94 | "Pass a Console exclusion when complete verified implementation evidence establishes no Console invocation." | Implementation acceptance | Silent logger with broad Console allowance: pass Console exclusion when verified evidence shows no Console invocation. | not stated | 9 |
| BEHAVIOR_EXCLUSIONS95 | "Reject a Console exclusion; physical silence is a different property." | Implementation acceptance | Silent provider for actual Console boundary invocation: reject Console exclusion. | not stated | 9 |
| BEHAVIOR_EXCLUSIONS96 | "May satisfy the physical class exclusion while failing the service exclusion." | Implementation acceptance | No process-output mechanisms but Console invocation remains: may satisfy physical exclusion while failing service exclusion. | not stated | 9 |
| BEHAVIOR_EXCLUSIONS97 | "Refuse absence certification; report the evidence gap." | Implementation acceptance | Unknown foreign behavior or precompiled broad allowance only: refuse absence certification and report evidence gap. | not stated | 9 |
| BEHAVIOR_EXCLUSIONS98 | "Reject with provenance to that contribution." | Implementation acceptance | Callback, startup, cleanup, or generated entry introduces forbidden behavior: reject with provenance. | not stated | 9 |
| BEHAVIOR_EXCLUSIONS99 | "Same canonical restriction set and independent child outcome." | Implementation acceptance | Reordered/duplicate bans or another target child: same canonical restriction set and independent child outcome. | not stated | 9 |
| BEHAVIOR_EXCLUSIONS100 | "Same exclusion admissibility for the same semantic selection." | Implementation acceptance | Optimization families enabled/disabled: same exclusion admissibility for same semantic selection. | not stated | 9 |
| BEHAVIOR_EXCLUSIONS101 | "Independent consumer rejects." | Implementation acceptance | Stale scope, provider, entry, policy, or evidence substituted: independent consumer rejects. | not stated | 9 |
| BEHAVIOR_EXCLUSIONS102 | "Reject under the existing installation envelope." | Implementation acceptance | Replacement introduces previously excluded behavior: reject under existing installation envelope. | not stated | 9 |
| BEHAVIOR_EXCLUSIONS103 | "No false all-crash or recovery guarantee." | Implementation acceptance | Only Trap excluded but another terminal cause remains: must not give false all-crash or recovery guarantee. | not stated | 9 |
| BEHAVIOR_EXCLUSIONS104 | "Exercise real source selection, dependency compilation, generated source, independent evidence consumption, and physical realization where required; a hand-built empty summary does not close the customer." | Implementation acceptance | Acceptance testing must use real selection, compilation, generated source, evidence consumption, and physical realization; hand-built empty summary insufficient. | not stated | 9 |
| BEHAVIOR_EXCLUSIONS105 | "Any unresolved semantic or trust amendment must be raised in [owner questions](../../../OWNER_QUESTIONS.md) before relying on it, not silently chosen by an implementation." | Implementation acceptance | Unresolved semantic or trust amendments must be raised in owner questions before reliance; implementations must not silently choose them. | not stated | 9 |

## Ambiguous sentences (both readings)

| quoted sentence | reading A | reading B |
| --- | --- | --- |
| "An authoritative build may forbid behavior in its exact selected executable composition, independently of conservative callable declarations." | The build may forbid behaviors beyond what conservative callable declarations alone would forbid. | The build may forbid behavior only within the exact selected executable composition, and this power is independent from (does not replace) conservative callable declarations. |
| "This is a product-admission requirement, not an effect mask or an assertion-specific mode." | Behavior exclusions are enforced only at product admission, never elsewhere. | Behavior exclusions are characterized as product-admission requirements as opposed to being categorized as effect masks or assertion-specific modes (without denying other enforcement contexts). |
| "A build without a Trap exclusion cannot authorize a Trap forbidden by an intermediate interface." | If the build omits a Trap exclusion, it cannot authorize Trap even when an intermediate interface would allow Trap. | If the build omits a Trap exclusion, it cannot add authorization for Trap in situations where an intermediate interface already forbids Trap. |
| "A declaration permitting Trap cannot override the build's exclusion." | When the build excludes Trap, declarations that permit Trap are ignored for admission purposes. | Callable declarations cannot be used to negate or opt out of a build-level Trap exclusion. |
| "The product check may establish a stronger fact from verified selected implementations than their broad published allowances." | Stronger facts apply only to implementations that were both selected and verified. | The product check may derive stronger facts from any verified evidence about selected implementations, even if those implementations' published allowances are broad. |
| "An operation explicitly reaching Console still counts when its selected provider is silent." | Console reach is determined by the operation's explicit reach annotation, regardless of whether the selected provider performs Console I/O. | A silently implemented provider still leaves the operation counted as Console-reaching if the operation is explicitly marked as reaching Console. |
| "Merely calling a trait named Console does not manufacture that row." | Naming a trait `Console` in a call site does not add a Console service row unless reach is explicit by other rules. | A dynamic call through a trait type named Console never contributes a service row, even if the trait is an authorized boundary trait. |
| "Unrecognized crash causes, unknown classifications, and ambiguous identities reject." | Authoring an exclusion with unrecognized/unknown/ambiguous operands rejects at authoring time. | The whole product admission rejects when checking encounters unrecognized causes, unknown classifications, or ambiguous identities. |
| "Only executing the call selects the exclusion." | Exclusion is recorded only if the call appears on a dynamically executed path (runtime). | Exclusion is recorded only if the call is in the executed build configuration path (static build script execution), not merely present in source. |
| "Repeated selections combine by set union; duplicates are idempotent, order cannot change the result, and later selections cannot remove earlier restrictions." | Later calls cannot remove restrictions selected by earlier calls in the same build script execution. | Later build configurations or compositions cannot remove restrictions established by an earlier admitted product. |
| "Receiver-policy restrictions apply at explicit admission, not by default to artifact production." | Receiver-policy restrictions apply when admitting an executable, not when emitting non-executable artifacts by default. | Receiver-policy restrictions apply at admission events explicitly marked as such, not automatically during every artifact production step. |
| "Selection itself grants neither authority nor permission to change a requirement's meaning." | Selecting providers/implementations does not change requirement meanings. | Selecting exclusions on the Build value does not grant permission to alter requirement semantics in source. |
| "An unused unselected implementation in the source inventory need not satisfy a product exclusion, but remains subject to ordinary source checks." | Code not linked into the selected closure need not meet exclusions but must still type-check and meet ordinary source rules. | Source units present in the package but not selected as the implementation for a slot need not meet product exclusions. |
| "Consumer exclusions cover dependency contributions throughout that product's execution closure." | All dependencies' behavior in the execution closure must respect the consumer's exclusions. | Exclusions apply to what dependencies contribute at runtime within the closure, not to their independent publication metadata alone. |
| "Exclusions do not automatically apply to resolver activity, build-helper execution, or unrelated target children; those retain their own admission contracts." | Build-time resolver/helpers are outside exclusion checking unless their own admission applies exclusions. | Runtime of unrelated target children is outside the consumer execution closure for exclusion purposes. |
| "Absence checking may use stronger facts only when independently justified for the exact selected composition." | Stronger facts are allowed only with evidence tied to the exact selected composition. | Stronger facts beyond declarations are allowed whenever there is independent justification, but only for that composition (not for general API refinement). |
| "A direct abstract boundary invocation retains its explicitly declared service row even when native lowering erases the call." | Semantic absence checking must still count the service row after native erasure. | The declared service row remains part of the contract baseline even if the native layer has no call instruction. |
| "Sound guard proofs may exclude impossible paths under the admitted root premises." | Guard proofs may justify absence by ruling out paths impossible given root premises. | Only guard proofs that are sound may be used to exclude paths; unsound proofs must not count. |
| "The checker need not decide arbitrary semantic reachability: unsupported or incomplete evidence is failure to establish the exclusion." | Failure to prove absence is always a diagnostic failure, never silent acceptance. | The checker may give up on hard reachability questions and treat that as failure to establish exclusion (without requiring a full reachability decision procedure). |
| "A provider currently selected behind a mutable slot is insufficient unless replacement is constrained to preserve the exclusion." | Current mutable-slot selection never suffices without a replacement constraint. | Current selection suffices only when replacement is statically constrained to preserve the exclusion (otherwise insufficient). |
| "No private source need be exposed to author lookup, and no consumer may merely strip a dependency's declared allowance." | Consumers must use stronger checked evidence, not declaration stripping, and need not force disclosure of private dependency source. | Absence may be shown without exposing private source to the author, but consumers cannot establish absence only by ignoring declared allowances. |
| "A Trap exclusion does not exclude Abort or ProcessExit." | Trap exclusion checks only Trap outcomes; Abort and ProcessExit must be analyzed separately if excluded. | Selecting Trap exclusion must not be reported as excluding all crash-like termination including Abort and ProcessExit. |
| "The build can then reject the checking composition and accept the verified no-op composition under a Trap exclusion, without altering those source declarations." | Different Build selections on the same source may admit or reject without changing declarations. | Trap exclusion causes rejection of checking builds and acceptance of verified no-op builds while source text stays identical. |
| "An assertion repeating an admitted fact may disappear under ordinary optimization; preserving a check against assumptions requires a separate design." | Optimizer may remove redundant assertions of already-admitted facts. | Assertions that only repeat admissions may be removed; checks that guard against false assumptions need explicit non-admission design to survive optimization. |
| "an application Trap exclusion does not forbid a failing check in the separately admitted test closure" | Application Trap exclusion does not apply to the test root's closure. | Trap exclusion on the application does not forbid Trap used only to fail a test assertion in the test admission. |
| "May satisfy the physical class exclusion while failing the service exclusion." | Admission may pass physical class check and fail service check for the same composition (normative permission). | Such a composition is an example that sometimes satisfies physical exclusion without satisfying Console service exclusion (descriptive). |
| "No false all-crash or recovery guarantee." | Diagnostics and publication must not imply all crash causes are excluded when only Trap is excluded. | The checker must not certify recovery or absence of all crashes when other terminal causes remain possible. |

## Terms used without definition in this chapter

- authoritative build
- conservative callable declarations
- product-admission requirement
- effect mask
- assertion-specific mode
- checking implementation
- no-op assertion implementation
- optional logging
- public contracts
- configurations
- published crash ceilings
- service summaries
- service reach
- operational ceilings
- intermediate interface
- Private inference
- public crash coverage
- direct boundary declarations
- static callback dependencies
- fixed external requirements
- verified selected implementations
- broad published allowances
- callable's signature
- narrower requirement
- ordinary callers
- crash-effect polymorphism
- automatic API refinement
- root
- finite sets
- exact exclusions
- Crash cause
- Trap
- Abort
- Abstract service
- service contribution
- boundary requirement
- primitive operation
- boundary-parent closure
- Physical authority class
- terminal class
- shared mechanism-classification semantics
- Console
- selected provider
- trait named Console
- no-op logger
- Console-reaching operation
- Readable service
- service permission and terminal authority
- authorized declarations
- string labels
- versioned encoding
- installation-envelope coverage
- portable wire
- replacement support
- compiler-owned operation
- namespace
- activation
- activation's original Build value
- authorized helpers
- boundary trait
- executable dependency
- user-authored machine
- receiver-style calls
- implicit builder argument
- Build prelude
- PhysicalAuthorityClass
- ProcessOutput
- payload-free cases
- closed terminal-authority vocabulary
- evaluated value
- toolchain case identity
- selecting machine
- authored source span
- canonical union
- same-spelled user declaration
- Build selections
- product configuration
- dependency requests
- admitted build
- set union
- target restrictions
- source restrictions
- authority restrictions
- receiver-policy restrictions
- explicit admission
- artifact production
- permission grant table
- debug/release switch
- checked static applications
- provider mechanisms
- Build provider slot
- requirement's meaning
- target child
- executable root
- component closure
- package acquired by the build
- admitted entry
- generated adapters
- argument evaluation
- callbacks
- timers
- workers
- cleanup
- retained providers
- source inventory
- ordinary source checks
- execution closure
- Consumer exclusions
- dependency contributions
- independently published components
- receiving composition
- resolver activity
- build-helper execution
- unrelated target children
- admission contracts
- artifact-only output
- checked executable
- generated source
- root/provider selections
- executable product
- authored build execution
- build callback
- unfinished product
- staged execution
- product-selection boundary
- sound conservative account
- semantic operations
- closed calls
- verified realization evidence
- contract-checking baseline
- independently justified
- direct abstract boundary invocation
- service row
- native lowering
- Native instruction scanning
- producer-written empty row
- test run
- semantic absence
- Sound guard proofs
- admitted root premises
- arbitrary semantic reachability
- Optional optimization selection
- no-op assertion customer
- inlining
- dead-code-elimination pass
- native realization
- accounted mechanisms
- static selected call
- exact application
- dynamic calls
- installation-bound calls
- admitted target
- fixed contract
- mutable slot
- opaque dependency
- precompiled dependency
- may-Trap
- may-Console
- stronger checked evidence
- author lookup
- Retained evidence
- exclusion sets
- entry roster
- semantic subjects
- target/realization identities
- checker schema
- explicitly accepted assumptions
- cached success
- equal native bytes
- unverified host contract
- attributable call/entry/provider context
- witnessed runtime execution
- partial successful product
- exact product admission
- owner-authorized component installation/replacement envelope
- receiving composition
- joined closure
- old receipt
- weaker policy
- Runtime rebinding
- new entries
- owner-authorized composition
- Candidate code
- hardware failures
- termination
- cleanup completion
- availability
- confidentiality
- recovery
- ProcessExit
- component custody
- external survivor obligations
- assertion
- shared requirement
- checking/no-op
- guarded Trap
- Public wrappers
- explicit allowances
- checking composition
- verified no-op composition
- unrelated possible Trap
- asserted condition
- subsequent safe operations
- optional diagnostics
- eager
- divergence
- borrow validity
- ownership obligations
- safely removable computation
- delayed computation
- named machine arguments
- zero-overhead guarantee
- arbitrary preconditions
- postconditions
- admissions
- admitted fact
- implicit unwind
- flush
- admission and diagnostics rules
- runtime checking
- Build-time tests
- mandatory checks
- verdict
- optional checking/no-op diagnostic contract
- registered requirement
- satisfiers
- attribute
- global build switch
- Test and application roots
- separately admitted test closure
- public Trap ceilings
- possible failing assertion
- assertion-specific omission
- intermediate API
- verified implementation evidence
- Console invocation
- Console boundary invocation
- physical silence
- process-output mechanisms
- Unknown foreign behavior
- absence certification
- evidence gap
- provenance
- canonical restriction set
- independent child outcome
- Optimization families
- semantic selection
- Independent consumer
- installation envelope
- terminal cause
- all-crash guarantee
- recovery guarantee
- dependency compilation
- independent evidence consumption
- hand-built empty summary
- API/codec work
- implementation limits
- owning tasks
- unresolved semantic or trust amendment
- owner questions

CHAPTER COMPLETE

