# Service reach and operational ceilings (effects.md L1–326)

Source: `wiki/spec/language/effects.md` lines 1–326 (read).

## Rules

| rule id | the exact sentence, quoted | the heading it is under | what must be checked, in plain words | when it must hold | lane |
| --- | --- | --- | --- | --- | --- |
| EFFECTS1 | "`reaches A + B` \| Bounds service reach by the named boundary services and their boundary parents." | Declarations | `reaches A + B` must bound normalized reach to named services and boundary parents. | not stated | 3 |
| EFFECTS2 | "`reaches none` \| Explicitly bounds service reach by the empty set." | Declarations | `reaches none` must bound normalized reach to the empty set. | not stated | 3 |
| EFFECTS3 | "`invokes handler` \| May synchronously enter that binding before returning." | Declarations | Direct synchronous handler invocations must be checked against an `invokes handler` clause when present. | at a call | 3 |
| EFFECTS4 | "`suspends` \| May park the current activation." | Declarations | Possible suspension must be reflected in the `suspends` ceiling and call acknowledgements. | not stated | 3 |
| EFFECTS5 | "`blocks` \| May occupy its worker while waiting." | Declarations | Possible worker blocking must be reflected in the `blocks` ceiling and call acknowledgements. | not stated | 3 |
| EFFECTS6 | "`crashes Cause` with route guards \| May leave without a successor or cleanup under a covered guard." | Declarations | Guarded crash routes must be authorized only under matching `crashes Cause` clauses with route guards. | on every edge | 3 |
| EFFECTS7 | "`crashes none` \| Promises no crash route of any cause." | Declarations | Programs promising `crashes none` must have no crash routes of any cause in checked summary. | not stated | 3 |
| EFFECTS8 | "declaring or calling one does not implicitly add its identity." | Declarations | Declaring or calling a boundary trait must not implicitly add that trait's identity to the service reach row. | not stated | 3 |
| EFFECTS9 | "A boundary call contributes its requirement's explicit reach ceiling, with boundary-parent closure." | Declarations | A boundary call must contribute reach from the requirement's explicit ceiling including boundary-parent closure. | at a call | 3 |
| EFFECTS10 | "Every boundary-trait machine requirement must declare that ceiling, using `reaches none` when it permits no services, and must likewise write `crashes` under open callable contracts." | Declarations | Boundary-trait machine requirements must declare a reach ceiling (`reaches none` when no services) and a `crashes` clause per open callable contracts. | not stated | 3 |
| EFFECTS11 | "A memberless service-identity trait has no operation requiring an annotation." | Declarations | Memberless service-identity traits must not require effect annotations on operations. | not stated | 7 |
| EFFECTS12 | "Ordinary traits are not service identities." | Declarations | Ordinary traits must not be treated as namable service identities in reach rows. | not stated | 7 |
| EFFECTS13 | "Mutation through ordinary borrows adds no service identity;" | Declarations | Local mutation through ordinary borrows must not add a service identity to reach. | at the operation | 2 |
| EFFECTS14 | "A checked body may omit `reaches`, including when it directly calls a boundary operation." | Declarations | Omitted `reaches` on a checked body must trigger reach inference rather than rejection. | not stated | 3 |
| EFFECTS15 | "Its reach is then inferred from every checked call and pinned requirement contract." | Declarations | Inferred reach must union contributions from every checked call and pinned requirement contract. | not stated | 3 |
| EFFECTS16 | "`main` has no annotation exemption because no checked body needs one." | Declarations | `main` must follow the same reach omission and inference rules as other checked bodies. | not stated | 3 |
| EFFECTS17 | "The complete derived reach must fit its normalized ceiling." | Declarations | Normalized derived reach must be a subset of the authored normalized ceiling when a clause is present. | not stated | 3 |
| EFFECTS18 | "`reaches A + B` permits a body reaching only `A`;" | Declarations | An authored `reaches A + B` ceiling must accept bodies whose derived reach is only `A`. | not stated | 3 |
| EFFECTS19 | "`reaches none` rejects any nonempty derived service row." | Declarations | `reaches none` must reject programs whose derived service row is nonempty after normalization. | not stated | 3 |
| EFFECTS20 | "The keyword with no value is malformed." | Declarations | A `reaches` keyword without a value must be rejected as malformed. | not stated | 3 |
| EFFECTS21 | "Omission requests inference and is distinct from an explicit empty clause." | Declarations | Omitted `reaches` must trigger inference and must not be treated as an explicit empty clause. | not stated | 3 |
| EFFECTS22 | "Reach does not count local mutation as a service," | Declarations | Reach derivation must not count local mutation as a service contribution. | not stated | 3 |
| EFFECTS23 | "All machine clauses have no clause-ending semicolon." | Declarations | Machine effect clauses must not end with a semicolon before the next clause or body. | not stated | 10 |
| EFFECTS24 | "A checked body writes `reaches none suspends blocks` followed directly by `{ ... }`." | Declarations | Checked machine bodies must place effect clauses immediately before `{ ... }` without intervening semicolons. | not stated | 10 |
| EFFECTS25 | "do not insert semicolons between their clauses." | Declarations | Nested callable requirement headers must not have semicolons between clauses. | not stated | 10 |
| EFFECTS26 | "Boundary requirements publish their explicit ceilings." | Declarations | Boundary requirements must publish their explicit reach (and related) ceilings in the interface contract. | not stated | 3 |
| EFFECTS27 | "Opaque supplies inherit the governing requirement's admitted contract, not an inferred native-body summary;" | Declarations | Opaque provider checking must use the governing requirement's admitted contract, not a native-body reach summary. | not stated | 9 |
| EFFECTS28 | "A selected implementation must fit every authored requirement ceiling even when its own optional clause is absent." | Declarations | Selected implementations must satisfy every authored requirement ceiling even if the implementation omits its own reach clause. | at a call | 3 |
| EFFECTS29 | "Foreign and assumed supplies retain their separate trust admission and physical authority classifications." | Declarations | Foreign and assumed supplies must keep separate trust admission and physical authority checking. | not stated | 9 |
| EFFECTS30 | "Pure arithmetic may have a selected boundary implementation without acquiring an arithmetic-service reach." | Declarations | Selecting a boundary implementation for pure arithmetic must not by itself add an arithmetic-service reach row. | not stated | 3 |
| EFFECTS31 | "Reach declarations and propagation grant no capability or provider authority." | Declarations | Reach checking must not treat reach clauses as granting capability or provider authority. | not stated | 3 |
| EFFECTS32 | "Negation and Boolean row-restriction syntax are outside this contract." | Declarations | Negation and Boolean row-restriction syntax must be rejected or ignored as outside this contract. | not stated | 3 |
| EFFECTS33 | "The compiler must publish a checked reach summary whether or not the author wrote a clause." | Declarations | A checked reach summary must always be published in checked output. | not stated | 3 |
| EFFECTS34 | "An explicit published ceiling may conservatively remain wider than its checked body;" | Declarations | When an authored ceiling is present, published reach may be the authored ceiling even if wider than the body's derived reach. | not stated | 3 |
| EFFECTS35 | "Omission on a bodyless non-boundary requirement defers to the selected implementation's published reach." | Deferred requirement reach | Bodyless non-boundary requirements with omitted `reaches` must defer reach to the selected implementation's published reach. | not stated | 3 |
| EFFECTS36 | "The generic carries a row dependency until that selection is known." | Deferred requirement reach | Until implementation selection is known, generics must carry an unresolved reach row dependency. | not stated | 7 |
| EFFECTS37 | "Checked body, including public APIs \| Infer from checked calls." | Deferred requirement reach | Checked bodies with omitted `reaches` must infer reach from checked calls. | not stated | 3 |
| EFFECTS38 | "Authored clause \| Bound the derived reach." | Deferred requirement reach | When a checked body has an authored `reaches` clause, derived reach must be bounded by that clause. | not stated | 3 |
| EFFECTS39 | "Bodyless non-boundary requirement \| Open: defer to the selected implementation's published reach." | Deferred requirement reach | Bodyless non-boundary requirements with omitted `reaches` must remain open and defer to selected implementation reach. | not stated | 3 |
| EFFECTS40 | "`reaches none` requires empty reach." | Deferred requirement reach | `reaches none` on a bodyless non-boundary requirement must require admitted implementations to have empty reach. | not stated | 3 |
| EFFECTS41 | "Boundary requirement \| Reject the open contract." | Deferred requirement reach | Boundary requirements with omitted `reaches` must reject as an open contract in a closed position. | not stated | 3 |
| EFFECTS42 | "Publish a stable ceiling across providers." | Deferred requirement reach | Boundary requirements with authored `reaches` must publish a stable ceiling across providers. | not stated | 3 |
| EFFECTS43 | "Runtime callable type \| Reject the open contract." | Deferred requirement reach | Runtime callable types with omitted `reaches` must reject as an open contract in a closed position. | not stated | 3 |
| EFFECTS44 | "Publish a fixed finite ceiling; `reaches none` states empty reach." | Deferred requirement reach | Runtime callable types with authored `reaches` must publish a fixed finite ceiling; `reaches none` means empty reach. | not stated | 3 |
| EFFECTS45 | "Every public machine publishes its checked summary, including unresolved static dependencies on generic interfaces." | Deferred requirement reach | Public machines must publish checked summaries including unresolved static generic reach dependencies. | not stated | 3 |
| EFFECTS46 | "A requirement relying on service-free behavior must state `reaches none`;" | Deferred requirement reach | Requirements that rely on service-free behavior must author `reaches none`. | not stated | 7 |
| EFFECTS47 | "current pure implementations cannot supply that guarantee to an otherwise open generic body." | Deferred requirement reach | Pure implementations must not be accepted as satisfying service-free guarantees for otherwise open generic requirement bodies. | not stated | 7 |
| EFFECTS48 | "No `reaches _` spelling is introduced on originating requirements;" | Deferred requirement reach | Originating requirements must not use a `reaches _` spelling. | not stated | 7 |
| EFFECTS49 | "`reaches` \| Open. No service bound is authored; the selected operation's published reach is retained as a dependency." | Open callable contracts | Callable contracts omitting `reaches` must be open and retain the selected operation's published reach as a dependency after compile-time selection. | not stated | 3 |
| EFFECTS50 | "`crashes` \| Open. No crash bound is authored; the selected operation's published guarded crash routes are retained." | Open callable contracts | Callable contracts omitting `crashes` must be open and retain the selected operation's published crash routes as a dependency. | not stated | 3 |
| EFFECTS51 | "`suspends`, `blocks` \| Does not suspend; does not block." | Open callable contracts | Callable contracts omitting `suspends` and `blocks` must default to does-not-suspend and does-not-block. | not stated | 3 |
| EFFECTS52 | "`terminates` \| No termination promise." | Open callable contracts | Callable contracts omitting `terminates` must carry no termination promise from that omission alone. | not stated | 4 |
| EFFECTS53 | "An open contract admits only a selection made at compile time, where its dependency is resolved by substituting the selected operation's published contract." | Open callable contracts | Open `reaches`/`crashes` contracts must admit only compile-time selections that resolve dependencies via the selected operation's published contract. | not stated | 3 |
| EFFECTS54 | "A closed position needs both clauses written, with `none` for empty:" | Open callable contracts | Closed positions must have both `reaches` and `crashes` written, using `none` for empty bounds. | not stated | 3 |
| EFFECTS55 | "An open contract in a closed position rejects, and the diagnostic names the missing clause." | Open callable contracts | Open reach or crash contracts in closed positions must reject and name the missing clause in the diagnostic. | not stated | 3 |
| EFFECTS56 | "No position reinterprets an omitted clause as empty," | Open callable contracts | No syntactic position may treat an omitted `reaches` or `crashes` as `none` without explicit authoring. | not stated | 3 |
| EFFECTS57 | "Checking a statically known selection against an explicitly closed destination is ordinary contract refinement." | Open callable contracts | Statically known selections against closed destinations must be checked as ordinary contract refinement. | at a call | 3 |
| EFFECTS58 | "A checked machine definition is not a callable contract and keeps the rules below: its body infers reach, a private body may infer the other axes, and a public body with no `crashes` clause promises none." | Open callable contracts | Checked machine definitions must use body inference rules; private bodies may infer suspension/blocking/crashes; public bodies without `crashes` must promise none. | not stated | 10 |
| EFFECTS59 | "Their family interface names enter the row only if explicitly named, never merely because provider selection uses those traits." | Implementation boundaries and capability identities | Family interface names must enter the reach row only when explicitly named, not merely from provider selection traits. | not stated | 9 |
| EFFECTS60 | "Ordinary portable helpers propagate the declared capability rows." | Implementation boundaries and capability identities | Portable helpers must propagate declared capability rows per the usual rules. | not stated | 3 |
| EFFECTS61 | "Naming it in a row creates neither a provider-selection demand nor a `Binding` value." | Implementation boundaries and capability identities | Naming a boundary trait in a reach row must not create provider-selection demand or a `Binding` value. | not stated | 3 |
| EFFECTS62 | "Actual boundary calls still need their own selected implementation and applicable authority." | Implementation boundaries and capability identities | Boundary calls must still require selected implementation and applicable authority regardless of row naming. | at a call | 3 |
| EFFECTS63 | "Audit registration changes presentation and acceptance, not reach, inheritance, authority or attenuation." | Implementation boundaries and capability identities | Audit registration must not change reach, inheritance, authority, or attenuation checking. | not stated | 9 |
| EFFECTS64 | "On published surfaces omitted `suspends` means never parks and omitted `blocks` means never blocks a worker." | Implementation boundaries and capability identities | Published callable surfaces with omitted `suspends`/`blocks` must be checked as never suspend / never block. | not stated | 3 |
| EFFECTS65 | "On a public checked body omitted `crashes` means no crash route, exactly as `crashes none`." | Implementation boundaries and capability identities | Public checked bodies omitting `crashes` must be treated as `crashes none`. | not stated | 3 |
| EFFECTS66 | "Private checked bodies may omit any of the three to request inference." | Implementation boundaries and capability identities | Private checked bodies omitting suspension/blocking/crash clauses must allow inference for those axes. | not stated | 3 |
| EFFECTS67 | "On a bodyless non-boundary requirement omitted `crashes` defers to the selected implementation," | Implementation boundaries and capability identities | Bodyless non-boundary requirements with omitted `crashes` must defer crash bounds to the selected implementation. | not stated | 3 |
| EFFECTS68 | "`crashes none` admits only crash-free selections." | Implementation boundaries and capability identities | `crashes none` on bodyless non-boundary requirements must admit only implementations with no crash routes. | not stated | 3 |
| EFFECTS69 | "A boundary requirement and a runtime callable type must write the clause under open callable contracts." | Implementation boundaries and capability identities | Boundary requirements and runtime callable types must explicitly write `crashes` (closed position). | not stated | 3 |
| EFFECTS70 | "their call-site markers are checked against a known bound." | Implementation boundaries and capability identities | Call-site suspension and blocking markers must be checked against the published bound on published surfaces. | at a call | 3 |
| EFFECTS71 | "a public machine that can crash states its causes and guards, and callers permit or disprove them." | Implementation boundaries and capability identities | Public machines with crash routes must state causes and guards; callers must permit or disprove those crash possibilities. | at a call | 3 |
| EFFECTS72 | "a body cannot reach beyond the authority it holds," | Implementation boundaries and capability identities | Derived reach must not exceed held authority. | not stated | 3 |
| EFFECTS73 | "A public generic that admits a suspending, blocking or crashing callback declares the corresponding ceiling." | Implementation boundaries and capability identities | Public generics admitting suspending, blocking, or crashing callbacks must declare the corresponding operational ceilings. | not stated | 7 |
| EFFECTS74 | "Recoverable failure and cancellation are result sums, not `fails` clauses." | Implementation boundaries and capability identities | Recoverable failure must be modeled as result sums; `fails` clauses must not be introduced for them. | not stated | 1 |
| EFFECTS75 | "Calls and providers propagate it through the same conservative ceilings as other services." | Process-exit reach | `ProcessExit` reach must propagate through calls and providers under conservative ceilings like other services. | at a call | 3 |
| EFFECTS76 | "The ceiling permits exit; actual domain-bound capability authority is still required." | Process-exit reach | `reaches ProcessExit` permits exit only in combination with required domain-bound capability authority. | at a call | 3 |
| EFFECTS77 | "Console I/O alone grants neither," | Process-exit reach | Console I/O alone must not satisfy ProcessExit authority requirements. | not stated | 3 |
| EFFECTS78 | "importing core or selecting a provider creates no authority." | Process-exit reach | Importing core or selecting a provider must not by itself create exit authority. | not stated | 9 |
| EFFECTS79 | "A helper that may exit retains its normal continuation when it returns." | Process-exit reach | Helpers that may exit must retain normal continuation when they return normally. | at normal return | 3 |
| EFFECTS80 | "Reach alone supplies neither a guaranteed exit nor a guarded guarantee of return." | Process-exit reach | Reach annotations alone must not be treated as guaranteeing exit or return. | not stated | 3 |
| EFFECTS81 | "`terminates` independently excludes divergence under its premises; it does not authorize an endpoint." | Process-exit reach | `terminates` must exclude divergence only under its premises and must not authorize ProcessExit or other endpoints. | not stated | 4 |
| EFFECTS82 | "No completion keyword or second effect row is introduced." | Process-exit reach | Process exit must not add a completion keyword or second effect row beyond existing rules. | not stated | 3 |
| EFFECTS83 | "Exhaustive transitions handle its cases," | Recoverable outcomes and strict use | Recoverable failure result cases must be handled under exhaustive transition rules. | on every edge | 1 |
| EFFECTS84 | "a case payload and its guarantees are available only on the corresponding arm." | Recoverable outcomes and strict use | Result-sum case payloads and guarantees must be available only on the matching transition arm. | on every edge | 1 |
| EFFECTS85 | "A non-Unit call result cannot be silently discarded." | Recoverable outcomes and strict use | Non-Unit call results used without binding or explicit discard must reject. | at the operation | 2 |
| EFFECTS86 | "Intentional discard is `_ = call();`, subject to the result's ordinary ownership and legal-disposition obligations;" | Recoverable outcomes and strict use | Discard must use `_ = call();` and still satisfy ownership and legal-disposition obligations on the result. | at the operation | 2 |
| EFFECTS87 | "the marker cannot erase linear custody." | Recoverable outcomes and strict use | The discard marker must not erase linear custody obligations. | at the operation | 2 |
| EFFECTS88 | "Failure propagation is an explicit checked edge returning the caller's outcome." | Recoverable outcomes and strict use | Failure propagation must be an explicit checked edge returning the caller's outcome type. | on every edge | 3 |
| EFFECTS89 | "There is no implicit propagation operator `?` or `fails` clause." | Recoverable outcomes and strict use | Implicit `?` propagation and `fails` clauses must not be accepted. | not stated | 3 |
| EFFECTS90 | "An unproved obligation rejects;" | Recoverable outcomes and strict use | Unproved proof obligations must reject compilation. | not stated | 8 |
| EFFECTS91 | "prover incompleteness neither constructs an error result nor authorizes a trap or abort." | Recoverable outcomes and strict use | Prover incompleteness must not fabricate error results or authorize trap/abort. | not stated | 8 |
| EFFECTS92 | "There is no implicit `expect`/`unwrap` escape from proof obligations." | Recoverable outcomes and strict use | Implicit expect/unwrap escapes from proof obligations must not be provided. | not stated | 8 |
| EFFECTS93 | "A failure case proved unreachable needs no executable handler, under ordinary exhaustive transition checking." | Recoverable outcomes and strict use | Unreachable failure cases need no executable handler when exhaustive transition checking proves unreachability. | not stated | 8 |
| EFFECTS94 | "Resolve each authored service occurrence to its exact boundary-trait identity." | Normalization and composition | Each authored service occurrence must resolve to an exact boundary-trait identity for normalization. | not stated | 3 |
| EFFECTS95 | "parents(R) subset-of normalize(R)" | Normalization and composition | Normalization must include boundary-parent closure so parents of R are subset of normalize(R). | not stated | 3 |
| EFFECTS96 | "Duplicate source occurrences retain their provenance but add no duplicate semantic member." | Normalization and composition | Duplicate occurrences keep provenance but must not add duplicate semantic row members. | not stated | 3 |
| EFFECTS97 | "There is no ranking or name-based service discovery." | Normalization and composition | Reach normalization must not use ranking or name-based service discovery. | not stated | 3 |
| EFFECTS98 | "derived body reach     subset-of authored ceiling, when a clause is present" | Normalization and composition | Derived body reach must be subset of authored ceiling when a reach clause is present. | not stated | 3 |
| EFFECTS99 | "published reach        = authored ceiling, when a clause is present" | Normalization and composition | Published reach must equal authored ceiling when a clause is present. | not stated | 3 |
| EFFECTS100 | "published reach        = derived body reach, otherwise" | Normalization and composition | Published reach must equal derived body reach when no authored reach clause is present. | not stated | 3 |
| EFFECTS101 | "provider reach         subset-of requirement reach ceiling, when one is declared" | Normalization and composition | Provider reach must be subset of the requirement's declared reach ceiling. | not stated | 3 |
| EFFECTS102 | "body/callee/provider may_suspend implies corresponding ceiling permits suspension" | Normalization and composition | If body, callee, or provider may suspend, the corresponding ceiling must permit suspension. | not stated | 3 |
| EFFECTS103 | "body/callee/provider may_block   implies corresponding ceiling permits blocking" | Normalization and composition | If body, callee, or provider may block, the corresponding ceiling must permit blocking. | not stated | 3 |
| EFFECTS104 | "Reach derivation computes conservative fixed points over recursive checked-call components." | Normalization and composition | Reach derivation over recursion must compute conservative fixed points. | not stated | 3 |
| EFFECTS105 | "Known private helpers are included transitively." | Normalization and composition | Known private helpers must be included transitively in reach derivation. | not stated | 3 |
| EFFECTS106 | "Imported checked machines supply their published summaries, without requiring access to private source." | Normalization and composition | Reach from imported checked machines must use published summaries only. | not stated | 3 |
| EFFECTS107 | "Boundary calls and dynamic calls without retained narrower evidence use their pinned requirement ceilings, not a narrower provider body eventually selected by Build or at run time." | Normalization and composition | Without narrower evidence, boundary and dynamic calls must use pinned requirement ceilings, not narrower selected provider bodies. | at a call | 3 |
| EFFECTS108 | "A checked machine-value construction instead retains the selected implementation's available checked contract as described below." | Normalization and composition | Checked machine-value construction must retain the selected implementation's available checked contract for reach/effects. | at the operation | 3 |
| EFFECTS109 | "Service union and suspension/blocking booleans remain separate;" | Normalization and composition | Service reach union must remain separate from suspension and blocking boolean checking. | not stated | 3 |
| EFFECTS110 | "operational acknowledgements use the applicable retained call envelope." | Normalization and composition | Call-site operational acknowledgements must use the applicable retained call envelope. | at a call | 3 |
| EFFECTS111 | "Every known checked helper on the path propagates the corresponding obligations." | Normalization and composition | Known checked helpers on call paths must propagate corresponding reach and operational obligations. | at a call | 3 |
| EFFECTS112 | "Static generic calls follow the dependency rule below." | Normalization and composition | Static generic calls must follow static binder reach dependency rules referenced from this chapter. | at a call | 7 |
| EFFECTS113 | "When checking establishes which machine a callable value selects, its calls use that machine's retained checked operational contract rather than the callable type's potentially wider bound." | Known dynamic selections | When selection is established, calls must use the retained checked operational contract of the selected machine, not the wider callable type bound. | at a call | 3 |
| EFFECTS114 | "Construction, forwarding and storage do not by themselves discard that established evidence." | Known dynamic selections | Construction, forwarding, and storage must not alone discard established dynamic selection evidence. | not stated | 3 |
| EFFECTS115 | "Joins retain all possible demands and only guarantees justified for every possible selection; an unknown alternative contributes the requirement ceiling." | Known dynamic selections | Join points must retain all possible operational demands; guarantees must hold for every possible selection; unknown alternatives use requirement ceiling. | on every edge | 3 |
| EFFECTS116 | "Narrowing uses an available checked contract, not private-source inspection or an optimization's prediction." | Known dynamic selections | Narrowing must use available checked contracts only, not private source or optimization predictions. | not stated | 3 |
| EFFECTS117 | "Public and explicitly annotated callable ceilings remain published bounds; a smaller implementation body does not silently rewrite one." | Known dynamic selections | A smaller implementation body must not silently rewrite public or annotated callable ceilings. | not stated | 3 |
| EFFECTS118 | "A reusable body accepting an unknown machine value must remain valid for every fitting selection." | Known dynamic selections | Reusable bodies with unknown machine values must type-check and contract-check for every fitting selection. | not stated | 3 |
| EFFECTS119 | "It may publish checked dependencies on the input's envelope, but cannot assume the harmless selection used by one current caller." | Known dynamic selections | Such bodies may publish envelope dependencies but must not assume a particular caller's selection for soundness. | not stated | 3 |
| EFFECTS120 | "A narrower authored requirement or retained checked specialization/dependency justifies a narrower application, not the observed set of callers." | Known dynamic selections | Narrower application must be justified by requirement or specialization, not by observed callers alone. | not stated | 3 |
| EFFECTS121 | "Missing selection evidence falls back to the governing requirement contract, not to a guessed implementation." | Known dynamic selections | Without selection evidence, checking must use the governing requirement contract, not guess an implementation. | at a call | 3 |
| EFFECTS122 | "It changes neither boundary requirements selected by Build nor their admission," | Known dynamic selections | Known-dynamic narrowing must not change Build-selected boundary requirements or their admission rules. | not stated | 9 |
| EFFECTS123 | "and it creates no runtime authority." | Known dynamic selections | Known-dynamic narrowing must not create runtime authority. | not stated | 3 |
| EFFECTS124 | "This does not shrink those ceilings, alter generic body checking, or make a broad callable satisfy a narrower requirement." | Selected-product exclusions | Build-level behavior exclusions must not shrink published ceilings, alter generic body checking, or make broad callables satisfy narrower requirements. | not stated | 9 |
| EFFECTS125 | "A product-level absence claim must retain exact selections, complete entry/call coverage and independently checked evidence." | Selected-product exclusions | Product-level absence claims must retain exact selections, full entry/call coverage, and independent checked evidence. | not stated | 9 |
| EFFECTS126 | "Opaque or open calls retain their contract conservatively unless stronger applicable evidence exists." | Selected-product exclusions | Opaque or open calls must retain conservative contracts unless stronger applicable evidence exists. | at a call | 3 |
| EFFECTS127 | "No optional optimization, provider choice alone, or caller permission can rewrite a public summary." | Selected-product exclusions | Optional optimization, provider choice alone, or caller permission must not rewrite a public summary. | not stated | 9 |
| EFFECTS128 | "An actual boundary invocation retains its explicit abstract reach row even when a silent realization performs no physical I/O." | Selected-product exclusions | Boundary invocations must retain explicit abstract reach rows even when realization performs no physical I/O. | at a call | 3 |
| EFFECTS129 | "Its requirement identity remains separately available for provider selection and audit, even with empty reach." | Selected-product exclusions | Boundary requirement identity must remain available for provider selection and audit even when reach is empty. | not stated | 9 |
| EFFECTS130 | "Ordinary calls continue to use the contract and dependency rules above." | Selected-product exclusions | Ordinary calls under behavior exclusions must still use contract and dependency rules from this chapter. | at a call | 3 |
| EFFECTS131 | "Verified implementation evidence may establish that a composition cannot exhibit behavior permitted by its published callable ceilings." | Selected-product exclusions | Verified implementation evidence may support product absence claims without rewriting callable ceilings during ordinary checking. | not stated | 9 |
| EFFECTS132 | "A wake operation may reach a scheduler without suspending." | Declarations | Wake operations that reach a scheduler must be allowed without requiring a `suspends` clause solely for that reach. | not stated | 3 |
| EFFECTS133 | "If `Filesystem` inherits `Readable`, reaching `Filesystem` also reaches `Readable`." | Normalization and composition | Normalization must expand reach to include inherited boundary-parent identities such as `Readable` when reaching `Filesystem`. | not stated | 3 |
| EFFECTS134 | "Boundary traits supply identities that may be named in `reaches`;" | Declarations | Names in `reaches` must resolve to identities supplied by boundary traits. | not stated | 3 |

## Ambiguous sentences (both readings)

| quoted sentence | reading A | reading B |
| --- | --- | --- |
| "A present clause is an upper bound, not an assertion of exact equality and not an additive contribution." | The authored ceiling is only an upper bound on derived reach, never a lower bound or exact-equality claim. | The clause does not add to other ceilings compositely; it bounds independently of other contributions. |
| "empty reach promises neither termination nor absence of blocking or crashes." | Programs with empty reach may still terminate, block, or crash; the compiler must not infer those negatives from empty reach alone. | Authors of `reaches none` are not promising termination or non-blocking/non-crashing behavior in the same clause. |
| "An explicit published ceiling may conservatively remain wider than its checked body;" | Published summary may list a wider ceiling than the body's derived reach when the author wrote a wider clause. | Review artifacts may keep declared ceiling wider even when checking proved a narrower derived reach. |
| "current pure implementations cannot supply that guarantee to an otherwise open generic body." | A pure implementation cannot close an open generic requirement's reach at satisfaction time. | Service-free pure code cannot be used to prove `reaches none` for callers of open generic APIs without explicit `reaches none` on the requirement. |
| "A contract that omits `reaches` or `crashes` is open." | Omitting either axis makes that axis open independently. | A contract is open only when both omitted clauses are treated together as leaving the whole contract without fixed bounds. |
| "a signature copied from a requirement into a field type cannot change from \"whatever the selection does\" to \"nothing\" without the author writing `reaches none` or `crashes none`." | Copying preserves openness until explicit `none` is written in the closed position. | Type copying never changes contract meaning; only explicit `none` in the new closed position closes it. |
| "On published surfaces omitted `suspends` means never parks and omitted `blocks` means never blocks a worker." | Applies only to callable contracts on published surfaces (requirements, trait members, callable types). | Applies to any published machine summary including public checked bodies' published interfaces. |
| "Private checked bodies may omit any of the three to request inference." | Each axis may be omitted independently on private bodies to request inference for that axis. | Inference is requested only when all three axes are omitted together. |
| "Service reach remains inferred on public bodies:" | Public checked bodies never author reach; reach is always inferred on them. | Public bodies may still author `reaches` ceilings, but body checking still derives reach from calls subject to the ceiling. |
| "May-ceilings permit behavior; they do not assert that every run performs it." | `suspends`/`blocks`/`reaches` clauses are permission bits, not must-happen facts. | The compiler must not require static evidence that permitted behavior occurs on every execution path. |
| "Reach alone supplies neither a guaranteed exit nor a guarded guarantee of return." | `reaches ProcessExit` does not prove the call exits or returns on any path. | Reach metadata is insufficient for proof obligations about exit or return without authority and control-flow facts. |
| "prover incompleteness neither constructs an error result nor authorizes a trap or abort." | The compiler must not codegen error/trap from proof failure alone. | Incomplete provers must not be treated as proving failure cases real for effects checking. |
| "Joins retain all possible demands and only guarantees justified for every possible selection;" | At control-flow joins, operational demands union; guarantees must be valid for all selections still possible. | Join typing for callable values unions demands but may narrow guarantees only with join evidence valid for all arms. |
| "This does not shrink those ceilings, alter generic body checking, or make a broad callable satisfy a narrower requirement." | Behavior exclusions are orthogonal to ceiling strings used in ordinary checking. | Product admission may prove absence but must not feed back into broad types satisfying narrow requirements during type checking. |
| "No optional optimization, provider choice alone, or caller permission can rewrite a public summary." | Optimizations and provider picks cannot change published checked summaries. | Caller-side permissions cannot retroactively narrow what was published on a public API summary. |

## Terms used without definition in this chapter

- service reach
- operational ceilings
- direct synchronous invocations
- possible suspension
- worker blocking
- effect row
- boundary services
- boundary parents
- handler binding
- route guards
- boundary traits
- service identity
- wake operation
- scheduler
- optional-reach-ceilings
- checked body
- pinned requirement contract
- normalized ceiling
- derived reach
- service row
- machine clauses
- bodyless declaration
- declaration semicolon
- nested callable requirements
- header delimiters
- opaque supplies
- native-body summary
- realizer
- foreign supplies
- assumed supplies
- pure arithmetic
- arithmetic-service reach
- checked reach summary
- manifest admission
- `Build::audit_service`
- deferred-callback-reach
- bodyless non-boundary requirement
- row dependency
- structural machine requirements
- nominal machine requirements
- conformance evidence binders
- runtime callable type
- open contract
- closed position
- callable contract
- `requires machine` requirement
- static selection
- contract refinement
- checked machine definition
- OPEN-CALLABLE-CONTRACTS
- OS-specific provider interface
- family interface names
- capability rows
- provider-selection demand
- `Binding` value
- critical-service audit policy
- call-site markers
- domain-ending outcome
- dependency terms
- static binder contract dependencies
- may-ceilings
- `fails` clauses
- ProcessExit requirement
- domain-bound capability authority
- Console I/O
- terminal transfer
- recoverable failure
- result sum
- exhaustive transitions
- legal-disposition obligations
- linear custody
- failure propagation
- prover incompleteness
- Trapping operation
- boundary-trait identity
- semantic row
- boundary-parent closure
- direct invocation
- ranking
- name-based service discovery
- may_suspend
- may_block
- conservative fixed points
- recursive checked-call components
- machine-value construction
- operational acknowledgements
- call envelope
- known-dynamic-contract-narrowing
- callable value
- in-memory reader
- filesystem reach
- specialization
- runtime callable envelopes
- selected-product exclusions
- product-level absence claim
- entry/call coverage
- silent realization
- physical I/O
- abstract reach row
- transparent-refinement form
- explicit-callable-reach
- open-callable-contracts
- mandatory direct-call annotations
- additive checked-body rows

CHAPTER COMPLETE
