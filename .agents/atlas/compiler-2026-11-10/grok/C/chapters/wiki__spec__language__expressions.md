# Expressions and evaluation order

Source: `wiki/spec/language/expressions.md` lines 1-490 (read).

## Rules

rule id | the exact sentence, quoted | the heading it is under | what must be checked, in plain words | when it must hold | lane
--- | --- | --- | --- | --- | ---
EXPRESSIONS1 | "Expressions retain their selected operation identities and source evaluation schedule through checking, semantic evaluation, and realization." | Expressions and evaluation order | Retain selected operation identity and source evaluation schedule through checking, evaluation, and realization | not stated | 1
EXPRESSIONS2 | "All authored arguments left to right." | Evaluation schedule | Named machine call arguments evaluate left to right | at the operation | 3
EXPRESSIONS3 | "Left operand, then right operand." | Evaluation schedule | Strict binary operators evaluate left operand then right | at the operation | 0
EXPRESSIONS4 | "Left; right only when left is true." | Evaluation schedule | `&&` evaluates left; evaluates right only when left is true | at the operation | 0
EXPRESSIONS5 | "Left; right only when left is false." | Evaluation schedule | `||` evaluates left; evaluates right only when left is false | at the operation | 0
EXPRESSIONS6 | "Collection, then index." | Evaluation schedule | Index evaluates collection then index | at the operation | 6
EXPRESSIONS7 | "Capture the destination place, then evaluate the value, then replace at the captured place." | Evaluation schedule | Assignment captures place, evaluates value, replaces at captured place | at the operation | 0
EXPRESSIONS8 | "Present start, then present end." | Evaluation schedule | Range evaluates present start then present end | at the operation | 6
EXPRESSIONS9 | "Increasing element index." | Evaluation schedule | Fixed-array literal elements evaluate in increasing index order | at the operation | 1
EXPRESSIONS10 | "Authored field-expression order." | Evaluation schedule | Record or case literal fields evaluate in authored order | at the operation | 1
EXPRESSIONS11 | "Subject once before dispatch; only the selected arm runs." | Evaluation schedule | Transition dispatch evaluates subject once; only selected arm runs | at the operation | 10
EXPRESSIONS12 | "Subject once; first matching arm only." | Evaluation schedule | Value-position match evaluates subject once; first matching arm only | at the operation | 1
EXPRESSIONS13 | "Every evaluated child runs exactly once." | Evaluation schedule | Each evaluated child runs exactly once | at the operation | 1
EXPRESSIONS14 | "Unary operations, borrows, value casts, membership tests, and member access have one immediate runtime child." | Evaluation schedule | Listed unary forms have exactly one immediate runtime child | at the operation | 1
EXPRESSIONS15 | "it has no runtime operand to evaluate and does not invoke the machine." | Evaluation schedule | Machine-entry address query has no runtime operand and does not invoke the machine | at the operation | 3
EXPRESSIONS16 | "The short-circuit and dispatch rows are the closed selective set for these forms;" | Evaluation schedule | Only listed short-circuit and dispatch forms may use selective evaluation | at the operation | 1
EXPRESSIONS17 | "a new selective form needs an explicit schedule, not an implicit lazy exception." | Evaluation schedule | New selective evaluation forms require an explicit schedule in the language definition | not stated | 1
EXPRESSIONS18 | "The right operand of `&&` may use the left operand's true-path facts; `||` may use its false-path facts." | Evaluation schedule | Short-circuit right operand may use path facts from left operand | at the operation | 8
EXPRESSIONS19 | "These assumptions do not escape the expression." | Evaluation schedule | Short-circuit path assumptions must not escape the enclosing expression | at the operation | 8
EXPRESSIONS20 | "Writes invalidate overlapping facts even within a selected path." | Evaluation schedule | Writes invalidate overlapping facts even on a selected short-circuit path | at the operation | 8
EXPRESSIONS21 | "An effectful guard cannot be replayed as a fact about operands it may have changed." | Evaluation schedule | Effectful guard must not be replayed as unchanged operand fact | at the operation | 8
EXPRESSIONS22 | "Named fields are evaluated in authored order and installed at their declared identities." | Evaluation schedule | Record or case named fields evaluate in authored order at declared identities | at the operation | 1
EXPRESSIONS23 | "has no evaluated operand." | Evaluation schedule | Final `..` omission marker has no evaluated operand | at the operation | 1
EXPRESSIONS24 | "It authorizes zero establishment of omitted runtime fields under their existing validity and custody rules;" | Evaluation schedule | `..` authorizes zero establishment of omitted runtime fields under validity and custody rules | at the operation | 1
EXPRESSIONS25 | "it does not reorder the authored expressions or add a constructor call." | Evaluation schedule | `..` must not reorder authored expressions or add a constructor call | at the operation | 1
EXPRESSIONS26 | "Reordering must preserve results, effects, failures, moves, and cleanup under the permitted observation contract." | Evaluation schedule | Permitted evaluation reordering preserves results, effects, failures, moves, cleanup | not stated | 1
EXPRESSIONS27 | "Crash routes have no cleanup successor." | Evaluation schedule | Crash routes must have no cleanup successor | on every edge | 2
EXPRESSIONS28 | "Transition dispatch may select a final call whose result completes the current invocation, not an arbitrary lexical ancestor." | Calls, places, and temporaries | Transition final call result completes current invocation, not arbitrary ancestor | at a call | 3
EXPRESSIONS29 | "A named machine call selects a static path and supplies every ordinary parameter explicitly." | Calls, places, and temporaries | Named machine calls use static path and explicit full ordinary parameter list | at a call | 3
EXPRESSIONS30 | "not `buffer.clear()`." | Calls, places, and temporaries | Qualified static calls use qualified form, not dot invocation on instance | at a call | 3
EXPRESSIONS31 | "Dot access remains data-field projection; it does not select or invoke a machine." | Calls, places, and temporaries | Dot access is field projection only; must not select or invoke machine | at the operation | 1
EXPRESSIONS32 | "Every declared parameter is explicit and appears in source order; there is no receiver or extra binding argument." | Calls, places, and temporaries | Boundary requirement calls have explicit parameters in source order; no receiver or extra binding | at a call | 9
EXPRESSIONS33 | "Checking retains that slot demand, and realization rejects unless provider planning selects one admitted backing." | Calls, places, and temporaries | Retain provider-slot demand; realization rejects without admitted backing selection | not stated | 9
EXPRESSIONS34 | "No ambient source search, hidden value, or implicit argument insertion repairs missing or ambiguous supply." | Calls, places, and temporaries | Must not repair missing or ambiguous boundary supply via search, hidden value, or implicit args | not stated | 9
EXPRESSIONS35 | "Calls obey the [suspension/blocking acknowledgement and position rules](effects.md#call-site-acknowledgements)." | Calls, places, and temporaries | Calls obey suspension/blocking acknowledgement and position rules | at a call | 3
EXPRESSIONS36 | "Machine invocations require their full ordinary argument list." | Calls, places, and temporaries | Machine invocations require complete ordinary argument list | at a call | 3
EXPRESSIONS37 | "This grants no implicit machine-to-function conversion, runtime closure, pending execution or obligation to finish applying the term." | Calls, places, and temporaries | Mathematical function application grants no implicit machine conversion, closure, or pending execution | at a call | 8
EXPRESSIONS38 | "Expected-type and executable-demand checks still apply." | Calls, places, and temporaries | Expected-type and executable-demand checks still apply to function-term application | at a call | 3
EXPRESSIONS39 | "These are compiler-known core queries, not casts or equal-size bit reinterpretation." | Calls, places, and temporaries | Address and layout queries are core queries, not casts or bit reinterpretation | at the operation | 1
EXPRESSIONS40 | "A qualified machine name does not capture a data instance." | Calls, places, and temporaries | Qualified machine name must not capture a data instance | at the operation | 1
EXPRESSIONS41 | "Static arguments select types or exact machine applications without introducing runtime type objects." | Calls, places, and temporaries | Static arguments select types or exact applications without runtime type objects | at the operation | 7
EXPRESSIONS42 | "These address/layout queries do not construct [runtime machine values](machine_values.md): a callable value is established separately by exact named selection and contract refinement." | Calls, places, and temporaries | Address/layout queries must not construct runtime machine values | at the operation | 10
EXPRESSIONS43 | "Assignment requires writable storage, compatible value type, and the applicable [invariant-window obligations](dependent_values.md#invariant-windows)." | Calls, places, and temporaries | Assignment requires writable storage, compatible type, and invariant-window obligations | at the operation | 0
EXPRESSIONS44 | "It first evaluates the destination's collections and selectors in their ordinary order, once, capturing the exact storage location." | Calls, places, and temporaries | Assignment captures destination once after evaluating collections and selectors in ordinary order | at the operation | 0
EXPRESSIONS45 | "Changing a selector or reference binding during the right-hand side does not redirect the pending store; no destination expression is reevaluated for cleanup or installation." | Calls, places, and temporaries | RHS changes to selectors or refs must not redirect pending store; no destination reevaluation | at the operation | 0
EXPRESSIONS46 | "Place validity and bounds are established when the destination is captured." | Calls, places, and temporaries | Establish place validity and bounds at destination capture | at the operation | 2
EXPRESSIONS47 | "Ordinary loan and ownership rules must keep that backing valid through the store;" | Calls, places, and temporaries | Loans and ownership keep captured backing valid through store | at the operation | 2
EXPRESSIONS48 | "capturing a place does not permit the right-hand side to invalidate it." | Calls, places, and temporaries | Captured place must remain valid; RHS must not invalidate it | at the operation | 2
EXPRESSIONS49 | "Capturing identifies storage, not a read or move of its old value." | Calls, places, and temporaries | Place capture identifies storage only; not read or move of old value | at the operation | 2
EXPRESSIONS50 | "Right-hand-side effects still invalidate the facts they overlap as they occur." | Calls, places, and temporaries | RHS effects invalidate overlapping facts as they occur | at the operation | 8
EXPRESSIONS51 | "After evaluating the value, the store invalidates and replaces facts about the captured destination, as specified by [mutation and subject identity](state_contracts.md#mutation-and-subject-identity)." | Calls, places, and temporaries | After value eval, store invalidates and replaces destination facts per mutation rules | at the operation | 8
EXPRESSIONS52 | "A block introduces local scope and cleanup edges, not a new machine declaration." | Calls, places, and temporaries | Block introduces scope and cleanup edges only, not new machine declaration | at entry | 2
EXPRESSIONS53 | "A terminal value completes the invocation." | Calls, places, and temporaries | Terminal value completes invocation | at normal return | 3
EXPRESSIONS54 | "A call-shaped transition arm is a [terminal call](state_contracts.md#terminal-calls-in-transition-arms), with ordinary call, suspension and completion obligations." | Calls, places, and temporaries | Call-shaped transition arm is terminal call with ordinary call obligations | at a call | 3
EXPRESSIONS55 | "Temporary storage must outlive every derived borrow and receive cleanup on every cleanup-bearing exit." | Calls, places, and temporaries | Temporaries outlive derived borrows and get cleanup on every cleanup-bearing exit | on every edge | 2
EXPRESSIONS56 | "For named machine calls, mutable and write-only lending require explicit `&mut` and `&write` expressions." | Calls, places, and temporaries | Named calls require explicit `&mut` and `&write` for mutable and write-only lending | at a call | 2
EXPRESSIONS57 | "The token-specific collection adaptation for [indexing](#indexing-and-ranges) does not change named-call argument admission." | Calls, places, and temporaries | Indexing collection adaptation must not change named-call argument admission | at a call | 6
EXPRESSIONS58 | "Checked identity and published consumers retain the access kind and rejoin the explicit argument with its declared parameter type; source spelling cannot substitute for that identity." | Calls, places, and temporaries | Retain access kind and parameter type identity; source spelling cannot substitute | at a call | 2
EXPRESSIONS59 | "Numeric helper names such as `min`, `max`, `abs`, `clamp` and `sqrt` select [ordinary standard-library machines](numeric_values.md#library-numeric-helpers), not compiler-known call forms." | Operator declarations | Listed numeric helpers resolve as ordinary library machines, not compiler-known forms | at a call | 3
EXPRESSIONS60 | "none is implicitly available by spelling alone." | Operator declarations | Numeric helpers not implicitly available by spelling alone | at a call | 3
EXPRESSIONS61 | "Equality between type expressions in generic constraints and static branches uses [static type equality](generics.md#static-type-equality), not runtime value comparison or user operator discovery." | Operator declarations | Type-expression equality uses static type equality, not runtime `==` or operator discovery | not stated | 7
EXPRESSIONS62 | "It produces no type object or runtime metadata lookup;" | Operator declarations | Static type equality produces no type object or runtime metadata lookup | not stated | 7
EXPRESSIONS63 | "Builtin comparisons between fixed integers of different widths or signedness follow [exact integer equality and order](numeric_values.md#integer-equality-and-order), not a common-carrier promotion." | Operator declarations | Cross-width integer comparisons follow exact integer rules, not common-carrier promotion | at the operation | 5
EXPRESSIONS64 | "This does not change operator selection or add conversions to ordinary machine arguments." | Operator declarations | Cross-width integer rule must not add conversions to ordinary machine arguments | at a call | 5
EXPRESSIONS65 | "Data is not a code namespace;" | Operator declarations | Data types are not code namespaces for operators | not stated | 1
EXPRESSIONS66 | "Without a token, the declaration is an ordinary named machine." | Operator declarations | Machine declaration without fixed token is ordinary named machine | not stated | 10
EXPRESSIONS67 | "There is no separate `operator` declaration introducer or second executable identity for a token-bearing machine." | Operator declarations | No separate operator introducer or second executable identity for token machines | not stated | 10
EXPRESSIONS68 | "The token vocabulary is closed and compiler-owned: lexical spelling, precedence, associativity, fixity, allowed arities, and source-position mapping are fixed." | Operator declarations | Token vocabulary closed; lexer/precedence/associativity/fixity/arity/position mapping fixed | not stated | 1
EXPRESSIONS69 | "Tokens are not strings or user-defined punctuation." | Operator declarations | Operator tokens are not strings or user-defined punctuation | not stated | 1
EXPRESSIONS70 | "Declaration path plus normalized signature/overload is canonical operation identity." | Operator declarations | Canonical operation identity is declaration path plus normalized signature/overload | not stated | 1
EXPRESSIONS71 | "Checked and Terminal calls retain that identity;" | Operator declarations | Checked and Terminal calls retain canonical operation identity | not stated | 1
EXPRESSIONS72 | "adding, removing, or changing the public token binding is a breaking source revision." | Operator declarations | Public token binding changes are breaking source revisions | not stated | 1
EXPRESSIONS73 | "There is no alternate `spelling` clause." | Operator declarations | No alternate spelling clause for operators | not stated | 1
EXPRESSIONS74 | "Left." | Operator declarations | Multiplicative and additive binary operators associate left | at the operation | 0
EXPRESSIONS75 | "A suffixed form has the precedence, associativity and arity of its base operator." | Operator declarations | Policy-suffixed operators inherit base precedence, associativity, arity | at the operation | 5
EXPRESSIONS76 | "Lexing takes the longest operator, so `a +!b` is a trapping add;" | Operator declarations | Lexer uses longest operator match for suffixed forms | not stated | 1
EXPRESSIONS77 | "The suffixed forms are builtin integer operations only, so a `machine` declaration cannot bind one." | Operator declarations | Reject machine binding to policy-suffixed integer operators | not stated | 10
EXPRESSIONS78 | "after `as`, an optional carrier type and then every following `=D`, `+D` or `-D` entry belong to the cast." | Operator declarations | Cast entries after `as` belong to single postfix cast | at the operation | 5
EXPRESSIONS79 | "A `+` or `-` directly after a cast therefore begins an entry, never an addition or subtraction," | Operator declarations | `+`/`-` directly after cast begin cast entry, not arithmetic | at the operation | 5
EXPRESSIONS80 | "Grouping is separate from the evaluation schedule." | Operator declarations | Parentheses grouping distinct from evaluation schedule | not stated | 1
EXPRESSIONS81 | "One declaration binds at most one fixed token." | Operator declarations | At most one fixed token per machine declaration | not stated | 10
EXPRESSIONS82 | "duplicate participating token/shape candidates reject." | Operator declarations | Reject duplicate participating token/shape candidates | not stated | 10
EXPRESSIONS83 | "Return carrier or predicate refinement alone cannot distinguish overloads." | Operator declarations | Return carrier or predicate refinement alone cannot distinguish overloads | not stated | 7
EXPRESSIONS84 | "There is no hidden receiver operand." | Operator declarations | Operator selection has no hidden receiver operand | not stated | 3
EXPRESSIONS85 | "Being an operator grants no mutation: mutation requires an admitted mutable operand shape." | Operator declarations | Operator syntax grants no mutation without admitted mutable operand shape | at the operation | 2
EXPRESSIONS86 | "Checked consumers retain the authored selection occurrence, not a guess from the leaf name." | Operator declarations | Retain authored selection occurrence, not leaf-name guess | not stated | 1
EXPRESSIONS87 | "Merely occupying operand position zero does not grant receiver status or an implicit loan." | Operator declarations | Operand position zero does not imply receiver or implicit loan | at the operation | 2
EXPRESSIONS88 | "If permitted adaptations leave competing candidates, ordinary ambiguity rejection applies, not an implicit preference for an owned or borrowed overload." | Operator declarations | Competing operator candidates reject; no implicit owned/borrowed preference | not stated | 10
EXPRESSIONS89 | "Their conformances supply implementations, never new token bindings." | Operator declarations | Trait conformances supply implementations only, never new token bindings | not stated | 7
EXPRESSIONS90 | "A trait-backed token use requires one exact conformance already selected by an explicit proof-static binder in the surrounding source telescope." | Operator declarations | Trait-backed token use requires one exact conformance from explicit proof-static binder | at the operation | 8
EXPRESSIONS91 | "At generic specialization, that binder substitutes its exact requirement row before lowering; lowering performs no implementation search." | Operator declarations | Generic specialization substitutes binder row before lowering; lowering does no impl search | not stated | 7
EXPRESSIONS92 | "No binder rejects even when one conformance or Build provider is visible, and several applicable binders are ambiguous." | Operator declarations | Reject trait-backed token use with no binder or with ambiguous binders | not stated | 8
EXPRESSIONS93 | "`Build::select_provider` realizes an already-selected boundary requirement; it never supplies proof-static evidence, activates a trait's token, overrides a primitive/direct family, or changes source resolution." | Operator declarations | Build provider selection realizes prior selection only; must not supply proof evidence or change resolution | at a call | 9
EXPRESSIONS94 | "Changing provider configuration therefore cannot change what an operator token means." | Operator declarations | Provider configuration changes must not change operator token meaning | not stated | 9
EXPRESSIONS95 | "a second wrapper with that shape rejects rather than becoming a selectable alternative." | Operator declarations | Reject second canonical direct wrapper for same token/operand shape | not stated | 10
EXPRESSIONS96 | "When operator syntax selects a machine, that machine's `requires` is a call obligation in executable and proof-term formation." | Operator declarations | Selected machine `requires` is call obligation in executable and proof-term formation | at the operation | 8
EXPRESSIONS97 | "Resolution does not discharge it, and a proof-only caller cannot bypass it." | Operator declarations | Operator resolution does not discharge `requires`; proof-only caller cannot bypass | at a call | 8
EXPRESSIONS98 | "absence of executable equality does not silently select identity." | Operator declarations | Missing executable equality must not silently select identity relation | at the operation | 8
EXPRESSIONS99 | "any required conformance/provider selection is explicit, never an implicit search among visible satisfying machines." | Executable supply | Conformance/provider selection for operators explicit; no implicit search | not stated | 8
EXPRESSIONS100 | "A nonboundary direct machine outside a required-body context or the compiler catalog must have a checked body." | Executable supply | Nonboundary direct machine outside required-body/catalog must have checked body | not stated | 10
EXPRESSIONS101 | "Missing supply rejects at the declaration, not only when execution is demanded." | Executable supply | Reject missing executable supply at declaration | not stated | 10
EXPRESSIONS102 | "A separate `satisfies` machine cannot supply that missing body;" | Executable supply | `satisfies` cannot supply missing machine body | not stated | 10
EXPRESSIONS103 | "Do not create a direct-operator registry, package-level canonical-body search, or build-provider override for ordinary checked machine bodies." | Executable supply | Must not use direct-operator registry, canonical-body search, or build override for ordinary bodies | not stated | 9
EXPRESSIONS104 | "Removing operator punctuation must not silently turn required supply into an admitted theorem declaration or vice versa." | Executable supply | Removing operator token must not silently change required supply vs theorem declaration | not stated | 8
EXPRESSIONS105 | "No separate operator interpreter or producer-asserted body association is permitted." | Executable supply | No separate operator interpreter or producer-asserted body association | not stated | 10
EXPRESSIONS106 | "One declaration has one authorized supply; a catalog realization and a separate source body cannot compete." | Executable supply | One authorized supply per declaration; catalog and source body cannot compete | not stated | 10
EXPRESSIONS107 | "requires no build selection." | Executable supply | Compiler primitive such as `Float::meaning32` requires no build selection | not stated | 9
EXPRESSIONS108 | "with no build selection and no override," | Executable supply | Canonical `f32 + f32` has no build selection and no override | not stated | 9
EXPRESSIONS109 | "No realization can redefine its arithmetic contract." | Executable supply | Realization must not redefine canonical float arithmetic contract | not stated | 5
EXPRESSIONS110 | "Merely naming a declaration `Float::meaning32` grants no primitive implementation or proof authority." | Executable supply | Name alone does not grant primitive implementation or proof authority | not stated | 8
EXPRESSIONS111 | "Resolution considers the complete operand tuple's declared types and written casts, not flow facts." | Operator families | Operator resolution uses declared types and written casts only, not flow facts | at the operation | 1
EXPRESSIONS112 | "A declaration applies only when its instantiated parameter types equal the operands' complete qualified types," | Operator families | Declaration applies only when instantiated params equal operands' complete qualified types | at the operation | 7
EXPRESSIONS113 | "Association with a domain grants no authority to establish its membership;" | Operator families | Domain association grants no membership-establishment authority | not stated | 5
EXPRESSIONS114 | "competing participating meanings at a use reject." | Operator families | Reject competing participating operator meanings at a use | at the operation | 10
EXPRESSIONS115 | "no two declarations compose into one operation meaning." | Operator families | Two declarations must not compose into one operation meaning | not stated | 10
EXPRESSIONS116 | "Operator families are closed by default." | Operator families | Operator families closed by default | not stated | 10
EXPRESSIONS117 | "A third party owning neither operand introduces an explicit adapter domain rather than injecting an implementation into unrelated carriers." | Operator families | Third party must use explicit adapter domain, not inject into unrelated carriers | not stated | 7
EXPRESSIONS118 | "Candidate discovery uses participating domains, their owning packages, and the declared family, never an arbitrary scan of imports." | Operator families | Operator candidate discovery limited to participating domains, packages, family | not stated | 10
EXPRESSIONS119 | "For a closed direct family, only its semantic-home owner may publish its direct token bindings." | Operator families | Only semantic-home owner may publish direct token bindings of closed family | not stated | 10
EXPRESSIONS120 | "A declaration in an unauthorized package rejects there;" | Operator families | Reject operator declaration in unauthorized package | not stated | 10
EXPRESSIONS121 | "it does not enter another package's candidate set and wait to collide at a use." | Operator families | Unauthorized declaration must not enter another package's candidate set | not stated | 10
EXPRESSIONS122 | "Adding an unrelated dependency cannot change existing resolution, invalidate its typechecking, or introduce a collision." | Operator families | Unrelated dependency must not change resolution, typechecking, or cause collision | not stated | 10
EXPRESSIONS123 | "Resolution is a compile-time choice retained in the checked artifact; runtime dispatch does not repeat it." | Operator families | Operator resolution is compile-time only; runtime does not repeat | not stated | 1
EXPRESSIONS124 | "A declaration-surface change requires dependent recompilation, not a silent runtime rebind." | Operator families | Declaration-surface change requires recompilation; no silent runtime rebind | not stated | 10
EXPRESSIONS125 | "commutativity does not grant automatic candidate reversal." | Operator families | Commutativity does not grant automatic operator candidate reversal | not stated | 10
EXPRESSIONS126 | "Indexing and range slicing select ordinary core operators with visible bounds contracts; neither is raw pointer syntax." | Indexing and ranges | Indexing and ranges use core operators with bounds contracts; not raw pointers | at the operation | 6
EXPRESSIONS127 | "Mutable forms additionally require the appropriate loans and write authority." | Indexing and ranges | Mutable indexing/ranges require appropriate loans and write authority | at the operation | 2
EXPRESSIONS128 | "Descriptor construction implements the selected contract but cannot establish its proofs." | Indexing and ranges | Descriptor construction cannot establish contract proofs | at the operation | 8
EXPRESSIONS129 | "Bracket syntax must not fall back to declaration-free indexing," | Contract-gated core operations | Bracket syntax must not use declaration-free indexing fallback | at the operation | 6
EXPRESSIONS130 | "and a separate indexing checker must not substitute a recognized bounds pattern for that contract." | Contract-gated core operations | Indexing checker must not substitute bounds pattern for selected contract | at the operation | 6
EXPRESSIONS131 | "Its bodyless supply is the sealed compiler primitive, not an unproved boundary claim or a user-selected foreign provider." | Contract-gated core operations | Core read primitive supply is sealed compiler primitive only | not stated | 10
EXPRESSIONS132 | "No mandatory checked forwarding wrapper sits between bracket syntax and the primitive." | Contract-gated core operations | No mandatory forwarding wrapper between bracket syntax and primitive | not stated | 10
EXPRESSIONS133 | "they do not authorize it." | Contract-gated core operations | Library helpers calling primitive do not authorize bracket indexing | not stated | 6
EXPRESSIONS134 | "Selection captures the collection and selector once under the evaluation order below," | Contract-gated core operations | Index selection captures collection and selector once per evaluation order | at the operation | 6
EXPRESSIONS135 | "and submits every requirement to ordinary call-contract checking." | Contract-gated core operations | Submit every index operation requirement to ordinary call-contract checking | at a call | 6
EXPRESSIONS136 | "An additional well-formed requirement is proved at the call site or the call rejects;" | Contract-gated core operations | Prove additional well-formed requirement at call site or reject call | at a call | 8
EXPRESSIONS137 | "a syntactic `index < collection.len` recognizer has no independent authority to admit an access." | Contract-gated core operations | Syntactic bounds recognizer has no independent admission authority | not stated | 8
EXPRESSIONS138 | "A same-spelled user field or unrelated length observation supplies no substitute evidence." | Contract-gated core operations | User field or unrelated length observation cannot substitute bounds evidence | at a call | 8
EXPRESSIONS139 | "Index bounds, readable backing and an unrestricted element; copying preserves the source loan." | Contract-gated core operations | By-copy read: index bounds, readable backing, unrestricted element; copy preserves loan | at a call | 6
EXPRESSIONS140 | "Index bounds, readable backing and the result's origin/lifetime within the source loan." | Contract-gated core operations | Shared borrow: index bounds, readable backing, result within source loan | at a call | 2
EXPRESSIONS141 | "Index bounds, exclusive access and result-origin/restoration obligations; a valid index alone grants no write authority." | Contract-gated core operations | Mutable access: bounds, exclusive access, restoration; index alone does not grant write | at a call | 2
EXPRESSIONS142 | "Ordered nonnegative endpoints within the live extent, resulting length and backing identity; mutable ranges require explicit exclusive borrowing." | Contract-gated core operations | Range: ordered endpoints in extent; mutable ranges need exclusive borrow | at a call | 6
EXPRESSIONS143 | "Live length, fixed extent and capacity are not interchangeable." | Contract-gated core operations | Live length, fixed extent, and capacity are distinct | at a call | 6
EXPRESSIONS144 | "Inclusive-range normalization must prove its endpoint and successor arithmetic;" | Contract-gated core operations | Prove inclusive-range endpoint and successor arithmetic | at a call | 8
EXPRESSIONS145 | "A global success bit, a different occurrence's proof or a valid bound on another view cannot replace that relation." | Contract-gated core operations | Per-occurrence bounds evidence cannot be replaced by global bit, other occurrence, or other view | not stated | 8
EXPRESSIONS146 | "Loan/access checking stays independent of numerical entailment; the primitive implementation must honor both." | Contract-gated core operations | Loan/access checking independent of numerical entailment; primitive honors both | not stated | 2
EXPRESSIONS147 | "`buffer[index]` supplies the collection at operand zero and forms the shared loan required by that parameter." | Collection arguments and results | `buffer[index]` supplies collection at operand zero and forms required shared loan | at the operation | 2
EXPRESSIONS148 | "This is the existing token-specific collection adaptation for `[]` and `[..]`, independent of receiver identity; it is not method-call sugar." | Collection arguments and results | Index bracket adaptation is token-specific, not method-call sugar | at the operation | 6
EXPRESSIONS149 | "and a mutable collection parameter requires an explicit `&mut buffer` argument." | Collection arguments and results | Named index call with `&mut` collection param requires explicit `&mut buffer` | at a call | 2
EXPRESSIONS150 | "This rule introduces neither write-only collection adaptation nor automatic borrowing for other operator operands." | Collection arguments and results | No write-only collection adaptation or automatic borrowing for other operands | at the operation | 2
EXPRESSIONS151 | "Collection borrowing does not copy or move the collection, weaken bounds or loan checks, or permit mutable access through shared authority." | Collection arguments and results | Collection borrow must not copy/move collection, weaken checks, or allow mutable via shared | at the operation | 2
EXPRESSIONS152 | "Collection and index expressions are each evaluated once, collection first," | Collection arguments and results | Collection and index evaluated once, collection first | at the operation | 6
EXPRESSIONS153 | "borrowing the collection does not borrow an owned index argument." | Collection arguments and results | Collection borrow does not borrow owned index argument | at the operation | 2
EXPRESSIONS154 | "There is no universal reference-return requirement or implicit dereference of every user-defined index result." | Collection arguments and results | No universal reference-return or implicit deref for user index results | at the operation | 2
EXPRESSIONS155 | "returning a value does not make it an assignable place." | Collection arguments and results | Index value result is not assignable place for write | at the operation | 0
EXPRESSIONS156 | "This adds no result-type-directed operator selection." | Collection arguments and results | No result-type-directed operator selection for indexing | at the operation | 6
EXPRESSIONS157 | "Creating a mutable subrange requires explicit `&mut items[start..end]`." | Borrowed range access | Mutable subrange requires explicit `&mut items[start..end]` | at the operation | 2
EXPRESSIONS158 | "The source must already permit exclusive access; the spelling cannot grant mutation through a shared ancestor." | Borrowed range access | Source must permit exclusive access; bare range cannot grant mutation via shared ancestor | at the operation | 2
EXPRESSIONS159 | "An expected `&mut [T]` type never inserts the mutable borrow, including on a local declaration." | Borrowed range access | Expected `&mut [T]` never auto-inserts mutable borrow on range | at the operation | 2
EXPRESSIONS160 | "simultaneous overlapping loans still obey ordinary compatibility rules." | Borrowed range access | Overlapping range loans obey ordinary compatibility rules | not stated | 2
EXPRESSIONS161 | "The explicit-new-loan rule does not copy an exclusive reference or extend its lifetime." | Borrowed range access | Explicit-new-loan rule must not copy exclusive ref or extend lifetime | at the operation | 2
EXPRESSIONS162 | "`a..b` is exclusive; `a..=b` is inclusive." | Range endpoints | Range `..` exclusive; `..=` inclusive | at the operation | 6
EXPRESSIONS163 | "Omitted endpoints use zero and collection length." | Range endpoints | Omitted range endpoints default to zero and collection length | at the operation | 6
EXPRESSIONS164 | "An invalid or overflowing endpoint calculation is a proof error, not a runtime panic." | Range endpoints | Invalid or overflowing range endpoint is proof error, not runtime panic | at a call | 8
EXPRESSIONS165 | "an inclusive maximum must not silently wrap its exclusive successor." | Range endpoints | Inclusive range maximum must not silently wrap exclusive successor | at a call | 8
EXPRESSIONS166 | "Range formation also proves start/end ordering and the independent lower bounds in [collection bounds](numeric_values.md#collection-bounds)." | Range endpoints | Range formation proves start/end ordering and collection lower bounds | at a call | 8
EXPRESSIONS167 | "Dynamic extents remain conservative without disjointness evidence." | Range endpoints | Dynamic range extents conservative without disjointness evidence | not stated | 2
EXPRESSIONS168 | "Byte indexing of UTF-8 is not character or grapheme indexing; those require separately named operations." | Range endpoints | UTF-8 byte indexing is not character/grapheme indexing; those need separate operations | at the operation | 6
EXPRESSIONS169 | "Application of a [mathematical function term](../proofs/mathematical_bindings.md#named-mathematical-definitions) instead consumes a prefix of its nested dependent-function parameters; the result may be another function term." | Calls, places, and temporaries | Function-term application consumes parameter prefix; result may be another function term | at a call | 8
EXPRESSIONS170 | "delegation adds no binding mechanism." | Operator declarations | Delegation in operator machine body adds no binding mechanism | not stated | 10
EXPRESSIONS171 | "Renaming or reordering generic binders without changing their occurrence relationships creates no new candidate:" | Operator declarations | Renamed or reordered generic binders with same occurrences are duplicate overloads | not stated | 7
EXPRESSIONS172 | "Fixed operator syntax is operand-directed; explicit named calls may additionally use [result-domain selection](domains.md#result-domain-overloads)." | Operator declarations | Operand-directed operator syntax; named calls may use result-domain selection | at the operation | 5
EXPRESSIONS173 | "Direct operators need no conformance selection." | Operator declarations | Direct operators require no conformance selection | at the operation | 7
EXPRESSIONS174 | "Value `==` retains that selected meaning in facts." | Operator declarations | Value `==` retains selected operator meaning in facts | at the operation | 8
EXPRESSIONS175 | "Ordinary `Nat - Nat` and named `Nat::subtract` still require the same operand-order evidence;" | Operator declarations | Named and operator forms require same operand-order evidence | at a call | 8
EXPRESSIONS176 | "Inactive same-carrier declarations may coexist;" | Operator families | Inactive same-carrier declarations may coexist | not stated | 10
EXPRESSIONS177 | "Owning an operand is not a blanket grant to extend somebody else's closed family." | Operator families | Owning operand does not grant extending another's closed family | not stated | 10
EXPRESSIONS178 | "Primitive identities are compiler-owned." | Operator families | Primitive operator identities are compiler-owned | not stated | 10
EXPRESSIONS179 | "whereas bodyless primitive supply requires its catalog entry." | Operator families | Bodyless primitive supply requires catalog entry | not stated | 10
EXPRESSIONS180 | "At a replacement boundary, a body changes only under the declaration's contract and laws." | Operator families | Body replacement only under declaration contract and laws | not stated | 10
EXPRESSIONS181 | "The selected declaration's complete instantiated contract governs admission." | Contract-gated core operations | Index/range admission governed by complete instantiated contract | at a call | 6
EXPRESSIONS182 | "requires index < items.len;" | Contract-gated core operations | Core by-copy read requires proved `index < items.len` | at a call | 8
EXPRESSIONS183 | "Their conclusions must describe the exact operands and be usable by other contracts;" | Contract-gated core operations | Acceleration strategy conclusions must describe exact operands and be reusable | not stated | 8
EXPRESSIONS184 | "Successful discharge retains per-occurrence evidence relating the selected declaration and instantiation to the exact collection, selector, extent and storage versions." | Contract-gated core operations | Retain per-occurrence evidence linking declaration to exact collection, selector, extent, versions | not stated | 8
EXPRESSIONS185 | "Writing through an index requires the selected operation to supply writable storage under its declared contract;" | Collection arguments and results | Indexed write requires operation supplying writable storage under contract | at the operation | 2
EXPRESSIONS186 | "Authors do not need `(&buffer)[index]` or a distinct `&[]` operator." | Collection arguments and results | Bracket syntax without extra `&[]` operator or parenthesized borrow wrapper | at the operation | 6
EXPRESSIONS187 | "Named calls remain ordinary static calls: `at(buffer, index)` follows shared argument admission," | Collection arguments and results | Named index calls are ordinary static calls with shared admission | at a call | 3
EXPRESSIONS188 | "For core collections, a bare range `items[start..end]` forms a shared view when received as a reference." | Borrowed range access | Bare range forms shared view when used as reference for core collections | at the operation | 2
EXPRESSIONS189 | "Open forms are `a..`, `..b`, and `..`." | Range endpoints | Open range endpoint spellings are `a..`, `..b`, `..` | at the operation | 6
EXPRESSIONS190 | "An inclusive nonempty range also establishes nonemptiness." | Range endpoints | Inclusive nonempty range establishes nonemptiness | at a call | 8
EXPRESSIONS191 | "Borrow conflicts use normalized extents: a live `items[1..]` need not conflict with `items[0]`," | Range endpoints | Borrow conflicts use normalized range extents | not stated | 2
EXPRESSIONS192 | "Operand order, once-only evaluation, conditional-arm invocation, result qualifications, and contract obligations survive ordinary lowering and independent Terminal replay." | Executable supply | Listed evaluation and contract properties survive lowering and Terminal replay | not stated | 1
EXPRESSIONS193 | "Token calls and named calls retain the same declaration/body association and exact static arguments." | Executable supply | Token and named calls retain same declaration/body association and static arguments | not stated | 10
EXPRESSIONS194 | "A realizing boundary machine uses the ordinary exact `satisfies` relationship without redeclaring the token." | Executable supply | Realizing boundary machine uses `satisfies` without redeclaring token | not stated | 9
EXPRESSIONS195 | "builtin operators accept unqualified operands only ([operands](domains.md#operands))." | Operator families | Builtin operators accept unqualified operands only per operands rules | at the operation | 5
EXPRESSIONS196 | "A boundary requirement uses the same qualified static form: `R::operation(arguments)`." | Calls, places, and temporaries | Boundary requirement uses qualified static `R::operation(arguments)` form | at a call | 9
EXPRESSIONS197 | "An operator binding is a property of an ordinary named machine declaration, package-private unless declared `public machine`." | Operator declarations | Operator binding on named machine; package-private unless `public machine` | not stated | 10
EXPRESSIONS198 | "Their call syntax follows ordinary resolution and argument evaluation;" | Operator declarations | Numeric helper calls follow ordinary resolution and argument evaluation | at a call | 3
EXPRESSIONS199 | "ordinary `==` on values keeps its selected value-comparison semantics." | Operator declarations | Value `==` keeps selected value-comparison semantics | at the operation | 0
EXPRESSIONS200 | "`machine +`, `machine ==`, `machine []`, and `machine [..]` use the same declaration grammar with their existing fixed-token arity rules." | Operator declarations | Token-bearing machines use ordinary machine grammar with fixed-token arity rules | not stated | 10
EXPRESSIONS201 | "A machine with no fixed token remains callable by name, including compiler-owned semantic operations without punctuation bindings." | Operator declarations | Machines without fixed token remain callable by name | at a call | 10
EXPRESSIONS202 | "An owned operand remains owned." | Operator declarations | Owned operator operand remains owned after selection | at the operation | 2
EXPRESSIONS203 | "A concrete type may publish one canonical direct wrapper for a token/operand shape;" | Operator declarations | At most one canonical direct wrapper per concrete type and token/operand shape | not stated | 10
EXPRESSIONS204 | "An explicit named requirement call selects other meanings." | Operator declarations | Explicit named requirement call may select non-operator meanings | at a call | 9
EXPRESSIONS205 | "Its own checked machine body." | Executable supply | Ordinary direct token machine supplies its own checked body | not stated | 10
EXPRESSIONS206 | "The explicitly selected implementation, under existing requirement rules." | Executable supply | Ordinary-trait bodyless token requirement uses explicitly selected implementation | not stated | 8
EXPRESSIONS207 | "An explicitly selected checked provider or direct Build-owned native binding under the complete requirement contract." | Executable supply | Boundary-trait bodyless requirement uses explicit provider or Build-native binding | not stated | 9
EXPRESSIONS208 | "Its authorized closed-catalog realization." | Executable supply | Compiler-owned bodyless machine uses authorized closed-catalog realization | not stated | 10
EXPRESSIONS209 | "satisfaction establishes compatibility with a requirement, not permission to select an implementation." | Executable supply | `satisfies` establishes compatibility only, not implementation selection permission | not stated | 10
EXPRESSIONS210 | "The body accepts ordinary machine contracts, nested machines, and transitions and obeys ordinary ownership, effects, termination, evaluation, and cleanup rules." | Executable supply | Operator machine bodies obey ordinary machine contracts and rules | not stated | SHARED
EXPRESSIONS211 | "A public machine may delegate to private helpers without exposing those helpers for consumer selection." | Executable supply | Public machine may delegate privately without exposing helpers for selection | not stated | 10
EXPRESSIONS212 | "Compiler ownership is exact declaration/signature/semantic-catalog identity, not a recognized spelling or a package opting in." | Executable supply | Compiler ownership keyed to declaration/signature/catalog identity | not stated | 10
EXPRESSIONS213 | "Canonical `f32 + f32` is likewise compiler-owned: the selected target's declared realization supplies it as a catalog intrinsic or a checked machine," | Executable supply | Canonical `f32 + f32` compiler-owned via target realization as intrinsic or checked machine | not stated | 9
EXPRESSIONS214 | "Validate ownership and duplicate participating token/normalized operand shapes against that owner's complete declaration set, independently of downstream imports." | Operator families | Validate closed-family token/shape ownership and duplicates against owner's declarations | not stated | 10
EXPRESSIONS215 | "Existing overload ambiguity checking still applies to legitimately participating families; declaration-owned supply removes body-choice ambiguity, not overloads." | Operator families | Overload ambiguity checking still applies; supply rules remove body-choice ambiguity only | not stated | 10
EXPRESSIONS216 | "The same contract model covers fixed arrays, borrowed slices and bounded byte carriers." | Contract-gated core operations | Same index/range contract model for arrays, slices, bounded byte carriers | at a call | 6
EXPRESSIONS217 | "open endpoints use the existing defaults." | Contract-gated core operations | Open range endpoints use existing defaults | at the operation | 6
EXPRESSIONS218 | "The core read's unsigned index supplies its nonnegative bound through its type, not through a special exemption." | Contract-gated core operations | Core read `u64` index nonnegative bound from type, not exemption | at a call | 5
EXPRESSIONS219 | "Lowering consumes that checked plan, emits the primitive directly and attaches the corresponding evidence to each Terminal bounds obligation." | Contract-gated core operations | Lowering uses checked plan, emits primitive, attaches Terminal bounds evidence | not stated | 6
EXPRESSIONS220 | "The [Terminal access contract](../terminal-psi/structural_access.md) continues to govern the emitted read, borrow or write." | Contract-gated core operations | Terminal access contract governs emitted index access | not stated | 6
EXPRESSIONS221 | "Inclusive ranges normalize to `a..(b+1)`: exclusive end validity is `b <= len`, inclusive end validity is `b < len`, exactly validity of index `b`." | Range endpoints | Inclusive range normalizes to exclusive successor; end validity per stated predicates | at a call | 8
EXPRESSIONS222 | "An existing mutable reference may still be forwarded under ordinary argument, move and reborrow rules:" | Borrowed range access | Existing mutable ref forwarded under ordinary argument/move/reborrow rules | at a call | 2
EXPRESSIONS223 | "it is not rejected merely for falling outside a fixed bounds-clause grammar." | Contract-gated core operations | Reject index call for failed proof, not merely non-grammar bounds clause shape | at a call | 8
EXPRESSIONS224 | "Existing operator selection and ambiguity rules still apply." | Contract-gated core operations | Ordinary operator selection and ambiguity rules apply to indexing | at the operation | 10

## Ambiguous sentences

Sentence | Reading A | Reading B
--- | --- | ---
"Expressions retain their selected operation identities and source evaluation schedule through checking, semantic evaluation, and realization." | Every expression form including non-operators must retain identity and schedule at all three stages. | Only expressions that have a selected operation identity and schedule are in scope; plain literals unstated. |
"neither changes evaluation order." | Declaration order and physical layout never affect evaluation order of field expressions. | They affect identity and bytes but the spec leaves whether they can affect observable side-effect order unstated beyond authored field order. |
"Shared lending may be implicit at a shared-reference parameter under ordinary argument-admission rules." | Only shared-reference parameters allow implicit shared lending at calls. | Other parameters might allow implicit shared lending when admission rules permit; only shared-reference is named. |
"Type/value role resolution happens before choosing that operation." | Role resolution is a fixed phase before static type equality is chosen. | Role resolution and operation choice might interleave when role is determined from context of the same expression. |
"Distinct normalized operand/domain shapes may share a token;" | Sharing a token across shapes is allowed whenever shapes are distinct after normalization. | Sharing requires shapes to remain distinguishable at each use; whether normalization can merge shapes is unstated here. |
"Open-family declaration syntax and dispatch-owner selection syntax remain unsettled; these rules do not approve a spelling." | No open-family syntax is ratified; compilers must reject or defer open families until spelled elsewhere. | Semantic rules for open families apply conceptually; only surface syntax is unsettled, not whether open families exist. |
"A commutative operand flip may be an explicit delegating operator;" | Commutative reversal requires an explicit delegating declaration at source level. | Implementations may flip operands internally if an explicit delegating operator exists somewhere in the family. |
"The examples are alternative borrows; simultaneous overlapping loans still obey ordinary compatibility rules." | The two `let` forms cannot hold simultaneous conflicting loans on the same range. | Examples are mutually exclusive authoring choices; whether both could exist in different scopes at once is unstated. |
"Thus `..=len-1` needs nonzero length," | Inclusive range to last index requires compile-time or proved nonzero length before the range is well-formed. | Needs nonzero length only when `len-1` is used as inclusive end in a specific proof obligation pattern; other `..=len-1` forms unstated. |
"Dynamic extents remain conservative without disjointness evidence." | Loan checking treats dynamic ranges as overlapping unless disjointness is proved. | Only borrow conflict analysis is conservative; other uses of dynamic extents may use sharper facts if available elsewhere. |

## Terms used without definition in this chapter

- selected operation identity
- source evaluation schedule
- semantic evaluation
- realization
- strict binary operator
- machine-entry address query
- static declaration application
- selective form / explicit schedule
- true-path facts / false-path facts
- live-fact identity
- `..` omission marker
- zero establishment
- canonical aggregate identity
- permitted observation contract
- reverse-establishment cleanup / reverse-declaration cleanup
- boundary requirement / qualified static form
- provider slot / admitted backing
- mathematical function term / nested dependent-function parameters
- compiler-known core queries
- runtime machine values / contract refinement
- invariant-window obligations
- terminal call
- cleanup-bearing exit
- token-specific collection adaptation
- static type equality
- common-carrier promotion
- operator binding / fixed token
- normalized signature/overload
- policy forms (Wrapping, Saturating, Trapping)
- cast entries / carrier type
- result-domain selection
- operand telescope
- collection-view adaptations
- proof-static binder / surrounding source telescope
- core identity (in facts)
- executable supply / bodyless machine
- boundary trait / ordinary trait
- closed-catalog realization
- semantic home / declared-domain constraint
- operator families / dispatch-owner / implementation key
- adapter domain
- sealed compiler primitive
- well-formed requirement
- Terminal bounds obligation / Terminal access contract
- bare-range-reference-access
- collection bounds / normalized extents
- disjointness evidence

CHAPTER COMPLETE
