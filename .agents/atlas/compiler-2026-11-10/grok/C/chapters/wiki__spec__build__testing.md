# Requirement-based test groups

Source: `wiki/spec/build/testing.md` lines 1–327 (read).

## Rules

| rule id | the exact sentence, quoted | the heading it is under | what must be checked, in plain words | when it must hold | lane |
| --- | --- | --- | --- | --- | --- |
| TESTING1 | "Tests are ordinary checked machines satisfying an explicitly registered machine requirement." | Requirement-based test groups | Each test machine must type-check and satisfy an explicitly registered machine requirement. | not stated | 7 |
| TESTING2 | "it does not introduce a machine species, trusted boundary, or special calling permission." | Requirement-based test groups | Build test-root selection must not create a new machine species, trusted boundary, or special calling permission. | not stated | 9 |
| TESTING3 | "The runner executes each enabled test across its configured target/engine plan before reporting the requested build successful." | Requirement-based test groups | Every enabled test must run on its configured target/engine plan before a successful build is reported. | not stated | 9 |
| TESTING4 | "Test discovery and verdict policy do not justify a second interpreter or weaker loan/custody rules." | Requirement-based test groups | Test machinery must not weaken loan/custody checking or add a second interpreter semantics path. | not stated | 2 |
| TESTING5 | "A fresh guest instance does not roll back effects in a genuine host provider." | Requirement-based test groups | Fresh guest state must not be modeled as rolling back genuine host provider effects. | not stated | 9 |
| TESTING6 | "Register an exact ordinary trait machine requirement, not a trait name or a method-name convention." | Registration and discovery | `group` registration must use an exact ordinary trait machine requirement operand. | at a call | 9 |
| TESTING7 | "Ambiguous overload paths reject rather than selecting by visible satisfiers:" | Registration and discovery | Ambiguous requirement overload paths must reject instead of picking among visible satisfiers. | at a call | 9 |
| TESTING8 | "`group` is a compiler-owned Build operation." | Registration and discovery | Only the compiler-owned `group` Build operation may register test groups. | at a call | 9 |
| TESTING9 | "Its requirement operand, and the service/provider operands of `service`, use the existing non-executing product-reference rules" | Registration and discovery | `group` and `service` operands must follow non-executing product-reference rules from scoped execution. | at a call | 9 |
| TESTING10 | "They resolve under the author's product scope to exact declaration applications, not host imports, strings, spelling matches, or evaluator table indices." | Registration and discovery | Operands must resolve to exact declaration applications in product scope; forbidden resolution forms must reject. | at a call | 9 |
| TESTING11 | "A same-named user machine gains no product-selection privilege." | Registration and discovery | User machines with the same spelling as Build operations must not gain product-selection privilege. | not stated | 9 |
| TESTING12 | "Registration is evaluated configuration in the same Build activation, not a new dependency edge." | Registration and discovery | Registration must be same-activation evaluated configuration, not a dependency edge. | not stated | 9 |
| TESTING13 | "Its product references remain inert until the final dependency join." | Registration and discovery | Product references in registration must stay inert until the final dependency join. | not stated | 9 |
| TESTING14 | "Each registration creates one group for the current package occurrence and selected target." | Registration and discovery | Each registration must create exactly one group for the current package occurrence and target. | not stated | 9 |
| TESTING15 | "the normalized result retains selections, not an escaping mutable handle." | Registration and discovery | Normalized registration result must retain selections only, not an escaping mutable handle. | not stated | 9 |
| TESTING16 | "Duplicate registration of the same requirement application rejects." | Registration and discovery | Registering the same requirement application twice must reject. | at a call | 9 |
| TESTING17 | "There is no privileged `run`, `default`, `Test`, or test attribute." | Registration and discovery | The compiler must not treat `run`, `default`, `Test`, or a test attribute as privileged registration. | not stated | 9 |
| TESTING18 | "After the package's final source roster is checked, collect every concrete, executable, package-local machine with a checked exact satisfaction edge to the registered requirement." | Registration and discovery | Discovery must collect concrete executable package-local machines with checked exact satisfaction to the registered requirement after final roster checking. | not stated | 9 |
| TESTING19 | "Multiple satisfiers are separate tests, not ambiguous provider candidates." | Registration and discovery | Each satisfier must become a separate test; they must not be merged as ambiguous provider candidates. | not stated | 9 |
| TESTING20 | "An inherited requirement retains its declaring identity; an alias or wrapper trait does not manufacture a second group identity." | Registration and discovery | Group identity must follow declaring requirement identity; alias or wrapper traits must not create duplicate group identity. | not stated | 7 |
| TESTING21 | "Discovery includes checked generated satisfiers of an already registered authored requirement, but cannot retroactively change the admitted build or register a requirement introduced by that build's generated source." | Registration and discovery | Generated satisfiers may be discovered only within registered authored requirements; retroactive build change or new requirement registration from generated source is forbidden. | not stated | 9 |
| TESTING22 | "Private tests run in their owning package's test scope without becoming public or granting other tests access to private declarations." | Registration and discovery | Private tests must stay in owning package test scope without becoming public or granting cross-test private access. | not stated | SHARED |
| TESTING23 | "Imported dependencies' tests are not automatically collected." | Registration and discovery | Dependency package tests must not be auto-collected into the consumer's roster. | not stated | 9 |
| TESTING24 | "A workspace member is tested when selected as a build root, not merely because it is a dependency or catalog entry." | Registration and discovery | Only workspace members selected as build roots enter testing; dependency or catalog membership alone must not trigger collection. | not stated | 9 |
| TESTING25 | "Discovery retains exact machine, requirement, package, and target identities and reports a deterministic roster." | Registration and discovery | Discovery output must retain exact identities and be deterministic. | not stated | 9 |
| TESTING26 | "It does not guess generic arguments or enumerate possible specializations:" | Registration and discovery | Discovery must not guess generic arguments or enumerate specializations. | not stated | 7 |
| TESTING27 | "an open generic satisfier needs an ordinary concrete wrapper before it is runnable." | Registration and discovery | Open generic satisfiers must be treated non-runnable until wrapped in an ordinary concrete wrapper. | not stated | 7 |
| TESTING28 | "Report non-runnable candidates rather than silently omitting them." | Registration and discovery | Non-runnable discovered candidates must be reported, not silently dropped. | not stated | 9 |
| TESTING29 | "A registered group with no tests reports zero tests; it is not evidence that any test passed." | Registration and discovery | Empty groups must report zero tests and must not count as passing evidence. | not stated | 9 |
| TESTING30 | "A runnable entry has no observed result and receives only arguments established by its explicit installed runner contract." | Entry contracts and ordinary calls | Runnable test entries must have no observed result and only runner-contract arguments. | at entry | 3 |
| TESTING31 | "A parameterless test constructs its fixture in ordinary local or static storage." | Entry contracts and ordinary calls | Parameterless tests must build fixtures in ordinary local or static storage per entry rules. | at entry | 1 |
| TESTING32 | "A parameterized root is an installed ordinary call whose contract supplies each argument and its custody; a mutable parameter alone requests no allocation." | Entry contracts and ordinary calls | Parameterized test roots must be installed ordinary calls supplying arguments and custody; mutable parameter alone must not imply allocation. | at a call | 2 |
| TESTING33 | "Boundary calls in the checked test closure demand exact provider slots independently of that context; missing, ambiguous, or unauthorized supply rejects before invocation." | Entry contracts and ordinary calls | Boundary calls in the test closure must demand exact provider slots; missing, ambiguous, or unauthorized supply must reject before invoke. | at a call | 9 |
| TESTING34 | "Provider selection constructs no source routing value." | Entry contracts and ordinary calls | Provider selection for tests must not construct a source routing value. | at a call | 9 |
| TESTING35 | "The runner's installation must separately provide its actual argument." | Entry contracts and ordinary calls | For fixture-bearing requirements, the runner installation must separately supply the fixture argument value. | at entry | 9 |
| TESTING36 | "Every satisfier matches that declared parameter." | Entry contracts and ordinary calls | Each test satisfier must match the declared fixture parameter type/application. | not stated | 7 |
| TESTING37 | "Register different concrete applications separately; the runner neither infers a data subject nor guesses generic arguments or synthesizes a whole-trait map." | Entry contracts and ordinary calls | Distinct concrete trait applications must register separately; runner must not infer subjects, guess generics, or synthesize whole-trait maps. | not stated | 7 |
| TESTING38 | "The runner establishes all entry preconditions; registration grants none." | Entry contracts and ordinary calls | Entry preconditions must come from runner installation, not from registration alone. | at entry | 3 |
| TESTING39 | "A group may permit `Trap`; `satisfies` inherits that contract, not permission to bypass an intermediate contract." | Entry contracts and ordinary calls | Trap permission on the group flows to satisfiers via `satisfies` without bypassing intermediate contracts. | not stated | 3 |
| TESTING40 | "Services required by a test must fit both its requirement and the actual grant." | Entry contracts and ordinary calls | Required services must be compatible with both the requirement and the granted provider closure. | not stated | 9 |
| TESTING41 | "Inheritance cannot widen a parent's service-free requirement." | Entry contracts and ordinary calls | Inherited requirements must not widen a parent's service-free contract. | not stated | 7 |
| TESTING42 | "do not put build policy in the pure evaluator or add a parallel test compiler." | Entry contracts and ordinary calls | Test build policy must stay in build/product orchestration, not the pure evaluator or a parallel test compiler. | not stated | SHARED |
| TESTING43 | "Any ordinary machine may call a test machine subject to normal visibility and contract checking." | Entry contracts and ordinary calls | Calls to test machines must use ordinary visibility and contract checking only. | at a call | 3 |
| TESTING44 | "A test entry is a root for the runner, not a bodyless `boundary machine`, a new trusted assumption, or a privileged function." | Entry contracts and ordinary calls | Test entries must be runner roots only, not bodyless boundary machines, trusted assumptions, or privileged functions. | at entry | 9 |
| TESTING45 | "No test-only main, call permission, or automatic crash allowance is introduced." | Entry contracts and ordinary calls | The language must not add test-only mains, call permissions, or automatic crash allowances. | not stated | SHARED |
| TESTING46 | "It is not core vocabulary, a compiler-recognized identity, or a fixed taxonomy of categories." | Entry contracts and ordinary calls | `testing::Test::run` must not be treated as core vocabulary, compiler identity, or fixed taxonomy. | not stated | 9 |
| TESTING47 | "the compiler invents neither." | Entry contracts and ordinary calls | The compiler must not invent `Test::run` dependencies or registrations for templates. | not stated | 9 |
| TESTING48 | "Naming a requirement `default` does not register it." | Entry contracts and ordinary calls | A requirement named `default` must not auto-register as a test group. | not stated | 9 |
| TESTING49 | "`group.disabled` and `builder.tests.disabled` default to false, so ZII configuration does not silently disable testing." | Group configuration and authority | Disable flags must default false so configuration cannot silently disable tests. | not stated | 9 |
| TESTING50 | "Effective enablement requires neither disable flag to be set." | Group configuration and authority | A group is enabled only when neither disable flag is set. | not stated | 9 |
| TESTING51 | "it does not make dependency requests target-dependent or grant ambient environment access." | Group configuration and authority | Test configuration must not make dependency requests target-dependent or grant ambient environment access. | not stated | 9 |
| TESTING52 | "This contract introduces no debug/release mode or special CLI profile." | Group configuration and authority | This contract must not introduce debug/release modes or special CLI profiles for tests. | not stated | 9 |
| TESTING53 | "Ordinary provider compatibility, uniqueness, establishment, and admission apply." | Group configuration and authority | `service` selections must use ordinary provider compatibility, uniqueness, establishment, and admission checks. | at a call | 9 |
| TESTING54 | "It grants no capability." | Group configuration and authority | `service` registration must not by itself grant capabilities. | not stated | 9 |
| TESTING55 | "Benign captured output and a private in-memory filesystem require no approval ceremony, but are explicitly selected rather than inferred from names." | Group configuration and authority | Benign providers must be explicitly selected; name inference must not select them. | at a call | 9 |
| TESTING56 | "Disabled groups create no execution grant and are reported as disabled, not passed; their declarations remain subject to ordinary source checking." | Group configuration and authority | Disabled groups must not execute, must report disabled not passed, and declarations must still be source-checked. | not stated | SHARED |
| TESTING57 | "descendant package builds and groups can only attenuate them." | Group configuration and authority | Child builds and groups must only attenuate root-delegated authority, never widen it. | not stated | 9 |
| TESTING58 | "Selecting a host provider in a child cannot replace a root-selected virtual filesystem." | Group configuration and authority | Child host provider selection must not replace a root-selected virtual filesystem. | at a call | 9 |
| TESTING59 | "Operations supported by the delegated virtual interface use that backing; real-only or otherwise unavailable operations reject without a host fallback." | Group configuration and authority | Delegated virtual backing must be used for supported ops; real-only/unavailable ops must reject without host fallback. | at the operation | 9 |
| TESTING60 | "Restricted requests from the enabled test closure, including its fixtures and providers, enter install/update review before execution." | Group configuration and authority | Restricted requests in enabled test closure must enter install/update review before running tests. | not stated | 9 |
| TESTING61 | "New or widened requests require matching lock acceptance and actual executor grants." | Group configuration and authority | New or widened authority requests must match lock acceptance and executor grants. | not stated | 9 |
| TESTING62 | "A dependency's lock, runtime permission, group declaration, or std implementation supplies neither." | Group configuration and authority | Dependencies must not substitute for lock acceptance or executor grants. | not stated | 9 |
| TESTING63 | "Missing analysis is not an empty request." | Group configuration and authority | Missing restricted-request analysis must not be treated as an empty request. | not stated | 9 |
| TESTING64 | "Disabled/unselected configurations make no execution claim; later enabling them requires fresh applicable checking." | Group configuration and authority | Disabled configs make no execution claim; re-enablement needs fresh checking. | not stated | 9 |
| TESTING65 | "Host-backed operations need an admitted execution protocol with enforceable scope." | Group configuration and authority | Host-backed test operations require an admitted execution protocol with enforceable scope. | not stated | 9 |
| TESTING66 | "Selecting native execution grants no additional authority: unsupported providers cannot escape their configured backing through native code or ambient host APIs." | Group configuration and authority | Native engine selection must not grant extra authority; providers cannot escape configured backing via native or host APIs. | not stated | 9 |
| TESTING67 | "Unknown operations or missing environment support reject before execution, never pass as no-ops." | Group configuration and authority | Unknown assembly ops or missing assembly environment must reject before execution, never pass as no-ops. | not stated | 10 |
| TESTING68 | "Cross-compilation support alone is not execution availability." | Execution plan | Cross-compile capability must not count as engine execution availability on the host. | not stated | 9 |
| TESTING69 | "Unset engine configuration uses this default; it does not disable tests." | Execution plan | Default engine selection applies when unset; unset must not disable tests. | not stated | 9 |
| TESTING70 | "Exact API spelling is not a new language construct." | Execution plan | Build test engine configuration API spelling must not introduce new language constructs. | not stated | 9 |
| TESTING71 | "Before test effects, publish the execution plan with each test's exact group, target, engine, selected bindings and status: scheduled, explicitly disabled, or unavailable with a reason." | Execution plan | Execution plan with per-test status must be published before test effects. | not stated | 9 |
| TESTING72 | "Missing host adapter or engine support is not a test pass." | Execution plan | Missing adapter or engine support must not count as passing a test. | not stated | 9 |
| TESTING73 | "An unavailable required combination makes the run incomplete and prevents successful build publication;" | Execution plan | Unavailable required target/engine combos must mark run incomplete and block successful build publication. | not stated | 9 |
| TESTING74 | "Every enabled test must execute on at least one engine, and every scheduled invocation must pass." | Execution plan | Each enabled test must run on ≥1 engine; each scheduled invocation must pass for build success. | not stated | 9 |
| TESTING75 | "A started invocation that fails or exhausts its budget cannot be reclassified as optional unavailability after the fact." | Execution plan | Failed or budget-exhausted invocations must not be reclassified as optional unavailability post hoc. | not stated | 9 |
| TESTING76 | "Malformed bindings or missing authority reject; availability probing must not conceal an admission failure." | Execution plan | Malformed bindings or missing authority must reject; probing must not hide admission failures. | not stated | 9 |
| TESTING77 | "Engine-specific adapters must honor those services and exact selected foreign bindings;" | Execution plan | Engine adapters must honor configured services and selected foreign bindings. | not stated | 9 |
| TESTING78 | "a virtual filesystem cannot become a real filesystem by choosing native execution." | Execution plan | Native engine choice must not turn virtual filesystem backing into real filesystem. | not stated | 9 |
| TESTING79 | "There is no silent fallback or engine-specific weakening of the test oracle." | Execution plan | Tests must not silently fallback engines or weaken the oracle per engine. | not stated | 9 |
| TESTING80 | "deterministic comparison observations must agree." | Execution plan | Deterministic comparison observations must agree across engines that run. | not stated | SHARED |
| TESTING81 | "Neither engine executes merely because its compilation succeeded." | Execution plan | Compilation success alone must not count as executing a test on that engine. | not stated | 9 |
| TESTING82 | "Automatic build-time tests cannot execute assumed binding contracts or use their assumptions, including through fixtures, helpers, provider selections and erased evidence." | Scheduling, execution, and outcomes | Build-time test runner must not execute or use runtime-only assumed binding contracts or their assumptions. | not stated | 8 |
| TESTING83 | "Runtime acceptance and live-host grants do not override this restriction." | Scheduling, execution, and outcomes | Runtime acceptance and live-host grants must not override the assumed-binding prohibition in automatic tests. | not stated | 9 |
| TESTING84 | "A separately executed runtime harness is a runtime product under its own acceptance, not permission to launch the prohibited relationship inside the automatic build-time runner." | Scheduling, execution, and outcomes | Separate runtime harness acceptance must not permit prohibited assumed-binding use inside the build-time runner. | not stated | 9 |
| TESTING85 | "Execute admitted build configuration and freeze its group selections." | Scheduling, execution, and outcomes | Lifecycle step 1: admitted build configuration runs and group selections freeze. | not stated | 9 |
| TESTING86 | "Finish source checking and discovery; construct separate test roots and provider closures for each selected package/target occurrence and expose its target/engine execution plan." | Scheduling, execution, and outcomes | Lifecycle step 2: after checking/discovery, construct separate test roots and provider closures per package/target and publish engine plan. | not stated | 9 |
| TESTING87 | "Reconstruct test-time authority requests, check lock agreement and actual grants, and verify Psi and entry provisioning before execution." | Scheduling, execution, and outcomes | Lifecycle step 3: authority requests, locks, grants, Psi, and entry provisioning verified before execution. | not stated | 9 |
| TESTING88 | "Run every scheduled test/engine invocation with fresh activation, context, captured-output and mock-provider state." | Scheduling, execution, and outcomes | Lifecycle step 4: scheduled invocations run with fresh activation/context and isolated capture/mock state. | not stated | 9 |
| TESTING89 | "Settle its ordinary owned resources on normal completion." | Scheduling, execution, and outcomes | Each invocation must settle ordinary owned resources on normal completion. | at normal return | 2 |
| TESTING90 | "Require the execution plan's pass and coverage conditions before publishing the new successful build result set." | Scheduling, execution, and outcomes | Lifecycle step 5: pass and coverage conditions must hold before publishing successful build results. | not stated | 9 |
| TESTING91 | "Possible `Trap` and service reach are legitimate declared test behavior, not grounds for pretending the invocation is a constant evaluation." | Scheduling, execution, and outcomes | Tests must not be treated as constant/hermetic evaluation because they may Trap or reach services. | not stated | 9 |
| TESTING92 | "a host-native-only operation does not acquire a Psi implementation by being used in a test." | Scheduling, execution, and outcomes | Host-native-only operations used in tests must not gain a Psi implementation implicitly. | not stated | 9 |
| TESTING93 | "Exhaustion is not catchable test behavior." | Scheduling, execution, and outcomes | Work/storage exhaustion must not be catchable as ordinary test behavior. | not stated | 4 |
| TESTING94 | "No termination proof is inferred from a timeout, and no termination guarantee is waived when one is declared." | Scheduling, execution, and outcomes | Timeouts must not infer termination proofs or waive declared termination guarantees. | not stated | 4 |
| TESTING95 | "Parallel execution must preserve per-invocation state and actual authority isolation." | Scheduling, execution, and outcomes | Parallel test runs must isolate per-invocation state and authority. | not stated | 9 |
| TESTING96 | "Report results in test/target/engine roster order, not host completion order;" | Scheduling, execution, and outcomes | Results must be reported in roster order, not completion order. | not stated | 9 |
| TESTING97 | "registration supplies no cross-test shared fixture state." | Scheduling, execution, and outcomes | Registration must not supply shared fixture state across tests. | not stated | 9 |
| TESTING98 | "Normal return is a pass only after required child activations and custody have settled or transferred under the ordinary contract." | Scheduling, execution, and outcomes | Normal return counts as pass only after child activations and custody settle or transfer. | at normal return | 2 |
| TESTING99 | "Normal completion and settled obligations" | Scheduling, execution, and outcomes | Table: normal completion with settled obligations is pass for that invocation only. | at normal return | 2 |
| TESTING100 | "Contract-permitted `Trap` or `Abort`" | Scheduling, execution, and outcomes | Table: contract-permitted Trap or Abort is failed test and blocks successful build publication. | not stated | 3 |
| TESTING101 | "Process exit instead of returning to the runner" | Scheduling, execution, and outcomes | Table: process exit without return to runner is not a pass; report terminal outcome. | not stated | 9 |
| TESTING102 | "Work/storage exhaustion during a scheduled invocation" | Scheduling, execution, and outcomes | Table: budget exhaustion is incomplete execution and blocks successful build. | not stated | 4 |
| TESTING103 | "Required engine unavailable, or enabled test has no runnable engine" | Scheduling, execution, and outcomes | Table: required engine unavailable or no runnable engine is incomplete execution and blocks build. | not stated | 9 |
| TESTING104 | "Optional engine unavailable before execution" | Scheduling, execution, and outcomes | Table: optional engine unavailable must report not run with reason; never count as pass. | not stated | 9 |
| TESTING105 | "Malformed contract, missing authority, or invalid evidence" | Scheduling, execution, and outcomes | Table: malformed contract, missing authority, or invalid evidence must reject before affected execution. | not stated | SHARED |
| TESTING106 | "Interpreter/checker invariant failure" | Scheduling, execution, and outcomes | Table: interpreter/checker invariant failure is internal failure, never ordinary test failure. | not stated | SHARED |
| TESTING107 | "Disabled group or explicitly excluded engine" | Scheduling, execution, and outcomes | Table: disabled group or excluded engine is not run and never reported passed. | not stated | 9 |
| TESTING108 | "Trap handling belongs to the runner's containment boundary, not language-level unwinding or cleanup." | Scheduling, execution, and outcomes | Trap handling must use runner containment, not language unwinding/cleanup semantics. | not stated | 9 |
| TESTING109 | "It cannot reclaim externally held resources merely by discarding interpreter memory or assuming a native trap is recoverable." | Scheduling, execution, and outcomes | Trap containment must not claim to reclaim external resources via discarding memory or assuming native trap recovery. | not stated | 9 |
| TESTING110 | "Continue other tests only when the failed activation can be safely contained; otherwise stop without successful publication." | Scheduling, execution, and outcomes | Further tests may run only if failed activation is safely contained; otherwise stop without successful publication. | not stated | 9 |
| TESTING111 | "A test verdict must use a mandatory checked comparison or validation machine, not a checking/no-op diagnostic whose no-op selection would silently pass it." | Assertions and trusted claims | Test verdicts must use mandatory checked comparison/validation; no-op diagnostic contracts must not silently pass. | not stated | SHARED |
| TESTING112 | "Production may exclude `Trap` while the separate test closure permits it; neither setting rewrites callable contracts." | Assertions and trusted claims | Trap exclusion on production vs test closure must not rewrite callable contracts. | not stated | 9 |
| TESTING113 | "Tests of admitted guarantees must not assume the guarantee being tested to establish their verdict." | Assertions and trusted claims | Guarantee tests must not assume the guarantee under test when forming the verdict. | not stated | 8 |
| TESTING114 | "Registration adds no optimization barrier or alternate proof semantics." | Assertions and trusted claims | Test registration must not add optimization barriers or alternate proof semantics. | not stated | 8 |
| TESTING115 | "Test roots and application roots are separate selections over checked source." | Products, reuse, and native harnesses | Application and test roots must be separate checked selections. | not stated | 9 |
| TESTING116 | "Their exclusive entries, fixtures and providers are not roots of the published application Psi or native image." | Products, reuse, and native harnesses | Test-only entries, fixtures, and providers must not be application product roots. | not stated | 9 |
| TESTING117 | "Do not rely on optional dead-code elimination or remove still-called code after testing." | Products, reuse, and native harnesses | Application builds must not rely on DCE or post-test removal to drop still-called test code. | not stated | 9 |
| TESTING118 | "If application code calls a test machine, that ordinary reachable code belongs to the application and must satisfy its contracts and exclusions." | Products, reuse, and native harnesses | Application-reachable code calling test machines must satisfy application contracts and exclusions. | at a call | 3 |
| TESTING119 | "A different target, provider closure, root-sensitive fact or optimization premise requires the affected work again; equal source bytes alone do not permit reuse." | Products, reuse, and native harnesses | Reuse of checked/lowered artifacts requires matching premises; equal bytes alone insufficient. | not stated | 9 |
| TESTING120 | "Produce a native test image only for a scheduled native run; never append it to the application image." | Products, reuse, and native harnesses | Native test images must only be produced for scheduled native runs and must not append to application image. | not stated | 9 |
| TESTING121 | "It does not mark its main as a test, gain private visibility, or implicitly collect the groups." | Products, reuse, and native harnesses | Custom native harness main must not become a test main, gain private visibility, or auto-collect groups. | not stated | 9 |
| TESTING122 | "Neither creates a special source-level test main." | Products, reuse, and native harnesses | Neither build runner nor custom harness may introduce a special source-level test main. | not stated | 9 |
| TESTING123 | "Native execution needs its own real containment and authority; an in-process trap does not become recoverable by convention." | Products, reuse, and native harnesses | Native test execution must have real containment/authority; in-process trap must not become recoverable by convention. | not stated | 9 |
| TESTING124 | "a `pass` case must check," | The language corpus | Corpus `pass` packages must successfully check. | not stated | SHARED |
| TESTING125 | "a `fail` case must reject with its expected diagnostic fragment," | The language corpus | Corpus `fail` packages must reject with the expected diagnostic fragment. | not stated | SHARED |
| TESTING126 | "a repository-wide Rust harness or a foreign-language driver program does not substitute for one." | The language corpus | Language compiler behavior tests must be `tests/omega` cases, not external harness substitutes. | not stated | 9 |
| TESTING127 | "the native exit status must equal the interpreted one; a disagreement is a compiler defect." | The language corpus | In `both` mode, native and interpreted exit status must match; mismatch is compiler defect. | not stated | SHARED |
| TESTING128 | "A case the interpreter declines records that it was declined rather than a result." | The language corpus | Declined interpreter runs must record declined status, not a faux result. | not stated | 9 |
| TESTING129 | "Require exact discovery, fresh state, distinct failure/incomplete diagnostics, and application outputs free of test-only roots." | Implementation acceptance | Acceptance package must show exact discovery, fresh state, distinct diagnostics, and application outputs without test-only roots. | not stated | 9 |
| TESTING130 | "Helper tests alone do not deliver this build-to-execution contract." | Implementation acceptance | Helper-only tests must not satisfy the full build-to-execution acceptance contract. | not stated | 9 |
| TESTING131 | "a scheduled native test failure or budget exhaustion must not turn into a successful optional skip." | Implementation acceptance | Native test failure or budget exhaustion must not be reported as successful optional skip. | not stated | 9 |
| TESTING132 | "Groups are enabled on registration." | Group configuration and authority | A registered group must be enabled unless explicitly disabled by configuration flags. | not stated | 9 |
| TESTING133 | "`service<Requirement, Provider>()` requests a checked provider selection within the group's scope." | Group configuration and authority | `service` must request checked provider selection scoped to the group. | at a call | 9 |
| TESTING134 | "Tests containing checked assembly use the same supported instruction semantics and explicitly supplied environment." | Group configuration and authority | Test assembly must use supported instruction semantics with an explicitly supplied environment. | not stated | 10 |
| TESTING135 | "For each enabled group and selected target, the default is to run every applicable, permitted engine available on the executing host, including interpreted and native." | Execution plan | Default engine plan must run all applicable permitted engines on the host unless narrowed. | not stated | 9 |
| TESTING136 | "Both engines run the same test contract and inputs against the configured services, with fresh instance-local state." | Execution plan | Each engine must execute the same contract and inputs against configured services with fresh state. | not stated | 9 |
| TESTING137 | "Report each engine's outcome separately;" | Execution plan | Outcomes must be reported per engine, not merged. | not stated | 9 |
| TESTING138 | "Preserve diagnostics on failure; live effects have no rollback." | Scheduling, execution, and outcomes | Failures must preserve diagnostics; live effects must not be rolled back. | not stated | 9 |
| TESTING139 | "Use the existing validation boundary without the disputed assumption." | Assertions and trusted claims | Guarantee tests must use the validation boundary without assuming the guarantee under test. | not stated | 8 |
| TESTING140 | "Reuse source preparation and compatible checked/lowered representations." | Products, reuse, and native harnesses | Test and application builds may reuse compatible checked/lowered artifacts when premises match. | not stated | 9 |
| TESTING141 | "Interpreted tests execute Psi; native tests lower separate runner roots to a test executable." | Products, reuse, and native harnesses | Interpreted tests run Psi; native tests must lower to a separate test executable root. | not stated | 9 |
| TESTING142 | "`tests/omega` is the Omega language's test suite." | The language corpus | Language regression testing must use the `tests/omega` corpus as the authoritative suite. | not stated | 9 |
| TESTING143 | "Compiler behavior is tested by adding a case here;" | The language corpus | Compiler behavior changes must add or extend a `tests/omega` case. | not stated | 9 |
| TESTING144 | "Keep negative controls for inherited-identity aliases, unresolved generic tests, dependency test non-discovery, private access, missing service establishment, child attempts to widen virtual authority, and unaccepted live-host requests." | Implementation acceptance | Acceptance evidence must include negative controls for the listed failure modes. | not stated | 9 |
| TESTING145 | "Run the no-std variant and an ordinary native harness calling a shared test machine." | Implementation acceptance | Acceptance must include a no-std variant and a native harness invoking a shared test machine. | not stated | 9 |
| TESTING146 | "Run a pure group and a virtual-filesystem group on both engines, reporting separate results and matching deterministic observations." | Implementation acceptance | Acceptance must run pure and virtual-filesystem groups on both engines with separate results and matching deterministic observations. | not stated | 9 |
| TESTING147 | "Cover a native-only binding with optional and required interpretation, explicit engine exclusion, no runnable engine, and a native attempt to escape virtual backing." | Implementation acceptance | Acceptance must cover native-only bindings, optional/required interpretation, engine exclusion, no runnable engine, and virtual-backing escape attempts. | not stated | 9 |
| TESTING148 | "Inspect both application products for absent test-only roots;" | Implementation acceptance | Acceptance must verify application Psi and native products omit test-only roots. | not stated | 9 |

## Ambiguous sentences (both readings)

| quoted sentence | reading A | reading B |
| --- | --- | --- |
| "Build configuration selects test roots and their execution environment; it does not introduce a machine species, trusted boundary, or special calling permission." | Build configuration is the only mechanism that picks test roots and environments. | Besides selecting roots/environment, build configuration must not additionally introduce new machine species, boundaries, or call permissions. |
| "Interpreted runner instances use the same embedding lifecycle as other Psi consumers, with selected group providers and fresh guest state." | Test interpreter embedding must follow the same lifecycle rules as any Psi consumer embedding. | Test runs must use embedding lifecycle plus group providers and fresh guest state as additional requirements beyond other consumers. |
| "An individual test need not supply a whole-trait conformance." | A single test machine may satisfy only part of a trait's requirements without rejection. | Discovery must not require whole-trait conformance evidence before listing a satisfier as a test. |
| "Other fixture construction and parameterized helper calls are ordinary test code." | Fixture/helpers are checked as ordinary code inside the test closure only. | Fixture/helpers use ordinary language rules everywhere, including if called from non-test contexts. |
| "Requirements retain ordinary reach, crash, suspension, blocking, termination, ownership, and resource contracts." | Registered requirements do not relax those contract dimensions for tests. | Tests inherit the requirement's ordinary contracts, which may themselves permit Trap or services. |
| "Libraries own test bodies, group contracts and fixtures." | Only library-authored code may contain test bodies (normative ownership). | Test bodies live in libraries by architectural convention; build still accepts test machines authored elsewhere if visible. |
| "Projects may register it, arbitrary project requirements, or both." | A project may register `Test::run`, custom requirements, or register multiple kinds together. | Registration choices are independent per project without mutual exclusion between std convenience and custom requirements. |
| "Benign captured output and a private in-memory filesystem require no approval ceremony, but are explicitly selected rather than inferred from names." | No extra approval workflow is required for those benign providers when explicitly selected. | Benign providers skip approval ceremony entirely even if selection is implicit, as long as names suggest benign intent. |
| "Disabled groups create no execution grant and are reported as disabled, not passed; their declarations remain subject to ordinary source checking." | Disabled groups skip execution but declarations in the group still must pass ordinary checking. | Disabled groups are excluded from execution grants and pass/fail reporting, while source checking applies only to declarations that would run if enabled. |
| "Fresh interpreter state does not isolate arbitrary foreign code, erase live external effects, or roll back a real filesystem." | Implementations must not claim interpreter freshness provides those isolations. | Tests may still use fresh interpreter state for other purposes, but must not treat it as isolation/rollback of host effects. |
| "Private model state is fresh per test unless an admitted shared resource contract says otherwise;" | Fresh private model state is mandatory default for each test invocation. | Shared resource contracts may admit shared model state; otherwise freshness is required. |
| "Cross-compilation support alone is not execution availability." | A target may be cross-compiled without implying the host can execute tests for that target. | Engine availability on the host requires more than toolchain support for the target ABI. |
| "an optional unavailable combination stays visible without blocking another engine's run." | Optional engine failure is reported but does not block other engines. | Optional unavailability is visible in the plan and may still block publication if no engine passes. |
| "This is execution of ordinary target-semantic code by the selected engines, not hermetic semantic-evaluation admission." | Tests must not be admitted under constant-evaluation hermetic rules. | Test code uses target semantics but might still share some evaluation infrastructure with non-hermetic paths. |
| "Its native invocation may run when scheduled and admitted, while interpretation is reported unavailable;" | Native may run even when interpretation unavailable, subject to execution plan publication rules. | Native and interpreter availability are independent; publication outcome depends on which engines are required vs optional. |
| "unset limits inherit policy rather than mean unlimited." | Missing explicit limits pick up sponsor policy defaults. | Unset limits are finite because policy applies, not because tests are unlimited. |
| "Passing does not prove universal correctness, justify an axiom, or grant authority." | A passing test build must not be reported as proof, axiom justification, or authority grant. | Pass verdict is scoped to scheduled invocations only, without broader epistemic or authority claims. |
| "The existing assertion contracts remain unchanged, including eager arguments, public crash allowances, and independent product exclusions." | Test verdict machinery must not alter assertion contract rules elsewhere. | Assertions in tests use the same contracts as production, including unchanged eager evaluation behavior. |
| "Tests may exercise algorithms whose memory safety is already proved; functional results remain a separate subject." | Memory-safety proofs do not automatically establish functional correctness tested by verdict machines. | Tests may call proved-safe algorithms while still needing separate functional checks for results. |
| "Interpreted success alone is not a claim that native lowering, ABI, hardware, or foreign-provider behavior was exercised." | Publication must not infer native/ABI/hardware coverage from interpreter-only success. | Interpreter success is sufficient for build pass when native engine optional or not required. |
| "This differs from the build runner's generated test product, which uses the registered roster and owning-package test scope." | Custom harness and build runner products differ in roster collection and scope rules. | Only the build runner auto-collects registered groups; custom harness must manually invoke visible tests. |
| "and a runnable case (the `run` tier and `*_exit` pass cases) has an ordinary program entry whose exit status and output are its result." | Runnable corpus tiers are defined to include `run` and `*_exit` pass cases with program entry outcomes. | Only cases with ordinary program entries count as runnable; other pass cases merely check. |
| "Native execution on the host proves nothing about another target, so a target-specific result belongs to a case built for that target." | Host-native corpus results are authoritative only for the host target unless the case targets another platform. | Cross-target claims require building/running the case for that target, not inferring from host runs. |
| "Use one small package with two satisfiers of one registered requirement, then vary it through the same path: a failing check, a context with a virtual filesystem, and a second group." | Implementation acceptance requires one reference package demonstrating the listed variations. | Multiple acceptance scenarios may reuse one package path with different satisfiers and groups. |

## Terms used without definition in this chapter

- Requirement-based test groups
- ordinary checked machines
- explicitly registered machine requirement
- build configuration
- test roots
- execution environment
- machine species
- trusted boundary
- special calling permission
- runner
- target/engine plan
- embedding lifecycle
- Psi consumers
- group providers
- fresh guest state
- native runs
- separate test products
- admitted execution resources
- test discovery
- verdict policy
- loan/custody rules
- genuine host provider
- exact ordinary trait machine requirement
- trait name convention
- method-name convention
- ambiguous overload paths
- visible satisfiers
- compiler-owned Build operation
- non-executing product-reference rules
- author's product scope
- exact declaration applications
- host imports
- spelling matches
- evaluator table indices
- product-selection privilege
- Build activation
- dependency edge
- product references
- final dependency join
- package occurrence
- selected target
- normalized result
- escaping mutable handle
- requirement application
- privileged `run`
- test attribute
- final source roster
- concrete executable package-local machine
- checked exact satisfaction edge
- provider candidates
- whole-trait conformance
- declaring identity
- alias trait
- wrapper trait
- group identity
- checked generated satisfiers
- admitted build
- generated source
- test scope
- workspace member
- build root
- catalog entry
- deterministic roster
- open generic satisfier
- concrete wrapper
- non-runnable candidates
- runnable entry
- observed result
- installed runner contract
- parameterless test
- parameterized root
- mutable parameter
- entry storage rules
- WCSU
- checked test closure
- exact provider slots
- source routing value
- fixture-bearing installed requirement
- data subject
- whole-trait map
- entry preconditions
- intermediate contract
- service-free requirement
- trait satisfaction
- pure evaluator
- parallel test compiler
- bodyless boundary machine
- privileged function
- testing::Test::run
- convenience requirement
- core vocabulary
- compiler-recognized identity
- fixed taxonomy
- ZII configuration
- ambient environment access
- debug/release mode
- special CLI profile
- checked provider selection
- provider compatibility
- approval ceremony
- captured output
- private in-memory filesystem
- execution grant
- authority hierarchy
- backing services
- bounded capabilities
- virtual filesystem
- delegated virtual interface
- host fallback
- install/update review
- restricted requests
- lock acceptance
- executor grants
- missing analysis
- execution protocol
- enforceable scope
- checked assembly
- private model state
- shared resource contract
- work-budget pauses
- machine-owner authority
- host machine authority
- target environment
- execution engine
- cross-compilation support
- execution availability
- execution plan
- host adapter
- scheduled invocation
- optional unavailability
- availability probing
- admission failure
- instance-local state
- engine-specific adapters
- foreign bindings
- test oracle
- deterministic comparison observations
- assumed binding contracts
- runtime harness
- automatic build-time runner
- group selections
- provider closures
- package/target occurrence
- test-time authority requests
- captured-output state
- mock-provider state
- successful build result set
- hermetic semantic-evaluation admission
- sponsor-controlled work and storage limits
- cross-test shared fixture state
- child activations
- roster order
- host completion order
- containment boundary
- language-level unwinding
- externally held resources
- mandatory checked comparison
- validation machine
- checking/no-op diagnostic
- optimization barrier
- alternate proof semantics
- admitted guarantees
- validation boundary
- test roots
- application roots
- dead-code elimination
- root-sensitive fact
- optimization premise
- native test image
- application image
- custom native harness
- build runner's generated test product
- registered roster
- owning-package test scope
- source-level test main
- in-process trap
- tests/omega
- pass case
- fail case
- runnable case
- run tier
- *_exit pass cases
- diagnostic fragment
- corpus runner
- omega run --both
- compiler defect
- interpreter declines
- implementation acceptance
- negative controls
- inherited-identity aliases
- unresolved generic tests
- dependency test non-discovery
- missing service establishment
- virtual authority
- live-host requests
- no-std variant
- build-to-execution contract
- native-only binding
- engine exclusion
- virtual backing
- successful optional skip

CHAPTER COMPLETE
