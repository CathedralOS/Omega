# Semantic evaluation

Source: `wiki/spec/language/evaluation.md` lines 1–358 (read).

## Rules

rule id | the exact sentence, quoted | the heading it is under | what must be checked, in plain words | when it must hold | lane
--- | --- | --- | --- | --- | ---
EVALUATION1 | "the concrete invocation's complete normalized contract determines admission." | Semantic evaluation | Semantic evaluation admission must be decided from the invocation's complete normalized contract, not from syntax or position alone | at a call | 3
EVALUATION2 | "There is no `comptime`, `#run`, `const machine`, macro language, or `is_build_time()` observation." | Semantic evaluation | Programs must not use these constructs as semantic-evaluation or phase-observation mechanisms | not stated | SHARED
EVALUATION3 | "Neither empty reach nor a representable result carrier supplies a missing algorithm." | Semantic evaluation | Missing executable algorithm must not be admitted because reach is empty or a result carrier exists | at a call | 3
EVALUATION4 | "For an eligible runtime-capable machine, evaluation produces exactly the value that target execution would produce with the same arguments, selected conformances/providers, and target semantics." | Semantic evaluation | Evaluated value must match target execution under same arguments, conformances/providers, and target semantics | at the operation | SHARED
EVALUATION5 | "Proof-only machines need no runtime representation but obey the same core value semantics and cannot observe the compiler phase." | Semantic evaluation | Proof-only evaluation must use core value semantics without runtime representation and must not observe compiler phase | at the operation | 8
EVALUATION6 | "An external build observation reaches semantic evaluation only through staged value/artifact custody, its recorded input and receipt, and compiler materialization of an ordinary value." | Semantic evaluation | External build observations must enter evaluation only via staged custody, recorded input/receipt, and materialized ordinary values | not stated | 9
EVALUATION7 | "There is no ambient-host shortcut into a proof, type, layout, or constant." | Semantic evaluation | Host ambient state must not short-circuit into proof, type, layout, or constant evaluation | not stated | 9
EVALUATION8 | "They are not semantic-evaluation requests: a declared trap can fail a test and admitted service calls can exercise mocks." | Semantic evaluation | Build-time tests must be classified under the separate runner contract, not semantic-evaluation admission | not stated | 9
EVALUATION9 | "This does not relax constant/proof evaluation admission or expose host state to it." | Semantic evaluation | Build-time test runner allowances must not weaken evaluation admission or expose host state to evaluation | not stated | SHARED
EVALUATION10 | "Neither executing such a relationship nor using its admitted facts to justify computation is eligible here, including through helpers, selected providers or erased evidence." | Invocation admission | Assumed binding execution or use of its facts for computation must reject in semantic evaluation | at a call | 9
EVALUATION11 | "Empty reach and root runtime acceptance cannot override that exclusion." | Invocation admission | Empty reach and runtime root acceptance must not override assumed-binding exclusion from evaluation | at a call | 3
EVALUATION12 | "Inspecting runtime declarations and checking their conditional proofs is not execution of the relationship." | Invocation admission | Declaration inspection and conditional proof checking must not be treated as executing assumed bindings for admission | not stated | 8
EVALUATION13 | "Specialize the selected machine's complete contract at its concrete arguments and available facts." | Invocation admission | Admission must specialize the machine's complete contract at concrete arguments and available facts | at a call | 3
EVALUATION14 | "Ordinary checked termination." | Invocation admission | Invoked machine must have ordinary checked termination before evaluation admission | at a call | 4
EVALUATION15 | "Empty runtime service reach." | Invocation admission | Runtime service reach must be empty for evaluation admission | at a call | 3
EVALUATION16 | "No possible suspension or blocking." | Invocation admission | Invocation must have no possible suspension or blocking | at a call | 3
EVALUATION17 | "No unhandled failure, trap, or abort route for the demanded value." | Invocation admission | Every failure, trap, or abort route for the demanded value must be handled or discharged | at a call | 3
EVALUATION18 | "No runtime authority acquisition or consumption." | Invocation admission | Evaluation must not acquire or consume runtime authority | at the operation | 2
EVALUATION19 | "No escaping runtime mutation." | Invocation admission | Runtime mutation must not escape the hermetic evaluation boundary | at the operation | 2
EVALUATION20 | "Only proof/build-admissible trust and resource inputs." | Invocation admission | Trust and resource inputs must be proof/build-admissible only | at entry | 8
EVALUATION21 | "A checked computation or executable realization for every demanded data/control dependency; an axiomatic mathematical result is not an executable provider." | Invocation admission | Each demanded dependency must have checked computation or executable realization; axiomatic math alone must not satisfy data/control demand | at a call | 8
EVALUATION22 | "Empty reach alone proves none of the other axes." | Invocation admission | Compiler must not admit evaluation from empty reach without the other admission axes | at a call | 3
EVALUATION23 | "their consumers use the application's specialized published reach, not the union of all possible selections." | Invocation admission | Static callback reach for evaluation must use specialized published reach, not union of all selections | at a call | 3
EVALUATION24 | "The generic body must already be valid under its callback requirements; evaluation cannot repair a carry or invariant-window failure." | Invocation admission | Generic body must be valid under callback requirements before evaluation; carry or invariant-window failures must not be repaired by evaluation | at a call | 3
EVALUATION25 | "These rules are shared with ordinary runtime callers and do not grant evaluation-only narrowing of opaque contracts." | Invocation admission | Evaluation must not narrow opaque contracts beyond rules shared with runtime callers | not stated | 3
EVALUATION26 | "Public trait requirements establish the floor; an incompatible conformance rejects at its declaration." | Invocation admission | Incompatible trait conformance must reject at declaration; public requirements are the admission floor | not stated | 7
EVALUATION27 | "also applies before evaluation, not only after the resulting value is used." | Invocation admission | Package declaration-selection authority must be checked before evaluation, not only at value use | before evaluation | 9
EVALUATION28 | "Permission to select a declaration does not establish its execution meaning." | Invocation admission | Declaration selection permission must not be treated as establishing execution meaning for evaluation | not stated | 9
EVALUATION29 | "An unresolved authored operator cannot be evaluated using the builtin meaning of its token, including when reached through a helper machine." | Invocation admission | Unresolved authored operators must reject evaluation via builtin token meaning, including through helpers | at a call | 3
EVALUATION30 | "[Interpreted assembly](assembly.md#interpreted-execution) is subject to these same rules." | Invocation admission | Interpreted assembly evaluation must satisfy the same invocation admission rules | at a call | 10
EVALUATION31 | "having an interpreter handler does not make device state, physical memory, interrupts or runtime-only trust into hermetic inputs." | Invocation admission | Interpreter handler presence must not admit device state, physical memory, interrupts, or runtime-only trust as hermetic inputs | at entry | 10
EVALUATION32 | "A direct token-bearing machine evaluates its declaration-owned checked body through the ordinary machine evaluator." | Invocation admission | Direct token-bearing machines must evaluate via declaration-owned checked body and ordinary machine evaluator | at a call | 10
EVALUATION33 | "Compiler-catalog semantic primitives retain their exact catalog execution, canonical float operations evaluate through the selected target's declared realization, and boundary operators retain the admitted selected provider contract." | Invocation admission | Catalog primitives, float ops, and boundary operators must use their respective admitted execution routes | at the operation | 5
EVALUATION34 | "Token spelling alone chooses none of these routes." | Invocation admission | Route selection must not be determined by token spelling alone | at a call | 3
EVALUATION35 | "`divide(10, 0)` rejects before execution and identifies the undischarged route and call chain." | Invocation admission | Trap-capable calls with undischarged routes must reject before execution with route and call chain identification | at a call | 3
EVALUATION36 | "Reaching a forbidden terminal route despite admission is a checker/accepted-assumption consistency failure with a trace, not a normal machine result." | Invocation admission | If a forbidden terminal route is reached after admission, report consistency failure with trace, not a normal result | at the operation | 3
EVALUATION37 | "Only a recursive value snapshot crosses back." | Invocation admission | Hermetic bridge return must be only a recursive value snapshot | at normal return | 2
EVALUATION38 | "Interpreter cells, references, and aliases into compiler, other-invocation, or runtime state cannot escape." | Invocation admission | Interpreter cells, refs, and aliases into compiler, other invocations, or runtime must not escape evaluation | at normal return | 2
EVALUATION39 | "that does not widen the hermetic bridge." | Invocation admission | Augmenting build API returning mutated argument snapshots must not widen hermetic bridge rules | at normal return | 9
EVALUATION40 | "cannot smuggle an interpreter identity or host pointer through that value snapshot." | Invocation admission | Value snapshots must not carry interpreter identity or host pointers per address-exposure rules | at normal return | 2
EVALUATION41 | "Runtime address observations are not canonical target-semantic observations." | Invocation admission | Runtime address observations must not be treated as canonical target-semantic observations in evaluation | at the operation | 2
EVALUATION42 | "Recording an executable entry relocation for later realization is distinct from evaluating its eventual numeric address as a constant, type argument, or proof input." | Invocation admission | Entry relocation recording must not be conflated with evaluating numeric address as constant, type arg, or proof input | at a call | 9
EVALUATION43 | "Data `size_of<T>()` and `align_of<T>()` consume the selected validated layout under these ordinary rules." | Invocation admission | `size_of` and `align_of` must use selected validated layout under ordinary evaluation rules | at a call | 1
EVALUATION44 | "`code_layout<Machine>()` is late native artifact metadata, not a value computed here from an unfinished product." | Invocation admission | `code_layout` must not be computed as evaluation value from unfinished product | at a call | 9
EVALUATION45 | "Its code-region sizes cannot feed that same product's types, constants or layout evaluation." | Invocation admission | Code-region sizes from `code_layout` must not feed same product's type, constant, or layout evaluation | not stated | 9
EVALUATION46 | "a width mismatch rejects rather than returning a slice into evaluator storage." | Invocation admission | Fixed-array byte literal width mismatch must reject, not return slice into evaluator storage | at the operation | 6
EVALUATION47 | "Dynamic owned sequences use the ordinary collection model when const-evaluable, not a special compiler byte-blob type." | Invocation admission | Const-evaluable dynamic sequences must use ordinary collection model | at the operation | 6
EVALUATION48 | "Neither is user-satisfiable." | Invocation admission | `ConstEvaluable` and `ConstMaterializable` judgments must not be user-satisfiable obligations | not stated | 8
EVALUATION49 | "Proof-only references retain their full assumption closure without introducing runtime demand." | Invocation admission | Proof-only references must keep full assumption closure and introduce no runtime demand | not stated | 8
EVALUATION50 | "an axiomatic Boolean cannot control materialization merely because its alternatives are constants." | Invocation admission | Axiomatic Boolean must not control materialization solely because branches are constant | at the operation | 8
EVALUATION51 | "cannot silently remove the retained trust record." | Invocation admission | Checked transformations must not silently drop retained trust records | at the operation | 8
EVALUATION52 | "Diagnose the unrealized mathematical use where evaluation is requested, rather than suggesting ordinary provider selection." | Invocation admission | Unrealized mathematical use must be diagnosed at evaluation request site | at a call | 8
EVALUATION53 | "Executable checking and lowering enforce the same distinction." | Invocation admission | Lowering must enforce the same proof-only vs executable distinction as evaluation | not stated | SHARED
EVALUATION54 | "There is only the ordinary `terminates` guarantee." | Termination and sponsorship | Evaluation termination must use only the ordinary `terminates` guarantee | not stated | 4
EVALUATION55 | "A terminating invocation can still be too expensive for its sponsor." | Termination and sponsorship | Termination proof does not alone grant resource admission; expense may still reject | at a call | 4
EVALUATION56 | "Resource admission uses either a certified maximum fitting the compiler grant or deterministic metering against an explicit sponsor budget." | Termination and sponsorship | Resource admission must use certified maximum within grant or deterministic metering against explicit sponsor budget | at entry | 4
EVALUATION57 | "Reproducible published builds retain the certified ceiling or the admitted budget and usage receipt." | Termination and sponsorship | Reproducible published builds must retain certified ceiling or admitted budget and usage receipt | not stated | 9
EVALUATION58 | "Additional payload ceilings require a named compiler-owned account whose complete allocation lifetime can be charged before entry and released exactly; they do not promise generic temporary-memory or RSS bounds." | Termination and sponsorship | Extra payload ceilings must use named compiler-owned accounts with exact charge/release; must not imply generic temp-memory or RSS bounds | at entry | 4
EVALUATION59 | "Only the root selects or raises/removes ceilings." | Termination and sponsorship | Only evaluation root may select, raise, or remove resource ceilings | not stated | 4
EVALUATION60 | "Dependencies may report expected usage, not grant themselves more." | Termination and sponsorship | Dependencies must not self-grant higher ceilings; may only report expected usage | not stated | 4
EVALUATION61 | "Evaluated code cannot inspect its remaining budget, request increases, catch exhaustion, or branch on policy." | Termination and sponsorship | Evaluated code must not observe budget, request increases, catch exhaustion, or branch on sponsorship policy | at the operation | 4
EVALUATION62 | "Raising a grant changes whether a result is obtained, never which result." | Termination and sponsorship | Grant changes affect admission outcome only, not semantic result identity | not stated | 4
EVALUATION63 | "Exhaustion is a build-resource error, not divergence or a failed termination proof." | Termination and sponsorship | Resource exhaustion must be reported as build-resource error, not as divergence or termination-proof failure | at the operation | 4
EVALUATION64 | "Scheduling and host speed cannot change canonical charges or aggregate admission." | Termination and sponsorship | Host scheduling and speed must not alter canonical charges or aggregate admission | not stated | 4
EVALUATION65 | "displayed wall time has no accounting or admission force." | Termination and sponsorship | Wall-clock display must not affect accounting or admission decisions | not stated | 4
EVALUATION66 | "The host evaluator consumes a sealed, compiler-owned, versioned target capsule also consumed by target realization." | Target-semantic capsule | Evaluator must use the same sealed compiler-owned versioned target capsule as target realization | at entry | 9
EVALUATION67 | "The host's filesystem, environment, clock, randomness, network, pointer width, floating behavior, and process state are not inputs." | Target-semantic capsule | Listed host facilities must not be evaluator inputs | at entry | 9
EVALUATION68 | "`build.omg` selects the capsule, not individual proof facts." | Target-semantic capsule | Capsule selection comes from `build.omg`, not from ad hoc proof facts | not stated | 9
EVALUATION69 | "Packages derive values, propositions, and plans; they cannot invent primitive observations or redefine carrier meaning." | Target-semantic capsule | Packages must not invent primitive observations or redefine carrier meaning | not stated | 9
EVALUATION70 | "There is no source-visible general `BuildWorld` or runtime reflection object." | Target-semantic capsule | Source must not expose general `BuildWorld` or runtime reflection object for evaluation | not stated | 9
EVALUATION71 | "A subject not fixed by target closure rejects." | Target-semantic capsule | Target observations without subject fixed by target closure must reject | at a call | 9
EVALUATION72 | "Portable equality is equality of `FloatMeaning`; exact computed NaN payloads need a proof or selected realization fixing them." | Target-semantic capsule | Float portable equality uses `FloatMeaning`; exact NaN payloads need proof or selected realization | at the operation | 5
EVALUATION73 | "Every selected realization and semantic control state affecting the result participates in identity." | Target-semantic capsule | Result identity must include all selected realizations and semantic control states affecting the result | not stated | 5
EVALUATION74 | "It cannot add/remove fields or cases, change multiplicity, or splice declarations." | Target-dependent applications | Target-dependent observations must not alter record/case shape, multiplicity, or splice declarations | not stated | 1
EVALUATION75 | "Before closure, export a symbolic recipe, not a guessed concrete value." | Target-dependent applications | Target-polymorphic items before closure must export symbolic recipe, not guessed concrete value | not stated | 9
EVALUATION76 | "Different target applications cannot share values or artifact identities merely because their declaration paths match." | Target-dependent applications | Distinct target applications must not share values or artifact identities from declaration-path equality alone | not stated | 9
EVALUATION77 | "Build orchestration cannot splice an existing schema." | Target-dependent applications | Build orchestration must not splice existing ABI schemas for differing native field/case sets | not stated | 9
EVALUATION78 | "Derived values, proofs, plans, public signatures, caches, and artifacts retain both." | Target-dependent applications | Listed artifacts must retain both observation-application and selected-realization dependencies | not stated | SHARED
EVALUATION79 | "Folding preserves dependencies and their target-closure receipt; verification reconstructs the projection or selected realization instead of trusting a folded scalar." | Target-dependent applications | Constant folding must preserve dependencies; verification must reconstruct projections/realizations | not stated | 8
EVALUATION80 | "Fine-grained replay may remove that key only after checking both dependency kinds." | Target-dependent applications | Fine-grained cache replay may drop whole-capsule key only after both dependency kinds are checked | not stated | 9
EVALUATION81 | "Independently closed artifacts compose only when applications agree." | Target-dependent applications | Artifact composition requires agreement on target-dependent applications | not stated | 9
EVALUATION82 | "Adding, removing, or changing a published-signature dependency is a breaking semantic-API revision; private changes alter target artifact identity and require rebuilding/relinking without changing the public contract." | Target-dependent applications | Published dependency changes are breaking API revisions; private dependency changes alter artifact identity and require rebuild/relink | not stated | 9
EVALUATION83 | "A mismatch identifies both producer/consumer closures and traces the mismatching type argument to the introducing observation or realization." | Target-dependent applications | Dependency mismatch diagnostics must name producer/consumer closures and trace mismatching type argument to source observation or realization | not stated | 9
EVALUATION84 | "A value used as a canonical generic/domain index needs decidable structural equality and one unique canonical encoding." | Canonical static identities | Canonical index values must have decidable structural equality and unique canonical encoding | not stated | 7
EVALUATION85 | "Its value, not its evaluation trace, enters identity." | Canonical static identities | Generic/domain index identity must use value, not evaluation trace | not stated | 7
EVALUATION86 | "arbitrary machine evaluation does not participate in type equality." | Canonical static identities | Arbitrary machine evaluation must not participate in type equality for open symbolic indices | not stated | 7
EVALUATION87 | "Neither normalization nor a safe final result removes the authored expression's formation obligations." | Canonical static identities | Normalization or safe final result must not erase authored expression formation obligations | not stated | 8
EVALUATION88 | "exhaustion does not produce an alternate symbolic canonical value." | Canonical static identities | Resource exhaustion must not yield alternate symbolic canonical index value | at the operation | 7
EVALUATION89 | "Proof-policy or search-algorithm changes cannot rename an existing index or select a different conformance." | Canonical static identities | Proof-policy or search-algorithm changes must not rename existing index or change conformance selection | not stated | 8
EVALUATION90 | "Float/text values, references, slices, dynamic identities, and boundary-opaque values are not canonical generic atoms." | Canonical static identities | Listed value kinds must reject use as canonical generic/domain atoms | not stated | 7
EVALUATION91 | "Quotients and constrained records require a proved canonical representative and required facts at use." | Canonical static identities | Quotient and constrained record indices must have proved canonical representative and required facts at use | at a call | 8
EVALUATION92 | "an indexed `Rat` proves positive denominator, cancelled signed coordinates, and gcd-reduced numerator magnitude and denominator at the use site." | Canonical static identities | Indexed `Rat` must prove positive denominator, cancelled coordinates, and gcd-reduced numerator/denominator at use | at a call | 8
EVALUATION93 | "Changing accounting weights does not invalidate a semantically unchanged result." | Result caching and published evidence | Cache validity for semantic results must not depend on accounting weight changes alone | not stated | 9
EVALUATION94 | "A cache hit replacing work in the current build graph receives its recorded logical charge, so warm and cold builds make the same hard-ceiling decision." | Result caching and published evidence | Cache hits must apply recorded logical charge so warm and cold builds agree on hard ceiling | not stated | 4
EVALUATION95 | "Linking an already-built dependency does not charge its historical production cost." | Result caching and published evidence | Linking prebuilt dependency must not charge historical production cost to current evaluation budget | not stated | 4
EVALUATION96 | "a smaller discovery budget cannot invalidate reuse." | Result caching and published evidence | Smaller consumer proof-discovery budget must not invalidate reuse of published proof judgment | not stated | 8
EVALUATION97 | "a cache does not turn erased evidence into runtime values." | Result caching and published evidence | Cached proof artifacts must not materialize erased evidence as runtime values | not stated | 8
EVALUATION98 | "A trait machine body is its overridable default, without a `default` keyword." | Trait bodies and generators | Trait machine bodies must be modeled as overridable defaults without `default` keyword syntax | not stated | 7
EVALUATION99 | "A generic default does not inherit a conformance author's or caller's private access." | Trait bodies and generators | Generic trait defaults must not inherit conformance author or caller private access | not stated | 7
EVALUATION100 | "they grant no runtime type-to-generic conversion, source splicing, boundary authority, or exemption from evaluator admission." | Trait bodies and generators | TypeSchema reflection must not grant runtime type-to-generic conversion, splicing, boundary authority, or admission exemption | at a call | 7
EVALUATION101 | "Handwritten equality owns its coverage and is not automatically substitutable equality; that requires explicit quotient laws." | Trait bodies and generators | Handwritten equality must not be treated as generated substitutable equality without explicit quotient laws | not stated | 8
EVALUATION102 | "account for each component, rename with `as`, or explicitly waive bindings or remaining components." | Trait bodies and generators | Record destructuring in generators must account for each component, rename with `as`, or explicitly waive | at the operation | 1
EVALUATION103 | "An initializer is evaluated once; copy-eligible field reads retain that subject's snapshots." | Trait bodies and generators | Initializer expressions evaluate once; copy-eligible field reads keep subject snapshots | at the operation | 1
EVALUATION104 | "This does not make non-copy fields implicitly copyable or movable from borrowed storage." | Trait bodies and generators | Generator destructuring must not implicitly copy or move non-copy fields from borrowed storage | at the operation | 2
EVALUATION105 | "a borrowed case view retains its source's loan rather than copying the carrier." | Trait bodies and generators | Borrowed case views must retain source loan, not copy carrier | at the operation | 2
EVALUATION106 | "No reference patterns or new binding modes are introduced." | Trait bodies and generators | Generator destructuring must not introduce reference patterns or new binding modes | not stated | 2
EVALUATION107 | "Larger borrowed fields require an otherwise authorized explicit borrow expression or waiver." | Trait bodies and generators | Larger borrowed fields require authorized explicit borrow or waiver | at the operation | 2
EVALUATION108 | "Body-shape coverage is not a requirable value proposition." | Trait bodies and generators | Body-shape coverage must not be required as a value proposition | not stated | 8
EVALUATION109 | "Reflection visitation establishes its declared member/case coverage through checked elaboration, not an arbitrary body-coverage proposition or a separate generator-unroll keyword." | Trait bodies and generators | Reflection visitation coverage must come from checked elaboration, not body-coverage propositions or unroll keywords | not stated | 7
EVALUATION110 | "the mutable context loan does not escape or mutate compiler state out of band." | Trait bodies and generators | Reflection selection context loans must not escape or mutate compiler state out of band | at normal return | 2
EVALUATION111 | "requires independent resolution, coverage, access, and requirement validation." | Trait bodies and generators | Selection result contract requires separate resolution, coverage, access, and requirement checks | at a call | 7
EVALUATION112 | "Choosing a static declaration is not manufacturing proof or a runtime callable." | Trait bodies and generators | Static declaration choice must not synthesize proof obligations or runtime callables by itself | at a call | 8
EVALUATION113 | "Suspension and blocking admission likewise uses [published operational dependencies](effects.md#static-callback-operational-dependencies) after substitution." | Invocation admission | Suspension and blocking admission must use published operational dependencies after substitution | at a call | 3
EVALUATION114 | "A trap-capable division can evaluate at `divide(10, 2)` after proving the denominator nonzero." | Invocation admission | Trap-capable operations may evaluate only after discharging the controlling proof obligation (e.g. nonzero denominator) | at a call | 5
EVALUATION115 | "Structural equality and hashing retain their ordinary type meaning; constant pool interning is an unobservable emission optimization." | Invocation admission | Structural equality and hashing keep ordinary type meaning; interning must be unobservable to program semantics | at the operation | 1
EVALUATION116 | "Relevant branches and calls still require their inputs:" | Invocation admission | Branches and calls under proof-only references must still have required inputs available | at a call | 8
EVALUATION117 | "A closed acyclic checked body supplies it locally; a cyclic body proves `terminates by ...`; an open or separately compiled contract declares `terminates` when consumers may rely on it." | Termination and sponsorship | Termination must be supplied locally, proved for cycles, or declared on open contracts per the stated forms | not stated | 4
EVALUATION118 | "Progress may name the invocation, sponsor, usage, and largest active call path;" | Termination and sponsorship | Progress reporting may include invocation, sponsor, usage, and largest active call path | not stated | 4
EVALUATION119 | "Its closed typed projection vocabulary belongs to the language/toolchain schema, not replaceable providers or extensible package reflection." | Target-semantic capsule | Projection vocabulary must be fixed schema, not provider-replaceable or package-extensible reflection | not stated | 9
EVALUATION120 | "Observations identify their subject explicitly: address bound for one address space, endianness for one data layout, floating semantics for one format/mode, or an entry guarantee for one target profile." | Target-semantic capsule | Each target observation must name its subject (address space, layout, float mode, or profile) explicitly | at a call | 9
EVALUATION121 | "Floating operations use runtime `FloatSemantics`, including format rounding, classification, conversion, square root, and fused versus unfused arithmetic." | Target-semantic capsule | Floating evaluation must use runtime `FloatSemantics` for listed operations | at the operation | 5
EVALUATION122 | "An observation can occupy any position available to an equivalent ordinary constant, including array length, const-generic argument, proof expression, further evaluation, and layout/calling-plan input." | Target-dependent applications | Target observations may appear only in positions ordinary constants may occupy | not stated | 9
EVALUATION123 | "Its declaration retains one source identity; closed applications retain exact type/const/machine substitutions and target dependencies." | Target-dependent applications | Target-polymorphic declarations keep one source identity; closed apps retain exact substitutions and target deps | not stated | 9
EVALUATION124 | "Its target dependency comes from the exact Build selection, not a target modifier on its declaration." | Target-dependent applications | Target dependency for literal-only providers must come from exact Build selection, not declaration target modifier | not stated | 9
EVALUATION125 | "Exact used dependencies are normative compatibility identity." | Target-dependent applications | Compatibility identity must use exact retained dependencies as normative | not stated | 9
EVALUATION126 | "Dependency unions retain a compact origin DAG through aliases, constants, generic applications, projections, and selected plans." | Target-dependent applications | Dependency unions must retain compact origin DAG through listed dependency forms | not stated | 9
EVALUATION127 | "Generic, monomorphized, structural, indexed-qualification, and constrained-call uses retain the same closed atom or const-binder identity." | Canonical static identities | Listed generic uses must share the same closed atom or const-binder identity | not stated | 7
EVALUATION128 | "Preserve the exact selected operation, carrier and policy." | Canonical static identities | Integer index identity must preserve exact selected operation, carrier, and policy | not stated | 5
EVALUATION129 | "A fully closed index with a concrete implementation obtains its canonical value through admitted evaluation." | Canonical static identities | Fully closed index with concrete implementation must obtain canonical value only through admitted evaluation | at a call | 7
EVALUATION130 | "Eligible atoms are integers, Booleans, fixed arrays, records, cases, and [static callable selections](machine_values.md#static-callable-selections), with declaration order determining aggregate identity." | Canonical static identities | Only listed kinds are eligible canonical atoms; aggregate identity uses declaration order | not stated | 7
EVALUATION131 | "Missing telemetry may require remeasurement without invalidating that result." | Result caching and published evidence | Missing telemetry may trigger remeasurement but must not invalidate semantically unchanged cached result | not stated | 9
EVALUATION132 | "Published proof-only contracts retain their propositions, exact subjects, selected conformances, premises and semantic dependencies." | Result caching and published evidence | Cached or reused proof contracts must retain propositions, subjects, conformances, premises, and semantic dependencies | not stated | 8
EVALUATION133 | "Loading and preparation retain their separate resource admission." | Result caching and published evidence | Artifact loading and preparation must use separate resource admission from proof reuse | at entry | 4
EVALUATION134 | "Generated equality follows the current field set." | Trait bodies and generators | Generated equality must cover exactly the current record field set | not stated | 1
EVALUATION135 | "Record destructuring follows the shared [data-pattern and irrefutable-binding rules](patterns.md#irrefutable-data-bindings):" | Trait bodies and generators | Generator record destructuring must follow irrefutable data-binding rules | at the operation | 1
EVALUATION136 | "Owned decomposition uses ordinary move/copy disposition and cleanup;" | Trait bodies and generators | Owned decompositions must use ordinary move/copy disposition and cleanup | at the operation | 2
EVALUATION137 | "That root freezes completed selections into an ordinary symbolic result snapshot;" | Trait bodies and generators | Evaluation root must freeze completed reflection selections into symbolic result snapshot | at normal return | 7
EVALUATION138 | "Fixed arrays and recursively owned records may cross as values." | Invocation admission | Fixed arrays and recursively owned records may be returned across hermetic bridge as values | at normal return | 1
EVALUATION139 | "Handled result sums remain ordinary values." | Invocation admission | Handled error/result sums from evaluation remain ordinary values in the semantic model | at the operation | 1
EVALUATION140 | "This section governs systems generic/domain indices and their published specialization identity." | Canonical static identities | Rules in this section apply to systems generic/domain indices and published specialization identity | not stated | 7
EVALUATION141 | "It does not replace the mathematical core's dependent typing or definitional conversion, and does not limit proof-only quantification to the finite list of canonical runtime atoms below." | Canonical static identities | Mathematical dependent typing and proof-only quantification are not narrowed by the finite atom list alone | not stated | 8
EVALUATION142 | "The shared identity profile fixes canonical typed ordering and encoding without distributive expansion or a search cutoff." | Canonical static identities | Canonical ordering/encoding must not use distributive expansion or search cutoff in shared identity profile | not stated | 7
EVALUATION143 | "[Compatibility proofs](../proofs/contracts.md#proved-index-compatibility) may use bounded algebraic expansion under recorded Build proof-work policy." | Canonical static identities | Compatibility proofs may use bounded algebraic expansion only under recorded Build proof-work policy | not stated | 8
EVALUATION144 | "Specialization carries an already checked generic derivation under exact substitutions instead of repeating discovery." | Canonical static identities | Specialization must carry checked generic derivation under exact substitutions without rediscovering | not stated | 7
EVALUATION145 | "Semantic result identity is separate from accounting identity:" | Result caching and published evidence | Cache keys must separate semantic result identity from accounting identity | not stated | 9
EVALUATION146 | "A host-authorized consumer of a trusted producer's exact artifact reuses the established judgment under [Psi admission](../terminal-psi/product.md#publication-and-separate-consumption)." | Result caching and published evidence | Authorized consumers may reuse producer judgment only under Psi admission rules | not stated | 8
EVALUATION147 | "[Semantic reflection](reflection.md) may inspect an explicitly selected type under the lexical author's ordinary dependency and visibility authority;" | Trait bodies and generators | Reflection must respect lexical author dependency and visibility on explicitly selected type | at a call | 7
EVALUATION148 | "An owner can instead supply a checked visitor/operation through an explicit requirement or conformance." | Trait bodies and generators | Private access gaps must be filled via explicit requirement or conformance, not implicit inheritance | not stated | 7
EVALUATION149 | "A separate trait-level coverage gate is deferred." | Trait bodies and generators | Compiler must not enforce a separate trait-level coverage gate (deferred) | not stated | 7
EVALUATION150 | "Reflection selection contexts are owned within an evaluation root and lent to policy machines through ordinary explicit parameters." | Trait bodies and generators | Selection contexts are owned at evaluation root and loaned only via explicit parameters | at a call | 2
EVALUATION151 | "The hermetic bridge creates a fresh owned interpreter value graph from every materialized argument and a fresh machine instance." | Invocation admission | Each evaluation creates fresh owned value graph and fresh machine instance | at entry | 2
EVALUATION152 | "Canonical type/index identity imposes the additional rules below." | Invocation admission | When type/index identity is canonical, additional rules in following section apply | not stated | 7
EVALUATION153 | "[Assumed binding contracts](../build/assumed_binding_contracts.md#runtime-only) are runtime-only." | Invocation admission | Assumed binding contracts must be classified runtime-only and excluded from semantic evaluation | not stated | 9
EVALUATION154 | "Admission is invocation-sensitive." | Invocation admission | Evaluation admission must be decided per concrete invocation, not per declaration alone | at a call | 3
EVALUATION155 | "A byte literal in an exact fixed-array result copies its bytes into the array;" | Invocation admission | Fixed-array results from byte literals must copy bytes into the array value | at the operation | 6
EVALUATION156 | "The second remains necessary for a selected ordinary provider containing only literals and no projection call." | Target-dependent applications | Selected-realization dependency must be retained even for literal-only providers without projection calls | not stated | 9
EVALUATION157 | "A whole-capsule and complete-realization-closure key is sound but conservatively rejects reuse." | Target-dependent applications | Implementations using whole-capsule keys may conservatively reject valid reuse | not stated | 9
EVALUATION158 | "An unscoped constant or public type application may be target-polymorphic." | Target-dependent applications | Unscoped constants and public type applications may carry target polymorphism before closure | not stated | 9
EVALUATION159 | "These requirements are stronger than ordinary evaluation or opaque runtime materialization." | Canonical static identities | Indexed `Rat` and related index rules impose obligations beyond ordinary evaluation or opaque materialization | at a call | 8
EVALUATION160 | "The compiler derives `ConstEvaluable(T, value)` for a complete pure/copy-eligible snapshot and `ConstMaterializable(value, layout)` for determined observable runtime bits." | Invocation admission | Compiler must derive `ConstEvaluable` and `ConstMaterializable` judgments per stated value/layout conditions | not stated | 8
EVALUATION161 | "Ordinary theorem contracts and named witness/law bundles follow [proof contracts](../proofs/contracts.md);" | Result caching and published evidence | Theorem and witness/law bundles in caching must follow proof-contract rules | not stated | 8
EVALUATION162 | "it is not restricted to an implicitly bound implementing type; no such type exists." | Trait bodies and generators | Semantic reflection at generic defaults must not assume an implicit implementing type | at a call | 7
EVALUATION163 | "Owned TypeSchema descriptions and typed per-member calls follow the reflection contract;" | Trait bodies and generators | TypeSchema descriptions and typed per-member calls must follow the reflection contract | at a call | 7
EVALUATION164 | "A checked transformation may remove that dependency, but cannot silently remove the retained trust record." | Invocation admission | Transformations may remove proof-only branch dependency only without silently dropping trust record | at the operation | 8
EVALUATION165 | "Peak live cells receive the same treatment." | Termination and sponsorship | Peak live cell accounting must follow the same certified-maximum or sponsor-budget rules as other logical-work measures | not stated | 4
EVALUATION166 | "Inputs include language/evaluator semantics, required target primitive meanings, exact selected conformance/provider plans, normalized implementations, and compiler-materialized arguments." | Target-semantic capsule | Evaluator inputs are limited to the listed compiler-prepared semantic and materialization inputs | at entry | 9
EVALUATION167 | "A future integer-width constructor would require its own admitted widths, identity, and lowering." | Target-dependent applications | Hypothetical integer-width constructors must have their own admitted widths, identity, and lowering before use | not stated | 5
EVALUATION168 | "Genuinely different native field/case sets require distinct nominal ABI schemas, privately selected by exact realizations behind one stable portable requirement." | Target-dependent applications | Distinct native field/case sets must use distinct ABI schemas behind one portable requirement | not stated | 9
EVALUATION169 | "Open symbolic indices remain normalized artifact data;" | Canonical static identities | Open symbolic indices must remain normalized artifact data for identity purposes | not stated | 7
EVALUATION170 | "The shared identity profile needs resolved types and operation contracts, not physical layout or target realization." | Canonical static identities | Shared identity profile must use resolved types and operation contracts only, not layout or target realization | not stated | 7
EVALUATION171 | "This includes compatibility of differently written indices: the consumer need not repeat proof discovery," | Result caching and published evidence | Proof reuse must allow differently written but compatible indices without repeating discovery | not stated | 8
EVALUATION172 | "The separate [proof-search-cache draft](../../drafts/reference/proof_search_cache.md) explores local search reuse, not a replacement admission route." | Result caching and published evidence | Proof-search-cache draft must not be treated as a replacement evaluation admission route | not stated | 8
EVALUATION173 | "A pure catalog operation can evaluate when its complete invocation is admitted;" | Invocation admission | Pure catalog assembly operations may evaluate only when the complete invocation is admitted | at a call | 10
EVALUATION174 | "Local mutation, allocation, temporary borrowing, and recursion are permitted implementation techniques when their invocation satisfies the complete contract." | Invocation admission | Evaluator implementation may use mutation, allocation, temp borrows, and recursion when invocation contract is satisfied | at the operation | 2
EVALUATION175 | "Temporary slices may exist inside evaluation." | Invocation admission | Temporary slices inside evaluation are permitted when other bridge rules hold | at the operation | 6
EVALUATION176 | "Trait requirements bound static callback selections;" | Invocation admission | Trait requirements must bind static callback selections for reach and operational admission | not stated | 3
EVALUATION177 | "The [UEFI entry profile](../build/uefi_entry.md) separates its stack guarantee from invocation admission." | Target-semantic capsule | UEFI stack guarantee must not be conflated with semantic-evaluation invocation admission | not stated | 9
EVALUATION178 | "A profile guarantee describes a selected contract, not proof that physical arrival satisfies it." | Target-semantic capsule | Profile guarantees are contractual selections, not proofs of physical hardware satisfaction | not stated | 9
EVALUATION179 | "Target-dependent geometry normally belongs to validated plan size/alignment and placed storage rather than a target-sized array, but the latter is not forbidden." | Target-dependent applications | Target-sized arrays are permitted though geometry normally lives in plan size/alignment and placed storage | not stated | 9
EVALUATION180 | "Two distinct normalized dependencies propagate transitively:" | Target-dependent applications | Both observation-application and selected-realization dependencies must propagate transitively | not stated | SHARED

## Ambiguous sentences

Sentence | Reading A | Reading B
--- | --- | ---
"The source position requests evaluation;" | Any syntactic site that triggers const/proof/plan evaluation is an evaluation request governed by this chapter. | "Source position" is a technical term with a narrower set of request kinds not fully enumerated here. |
"Evaluation changes when, not what." | Sponsor, budget, and caching affect whether evaluation runs and when it completes, not the semantic value produced when it succeeds. | Scheduling may change observable evaluation strategy while still requiring bitwise-identical results only for runtime-capable machines. |
"Mathematical checking is a different judgment: a well-typed symbolic term may refer to an admitted axiom without computing a value." | The compiler must separate mathematical checking from semantic evaluation and not require value computation for axiom references. | Mathematical checking is documented separately; this sentence does not state whether one pipeline pass or two is required. |
"A pure catalog operation can evaluate when its complete invocation is admitted;" | "Pure catalog operation" is a defined class; admission uses the same checklist as other machines. | Any operation listed in the assembly catalog is pure unless proven otherwise. |
"Local mutation, allocation, temporary borrowing, and recursion are permitted implementation techniques when their invocation satisfies the complete contract." | These techniques are allowed inside the evaluator implementation whenever the invocation contract is satisfied. | Permission applies only to compiler-internal implementation, not to user code executed during evaluation. |
"A terminating invocation can still be too expensive for its sponsor." | Termination proof is necessary but resource admission may still reject. | "Too expensive" might mean wall-clock timeout rather than logical-work budget; the sentence does not specify which. |
"Peak live cells receive the same treatment." | Peak live cell counts are subject to the same certified-maximum or sponsor-budget rules as other logical-work measures. | "Same treatment" refers only to reproducible published builds, not all evaluation. |
"The shared identity profile fixes canonical typed ordering and encoding without distributive expansion or a search cutoff." | Identity computation must not use distributive expansion or search cutoff in the shared profile. | Distributive expansion and search cutoff are forbidden for runtime indices but may still apply in compatibility proofs named in the next paragraph. |
"Target-dependent geometry normally belongs to validated plan size/alignment and placed storage rather than a target-sized array, but the latter is not forbidden." | Target-sized arrays are allowed when other rules are met; geometry usually lives in plans. | Using target-sized arrays is discouraged but semantically equivalent to plan geometry when admitted. |
"An unscoped constant or public type application may be target-polymorphic." | Such items may vary by target before closure without rejection. | Target-polymorphism requires explicit unscoped or public declaration shape; other forms are not discussed. |
"Genuinely different native field/case sets require distinct nominal ABI schemas, privately selected by exact realizations behind one stable portable requirement." | One portable requirement may hide multiple private ABI schemas selected by realization. | Distinct schemas are required in source declarations, not only in private lowering. |
"A whole-capsule and complete-realization-closure key is sound but conservatively rejects reuse." | Implementations may use whole-capsule keys knowing they reject valid reuse. | Whole-capsule keys are normative and required for soundness. |
"The second remains necessary for a selected ordinary provider containing only literals and no projection call." | Literal-only providers still carry selected-realization dependency for caching and identity. | The second dependency kind applies only when Build selection introduces target scope beyond literals. |
"This section governs systems generic/domain indices and their published specialization identity." | Rules apply to systems indices and published specialization identity only, not all generics. | "Systems" might mean compiler-internal generic indices, excluding user-facing type parameters. |
"Open symbolic indices remain normalized artifact data;" | Type equality uses normalized artifact data, not arbitrary evaluation. | Open indices are stored as artifacts but may still be compared via partial evaluation when closed. |
"Reflection selection contexts are owned within an evaluation root and lent to policy machines through ordinary explicit parameters." | Context ownership stays at evaluation root; loans use ordinary parameter passing only. | Policy machines may hold longer-lived handles if parameters are explicit enough. |
"Semantic reflection may inspect an explicitly selected type under the lexical author's ordinary dependency and visibility authority; it is not restricted to an implicitly bound implementing type; no such type exists." | Reflection at generic default sites uses lexical author visibility, not an implicit impl type. | Reflection may still require a conformance elsewhere; "no such type exists" applies only to default bodies without impl context. |

## Terms used without definition in this chapter

- semantic evaluation / evaluation / evaluator
- hermetic target-semantic world
- source position
- complete normalized contract / normalized contract
- mathematical checking
- admitted axiom
- empty reach
- representable result carrier
- eligible runtime-capable machine
- target execution / target semantics
- proof-only machine
- compiler phase
- build execution
- build-host services
- staged value/artifact custody
- build-time tests
- runner contract
- assumed binding contracts / relationship
- erased evidence
- root runtime acceptance
- specialize / complete contract
- ordinary checked termination
- runtime service reach
- suspension / blocking
- trap / abort route
- runtime authority
- escaping runtime mutation
- proof/build-admissible trust and resource inputs
- axiomatic mathematical result / executable provider
- static callback selections / specialized published reach
- carry / invariant-window failure
- opaque contracts
- public trait requirements
- package declaration-selection authority
- unresolved authored operator / builtin meaning
- interpreted assembly / pure catalog operation
- direct token-bearing machine
- compiler-catalog semantic primitives
- canonical float operations
- boundary operators
- hermetic bridge
- recursive value snapshot
- augmenting build API
- address exposure
- validated layout
- `code_layout<Machine>()` / late native artifact metadata
- ConstEvaluable / ConstMaterializable
- proof-only references / assumption closure
- trust record
- `terminates` / `terminates by ...`
- sponsor / sponsorship
- logical work / canonical Terminal schedule
- certified maximum / compiler grant
- sponsor budget / usage receipt
- payload ceilings / compiler-owned account
- target capsule / target realization
- language/evaluator semantics
- projection vocabulary
- BuildWorld
- target closure
- FloatSemantics / FloatMeaning
- UEFI entry profile
- target-polymorphic / symbolic recipe
- target-dependent geometry
- ABI schemas
- observation application / selected-realization application
- target-closure receipt
- whole-capsule key / fine-grained replay
- semantic-API revision
- origin DAG
- canonical generic/domain index
- shared identity profile
- distributive expansion / search cutoff
- Build proof-work policy
- canonical runtime atoms
- static callable selections
- boundary-opaque values
- indexed `Rat`
- ResultKey / UsageRecord / PolicyCharge
- accounting identity
- published proof-only contracts
- Psi admission
- erased evidence
- trait machine body / overridable default
- semantic reflection
- TypeSchema
- generated equality / substitutable equality
- quotient laws
- irrefutable data bindings
- body-shape coverage / trait-level coverage gate
- reflection selection contexts / evaluation root
- selection result contract

CHAPTER COMPLETE
