# Task activation and lifecycle

Source: `wiki/spec/build/task_runtime.md` lines 1–279 (read).

## Rules

rule id | the exact sentence, quoted | the heading it is under | what must be checked, in plain words | when it must hold | lane
--- | --- | --- | --- | --- | ---
TASK_RUNTIME1 | "`TaskRuntime::start<WorkResult, Job, Worker::run>(job)` and `TaskRuntime::try_start<WorkResult, Job, Worker::run>(job)` are boundary requirements invoked by path like any other; there is no receiver value, and the provider behind them is a build selection." | Task activation and lifecycle | Start and try-start must use path invocation with no receiver; the provider must come from build selection | at a call | 9
TASK_RUNTIME2 | "The machine argument is a compile-time machine symbol, not capture inference or a runtime function value." | Task activation and lifecycle | The worker machine operand must be a compile-time machine symbol, not capture inference or a runtime function value | at a call | 10
TASK_RUNTIME3 | "There is no bare spawn, implicit detach, future transformation, or privileged supervisor." | Task activation and lifecycle | Programs must not use bare spawn, implicit detach, future transformation, or privileged supervisor forms for task activation | not stated | 10
TASK_RUNTIME4 | "Their acceptance does not change these static entry APIs or grant task authority: use a statically selected trampoline with explicitly passed callable/context data when required, under all ordinary entry, lifetime, effect and custody contracts." | Task activation and lifecycle | Runtime machine values must not alter static entry APIs or grant task authority; required trampolines must be statically selected with explicit callable/context under ordinary contracts | not stated | 9
TASK_RUNTIME5 | "starting/controlling a task still requires a held runtime capability." | Task activation and lifecycle | Task start and control require a held runtime capability | at a call | 9
TASK_RUNTIME6 | "Selection does not grant ambient authority." | Task activation and lifecycle | Build provider selection must not be treated as granting ambient task authority | not stated | 9
TASK_RUNTIME7 | "Inline completion may shorten storage lifetime or embed an outcome, but does not settle the returned linear claim." | Three owners | Inline completion must not discharge the linear `Task<T>` claim returned at start | at normal return | 2
TASK_RUNTIME8 | "The compiler monomorphizes the machine argument and derives an activation description retaining exact machine contract/entry, argument/outcome layouts, target calling plan, whole-call-graph WCSU `StackPlan<M>`, canonical suspension crossings with live carry demands, activation-wide CPU/thread preservation, and cancellation/operational behavior required by the selected start operation." | Activation description and stack | Activation description must retain the listed contract, layout, plan, WCSU, crossings, carry, preservation, and operational facts for the selected start | not stated | 10
TASK_RUNTIME9 | "Planning is a generated artifact, not an implicit representation-identical cast." | Activation description and stack | Stack/plan data must be a generated artifact, not an implicit representation-identical cast | not stated | 10
TASK_RUNTIME10 | "Each local activation receives one fixed nonmoving stack and retains it while parked." | Activation description and stack | Local activations must use one fixed nonmoving stack retained while parked | across a pause | 10
TASK_RUNTIME11 | "Direct suspension allocates no separate continuation buffer." | Activation description and stack | Direct suspension must not allocate a separate continuation buffer | across a pause | 10
TASK_RUNTIME12 | "a numeric fit alone supplies no `StackLease`." | Activation description and stack | Numeric stack fit alone must not establish a `StackLease` | not stated | 2
TASK_RUNTIME13 | "Before execution the provider must establish backing satisfying the exact plan." | Activation description and stack | Provider must establish backing matching the exact stack plan before execution begins | at entry | 9
TASK_RUNTIME14 | "Ordinary calls acquire no new spill-exhaustion route." | Activation description and stack | Ordinary calls within activation must not gain a new spill-exhaustion route | at a call | 3
TASK_RUNTIME15 | "Alternate lowered representations are not runtime-selectable modes of this plan." | Activation description and stack | Lowered representations must not be selectable at runtime as alternate modes of the stack plan | not stated | 9
TASK_RUNTIME16 | "stack-capacity admission before execution." | Activation description and stack | Stack capacity admission must occur before execution | at entry | 9
TASK_RUNTIME17 | "The provider must guarantee usable backing for the full stack plan throughout the activation, including while parked." | Activation description and stack | Provider contract must guarantee usable backing for the full plan for the whole activation including while parked | across a pause | 9
TASK_RUNTIME18 | "deferred commitment must preserve that guarantee." | Activation description and stack | Deferred physical commitment must still preserve the backing guarantee | not stated | 9
TASK_RUNTIME19 | "Reserving an address range alone does not establish sufficient backing." | Activation description and stack | Address-range reservation alone must not count as sufficient backing | not stated | 9
TASK_RUNTIME20 | "it changes residency, never the plan or the bound" | Activation description and stack | First-touch residency policy must not change the stack plan or bound | not stated | 9
TASK_RUNTIME21 | "it is not a growth mechanism" | Activation description and stack | Region beyond the plan must not be used as stack growth | not stated | 9
TASK_RUNTIME22 | "Nonmoving frames preserve addresses relied on by live loans and outstanding operations across suspension." | Activation description and stack | Nonmoving frames must preserve addresses for live loans and outstanding operations across suspension | across a pause | 2
TASK_RUNTIME23 | "Target/provider selection and physical plans remain in the build-owned companion, not Psi checked semantics." | Activation description and stack | Target/provider selection and physical plans must be owned by build layer, not checked Psi semantics | not stated | 9
TASK_RUNTIME24 | "Report emission does not establish provider custody, a complete WCSU producer theorem, or executable task support." | Activation description and stack | Compiler must not treat JSON report emission as establishing custody, WCSU theorem, or executable task support | not stated | 9
TASK_RUNTIME25 | "Reject missing/duplicate selection, requirement drift, or narrowing of the published static-machine contract." | Activation description and stack | Must reject missing or duplicate runtime selection, requirement drift, or narrowing of published static-machine contract | not stated | 9
TASK_RUNTIME26 | "The domain-separated specialization commitment binds operation, exact checked requirement, package-qualified ownership and target/entry signature with parameter modes, and machine-contract commitment." | Activation description and stack | Specialization commitment must bind operation, exact requirement, ownership, target/entry signature with modes, and machine contract | not stated | 9
TASK_RUNTIME27 | "The runtime invocation receipt must join that static description, exact provider plan/operation, activation plan, runtime instance, and required preservation evidence." | Activation description and stack | Runtime invocation receipt must join static description, provider plan/operation, activation plan, instance, and preservation evidence | at a call | 9
TASK_RUNTIME28 | "Invocation and receipt identities are single-use within that instance." | Activation description and stack | Invocation and receipt identities must be single-use per runtime instance | at a call | 9
TASK_RUNTIME29 | "Static plans are not invocation receipts or routed source establishment." | Activation description and stack | Static plans must not be used as invocation receipts or routed source establishment | not stated | 9
TASK_RUNTIME30 | "`start<M>` returns `Task<T>` when capacity/admission obligations are discharged." | Transactional start | `start` must return `Task<T>` only after capacity and admission obligations are discharged | at normal return | 9
TASK_RUNTIME31 | "Known static plan/provider incompatibility rejects validation or admission rather than becoming avoidable runtime failure." | Transactional start | Static plan/provider incompatibility must reject at validation or admission, not defer to runtime | not stated | 9
TASK_RUNTIME32 | "Rejection returns every moved argument and caller-supplied reservation/lease, or proves transfer to another named authorized owner." | Transactional start | Failed start must return all moved arguments and reservations/leases or prove transfer to another authorized owner | at normal return | 2
TASK_RUNTIME33 | "No linear value disappears in a failed provider call." | Transactional start | Failed provider start must not cause any linear value to disappear | at normal return | 2
TASK_RUNTIME34 | "A pending task is returned only after provider custody and compatible storage exist." | Transactional start | `Task<T>` must be returned only when provider custody and compatible storage exist | at normal return | 9
TASK_RUNTIME35 | "an `available >= 1` fact suffices only with permission preventing another starter from spending that capacity." | Transactional start | Capacity availability fact must be paired with permission preventing concurrent starters from consuming the same capacity | at a call | 8
TASK_RUNTIME36 | "`Task<T>` is linear." | Linear task claim | `Task<T>` must be treated as a linear type | not stated | 2
TASK_RUNTIME37 | "`TaskLifecycle::request_cancel(&task)` retains the claim; it requests a transition, not proof that execution stopped." | Linear task claim | Cancel request must take `&Task<T>`, retain the claim, and must not be treated as proof of stopped execution | at a call | 3
TASK_RUNTIME38 | "`TaskLifecycle::finish(move task)` terminally consumes it and returns ordinary `TaskOutcome<T>` cases `Cancelled`, `Returned(value)`, or `Failed(receipt)`." | Linear task claim | Finish must move-consumes `Task<T>` and return only the listed `TaskOutcome` cases | at normal return | 3
TASK_RUNTIME39 | "Trap/abort remain declared crash routes, not fabricated returned cases." | Linear task claim | Trap and abort must remain crash routes, not `TaskOutcome` return cases | not stated | 3
TASK_RUNTIME40 | "Generic result containers retain each active payload's substituted linear debt." | Linear task claim | Generic result containers must retain substituted linear debt for active payloads | not stated | 2
TASK_RUNTIME41 | "These operations declare ordinary explicit parameters: `task: &Task<T>` for cancellation and `task: Task<T>` for completion." | Linear task claim | Cancel and finish must use the declared explicit parameter forms | at a call | 3
TASK_RUNTIME42 | "Carrier qualification supplies no implicit argument or storage, and consuming completion retains its declared suspension and blocking contract." | Linear task claim | Carrier qualification must not supply implicit arguments or storage; consuming completion must retain its declared suspension and blocking contract | at a call | 3
TASK_RUNTIME43 | "Copying, overwriting, branch loss, scope drop, or implicit fire-and-forget cannot discharge it." | Linear task claim | Task obligation must not be discharged by copy, overwrite, branch loss, scope drop, or implicit fire-and-forget | not stated | 2
TASK_RUNTIME44 | "Automatic cleanup cannot join, park, fail, or settle the task." | Linear task claim | Automatic cleanup must not join, park, fail, or settle a task claim | not stated | 2
TASK_RUNTIME45 | "A task backed by borrowed/partitioned storage cannot outlive it." | Linear task claim | Task lifetime must not exceed borrowed or partitioned backing storage | on every edge | 2
TASK_RUNTIME46 | "Closing a local runtime/pool or reclaiming storage requires reconciliation of every child claim and lease." | Linear task claim | Runtime/pool close or storage reclaim must reconcile every child claim and lease | not stated | 2
TASK_RUNTIME47 | "One ledger belongs to one admitted runtime instance." | Linear task claim | Child admission ledger must be scoped to one admitted runtime instance | not stated | 9
TASK_RUNTIME48 | "Each accepted child records exact activation admission and either storage owner/lease era or admitted inline completion." | Linear task claim | Each accepted child must record exact activation admission and storage owner/lease era or admitted inline completion | not stated | 9
TASK_RUNTIME49 | "Cancel requests retain that record." | Linear task claim | Cancel requests must retain the child admission record | at a call | 9
TASK_RUNTIME50 | "Settlement removes only the matching claim and storage relationship; cross-instance failure returns the claim." | Linear task claim | Settlement must remove only matching claim and storage relationship; cross-instance failure must return the claim | at normal return | 2
TASK_RUNTIME51 | "Reuse requires a fresh activation/lease era, not replay of old evidence." | Linear task claim | Reuse must require fresh activation or lease era, not replay of old evidence | not stated | 2
TASK_RUNTIME52 | "Parking establishes no result or cleanup edge; resumption continues that same invocation." | Suspension and cancellation | Parking must not establish a result or cleanup edge; resumption must continue the same invocation | across a pause | 10
TASK_RUNTIME53 | "The parked frames are compiler-owned and never an addressable field of `Task<T>`" | Suspension and cancellation | Parked frames must not be exposed as an addressable field of `Task<T>` | not stated | 10
TASK_RUNTIME54 | "Requesting it cannot dispose a parked continuation or unwind arbitrary frames." | Suspension and cancellation | Cancel request must not dispose parked continuation or unwind arbitrary frames | at a call | 10
TASK_RUNTIME55 | "only the terminal task operation settles the external claim." | Suspension and cancellation | Only the terminal task lifecycle operation may settle the external `Task<T>` claim | at normal return | 3
TASK_RUNTIME56 | "Architectural preemption may preserve opaque state at any instruction without becoming semantic suspension or granting migration/replacement/cancellation." | Suspension and cancellation | Architectural preemption must not be modeled as semantic suspension or as granting migration, replacement, or cancellation | not stated | 10
TASK_RUNTIME57 | "Semantic safe points require explicit may-suspend calls or authored scheduling operations such as a poll." | Suspension and cancellation | Semantic safe points must occur only at explicit may-suspend calls or authored scheduling operations like poll | at the operation | 10
TASK_RUNTIME58 | "State transitions, backedges, ordinary calls, allocation, and optimizer locations are not implicit safe points." | Suspension and cancellation | Listed control sites must not be treated as implicit cancellation safe points | not stated | 10
TASK_RUNTIME59 | "Optimizations cannot insert may-suspend polls into a non-suspending kernel." | Suspension and cancellation | Optimizer must not insert may-suspend polls into a non-suspending kernel | not stated | 10
TASK_RUNTIME60 | "Blocking creates no semantic safe point." | Suspension and cancellation | Blocking must not create a semantic cancellation safe point | at the operation | 10
TASK_RUNTIME61 | "Bounded-response/termination profiles need accepted finite-wait evidence for any otherwise unbounded wait." | Suspension and cancellation | Bounded-response or termination profiles must have accepted finite-wait evidence for otherwise unbounded waits | not stated | 4
TASK_RUNTIME62 | "Cancellation is cooperative and arrives through ordinary outcomes at declared wait/safe points." | Suspension and cancellation | Cancellation must be cooperative and arrive only through ordinary outcomes at declared wait or safe points | at the operation | 10
TASK_RUNTIME63 | "Starting with a retained exclusive reference excludes conflicting parent access." | Carry and loans | Task start with retained exclusive reference must exclude conflicting parent access | at a call | 2
TASK_RUNTIME64 | "Move independent work, split provably disjoint places, or use synchronized capabilities." | Carry and loans | When exclusive reference is retained at start, parent must move independent work, split disjoint places, or use synchronized capabilities | at a call | 2
TASK_RUNTIME65 | "A live reference carrier alone does not authorize a loan across park: its exact crossing needs storage lifetime/pinning, aliasing, cancellation-outcome, and address-stability evidence." | Carry and loans | Loan across park requires the listed evidence; live reference carrier alone is insufficient | across a pause | 2
TASK_RUNTIME66 | "blanket loan death is not the language rule." | Carry and loans | Compiler must not apply blanket loan invalidation at park as the default language rule | across a pause | 2
TASK_RUNTIME67 | "Local possible-suspension checks cannot be weakened by runtime selection." | Carry and loans | Local possible-suspension checks must not be weakened by runtime provider selection | not stated | 10
TASK_RUNTIME68 | "A runtime establishes each required CPU/thread preservation axis by checked construction or exact admitted receipt; portable activations demand neither." | Carry and loans | Required CPU/thread preservation must be established by checked construction or admitted receipt; portable activations must not demand preservation axes | not stated | 10
TASK_RUNTIME69 | "A host able to migrate outside semantic points must establish activation-wide preservation when possible live values need it." | Carry and loans | Hosts that may migrate outside semantic points must establish activation-wide preservation when live values need it | not stated | 10
TASK_RUNTIME70 | "Address stability follows from the nonmoving lease, not a freely asserted runtime property." | Carry and loans | Address stability must follow from nonmoving stack lease, not an unconstrained runtime assertion | not stated | 2
TASK_RUNTIME71 | "The runtime must implement the cancellation request contract exposed by `Task`" | Carry and loans | Selected runtime provider must implement the cancellation request contract exposed by `Task` | not stated | 9
TASK_RUNTIME72 | "there is no `yield` keyword, yielding contract clause or generator-colored machine." | Owned activations and typed replies | Must reject or not admit yield keyword, yielding contract clause, or generator-colored machine | not stated | 10
TASK_RUNTIME73 | "Preparation runs nothing, and a failed preparation returns every argument and resource it did not take into custody." | Owned activations and typed replies | Activation preparation must run nothing; failed preparation must return every argument and resource not taken into custody | at entry | 2
TASK_RUNTIME74 | "Backing stays fixed while the activation is live, including while suspended; moving the claim moves no stack." | Owned activations and typed replies | Activation backing must remain fixed while live including suspended; moving execution claim must not move the stack | on every edge | 10
TASK_RUNTIME75 | "There is no compiler-known yielded outcome, and the running state and a usable claim never coexist." | Owned activations and typed replies | Compiler must not model a known yielded outcome; running state and usable execution claim must not coexist | not stated | 10
TASK_RUNTIME76 | "Stepping does not park the caller." | Owned activations and typed replies | Stepping a ready activation must not park the stepping caller | at the operation | 10
TASK_RUNTIME77 | "termination needs evidence that the resumption reaches an actual stop." | Owned activations and typed replies | Termination claims must include evidence that resumption reaches an actual stop | not stated | 4
TASK_RUNTIME78 | "The lowest-level park always hands control to the execution owner; a higher-level wait that completes immediately does not invoke it." | Owned activations and typed replies | Lowest-level park must hand control to execution owner; immediate higher-level wait must not invoke that park | across a pause | 10
TASK_RUNTIME79 | "This is a compiler-recognized activation boundary, not an ordinary wrapper." | Owned activations and typed replies | Stepping boundary must be recognized as activation boundary, not checked as ordinary wrapper call only | at a call | 10
TASK_RUNTIME80 | "A park point retains its exact suspension occurrence, the live activation with its ownership and loan state, the published request, and the contract a reply must satisfy." | Owned activations and typed replies | Park point state must retain suspension occurrence, activation with ownership/loans, published request, and reply contract | across a pause | 10
TASK_RUNTIME81 | "Resuming requires the suspended activation together with the matching admissible reply; the pairing is checked, and a mismatched or stale reply returns both unchanged." | Owned activations and typed replies | Resume must check activation and admissible reply pairing; mismatch or staleness must return both unchanged | at a call | 10
TASK_RUNTIME82 | "No operation turns an arbitrary value into readiness." | Owned activations and typed replies | No operation may convert an arbitrary value into activation readiness | at the operation | 10
TASK_RUNTIME83 | "An early reply cannot start concurrent execution" | Owned activations and typed replies | Early reply must not start concurrent execution | at a call | 10
TASK_RUNTIME84 | "a wait protocol subscribes before concluding it is pending or rechecks after subscribing." | Owned activations and typed replies | Wait protocol must subscribe before concluding pending or recheck after subscribing | at the operation | 10
TASK_RUNTIME85 | "so no type is recovered from an opaque pause" | Owned activations and typed replies | Typed receiving must not recover types from an opaque pause without protocol endpoints | across a pause | 7
TASK_RUNTIME86 | "A suspended activation is never discarded" | Owned activations and typed replies | Suspended activation must not be discarded without drain, obligation transfer, or closing protocol | not stated | 2
TASK_RUNTIME87 | "`Task<T>` remains the separate observation and settlement claim and grants no execution custody." | Owned activations and typed replies | `Task<T>` must remain separate from execution custody and must not grant execution custody | not stated | 2
TASK_RUNTIME88 | "parking is bounded by the endpoints a machine holds" | Owned activations and typed replies | Machine parking must be bounded by pause-protocol endpoints the machine holds | across a pause | 10
TASK_RUNTIME89 | "Provisioning fixes maximum capacity; starts/settlements still account dynamically." | Library and foreign providers | Task pool provisioning must fix max capacity while starts and settlements still use dynamic accounting | not stated | 9
TASK_RUNTIME90 | "closure/task death must preserve a surviving owner for every undelivered linear payload." | Library and foreign providers | Closure or task death must leave a surviving owner for each undelivered linear payload | not stated | 2
TASK_RUNTIME91 | "It is an engineering direction, not permission to merge distinct host mechanisms under false guarantees." | Library and foreign providers | Wait-substrate direction must not authorize merging distinct host mechanisms under false guarantees | not stated | 9
TASK_RUNTIME92 | "Foreign execution either contributes admitted same-stack demand, declared on the foreign requirement as `stack N bytes`, or uses a separately provisioned provider stack while the caller accounts for its stub." | Library and foreign providers | Foreign execution must use declared same-stack demand on requirement or separate provider stack with caller stub accounting | at a call | 9
TASK_RUNTIME93 | "an ordinary hosted blocking executor is a package, not a task-runtime mode." | Library and foreign providers | Ordinary hosted blocking executor must not be classified as a task-runtime mode | not stated | 9
TASK_RUNTIME94 | "Pooling guarded native stacks can spare no-block workers but cannot prove completion, cancellation, or shutdown." | Library and foreign providers | Pooling guarded native stacks must not be used to prove completion, cancellation, or shutdown | not stated | 9
TASK_RUNTIME95 | "Post-return storage moves into a linear protocol claim; pre-return storage may be borrowed." | Library and foreign providers | Foreign callback storage after return must move into linear protocol claim; before return may be borrowed | at normal return | 2

## Ambiguous sentences

Sentence | Reading A | Reading B
--- | --- | ---
"A direct `Worker::run(job)` remains an ordinary call." | Task activation must go through boundary start APIs; direct run is always an ordinary call without task semantics. | Direct run is permitted as an ordinary call but the sentence does not state whether it can also substitute for start when a task is intended. |
"[Call acknowledgements](../language/effects.md#call-site-acknowledgements) describe only what the start/control operation may do to its caller." | Call-site acknowledgement checking for start/control is limited to caller effects named in that section. | The section is descriptive documentation only and does not bound compiler acknowledgement obligations. |
"Root profiles may select default providers and owners may override their slots" | Default provider selection is optional configuration; override replaces the slot selection. | Override may layer on defaults without fully replacing profile selection; interaction with duplicate selection rejection is unstated here. |
"`Task<T>` need not contain or borrow the stack" | `Task<T>` type checking must not require stack fields or borrows. | Runtime representations may still correlate with stack storage; the sentence speaks only to the type surface. |
"Inline completion may shorten storage lifetime or embed an outcome, but does not settle the returned linear claim." | Finish/settlement operations must still consume the linear claim after inline completion. | Inline completion might embed outcome data while the claim remains unsettled indefinitely until finish. |
"a serviced fault inside the plan is an implementation event, not an outcome." | Stack faults within the plan must not be surfaced as `TaskOutcome` or ordinary recoverable outcomes. | Faults may still abort/trap via declared crash routes; "not an outcome" applies only to lifecycle sum cases. |
"Compact specialization coordinates are reports." | Compact coordinates are non-authoritative reports for tools. | Specialization may still key off compact coordinates at build time; "reports" does not state they are ignored by the compiler. |
"`try_start<M>` handles genuine dynamic capacity/environmental uncertainty" | Only genuine dynamic uncertainty may use try_start semantics. | Any start may use try_start when the API is selected; "genuine" is not a separate compiler admission test in this sentence. |
"Application-level recoverable errors remain inside `T`" | Recoverable errors must be encoded in `T`, not as extra lifecycle variants. | Lifecycle operations may still fail before `T` is produced; errors after return use `T` only. |
"A never-suspending task may be finishable without being cancellable." | Cancel and finish capabilities may diverge when the machine never suspends. | Finish without cancel requires a profile or contract flag not named in this sentence. |
"Implementation may conservatively reject unsupported loans, but blanket loan death is not the language rule." | Conservative rejection is permitted for unsupported loans. | Unsupported loans must still be accepted when evidence is present; conservative rejection is optional for implementers only. |
"that does not promise every task eventually observes the request." | Compiler must not treat cancellation contract as guaranteeing eventual observation. | Runtime verification must not require eventual observation proof. |
"Until it exists, a manual generator interface keeps an other-pending-work outcome and a general `next` may suspend." | Before pause-protocol contract exists, generator interface must use other-pending-work outcome and may-suspend `next`. | The manual interface is optional library surface; compiler need not enforce until contract exists. |
"Wake need not park" | Wake operations are not required to park the waking context. | Wake may occur without semantic suspension at all; relationship to blocking waits is unstated. |

## Terms used without definition in this chapter

- `TaskRuntime` / boundary provider / build selection
- runtime capability / held runtime capability / ambient authority
- execution custody / physical storage / `Task<T>` ownership
- linear execution claim / provider/activation identity
- Arena pools / hosted providers / remote executors / inline execution / inline completion
- activation description / monomorphizes
- machine contract/entry / argument/outcome layouts / target calling plan
- WCSU / `StackPlan<M>` / canonical suspension crossings / live carry demands
- activation-wide CPU/thread preservation
- spill-exhaustion route / `StackLease` / nonmoving stack / nonmoving frames
- build-owned companion / `05_task_activations.json` / `05_carry_manifest.json`
- static binding / requirement drift / published static-machine contract
- domain-separated specialization commitment / compact specialization coordinates
- runtime invocation receipt / preservation evidence / routed source establishment
- capacity/admission obligations / `StartOutcome<T, Arguments>` / `Rejected(arguments, reason)`
- caller-supplied reservation/lease / named authorized owner
- `TaskLifecycle` / `TaskOutcome<T>` / crash routes / carrier qualification
- combined terminal stop operation / provider provenance / provenance edge
- child lease / ledger / activation/lease era
- Terminal suspension / parked continuation / execution claim (link target)
- cooperative cancellation / wait/safe points / semantic safe points / may-suspend
- architectural preemption / non-suspending kernel
- finite wait ceiling / logical-work analysis / bounded-response/termination profiles / finite-wait evidence
- retained exclusive reference / loan across park / address-stability evidence
- demand-driven carry / portable activations / provider identity and evidence
- cancellation conformance / universal runtime behavior lattice
- owned-activations (named decision) / linear claim (execution custody)
- preparation / stepping / ready activation / pending protocol
- park point / admissible reply / park contract / wait protocol
- typed endpoints / one-shot capability / opaque pause
- rendezvous library / `emit` / pause-protocol contract / suspension lease
- per-step progress / manual generator interface / other-pending-work outcome
- Arena-backed task pool / supervisors / mailbox storage
- shared wait-substrate / wake-one/wake-many boundary / positive progress (link)
- same-stack demand / `stack N bytes` / provider stack / callback plans/registration
- guarded native stacks / linear protocol claim

CHAPTER COMPLETE
