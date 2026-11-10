# Optimization selection and validation

Source: `wiki/spec/build/optimizations.md` lines 1–266 (read).

## Rules

| rule id | the exact sentence, quoted | the heading it is under | what must be checked, in plain words | when it must hold | lane |
| --- | --- | --- | --- | --- | --- |
| OPTIMIZATIONS1 | "Optimization changes a realization while preserving its semantic contract." | Optimization selection and validation | Applied optimizations must preserve the semantic contract of the realization they change | not stated | SHARED |
| OPTIMIZATIONS2 | "Optimization may rely on producer-established facts only for trusted input under [Psi admission](../terminal-psi/product.md#publication-and-separate-consumption)." | Optimization selection and validation | Facts used to justify optimization must come only from trusted input admitted under Psi admission rules | not stated | 9 |
| OPTIMIZATIONS3 | "A supplied certificate or canonical encoding does not authorize that trust." | Optimization selection and validation | Presence of a certificate or canonical encoding must not be treated as authorizing trust for optimization | not stated | 9 |
| OPTIMIZATIONS4 | "Interpreter containment does not authorize optimizing untrusted guest claims into omitted checks or lowering them to native code." | Optimization selection and validation | Untrusted guest claims must not be optimized into omitted checks or native lowering solely because they run under interpreter containment | not stated | SHARED |
| OPTIMIZATIONS5 | "The selection is duplicate-free, canonical, versioned, and identity-bearing." | Exact selection | Enabled optimization selections must be duplicate-free, canonical, versioned, and carry stable identity | not stated | 9 |
| OPTIMIZATIONS6 | "Human report emission is a separate explicit request." | Exact selection | Human-readable optimization reports must require a separate explicit request, not implicit emission | not stated | 9 |
| OPTIMIZATIONS7 | "No target, environment variable, compiler default, build mode, or broad `O1`/`O2`/`O3` level implicitly enables an optimization." | Exact selection | Optimizations must not be enabled implicitly by target, environment, defaults, build mode, or coarse `O` levels | not stated | 9 |
| OPTIMIZATIONS8 | "Every optimization has exactly one phase owner." | Exact selection | Each optimization rule must belong to exactly one owning phase | not stated | 9 |
| OPTIMIZATIONS9 | "The complete selection projects into closed phase-local sets for checked trees, Psi, abstract operations, target operations/instruction selection, pre-allocation, allocation recovery, post-allocation, and resolved layout." | Exact selection | Global selection must project into closed phase-local sets for each listed phase | not stated | 9 |
| OPTIMIZATIONS10 | "A stage consumes only its projection; downstream coordinators must not rescan the global selection to construct another schedule." | Exact selection | Each stage must use only its phase projection; downstream must not rebuild schedules from the global selection | not stated | 9 |
| OPTIMIZATIONS11 | "Projection retains the complete build-selection identity so evidence from different builds cannot be silently recombined." | Exact selection | Phase projections must retain full build-selection identity to prevent silent cross-build evidence recombination | not stated | 9 |
| OPTIMIZATIONS12 | "Psi-local pass identities do not import target-specific Omega optimization vocabulary." | Exact selection | Psi-local pass identities must not incorporate target-specific Omega optimization names | not stated | 9 |
| OPTIMIZATIONS13 | "Each stage has one ordered catalog." | Exact selection | Each optimization stage must have exactly one ordered rule catalog | not stated | 9 |
| OPTIMIZATIONS14 | "An exact entry binds its source name, rule and validator identities, policy, required analyses, invalidations, work budgets, and applicability." | Exact selection | Catalog entries must bind all listed metadata fields | not stated | 9 |
| OPTIMIZATIONS15 | "Target independence is explicit." | Exact selection | Target independence of rules must be explicit in catalog entries | not stated | 9 |
| OPTIMIZATIONS16 | "Target-specific selection checks applicability before dispatch and identifies the exact rule, required architecture, and actual architecture on rejection." | Exact selection | Target-specific dispatch must pre-check applicability and on rejection report rule identity and required versus actual architecture | not stated | 9 |
| OPTIMIZATIONS17 | "Independent rule validation checks applicability again." | Exact selection | Independent rule validation must re-check applicability | not stated | 9 |
| OPTIMIZATIONS18 | "Custody coordinators consume the selected entry, not a duplicate name-to-rule table." | Exact selection | Coordinators must consume the selected catalog entry, not a duplicate lookup table | not stated | 9 |
| OPTIMIZATIONS19 | "Unknown selection or encoding versions, unsupported targets, and unsupported compositions reject rather than dropping a rule or guessing compatibility." | Exact selection | Unknown versions, unsupported targets, or unsupported compositions must reject; must not drop rules or guess compatibility | not stated | 9 |
| OPTIMIZATIONS20 | "Catalog presence is not evidence of universal program-shape or publication support." | Exact selection | Catalog membership must not be treated as proof of universal program-shape or publication support | not stated | 9 |
| OPTIMIZATIONS21 | "Mandatory normalization, legalization, allocation, and realization remain necessary whether or not optional profitability rewrites are selected." | Exact selection | Required normalization, legalization, allocation, and realization phases must run even when no optional optimizations are selected | not stated | 9 |
| OPTIMIZATIONS22 | "An optimization normally consumes and produces the same representation." | Phase and product boundaries | Optimizations must normally read and write the same IR representation | not stated | SHARED |
| OPTIMIZATIONS23 | "A new representation requires a different semantic vocabulary, invariant set, or product boundary; optimization history alone is not such a boundary." | Phase and product boundaries | Representation change must coincide with a new vocabulary, invariants, or product boundary; optimization history alone must not justify a new representation | not stated | 9 |
| OPTIMIZATIONS24 | "Every phase receives its input and its exact phase selection, executes selected rules in canonical order under explicit bounds, and returns one typed stage result with execution evidence." | Phase and product boundaries | Each phase must take phased input and selection, run rules in canonical bounded order, and return one typed result with evidence | not stated | 9 |
| OPTIMIZATIONS25 | "Candidate ordering is deterministic;" | Phase and product boundaries | Ordering among optimization candidates within a phase must be deterministic | not stated | 9 |
| OPTIMIZATIONS26 | "fixed-point passes have explicit convergence and iteration budgets." | Phase and product boundaries | Fixed-point optimization passes must declare convergence criteria and iteration budgets | not stated | 9 |
| OPTIMIZATIONS27 | "Each applied transformation and the final representation keep an independent validator." | Phase and product boundaries | Each applied transform and the final IR must retain an independent validator | not stated | SHARED |
| OPTIMIZATIONS28 | "the corpus runner and the tests run those validators; the default compile path does not re-validate the input or output that the same compile just produced." | Phase and product boundaries | Corpus/tests must run stage validators; default compilation must not re-validate the same compile's immediate in/out | not stated | 9 |
| OPTIMIZATIONS29 | "Explicit [validator selection](command_line.md#independent-stage-validators) runs requested applicable validators inside a compilation and reports their actual coverage;" | Phase and product boundaries | Explicit validator selection must run only requested applicable validators and report coverage | not stated | 9 |
| OPTIMIZATIONS30 | "it does not change optimization selection or required admission checks." | Phase and product boundaries | Validator selection must not alter optimization selection or admission checks | not stated | 9 |
| OPTIMIZATIONS31 | "An empty selection is the same phase's identity execution: output equals input, with no pass manifests or transformation records, and the validators accept it as the identity." | Phase and product boundaries | Empty phase selection must yield identity output, no manifests or transform records, and validator acceptance | not stated | 9 |
| OPTIMIZATIONS32 | "Empty selection is not a separate compiler or publication route." | Phase and product boundaries | Empty selection must use the same compiler and publication routes as non-empty selection | not stated | 9 |
| OPTIMIZATIONS33 | "Disabling every selected rule yields the same identity-phase acceptance, diagnostics, and output as an initially empty selection, apart from the separately retained rollback provenance." | Phase and product boundaries | Disabling all selected rules must match empty-selection identity behavior except separate rollback provenance | not stated | 9 |
| OPTIMIZATIONS34 | "Checked-tree pruning runs only after every declaration of the check's [validation subjects](validation_subjects.md) and their required dependency closure has been parsed, resolved, typed, and checked;" | Phase and product boundaries | Checked-tree pruning must not run until validation subjects and dependency closure are fully parsed, resolved, typed, and checked | not stated | SHARED |
| OPTIMIZATIONS35 | "deleting unreachable code cannot make invalid source valid within that declared scope." | Phase and product boundaries | Pruning unreachable code must not cause invalid source to become valid within the declared validation scope | not stated | SHARED |
| OPTIMIZATIONS36 | "A root selection that changes the published product retains its own explicit identity, not one inferred from the remaining declarations." | Phase and product boundaries | Root product selection that changes publication must keep explicit identity, not one inferred from surviving declarations | not stated | 9 |
| OPTIMIZATIONS37 | "Shared frontend work precedes target fan-out under the [target-selection contract](configuration.md)." | Phase and product boundaries | Shared frontend work must complete before target fan-out per target-selection contract | not stated | 9 |
| OPTIMIZATIONS38 | "Terminal Psi is the immutable output of Psi optimization." | Phase and product boundaries | Published Terminal Psi after Psi optimization must be treated as immutable | not stated | 9 |
| OPTIMIZATIONS39 | "Publication retains the exact Psi selection and input/output semantic identities;" | Phase and product boundaries | Publication must record exact Psi selection and input/output semantic identities | not stated | 9 |
| OPTIMIZATIONS40 | "decoding checks the recorded output against the published semantics." | Phase and product boundaries | Decoding published Psi must verify recorded output against published semantics | not stated | 9 |
| OPTIMIZATIONS41 | "Proof identities remain internal validation provenance." | Phase and product boundaries | Proof identities must not be published as product semantics; they remain internal provenance | not stated | 8 |
| OPTIMIZATIONS42 | "Empty execution may claim only equal input and output identities." | Phase and product boundaries | Identity-phase empty execution may only assert equal input and output semantic identities | not stated | 9 |
| OPTIMIZATIONS43 | "A receiving interpreter or native lowerer does not rerun Psi passes." | Phase and product boundaries | Consumers must not rerun Psi optimization passes on received Terminal Psi | not stated | 9 |
| OPTIMIZATIONS44 | "A target-constrained build companion may carry pending post-Terminal selections, excluding earlier phases and bound to the complete build selection." | Phase and product boundaries | Post-Terminal companion artifacts may carry only pending post-Terminal selections bound to full build selection | not stated | 9 |
| OPTIMIZATIONS45 | "It is a proposal a receiving lowerer may refuse, not authority to execute a transformation." | Phase and product boundaries | Companion selections are proposals; lowerer may refuse; they must not authorize transformation by themselves | not stated | 9 |
| OPTIMIZATIONS46 | "A raw portable product leaves subsequent realization choices to its receiver." | Phase and product boundaries | Raw portable products must not bind receiver realization choices except as carried in the product | not stated | 9 |
| OPTIMIZATIONS47 | "A [home-layout transformation](../layouts/plans.md#home-layout-realization) may change physical storage while preserving the semantic contract." | Observations and resources | Layout transformations may change physical storage only while preserving semantic contract | not stated | 9 |
| OPTIMIZATIONS48 | "This includes compact case encodings without a separate tag." | Observations and resources | Compact case encodings without separate tags must still preserve semantic contract | not stated | 1 |
| OPTIMIZATIONS49 | "Applicable layout commitments, queries, recasts and aliasing observations constrain the change alongside ABI contracts." | Observations and resources | Layout changes must respect committed layouts, queries, recasts, aliasing observations, and ABI contracts | not stated | 9 |
| OPTIMIZATIONS50 | "All consumers must agree on the selected realization;" | Observations and resources | All pipeline consumers must agree on the chosen realization | not stated | 9 |
| OPTIMIZATIONS51 | "a later pass cannot invalidate an admitted cast, change an already-fixed layout observation, or impose new source mutation restrictions to enable its encoding." | Observations and resources | Later passes must not invalidate admitted casts, alter fixed layout observations, or add source mutation restrictions for encoding | not stated | SHARED |
| OPTIMIZATIONS52 | "A transformation preserves every observation admitted by the program and its selected boundary contracts:" | Observations and resources | Transforms must preserve all admitted program and boundary observations listed below | not stated | SHARED |
| OPTIMIZATIONS53 | "returned values, state transitions, and observable trap kinds and ordering;" | Observations and resources | Returned values, transitions, and trap kinds/ordering must be preserved | not stated | SHARED |
| OPTIMIZATIONS54 | "service/effect, atomic, volatile, placed-memory, and cleanup ordering;" | Observations and resources | Listed effect, atomic, volatile, memory-placement, and cleanup orderings must be preserved | not stated | 10 |
| OPTIMIZATIONS55 | "ABI-visible calls, arguments, results, clobbers, exit/unwind behavior, and externally visible storage;" | Observations and resources | ABI-visible call surfaces, clobbers, exit/unwind, and external storage must be preserved | not stated | 3 |
| OPTIMIZATIONS56 | "ownership, borrow, alias, and address-stability restrictions; and" | Observations and resources | Ownership, borrow, alias, and address-stability restrictions must be preserved | not stated | 2 |
| OPTIMIZATIONS57 | "provenance and debug roots required by the selected reporting contract." | Observations and resources | Required provenance and debug roots for the reporting contract must be preserved | not stated | 9 |
| OPTIMIZATIONS58 | "An unused result or empty reach alone does not prove purity." | Observations and resources | Purity must not be inferred from unused results or empty reach alone | not stated | SHARED |
| OPTIMIZATIONS59 | "Purity is a closed classification reconstructed from operation semantics." | Observations and resources | Purity classification must be derived only from operation semantics per closed rules | not stated | SHARED |
| OPTIMIZATIONS60 | "Calls, traps, services, atomics, volatile/policy-governed accesses, cleanup, and transitions are barriers unless an exact rule proves the proposed change preserves their contract." | Observations and resources | Listed effects are optimization barriers unless an exact rule proves contract preservation | not stated | SHARED |
| OPTIMIZATIONS61 | "Operation identity retains width, signedness, domain/provider, and arithmetic policy." | Observations and resources | Transformed operations must retain width, signedness, domain/provider, and arithmetic policy identity | not stated | 5 |
| OPTIMIZATIONS62 | "Exact operations retain the same discharged proof/check obligation;" | Observations and resources | Exact operations must keep the same discharged proof and check obligations | not stated | 8 |
| OPTIMIZATIONS63 | "wrapping, saturating, and trapping policies are not interchangeable." | Observations and resources | Wrapping, saturating, and trapping arithmetic policies must not be interchanged by optimization | not stated | 5 |
| OPTIMIZATIONS64 | "Shift-count rules and fused versus unfused floating-point operations remain distinct." | Observations and resources | Shift-count semantics and fused vs unfused float operations must remain distinct | not stated | 5 |
| OPTIMIZATIONS65 | "NaNs, payloads, signed zero, infinities, and rounding follow the named operation." | Observations and resources | Float NaN, payload, signed-zero, infinity, and rounding behavior must match the named operation | not stated | 5 |
| OPTIMIZATIONS66 | "There is no ambient fast-math switch." | Observations and resources | No global fast-math mode may alter float semantics | not stated | 5 |
| OPTIMIZATIONS67 | "A lossy family would require a separately accepted source-visible semantic contract declaring its observable differences, exact rule identity, and tests." | Observations and resources | Lossy optimization families require explicit accepted source-visible contract, rule identity, and tests | not stated | 5 |
| OPTIMIZATIONS68 | "Instruction count, code size, register pressure, compiler work, and elapsed time are costs, not [logical work](../resources/logical_work.md)." | Observations and resources | Listed compile/resource metrics must not be treated as logical work | not stated | 9 |
| OPTIMIZATIONS69 | "The Terminal work schedule is external deterministic accounting;" | Observations and resources | Terminal logical work must use external deterministic accounting schedule | not stated | 9 |
| OPTIMIZATIONS70 | "exhaustion is an incomplete consumer outcome, not a source result," | Observations and resources | Work exhaustion must surface as incomplete consumer outcome, not as a source-language result | not stated | 9 |
| OPTIMIZATIONS71 | "and native execution has no implicit work meter." | Observations and resources | Native execution must not imply an implicit work meter | not stated | 10 |
| OPTIMIZATIONS72 | "Fixed-work certificates use the immutable Terminal subject." | Observations and resources | Fixed-work certificates must bind to the immutable Terminal subject | not stated | 9 |
| OPTIMIZATIONS73 | "Relocating a source operation retains its exact work settlement and provenance at the new realization site;" | Observations and resources | Moved operations must retain exact work settlement and provenance at the new site | not stated | 9 |
| OPTIMIZATIONS74 | "mutable IR cannot invent loop charges." | Observations and resources | Optimized mutable IR must not introduce new loop work charges | not stated | 4 |
| OPTIMIZATIONS75 | "An evaluator of optimized IR needs an explicit accounting contract before activation." | Observations and resources | IR evaluators must require explicit accounting contract before activating optimized IR | not stated | 9 |
| OPTIMIZATIONS76 | "Final spill-inclusive frame demand remains bound to the exact optimized, allocated, and realized artifact." | Observations and resources | Frame demand evidence must bind to the exact final optimized allocated realized artifact | not stated | 9 |
| OPTIMIZATIONS77 | "Equal Terminal semantics alone cannot justify reusing physical demand evidence." | Observations and resources | Semantically equal Terminal Psi must not alone authorize reuse of physical stack demand evidence | not stated | 9 |
| OPTIMIZATIONS78 | "[Stack provisioning](../resources/storage.md) must establish backing before execution;" | Observations and resources | Stack backing must be established before execution per stack provisioning rules | not stated | 9 |
| OPTIMIZATIONS79 | "a size comparison alone does not." | Observations and resources | Size comparison alone must not satisfy stack provisioning | not stated | 9 |
| OPTIMIZATIONS80 | "Physical access validation and required target setup remain mandatory." | Observations and resources | Physical access validation and required target setup must remain mandatory after optimization | not stated | 10 |
| OPTIMIZATIONS81 | "Private spills may change native resource demand, never the source `crashes` ceiling." | Observations and resources | Private spills may alter native demand but must not raise source `crashes` ceiling | not stated | 3 |
| OPTIMIZATIONS82 | "The complete final artifact still satisfies [machine-state realization](machine_state_evidence.md)." | Observations and resources | Final artifact must satisfy machine-state realization evidence requirements | not stated | 10 |
| OPTIMIZATIONS83 | "Accepted proof obligations and ownership/borrow facts may justify check removal, arithmetic identities, non-aliasing, field/variant irrelevance, cleanup or transition reachability, and terminating or bounded loop transformations." | Evidence and control flow | Listed optimizations may use only accepted proof and ownership/borrow facts as justification | not stated | 8 |
| OPTIMIZATIONS84 | "Candidates and receipts retain the exact consumed fact identities." | Evidence and control flow | Optimization candidates and receipts must record exact consumed fact identities | not stated | 8 |
| OPTIMIZATIONS85 | "Evidence cannot disappear before its last dependent transformation or diagnostic reader." | Evidence and control flow | Evidence must remain available until the last dependent transform or diagnostic consumer | not stated | 8 |
| OPTIMIZATIONS86 | "Qualification rosters retain exact canonical paths, carriers, and establishment lineage through transformations and lowering." | Evidence and control flow | Qualification rosters must keep canonical paths, carriers, and establishment lineage through transforms and lowering | not stated | 8 |
| OPTIMIZATIONS87 | "Root shape or carrier equality cannot manufacture a field qualification." | Evidence and control flow | Equal root shape or carrier must not create field qualifications | not stated | 8 |
| OPTIMIZATIONS88 | "Calls and returns preserve their exact source and result contracts." | Evidence and control flow | Optimized calls and returns must preserve exact source and result contracts | at a call; at normal return | 3 |
| OPTIMIZATIONS89 | "At an actual control-flow join, an output retains only qualifications carried by every incoming occurrence through valid lineage." | Evidence and control flow | Join outputs may retain only qualifications common to all incoming paths with valid lineage | on every edge | 8 |
| OPTIMIZATIONS90 | "CSE/GVN distinguish unequal rosters unless a named transformation forms their common intersection and independently revalidates every use." | Evidence and control flow | CSE/GVN must treat unequal qualification rosters as distinct unless a named rule forms intersection and revalidates all uses | not stated | 8 |
| OPTIMIZATIONS91 | "Inventing a qualification is unsound;" | Evidence and control flow | Optimizer must not invent qualifications | not stated | 8 |
| OPTIMIZATIONS92 | "losing one may cause rejection, but fail-closed narrowing alone is not evidence of an exact-preserving Terminal transformation." | Evidence and control flow | Qualification loss may reject; fail-closed narrowing alone must not count as exact-preserving Terminal proof | not stated | 8 |
| OPTIMIZATIONS93 | "Analyses use explicit entry, exit, exceptional, cleanup, and transition edges." | Evidence and control flow | Analyses must model explicit entry, exit, exceptional, cleanup, and transition edges | not stated | SHARED |
| OPTIMIZATIONS94 | "Suspension is an interprocedural state of the exact call, not an invented local successor." | Evidence and control flow | Suspension must attach to the exact call interprocedurally; must not invent local successor edges | not stated | 3 |
| OPTIMIZATIONS95 | "Cyclic components retain verifier-derived topology, loop-carried arguments, ownership fixed points, and their distinct ranked, bounded, or unranked progress evidence under [control flow](../terminal-psi/control_flow.md)." | Evidence and control flow | Cycle analyses must retain topology, loop-carried args, ownership fixed points, and progress evidence per control-flow rules | not stated | 4 |
| OPTIMIZATIONS96 | "An analysis receipt does not itself authorize execution, a rewrite, a work certificate, or publication." | Evidence and control flow | Analysis receipts must not alone authorize execution, rewrite, work certificate, or publication | not stated | 8 |
| OPTIMIZATIONS97 | "Transformations must preserve or independently reconstruct the cycle and ranking evidence their consumers require." | Evidence and control flow | Transforms must preserve or rebuild required cycle and ranking evidence for downstream consumers | not stated | 4 |
| OPTIMIZATIONS98 | "Analyses are immutable products keyed by validated input revision and declared dependencies." | Atomic candidates and independent validation | Analysis products must be immutable and keyed by validated input revision and dependencies | not stated | 9 |
| OPTIMIZATIONS99 | "A rule states what it consumes and invalidates." | Atomic candidates and independent validation | Each rule must declare consumed and invalidated analysis products | not stated | 9 |
| OPTIMIZATIONS100 | "After a commit, each affected product is either preserved by proof or recomputed;" | Atomic candidates and independent validation | After a transform commit, affected analyses must be proved preserved or recomputed | not stated | 9 |
| OPTIMIZATIONS101 | "stale facts cannot be reused merely because their cached value remains available." | Atomic candidates and independent validation | Stale analysis facts must not be reused from cache without validity proof | not stated | 9 |
| OPTIMIZATIONS102 | "A candidate binds source and selection identities; exact rule, candidate, and affected region; required analyses and typed facts; immutable patch or plan; any predicted cost; provenance/work mapping; and consumed work." | Atomic candidates and independent validation | Candidates must bind all listed identity, rule, region, fact, plan, cost, provenance, and work fields | not stated | 9 |
| OPTIMIZATIONS103 | "Construction does not mutate the input." | Atomic candidates and independent validation | Building a candidate must not mutate the input representation | not stated | 9 |
| OPTIMIZATIONS104 | "Independent validation reconstructs preconditions, applies the plan to a fresh value, validates the result, and issues an identity-bound receipt." | Atomic candidates and independent validation | Independent validation must reconstruct preconditions, apply plan to fresh value, validate, and issue identity-bound receipt | not stated | 9 |
| OPTIMIZATIONS105 | "Only then may the manager commit the replacement." | Atomic candidates and independent validation | Manager may commit replacement only after independent validation receipt | not stated | 9 |
| OPTIMIZATIONS106 | "A matcher or producer cannot attest to itself by serving as its own validator." | Atomic candidates and independent validation | Producer/matcher must not act as its own validator for its candidates | not stated | 9 |
| OPTIMIZATIONS107 | "Validation reconstructs representation invariants, exact rule preconditions, stage accounting/invalidation/provenance, and source-to-lowered contracts at their owning boundaries." | Atomic candidates and independent validation | Validation must reconstruct invariants, rule preconditions, stage accounting, and source-to-lowered contracts at owning boundaries | not stated | SHARED |
| OPTIMIZATIONS108 | "Differential tests compare reference and transformed execution where an interpreter or executable oracle exists;" | Atomic candidates and independent validation | Where oracles exist, differential tests must compare reference and transformed execution | not stated | 9 |
| OPTIMIZATIONS109 | "test agreement is not the definition of semantic equivalence or substitute publication authority." | Atomic candidates and independent validation | Passing differential tests must not substitute for semantic equivalence proof or publication authority | not stated | 9 |
| OPTIMIZATIONS110 | "Surviving and synthesized values, instructions, blocks, edges, and bytes retain roots identifying source constructs, preserved operations, applied rules, and justifying facts, sufficient for required diagnostics and reports." | Atomic candidates and independent validation | IR artifacts must retain provenance roots for constructs, operations, rules, and facts needed for diagnostics and reports | not stated | SHARED |
| OPTIMIZATIONS111 | "Publication still needs the full validation chain binding child identities and rejecting detached, reordered, truncated, trailing, stale, or cross-source evidence." | Atomic candidates and independent validation | Publication must require full validation chain and reject malformed evidence | not stated | 9 |
| OPTIMIZATIONS112 | "Encoding, provenance, and cost estimates alone grant no publication authority." | Atomic candidates and independent validation | Encoding, provenance, or cost metadata alone must not authorize publication | not stated | 9 |
| OPTIMIZATIONS113 | "Machine-readable decision rows bind input, candidate, rule, verdict, consumed analyses/facts, validator, budget, and usage." | Decisions and nonauthoritative policy | Decision rows must bind all listed fields | not stated | 9 |
| OPTIMIZATIONS114 | "Publication rejoins each validated candidate declaration to its selected pass, complete rule contract, exact input revision, manifest evidence, and baseline-policy row." | Decisions and nonauthoritative policy | Publication must link each validated candidate to pass, rule contract, input revision, manifest, and policy row | not stated | 9 |
| OPTIMIZATIONS115 | "An applied row binds exactly one transformation commit;" | Decisions and nonauthoritative policy | Each applied decision row must bind exactly one transformation commit | not stated | 9 |
| OPTIMIZATIONS116 | "a skipped row binds none." | Decisions and nonauthoritative policy | Skipped decision rows must bind no transformation commit | not stated | 9 |
| OPTIMIZATIONS117 | "Coordinated rewriting manifest facts, predicted costs, and policy records cannot replace this independent reconstruction." | Decisions and nonauthoritative policy | Manifest, cost, and policy rewrites must not substitute for independent decision reconstruction | not stated | 9 |
| OPTIMIZATIONS118 | "Human reports project these rows, not the reverse." | Decisions and nonauthoritative policy | Human reports must be projections of machine rows, not authoritative sources | not stated | 9 |
| OPTIMIZATIONS119 | "Selection policies rank or choose already-declared validated candidates, separately from candidate construction and independent validation." | Decisions and nonauthoritative policy | Selection policies may only rank or choose validated candidates; must stay separate from construction and validation | not stated | 9 |
| OPTIMIZATIONS120 | "Explicitly supplied decisions use a versioned schema;" | Decisions and nonauthoritative policy | Externally supplied decision files must use a versioned schema | not stated | 9 |
| OPTIMIZATIONS121 | "their contexts bind source, selections, target, rule catalog, cost model, and the canonical candidate rows." | Decisions and nonauthoritative policy | Supplied decision contexts must bind source, selections, target, catalog, cost model, and canonical candidate rows | not stated | 9 |
| OPTIMIZATIONS122 | "Each decision chooses a listed candidate or skips with a reason;" | Decisions and nonauthoritative policy | Each decision must select a listed candidate or skip with recorded reason | not stated | 9 |
| OPTIMIZATIONS123 | "it does not author a rewrite or semantic contract." | Decisions and nonauthoritative policy | Decision records must not create rewrites or semantic contracts | not stated | 9 |
| OPTIMIZATIONS124 | "Recording independently reconstructs the rows;" | Decisions and nonauthoritative policy | Persisted decisions must be reconstructable independently from inputs | not stated | 9 |
| OPTIMIZATIONS125 | "replay requires exact context and row equality." | Decisions and nonauthoritative policy | Replay must require exact decision context and row equality | not stated | 9 |
| OPTIMIZATIONS126 | "Missing, malformed, stale, or mismatched supplied decisions reject or use an explicitly selected deterministic fallback." | Decisions and nonauthoritative policy | Bad supplied decisions must reject or use only an explicitly selected deterministic fallback | not stated | 9 |
| OPTIMIZATIONS127 | "Cost estimates rank candidates;" | Decisions and nonauthoritative policy | Cost estimates may rank candidates only | not stated | 9 |
| OPTIMIZATIONS128 | "they cannot establish eligibility or semantic correctness." | Decisions and nonauthoritative policy | Cost estimates must not establish rule eligibility or semantic correctness | not stated | 9 |
| OPTIMIZATIONS129 | "Keep deterministic selection usable without a model." | Decisions and nonauthoritative policy | Selection pipeline must remain usable with deterministic policy when no learned model is present | not stated | 9 |
| OPTIMIZATIONS130 | "Do not build a trainer, training corpus pipeline, model evaluator, or inference integration in the Rust reference compiler." | Decisions and nonauthoritative policy | Rust reference compiler must not include ML trainer, corpus pipeline, model evaluator, or inference integration | not stated | 9 |
| OPTIMIZATIONS131 | "A future learned policy belongs to separately justified work on the Omega-written product compiler and would choose through these same candidate and validation boundaries, not a second compiler path." | Decisions and nonauthoritative policy | Any future learned policy must use the same candidate and validation boundaries, not a parallel compiler path | not stated | 9 |
| OPTIMIZATIONS132 | "The repeatable `--disable-optimization <ExactName>` overlay subtracts exact case-sensitive source names without changing the authored build selection or its checked-compilation identity." | Release rollback and promotion | Disable overlay must subtract exact case-sensitive names without altering authored selection or checked-compilation identity | not stated | 9 |
| OPTIMIZATIONS133 | "Unknown and duplicate names reject." | Release rollback and promotion | Unknown or duplicate disable names must reject | not stated | 9 |
| OPTIMIZATIONS134 | "A known unselected name is an accepted visible no-op, subject to the requested product's phase restriction." | Release rollback and promotion | Known but unselected disable names are visible no-ops within product phase restrictions | not stated | 9 |
| OPTIMIZATIONS135 | "Settlement occurs before the selected stages execute." | Release rollback and promotion | Disable overlay settlement must complete before selected optimization stages run | not stated | 9 |
| OPTIMIZATIONS136 | "Effective selection feeds their execution and resulting artifact identities;" | Release rollback and promotion | Stage execution and artifact identities must use effective selection after overlay | not stated | 9 |
| OPTIMIZATIONS137 | "a separate receipt retains authored, requested, applied, and effective sets." | Release rollback and promotion | Rollback receipt must retain authored, requested, applied, and effective selection sets | not stated | 9 |
| OPTIMIZATIONS138 | "The overlay cannot add rules, change the catalog, or masquerade as source selection." | Release rollback and promotion | Disable overlay must not add rules, alter catalog, or impersonate source selection | not stated | 9 |
| OPTIMIZATIONS139 | "An empty request creates no rollback receipt;" | Release rollback and promotion | Empty disable request must produce no rollback receipt | not stated | 9 |
| OPTIMIZATIONS140 | "an empty effective set executes the ordinary identity phases." | Release rollback and promotion | Empty effective selection must run ordinary identity phases | not stated | 9 |
| OPTIMIZATIONS141 | "A product cannot claim rollback for stages it does not execute." | Release rollback and promotion | Products must not report rollback for phases they do not run | not stated | 9 |
| OPTIMIZATIONS142 | "Check-only production rejects nonempty requests;" | Release rollback and promotion | Check-only products must reject nonempty disable requests | not stated | 9 |
| OPTIMIZATIONS143 | "Terminal production admits checked-tree and Psi-phase rollback, not post-Terminal names;" | Release rollback and promotion | Terminal products may disable only checked-tree and Psi-phase names, not post-Terminal | not stated | 9 |
| OPTIMIZATIONS144 | "native production admits its phase-scoped overlay." | Release rollback and promotion | Native products admit disable overlay only for their own phase scope | not stated | 9 |
| OPTIMIZATIONS145 | "Invalid requests reject before frontend work or auxiliary output." | Release rollback and promotion | Invalid disable requests must reject before frontend work or auxiliary output | not stated | 9 |
| OPTIMIZATIONS146 | "Successful publication retains rollback provenance separately from effective optimization identity and reports exact requested, applied, and effective names." | Release rollback and promotion | Publication must separate rollback provenance from effective optimization identity and report exact name sets | not stated | 9 |
| OPTIMIZATIONS147 | "Caches bind source, selections, target facts, catalogs, validators, cost model, and any relevant workload/profile identities." | Release rollback and promotion | Optimization caches must key on all listed identity components | not stated | 9 |
| OPTIMIZATIONS148 | "Promotion is not automatic:" | Release rollback and promotion | Rules must not be promoted to recommended or default without explicit owner decision | not stated | 9 |
| OPTIMIZATIONS149 | "an exact rule may become recommended or default only through a separate owner decision supported by semantic/corruption, differential/reference, determinism/bounded-work, target/OS, compile-time/output-quality, and exact-rule rollback evidence." | Release rollback and promotion | Promotion to recommended/default requires owner decision and listed evidence categories | not stated | 9 |
| OPTIMIZATIONS150 | "No current default is implied." | Release rollback and promotion | Catalog must not imply current default optimizations without explicit promotion | not stated | 9 |
| OPTIMIZATIONS151 | "Promotion never creates a broad optimization level;" | Release rollback and promotion | Promotion must not create coarse `O1`/`O2`/`O3`-style levels | not stated | 9 |
| OPTIMIZATIONS152 | "diagnostics and manifests continue naming exact rules." | Release rollback and promotion | Diagnostics and manifests must name exact rule identities after promotion | not stated | 9 |
| OPTIMIZATIONS153 | "The build vocabulary exposes exact named selections through `builder.optimizations.enable(...)`." | Exact selection | Exact named optimization selections must be enabled only through `builder.optimizations.enable(...)` | not stated | 9 |
| OPTIMIZATIONS154 | "The current requirement is structural readiness for a possible future policy, not an ML implementation." | Decisions and nonauthoritative policy | Selection infrastructure must be structurally ready for future policy without requiring ML implementation now | not stated | 9 |

## Ambiguous sentences

| quoted sentence | reading A | reading B |
| --- | --- | --- |
| "Missing, malformed, stale, or mismatched supplied decisions reject or use an explicitly selected deterministic fallback." | Any bad supplied decision file causes hard rejection unless a fallback mode was explicitly selected in build configuration. | Only some defect classes reject; others always route to fallback when fallback is selected; which defects trigger which outcome is not enumerated. |
| "losing one may cause rejection, but fail-closed narrowing alone is not evidence of an exact-preserving Terminal transformation." | Qualification loss always rejects compilation when it would change semantics. | Qualification loss may reject but need not; fail-closed narrowing never suffices as proof of exact preservation even when compilation continues. |
| "A known unselected name is an accepted visible no-op, subject to the requested product's phase restriction." | Disable of a valid catalog name that was not in the authored selection succeeds silently as a no-op. | No-op acceptance applies only when the name's phase matches the product's allowed disable phases; otherwise it rejects. |
| "Check-only production rejects nonempty requests;" | Any nonempty `--disable-optimization` list on a check-only product is an error regardless of name validity. | Nonempty requests reject only when they would affect stages the check-only product would have run; empty effective set might still reject if request is nonempty. |
| "Terminal production admits checked-tree and Psi-phase rollback, not post-Terminal names;" | Post-Terminal rule names in a disable list on Terminal products always reject even if unselected. | Post-Terminal names are ignored or no-op like unselected names rather than rejected; "not post-Terminal names" means they are not applied, not that they error. |
| "Independent rule validation checks applicability again." | Every rule undergoes two applicability checks: at dispatch and again at independent validation. | Independent validation re-checks applicability only for rules that reached validation, not for rules skipped earlier. |
| "Publication still needs the full validation chain binding child identities and rejecting detached, reordered, truncated, trailing, stale, or cross-source evidence." | Publication is rejected unless the entire validation chain is present and evidence is well-formed. | Publication may proceed for some products with partial chains when evidence is optional; "still needs" refers to authority not a hard gate in all product modes. |
| "A product cannot claim rollback for stages it does not execute." | Manifests and reports must omit rollback fields for non-executed phases. | Products may record requested/applied sets but must not imply those phases ran rollback when they did not execute. |

## Terms used without definition in this chapter

- realization / semantic contract
- producer-established facts
- trusted input
- Psi admission
- certificate / canonical encoding
- interpreter containment
- guest claims
- `builder.optimizations.enable(...)` / build vocabulary
- exact named selection
- human report emission
- phase owner
- checked trees
- abstract operations
- target operations / instruction selection
- pre-allocation / allocation recovery / post-allocation
- resolved layout
- projection / downstream coordinators
- build-selection identity
- Psi-local pass identities
- ordered catalog / exact entry
- rule and validator identities
- policy (optimization policy)
- required analyses / invalidations / work budgets
- applicability
- custody coordinators
- encoding versions
- compositions
- catalog presence
- normalization / legalization / allocation / realization
- profitability rewrites
- representation / semantic vocabulary / invariant set / product boundary
- canonical order / explicit bounds
- typed stage result / execution evidence
- pass manifests / transformation records
- identity execution
- rollback provenance
- validation subjects / dependency closure
- checked-tree pruning
- root selection / published product
- shared frontend work / target fan-out
- Terminal Psi
- input/output semantic identities
- proof identities
- target-constrained build companion
- raw portable product
- home-layout transformation
- compact case encodings
- layout commitments / recasts / aliasing observations
- admitted cast / fixed layout observation
- observation (semantic observation)
- purity / closed classification
- barriers
- operation identity / domain/provider / arithmetic policy
- discharged proof/check obligation
- fast-math switch
- lossy family
- logical work / Terminal work schedule
- fixed-work certificates / immutable Terminal subject
- work settlement / loop charges
- accounting contract
- spill-inclusive frame demand
- physical demand evidence
- stack provisioning / backing
- physical access validation
- private spills / `crashes` ceiling
- machine-state realization
- qualification rosters / canonical paths / carriers / establishment lineage
- CSE / GVN
- fail-closed narrowing
- exceptional / cleanup / transition edges
- suspension (interprocedural state)
- cyclic components / loop-carried arguments / ownership fixed points
- progress evidence (ranked, bounded, unranked)
- analysis receipt
- validated input revision
- candidate / immutable patch or plan
- identity-bound receipt
- matcher / producer / manager (optimization pipeline roles)
- differential tests / oracle
- roots (provenance roots)
- validation chain
- machine-readable decision rows
- baseline-policy row
- selection policies
- versioned schema
- deterministic fallback
- learned policy / model (future)
- `--disable-optimization <ExactName>`
- authored / requested / applied / effective sets
- check-only production / Terminal production / native production
- promotion / recommended / default
- workload/profile identities

CHAPTER COMPLETE
