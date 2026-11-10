# Process exit

Source: `wiki/spec/language/process_exit.md` lines 1–133 (read).

## Rules

| rule id | the exact sentence, quoted | the heading it is under | what must be checked, in plain words | when it must hold | lane |
| --- | --- | --- | --- | --- | --- |
| PROCESS_EXIT1 | "[Service reach](effects.md), [termination](termination.md), and [ownership](ownership.md) remain independent contracts." | Process exit | Service reach, termination, and ownership must be checked and represented as independent contracts, not merged with process-exit semantics. | not stated | SHARED |
| PROCESS_EXIT2 | "The requirement explicitly declares `reaches ProcessExit`; that contribution does not arise implicitly from its containing boundary trait." | Canonical boundary and authority | A `reaches ProcessExit` contribution must come only from an explicit declaration on the requirement, not implicitly from the enclosing boundary trait. | not stated | 3 |
| PROCESS_EXIT3 | "Psi recognizes the exact admitted canonical requirement, including package and contract identity; a matching path, method spelling, or ordinary user declaration does not acquire its meaning." | Canonical boundary and authority | Only the exact admitted canonical `ProcessExit::exit_process` requirement (with package and contract identity) receives canonical meaning; lookalike paths, spellings, or user declarations must not be treated as canonical. | not stated | SHARED |
| PROCESS_EXIT4 | "Invocation requires actual authority bound to the caller's containing process execution domain by the admitted entry/runtime environment." | Canonical boundary and authority | Each process-exit invocation must have actual authority bound to the caller's containing process execution domain per the admitted entry/runtime environment. | at a call | 3 |
| PROCESS_EXIT5 | "Importing core, declaring `reaches ProcessExit`, or selecting a provider grants no authority." | Canonical boundary and authority | Core import, a `reaches ProcessExit` declaration, or provider selection alone must not satisfy exit authority. | at a call | 3 |
| PROCESS_EXIT6 | "Embedded components receive none by default; the host must explicitly delegate it through the ordinary capability machinery." | Canonical boundary and authority | Embedded components must have no default exit authority; only explicit host delegation through ordinary capability machinery may grant it. | not stated | 3 |
| PROCESS_EXIT7 | "The operation ends that bound domain, including its active and suspended activations." | Canonical boundary and authority | Lowering and checking must model exit as ending the bound domain, including active and suspended activations in that domain. | at the operation | 10 |
| PROCESS_EXIT8 | "It does not automatically end child processes or other domains." | Canonical boundary and authority | Exit must not be modeled as automatically ending child processes or other execution domains. | at the operation | 10 |
| PROCESS_EXIT9 | "A provider cannot reinterpret it as worker exit, remote-provider exit, killing another process, CPU halt, or reboot." | Canonical boundary and authority | Provider implementations must preserve canonical process-exit semantics and must not reinterpret the operation as worker exit, remote-provider exit, killing another process, CPU halt, or reboot. | not stated | 9 |
| PROCESS_EXIT10 | "Targets without a matching process abstraction do not supply this service." | Canonical boundary and authority | Targets lacking a matching process abstraction must not admit or supply the process-exit service. | not stated | 9 |
| PROCESS_EXIT11 | "Console I/O does not imply process-exit reach or authority." | Canonical boundary and authority | Console I/O reach or capabilities must not imply `ProcessExit` reach or exit authority. | not stated | 3 |
| PROCESS_EXIT12 | "Any retained console-exit convenience adapter must obtain legitimate exit authority and publish its additional reach." | Canonical boundary and authority | A console-exit convenience adapter must have legitimate exit authority and must publish any reach beyond ordinary console I/O. | not stated | 3 |
| PROCESS_EXIT13 | "Console must not inherit `ProcessExit` wholesale." | Canonical boundary and authority | Console service contracts must not wholesale inherit the `ProcessExit` boundary. | not stated | 3 |
| PROCESS_EXIT14 | "It produces no normal result or continuation and permits no source crash." | Progress and conditional calls | The canonical exit operation must have no normal result or continuation and must not permit a source crash on the exit path. | at the operation | 3 |
| PROCESS_EXIT15 | "Preceding observable events and the exact exit argument survive." | Progress and conditional calls | Semantic traces must retain preceding observable events and the exact exit argument through exit. | at the operation | 3 |
| PROCESS_EXIT16 | "`terminates` continues to promise eventual arrival at a permitted terminal outcome under the invocation's premises." | Progress and conditional calls | A `terminates` annotation must still promise eventual arrival at a permitted terminal outcome under the invocation's premises when exit is in scope. | not stated | 4 |
| PROCESS_EXIT17 | "It neither authorizes that outcome nor promises normal return." | Progress and conditional calls | `terminates` must not authorize a specific terminal outcome by itself and must not promise normal return. | not stated | 4 |
| PROCESS_EXIT18 | "callee/provider progress remains independently required." | Progress and conditional calls | Callee and provider progress obligations remain required independently of exit-specific termination rules. | at a call | 4 |
| PROCESS_EXIT19 | "An exit leaf needs no further cycle decrease, but operand evaluation and calls before it still need progress evidence." | Progress and conditional calls | At an exit leaf, cycle decrease need not be shown; operand evaluation and prior calls still require progress evidence. | at the operation | 4 |
| PROCESS_EXIT20 | "`reaches ProcessExit` is an ordinary conservative service ceiling, propagated through callees and provider contracts." | Progress and conditional calls | `reaches ProcessExit` must be treated as a conservative service ceiling propagated through callees and provider contracts. | not stated | 3 |
| PROCESS_EXIT21 | "The caller must establish the ceiling, actual domain-bound authority, argument requirements, and applicable crossing obligations; it need not prove the entire process resource-free." | Progress and conditional calls | At a call site, the caller must establish reach ceiling, domain-bound authority, argument requirements, and crossing obligations; whole-process resource-freedom must not be required. | at a call | 3 |
| PROCESS_EXIT22 | "Its normal continuation, result, and cleanup exist precisely when it returns; only the exit branch has no successor." | Progress and conditional calls | For helpers that may exit or return, normal continuation, result, and cleanup apply only on return branches; exit branches must have no successor. | on every edge | 2 |
| PROCESS_EXIT23 | "A public reach ceiling alone cannot prove which arguments select return." | Progress and conditional calls | Reach ceiling alone must not be used to prove which arguments lead to return versus exit. | not stated | 3 |
| PROCESS_EXIT24 | "Greater precision requires checked body knowledge or a supported pinned contract refinement, not inference from a selected provider." | Progress and conditional calls | Argument-selected return versus exit must rely on checked body knowledge or supported pinned contract refinement, not provider selection inference. | not stated | 8 |
| PROCESS_EXIT25 | "No guarded reach syntax is introduced." | Progress and conditional calls | The language surface must not add guarded reach syntax for process exit. | not stated | SHARED |
| PROCESS_EXIT26 | "Termination must cover every admitted path; reachable divergence defeats an unconditional promise." | Progress and conditional calls | Termination evidence must cover every admitted path; reachable divergence must defeat an unconditional termination promise. | not stated | 4 |
| PROCESS_EXIT27 | "Normal-return `ensures` clauses keep their partial-correctness meaning." | Progress and conditional calls | Normal-return `ensures` must retain partial-correctness meaning on paths that return normally. | at normal return | 0 |
| PROCESS_EXIT28 | "An unconditional eventual-release or protocol-completion guarantee must account for terminal outcomes explicitly or exclude abrupt exit." | Progress and conditional calls | Unconditional eventual-release or protocol-completion guarantees must either account for terminal outcomes explicitly or exclude abrupt exit. | not stated | 3 |
| PROCESS_EXIT29 | "It may abandon outstanding in-domain obligations, including linear obligations, without cleanup or discharge claims." | Abandonment and survivors | Exit must be permitted to abandon in-domain obligations including linear ones without requiring cleanup or discharge claims on the exit branch. | at the operation | 2 |
| PROCESS_EXIT30 | "It creates no release, cancellation, consumption, or protocol-completion receipt." | Abandonment and survivors | Exit must not create release, cancellation, consumption, or protocol-completion receipts. | at the operation | 2 |
| PROCESS_EXIT31 | "Resources already transferred to surviving owners are not reclaimed by this operation." | Abandonment and survivors | Exit must not reclaim resources already transferred to surviving owners. | at the operation | 2 |
| PROCESS_EXIT32 | "Exact-once disposition remains required on ordinary continuing and returning paths." | Abandonment and survivors | Exact-once disposition must still be enforced on continuing and returning paths. | on every edge | 2 |
| PROCESS_EXIT33 | "An exit branch has no post-exit ownership frontier to merge into a returning branch." | Abandonment and survivors | Exit branches must have no post-exit ownership frontier mergeable into a returning branch. | on every edge | 2 |
| PROCESS_EXIT34 | "Audit evidence may retain a statically reconstructed local frontier lower bound, but must not call it the complete process-wide obligation set or proof that unlisted state remains safe." | Abandonment and survivors | Audit evidence may keep a local frontier lower bound but must not label it the complete process-wide obligation set or safety proof for unlisted state. | not stated | 8 |
| PROCESS_EXIT35 | "Exit authority cannot invalidate guarantees relied upon by surviving components." | Abandonment and survivors | Granting or using exit authority must not invalidate guarantees surviving components rely on. | not stated | 3 |
| PROCESS_EXIT36 | "Crossing contracts must account for domain death through isolation, retained custody, owner-death recovery, or reset; incompatible crossings reject." | Abandonment and survivors | Crossing contracts must account for domain death via isolation, retained custody, owner-death recovery, or reset; incompatible crossings must reject. | not stated | 3 |
| PROCESS_EXIT37 | "These are obligations at the actual verified/admitted boundary, not a compiler guarantee about every host or peer." | Abandonment and survivors | Shared-memory validity, DMA cessation, and external lock release must be checked only at verified/admitted boundaries, not as universal compiler guarantees about every host or peer. | not stated | SHARED |
| PROCESS_EXIT38 | "It cannot silently waive a safety premise another verified component relies on." | Abandonment and survivors | Deployment or policy must not silently waive a safety premise another verified component relies on. | not stated | 9 |
| PROCESS_EXIT39 | "Reuse the existing [execution-domain recovery rules](effects.md#recovery-and-execution-domains);" | Abandonment and survivors | Exit-related recovery must reuse existing execution-domain recovery rules from effects. | not stated | 3 |
| PROCESS_EXIT40 | "no universal exit-safety trait or per-call whole-process resource census is added." | Abandonment and survivors | The language must not add a universal exit-safety trait or a per-call whole-process resource census mechanism. | not stated | SHARED |
| PROCESS_EXIT41 | "Entry return alone does not establish that other activations were settled." | Graceful completion and cleanup | Entry return must not be treated as proof that other activations were settled. | at normal return | 10 |
| PROCESS_EXIT42 | "The root/runtime contract must settle or legally transfer their custody before physical process completion; the bridge cannot conceal abandonment." | Graceful completion and cleanup | Root/runtime contracts must settle or legally transfer custody of other activations before physical process completion; the entry bridge must not hide abandonment. | not stated | 10 |
| PROCESS_EXIT43 | "Automatic cleanup must return normally, excluding both crashes and external completion." | Graceful completion and cleanup | Automatic cleanup paths must end in normal return, not crashes or external completion (including process exit). | at normal return | 2 |
| PROCESS_EXIT44 | "Its transitive contract cannot permit process exit merely because exit is terminating and non-crashing." | Graceful completion and cleanup | Transitive automatic-cleanup contracts must not permit process exit solely because exit is terminating and non-crashing. | not stated | 3 |
| PROCESS_EXIT45 | "There is no implicit unwind, exit-handler registry, or compiler-inserted cleanup of caller frames." | Graceful completion and cleanup | The compiler must not introduce implicit unwind, exit-handler registries, or compiler-inserted cleanup of caller frames for process exit. | not stated | SHARED |
| PROCESS_EXIT46 | "Checking and Terminal Psi retain an explicit closed external-completion kind and a terminal invocation with no normal result." | Verification and implementation boundary | Checking and Terminal Psi must retain an explicit closed external-completion kind and a terminal invocation representation with no normal result. | not stated | SHARED |
| PROCESS_EXIT47 | "An ordinary Unit call is not that representation; the special canonical contract does not change result omission for other machines." | Verification and implementation boundary | Ordinary Unit calls must not use the process-exit terminal representation; canonical process-exit contract must not alter result-omission rules for other machines. | not stated | SHARED |
| PROCESS_EXIT48 | "No `completes` keyword or public `never` type is introduced." | Verification and implementation boundary | The surface must not add a `completes` keyword or a public `never` type for process exit. | not stated | SHARED |
| PROCESS_EXIT49 | "Epsilon's independent bootstrap `never` rules are unchanged." | Verification and implementation boundary | Epsilon bootstrap `never` rules must remain unchanged by this decision. | not stated | SHARED |
| PROCESS_EXIT50 | "Provider conformance preserves the terminal outcome, exact arguments, progress, and preceding trace." | Verification and implementation boundary | Provider conformance checking must require preserving terminal outcome, exact arguments, progress premises, and preceding trace. | not stated | 9 |
| PROCESS_EXIT51 | "Returning mocks, divergence, or abort cannot substitute." | Verification and implementation boundary | Providers must not substitute returning mocks, divergence, or abort for canonical process exit. | not stated | 9 |
| PROCESS_EXIT52 | "An interpreter ends its simulated domain without killing the test runner." | Verification and implementation boundary | Interpreter providers must end the simulated domain on exit without terminating the host test runner. | at the operation | 9 |
| PROCESS_EXIT53 | "A defensive trap after an unexpectedly returning native exit contains a violated provider premise; it is not another permitted source outcome." | Verification and implementation boundary | A defensive trap after a native exit that returns must be treated as violated provider premise, not as a permitted source outcome. | at normal return | 9 |
| PROCESS_EXIT54 | "compare the exact semantic `i32` status." | Verification and implementation boundary | Terminal observations must compare the exact semantic `i32` exit status. | not stated | 5 |
| PROCESS_EXIT55 | "A host exposing fewer status bits does not authorize truncation of that trace; physical presentation belongs to the target contract." | Verification and implementation boundary | Fewer host status bits must not authorize truncating the semantic `i32` trace; physical presentation is governed by the target contract only. | not stated | 9 |

## Ambiguous sentences

| quoted sentence | reading A | reading B |
| --- | --- | --- |
| "Successful exit means the declared transfer completed, not that the status was zero, cleanup ran, or an application protocol succeeded." | "Declared transfer" means completion of the canonical exit invocation contract only. | "Declared transfer" includes separate entry-bridge or runtime transfer obligations beyond the exit call itself. |
| "Abrupt exit means no implicit cleanup, not instantaneous completion, bounded latency, or an atomic stop of all activations." | The compiler must not assume implicit cleanup on exit; timing and activation-stop ordering are explicitly out of scope for this clause. | "No implicit cleanup" might still permit compiler-visible cleanup on non-exit paths only, or might forbid any cleanup modeling on exit— the chapter does not say which non-exit paths remain. |
| "`terminates` continues to promise eventual arrival at a permitted terminal outcome under the invocation's premises." | Exit counts as a permitted terminal outcome for `terminates` when premises include admitted progress. | `terminates` might require distinguishing exit from divergence as terminal outcomes; the chapter does not list which outcomes count as "permitted." |
| "Exit does not establish them and does not necessarily violate them." | Post-exit states are irrelevant to `ensures`; clauses are neither established nor checked violated on exit. | `ensures` might still be evaluated for obligations visible before exit on the same path; only post-exit violation is undefined. |
| "Audit evidence may retain a statically reconstructed local frontier lower bound, but must not call it the complete process-wide obligation set or proof that unlisted state remains safe." | Retaining a lower bound is optional for audit evidence. | Audit evidence that retains a lower bound must still meet the prohibition on labeling it complete or safety-proof. |
| "Deployment policy may prohibit abrupt abandonment or accept lost work and external capacity under its contracts." | Deployment policy is outside the compiler and does not change checking rules. | The compiler must surface or enforce deployment constraints that prohibit abrupt exit when policy requires it. |
| "The root/runtime contract must settle or legally transfer their custody before physical process completion; the bridge cannot conceal abandonment." | This is an obligation on root/runtime contracts checked like other contracts, not hidden by entry-bridge lowering. | "Physical process completion" might mean only after native exit, or might include entry-bridge return as completion; the chapter names both bridge and physical completion without ordering them. |
| "Automatic cleanup must return normally, excluding both crashes and external completion." | Automatic cleanup cannot end in process exit or crash; it must return. | "External completion" might include only process exit or any non-return terminal effect; scope of "external completion" is not defined in this chapter. |

## Terms used without definition in this chapter

- service reach (as independent contract)
- termination (as independent contract)
- ownership (as independent contract)
- non-crashing external terminal outcome
- divergence
- declared transfer
- canonical `ProcessExit` boundary trait
- process management
- platform implementation
- package and contract identity
- admitted canonical requirement
- process execution domain
- admitted entry/runtime environment
- embedded components
- ordinary capability machinery
- active activations
- suspended activations
- child processes
- worker exit
- remote-provider exit
- process abstraction
- console-exit convenience adapter
- legitimate exit authority
- admitted provider progress premises
- bound domain
- source crash
- abrupt exit
- observable events
- permitted terminal outcome
- invocation's premises
- ranking witness
- cycle decrease
- progress evidence
- conservative service ceiling
- crossing obligations
- entire process resource-free
- public reach ceiling
- pinned contract refinement
- guarded reach syntax
- reachable divergence
- unconditional promise
- partial-correctness meaning
- eventual-release guarantee
- protocol-completion guarantee
- domain-ending abandonment outcome
- resource consumer
- linear obligations
- protocol-completion receipt
- surviving owners
- exact-once disposition
- ownership frontier
- audit evidence
- statically reconstructed local frontier lower bound
- process-wide obligation set
- surviving components
- crossing contracts
- domain death
- owner-death recovery
- shared-memory validity
- DMA cessation
- external locks
- verified/admitted boundary
- deployment policy
- safety premise
- execution-domain recovery rules
- universal exit-safety trait
- per-call whole-process resource census
- graceful shutdown
- entry bridge
- other activations
- root/runtime contract
- physical process completion
- automatic cleanup
- transitive contract
- implicit unwind
- exit-handler registry
- public terminal-event identity
- separately named effect
- closed external-completion kind
- terminal invocation
- ordinary Unit call
- result omission
- `completes` keyword
- public `never` type
- Epsilon's bootstrap `never` rules
- preceding trace
- simulated domain
- test runner
- defensive trap
- native exit
- terminal observations
- semantic `i32` status
- physical presentation
- target contract
- general completion annotations
- separately named terminal effects
- thread exit
- arbitrary process killing

CHAPTER COMPLETE
