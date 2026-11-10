# Concurrency and atomic observation

Source: `wiki/spec/language/concurrency.md` lines 1–173 (read).

## Rules

rule id | the exact sentence, quoted | the heading it is under | what must be checked, in plain words | when it must hold | lane
--- | --- | --- | --- | --- | ---
CONCURRENCY1 | "Concurrency activates [ordinary machines](machines.md) through an admitted [task runtime](../build/task_runtime.md)." | Concurrency and atomic observation | Machine activation for concurrency must go only through an admitted task runtime on ordinary machines | not stated | 10
CONCURRENCY2 | "It does not create an async machine species or transform results into futures." | Concurrency and atomic observation | Concurrency must not introduce an async machine species or treat results as futures | not stated | 10
CONCURRENCY3 | "Ownership prevents ordinary shared mutation;" | Concurrency and atomic observation | Ordinary shared mutation must be rejected when ownership rules forbid it | at the operation | 2
CONCURRENCY4 | "a shared reference crosses activations only under a sanctioned concurrent-access contract and compatible [carry](../resources/carry.md)." | Concurrency and atomic observation | A shared reference crossing activations must have a sanctioned concurrent-access contract and compatible carry | on every edge | 2
CONCURRENCY5 | "Copyability, exclusive transfer, and shared mutation are separate permissions." | Concurrency and atomic observation | Copyability, exclusive transfer, and shared mutation must be checked as separate permissions, not conflated | not stated | 2
CONCURRENCY6 | "There is no language `select` or privileged task-group construct." | Concurrency and atomic observation | Source must not admit a language-level `select` or privileged task-group construct | not stated | 10
CONCURRENCY7 | "Physical storage, provider custody, and task-claim ownership remain independently accountable." | Concurrency and atomic observation | Physical storage, provider custody, and task-claim ownership must each satisfy their own accountability checks | not stated | SHARED
CONCURRENCY8 | "Atomics are dedicated core types, not implicitly atomic ordinary integers." | Atomic operations | Ordinary integers must not be treated as atomics without the dedicated atomic core types | not stated | 10
CONCURRENCY9 | "Generic code uses one sealed core requirement per primitive operation, shared by ordinary atomics and admitted accessors:" | Atomic operations | Each primitive atomic operation in generic code must use exactly one sealed core requirement shared by atomics and admitted accessors | not stated | 7
CONCURRENCY10 | "Representation [recasts preserve access authority](../layouts/recasts.md#access-authority-and-concurrent-observation)." | Atomic operations | Representation recasts involving atomics must preserve access authority per the recasts specification | at the operation | 2
CONCURRENCY11 | "The explicit parameter providing access to the atomic storage is shared." | Atomic operations | The access parameter to atomic storage must be a shared reference or equivalent shared access | at a call | 2
CONCURRENCY12 | "Fixed representation, admitted width/alignment, total decode/encode, and round-trip laws remain operation requirements; arithmetic capability cannot manufacture hardware support." | Atomic operations | Atomic operations must meet representation, width/alignment, decode/encode, and round-trip requirements; arithmetic capability alone must not substitute for admitted hardware support | at the operation | 10
CONCURRENCY13 | "Exact-forwarding wrappers may derive conformance mechanically; other realizations need checked proof or admitted evidence." | Atomic operations | Non-exact-forwarding atomic realizations must have checked proof or admitted evidence; exact-forwarding may derive conformance mechanically | not stated | 8
CONCURRENCY14 | "The shared parameter of an atomic store still carries mutation permission;" | Atomic operations | The shared access parameter for an atomic store must carry mutation permission | at the operation | 2
CONCURRENCY15 | "casting an ordinary shared reference cannot manufacture it." | Atomic operations | Casting an ordinary shared reference must not create atomic-store mutation permission | at the operation | 2
CONCURRENCY16 | "an atomic shared view cannot become an ordinary read view without the backing exclusion required for non-atomic observation." | Atomic operations | Converting an atomic shared view to an ordinary read view must require the backing exclusion needed for non-atomic observation | at the operation | 2
CONCURRENCY17 | "Genuine exclusive access may support representation-compatible recasts while preserving all validity and custody obligations." | Atomic operations | Exclusive-access recasts must preserve validity and custody obligations | at the operation | 2
CONCURRENCY18 | "Atomic-to-atomic views retain their operation and observation requirements." | Atomic operations | Atomic-to-atomic view changes must retain the same operation and observation requirements | not stated | 10
CONCURRENCY19 | "This rule does not itself establish a mutex's payload-custody protocol." | Atomic operations | Recast/access-authority rules must not be used alone to establish mutex payload custody | not stated | 2
CONCURRENCY20 | "[Domain access policy](domains.md#domain-owned-read-and-write-authority) also applies to atomic events: observation requires the applicable read grant, mutation the write grant, and a read-modify-write event both." | Atomic operations | Domain access policy must apply to atomics: observation needs read grant; mutation needs write grant; RMW needs both | at the operation | 10
CONCURRENCY21 | "Shared atomic access is not an exemption from the domain's writer list;" | Atomic operations | Shared atomic access must still satisfy the domain writer list | at the operation | 10
CONCURRENCY22 | "authorized machines may lend the exact access to an atomic helper without making that helper a new establishment route." | Atomic operations | Lending access to an atomic helper must not create a new establishment route for the helper | at a call | 3
CONCURRENCY23 | "Policy and custody survive representation casts." | Atomic operations | Domain policy and custody obligations must remain in force after representation casts | at the operation | 2
CONCURRENCY24 | "None of these grants alone proves that a mutex holds exclusive access to its separate payload." | Atomic operations | Read/write grants for atomics must not alone prove exclusive mutex access to a separate payload | not stated | 8
CONCURRENCY25 | "Observing compare-exchange compares stored representation with `encode(expected)`, not user equality." | Atomic operations | Observing compare-exchange must compare stored representation to `encode(expected)`, not user-defined equality | at the operation | 10
CONCURRENCY26 | "Non-observing exchange uses the selected copyable key's encoding law." | Atomic operations | Non-observing exchange must use the selected copyable key's encoding law | at the operation | 10
CONCURRENCY27 | "Fetch proofs cover every provider-readable raw representation, authorize its exact transition and operand encoding, and connect decoded output to the logical operation." | Atomic operations | Fetch operations must have proofs covering raw representations, authorized transitions, operand encoding, and decoded-to-logical connection | not stated | 8
CONCURRENCY28 | "External reads/writes do not synthesize fetch/exchange." | Atomic operations | External non-atomic reads or writes must not count as fetch or exchange | at the operation | 10
CONCURRENCY29 | "Fetch and swap return the prior observed by the atomic instruction or successful retry attempt, never by a separate racing load." | Atomic operations | Fetch/swap results must be the value observed by the atomic instruction or successful retry, not a separate racing load | at the operation | 10
CONCURRENCY30 | "Compare-exchange success does not duplicate the expected value." | Atomic operations | Successful compare-exchange must not treat the expected value as an additional duplicate result | at the operation | 10
CONCURRENCY31 | "Outcome construction/zero initialization proves no atomic event; only the checked operation contract does." | Atomic operations | Zero initialization or outcome construction alone must not establish an atomic event; only the checked operation contract may | not stated | 8
CONCURRENCY32 | "The zero outcome tag is mismatch, not success." | Atomic operations | The zero-initialized outcome tag must be classified as mismatch, not success | not stated | 10
CONCURRENCY33 | "If its payload is not zero-valid, the outcome remains unusable until established." | Atomic operations | Outcomes with non-zero-valid payloads must be unusable until properly established | not stated | 2
CONCURRENCY34 | "Pre-execution storage can use an ordinary `Empty | Ready(outcome)` wrapper rather than altering the operation's cases." | Atomic operations | Pre-execution storage may use `Empty | Ready(outcome)` without changing the operation's outcome cases | not stated | 1
CONCURRENCY35 | "Ordering is explicit proof-static operation data, not a runtime policy choice." | Orderings | Memory ordering must be fixed proof-static data at compile time, not a runtime-selectable policy | at the operation | 10
CONCURRENCY36 | "Load/store/swap/fetch have one ordering; compare-exchange has separate success and failure orderings." | Orderings | Loads, stores, swap, and fetch must have one ordering each; compare-exchange must have distinct success and failure orderings | at the operation | 10
CONCURRENCY37 | "Loads allow `NoOrdering | Receive | GlobalOrder`; stores allow `NoOrdering | Publish | GlobalOrder`; RMW success allows all five." | Orderings | Only the listed orderings may be used for loads, stores, and RMW success respectively | at the operation | 10
CONCURRENCY38 | "Failure is read-only: it cannot publish or exceed the success ordering." | Orderings | Compare-exchange failure ordering must be read-only and must not publish or rank above the success ordering | at the operation | 10
CONCURRENCY39 | "These legality conditions may be proved by concrete case analysis or carried through generic `requires`/`ensures`; naming a condition supplies no proof." | Orderings | Ordering legality must be proved by case analysis or generic contracts; naming an ordering alone must not satisfy the check | not stated | 8
CONCURRENCY40 | "Conventional relaxed/acquire/release/acquire-release/sequentially-consistent terms are useful references, not additional source spellings." | Orderings | Conventional memory-model names must not be admitted as extra source spellings beyond the defined orderings | not stated | 10
CONCURRENCY41 | "Decisive compare-exchange returns exchanged or mismatch." | Orderings | Decisive compare-exchange outcomes must be only exchanged or mismatch | at the operation | 10
CONCURRENCY42 | "Its realization may retry unsuccessful load-linked/store-conditional attempts, retaining the target-relative work attribution." | Orderings | Decisive compare-exchange realization may retry LL/SC while retaining target-relative work attribution in analysis | not stated | 10
CONCURRENCY43 | "Single-attempt operations additionally return `Uncommitted`: comparison matched but the attempt did not commit, without asserting another participant caused it." | Orderings | Single-attempt operations must allow an `Uncommitted` outcome under the stated conditions | at the operation | 10
CONCURRENCY44 | "Both failure cases use failure ordering; success uses the RMW ordering." | Orderings | Compare-exchange failure paths must use failure ordering; success must use the RMW ordering | at the operation | 10
CONCURRENCY45 | "A nominal single source operation does not hide unbounded target retries from resource analysis." | Orderings | Resource analysis must account for unbounded target retries even when source shows a single attempt | not stated | 3
CONCURRENCY46 | "`Receive` has the strong release-consistency baseline." | Orderings | Unspecialized code must treat `Receive` as the strong release-consistency baseline unless a stated exception applies | not stated | 10
CONCURRENCY47 | "A target may select a weaker processor-consistent acquire only when a protocol proof covers every additional execution and preserves its published facts." | Orderings | Weaker-than-baseline acquire on a target requires a protocol proof covering all additional executions and preserving published facts | not stated | 8
CONCURRENCY48 | "Unspecialized shared code keeps the baseline unless its complete composition is proved." | Orderings | Unspecialized shared code must keep the `Receive` baseline unless complete composition is proved | not stated | 8
CONCURRENCY49 | "`Atomic::fence` accepts a dedicated `FenceOrdering` with `Receive | Publish | ReceivePublish`." | Fences and participants | `Atomic::fence` must accept only the dedicated `FenceOrdering` with the three listed variants | at a call | 10
CONCURRENCY50 | "It emits a normalized language event even when target refinement permits no machine instruction." | Fences and participants | Fence must emit a normalized language event even if no machine instruction is selected | at the operation | 10
CONCURRENCY51 | "A fence alone publishes no memory; synchronization requires a qualifying atomic observation." | Fences and participants | Fence alone must not be treated as publishing memory; synchronization requires a qualifying atomic observation | at the operation | 10
CONCURRENCY52 | "In particular, publication-fence followed by no-ordering store and a no-ordering load followed by receive-fence synchronize only when that load reads from the publication store." | Fences and participants | The stated fence/store/load/fence pattern synchronizes only when the load reads from the publication store | at the operation | 10
CONCURRENCY53 | "Scheduler migration may affect realization but cannot create source-visible synchronization." | Fences and participants | Scheduler migration must not be modeled as creating source-visible synchronization | not stated | 10
CONCURRENCY54 | "Portable atomics range over coherent atomic memory, not a target-selected semantic scope." | Fences and participants | Portable atomic semantics must range over coherent atomic memory, not an arbitrary target-selected scope | not stated | 10
CONCURRENCY55 | "Checked ISA barriers, device/cache/DMA visibility, and asynchronous same-context ordering have distinct participants and contracts." | Fences and participants | ISA barriers, device/cache/DMA visibility, and same-context async ordering must use distinct participants and contracts | not stated | 9
CONCURRENCY56 | "`Atomic::interruption_fence` orders ordinary code and an asynchronous handler on the same execution context, not cross-core or device observation; exact installed-root evidence is required" | Fences and participants | `Atomic::interruption_fence` must order only same-context ordinary code and async handler; cross-core/device exclusion; installed-root evidence required | at the operation | 10
CONCURRENCY57 | "The rules here preserve the required source guarantees; instruction selection does not by itself constitute the formal memory model." | Fences and participants | Formal memory-model guarantees must come from source rules, not from instruction selection alone | not stated | 10
CONCURRENCY58 | "Neither model replaces ordinary ownership, initialization, or conservation." | Protocol proofs | Atomic-event and transition analyses must not replace ordinary ownership, initialization, or conservation checks | not stated | SHARED
CONCURRENCY59 | "Protocol packages may require publication validity, linearizability, FIFO order, or no lost wakeups." | Protocol proofs | When a protocol package states such requirements, checks must enforce the required property | not stated | 8
CONCURRENCY60 | "A stale observation is erroneous only when it violates such a property." | Protocol proofs | Stale observations must be rejected only when they violate a required protocol property | not stated | 8
CONCURRENCY61 | "Initial composition obligations include absence of completion/lock cycles, reachable producer/close/cancellation/timeout for receives, achievable barrier arrival counts, and explicitly modeled or admitted external waits." | Protocol proofs | Initial composition must check for listed obligations including no completion/lock cycles, receive lifecycle reachability, barrier counts, and external waits | not stated | 8
CONCURRENCY62 | "Fairness and other positive progress depend on exact selected provider evidence, not service reach or absence of blocking." | Protocol proofs | Fairness and positive progress claims must rest on selected provider evidence, not mere reachability or lack of blocking | not stated | 8
CONCURRENCY63 | "There is no ambient environment-premise syntax or graph-shaped reach row." | Protocol proofs | Source must not admit ambient environment-premise syntax or graph-shaped reach rows for topology | not stated | 8
CONCURRENCY64 | "Parameter ownership/access, handle multiplicity, claims, and synchronous `invokes` edges constrain admitted topology." | Protocol proofs | Admitted topology must be constrained by parameter ownership/access, handle multiplicity, claims, and synchronous `invokes` edges | not stated | 3
CONCURRENCY65 | "When a concrete customer requires whole-composition proof, a sealed erased model must additionally retain activation creation, resource identities, wait/wake edges, priorities, placement, and provider evidence." | Protocol proofs | Whole-composition proof must retain the listed facts in a sealed erased model | not stated | 8
CONCURRENCY66 | "This extraction remains deferred, not implicit authority supplied by a bounded search or a proposed graph format." | Protocol proofs | Topology extraction must not grant implicit authority via bounded search or a proposed graph format | not stated | 8
CONCURRENCY67 | "Implementation properties travel with their conformances; composed deadlock, starvation, memory, and response guarantees require revalidation after topology or provider changes." | Protocol proofs | Implementation properties must stay with conformances; composed guarantees must be revalidated after topology or provider changes | not stated | 8
CONCURRENCY68 | "Dynamic creation is legal, but quantitative guarantees need fixed topology, conserved creation permits, enforced admission bounds, or a theorem quantified over the dynamic structure." | Protocol proofs | Quantitative guarantees with dynamic creation require one of the listed foundations | not stated | 8
CONCURRENCY69 | "Bounded exploration is a test, not a theorem." | Protocol proofs | Bounded exploration must not be treated as a theorem | not stated | 8
CONCURRENCY70 | "A theorem explicitly quantified over a bounded participant set remains valid within that stated bound." | Protocol proofs | Theorems quantified over a bounded participant set apply only within that bound | not stated | 8
CONCURRENCY71 | "External arrival rates are never inferred." | Protocol proofs | External arrival rates must not be inferred by the checker | not stated | 8
CONCURRENCY72 | "They require admitted provenance or an enforcing rate limiter that derives an admitted-work bound; its rejection or backpressure remains part of the service contract." | Protocol proofs | External arrival rates need admitted provenance or a rate limiter with admitted-work bound; rejection/backpressure stays in the service contract | not stated | 8
CONCURRENCY73 | "Multicore response and blocking guarantees select both a scheduler and a resource-sharing protocol." | Protocol proofs | Multicore response and blocking bounds must name both scheduler and resource-sharing protocol | not stated | 10
CONCURRENCY74 | "A single-core priority-ceiling theorem does not establish bounds for cross-core sharing." | Protocol proofs | Single-core priority-ceiling theorems must not be used alone for cross-core sharing bounds | not stated | 8
CONCURRENCY75 | "A deployment must partition resources per core, forbid that sharing, or select a proved multiprocessor protocol with its own blocking analysis." | Protocol proofs | Cross-core deployments must partition per core, forbid sharing, or use a proved multiprocessor protocol with blocking analysis | not stated | 10

## Ambiguous sentences

Sentence | Reading A | Reading B
--- | --- | ---
"Pools, supervisors, bounded mailboxes, and multiplexed case-bearing events are ordinary library data and policy." | The language/compiler treats these only as library constructs with no special checking. | "Ordinary" might still require specific library contracts to be checked at use sites; this sentence does not say whether the compiler has any role beyond ordinary type checking. |
"Generic code uses one sealed core requirement per primitive operation, shared by ordinary atomics and admitted accessors:" | Each primitive op has exactly one sealed requirement name used in generics. | Ordinary atomics and admitted accessors might share one requirement per op or one requirement object instantiated twice; sharing mechanism is not specified here. |
"Exact-forwarding wrappers may derive conformance mechanically; other realizations need checked proof or admitted evidence." | Mechanical derivation is optional for exact-forwarding wrappers. | Exact-forwarding wrappers must derive mechanically and cannot use manual proof unless they fail the forwarding criterion. |
"Genuine exclusive access may support representation-compatible recasts while preserving all validity and custody obligations." | Exclusive access is sufficient to allow compatible recasts when obligations are preserved. | Recasts are permitted only when access is genuinely exclusive and representation-compatible; whether "may" is permission or capability for the checker is not fixed. |
"Authorized machines may lend the exact access to an atomic helper without making that helper a new establishment route." | Lending is allowed for authorized machines without new establishment. | "May" could mean runtime permission or compile-time proof obligation for authorization before lend; what "authorized machines" means here is external to this chapter. |
"Pre-execution storage can use an ordinary `Empty | Ready(outcome)` wrapper rather than altering the operation's cases." | Wrappers are an allowed pattern for pre-execution storage. | Use of the wrapper might be required in some cases or always optional; "can" does not state when each applies. |
"Decisive compare-exchange returns exchanged or mismatch. Its realization may retry unsuccessful load-linked/store-conditional attempts, retaining the target-relative work attribution." | Outcomes are only exchanged/mismatch while implementation may retry internally. | Retries might surface additional observable states in analysis even though outcomes stay exchanged/mismatch. |
"Single-attempt operations additionally return `Uncommitted`: comparison matched but the attempt did not commit, without asserting another participant caused it." | `Uncommitted` is an allowed third outcome for single-attempt ops. | Single-attempt ops must return `Uncommitted` under the stated condition rather than merely may return it. |
"A target may select a weaker processor-consistent acquire only when a protocol proof covers every additional execution and preserves its published facts." | Weaker acquire is permitted with the stated proof. | "May select" might be target-package choice at build time or proof obligation in source; who selects is not stated. |
"It emits a normalized language event even when target refinement permits no machine instruction." | The fence always appears in the language/event model. | "Emits" might mean only for analysis/metadata while codegen is empty; observable runtime effect when no instruction is unclear. |
"Checked ISA barriers, device/cache/DMA visibility, and asynchronous same-context ordering have distinct participants and contracts." | These four areas must not be merged into one participant in checking. | "Distinct" might mean separate specification documents only, without saying how the compiler partitions checks. |
"Protocol packages may require publication validity, linearizability, FIFO order, or no lost wakeups." | Packages can optionally impose these properties. | When a package requires a property, whether violation is compile-time, proof-time, or runtime-only is not stated. |
"When a concrete customer requires whole-composition proof, a sealed erased model must additionally retain activation creation, resource identities, wait/wake edges, priorities, placement, and provider evidence." | The erased model must retain the listed items for whole-composition proof customers. | "Must retain" could obligate the proof artifact format or only the semantic content available somewhere in the proof closure. |
"Dynamic creation is legal, but quantitative guarantees need fixed topology, conserved creation permits, enforced admission bounds, or a theorem quantified over the dynamic structure." | Any one of the four foundations suffices for quantitative guarantees. | Multiple foundations might need to hold together; "or" might be exclusive disjunction or non-exhaustive examples. |
"A deployment must partition resources per core, forbid that sharing, or select a proved multiprocessor protocol with its own blocking analysis." | Exactly one of the three strategies is required for valid cross-core deployment claims. | A deployment might combine partition and protocol; whether options are mutually exclusive is not stated. |

## Terms used without definition in this chapter

- ordinary machines
- admitted task runtime
- async machine species
- futures
- sanctioned concurrent-access contract
- compatible carry
- copyability (permission)
- exclusive transfer (permission)
- shared mutation (permission)
- pools / supervisors / bounded mailboxes
- multiplexed case-bearing events
- privileged task-group construct
- physical storage
- provider custody
- task-claim ownership
- dedicated core types (atomics)
- sealed core requirement
- admitted accessors
- access policies
- outcome types
- canonical cases
- resident-custody rules
- operation-specific conformance
- exact-forwarding wrappers
- checked proof
- admitted evidence
- representation recasts
- access authority
- atomic shared view
- ordinary read view
- backing exclusion
- non-atomic observation
- genuine exclusive access
- validity and custody obligations
- mutex payload-custody protocol
- domain access policy
- read grant / write grant
- domain's writer list
- establishment route
- atomic helper
- encode(expected)
- user equality
- copyable key's encoding law
- fetch proofs
- provider-readable raw representation
- logical operation
- external reads/writes
- racing load
- outcome construction / zero initialization
- checked operation contract
- zero outcome tag
- zero-valid payload
- `Empty | Ready(outcome)` wrapper
- proof-static operation data
- `NoOrdering` / `Receive` / `Publish` / `ReceivePublish` / `GlobalOrder`
- RMW
- failure ordering / success ordering
- decisive compare-exchange
- load-linked/store-conditional
- target-relative work attribution
- single-attempt operations
- `Uncommitted`
- resource analysis
- strong release-consistency baseline
- processor-consistent acquire
- protocol proof
- published facts
- unspecialized shared code
- complete composition
- `FenceOrdering`
- normalized language event
- target refinement
- qualifying atomic observation
- publication-fence / receive-fence
- publication store
- `sequenced_before` / `reads_from` / `modification_order` / `synchronizes_with` / `happens_before` / `global_sequential_order`
- scheduler migration
- source-visible synchronization
- coherent atomic memory
- target-selected semantic scope
- checked ISA barriers
- device/cache/DMA visibility
- asynchronous same-context ordering
- participants (fence/barrier/device)
- device protocols
- scoped publication/acquisition
- custody release
- `Atomic::interruption_fence`
- asynchronous handler
- execution context
- installed-root evidence
- external-root admission
- formal memory model
- instruction selection
- Atomic-event analysis
- transition analysis
- concurrently activated machine graphs
- waits-for / owns / releases / unblocks edges
- conservation
- protocol packages
- publication validity
- linearizability
- FIFO order
- no lost wakeups
- stale observation
- initial composition obligations
- completion/lock cycles
- barrier arrival counts
- admitted external waits
- fairness
- positive progress
- selected provider evidence
- service reach
- ambient environment-premise syntax
- graph-shaped reach row
- handle multiplicity
- claims
- synchronous `invokes` edges
- admitted topology
- whole-composition proof
- sealed erased model
- activation creation
- wait/wake edges
- priorities / placement
- bounded search
- proposed graph format
- implementation properties
- conformances
- quantitative guarantees
- conserved creation permits
- enforced admission bounds
- bounded exploration
- bounded participant set
- external arrival rates
- admitted provenance
- enforcing rate limiter
- admitted-work bound
- backpressure
- service contract
- multicore response and blocking guarantees
- scheduler
- resource-sharing protocol
- single-core priority-ceiling theorem
- cross-core sharing
- proved multiprocessor protocol
- blocking analysis

CHAPTER COMPLETE
