# Invocation contracts and live facts

Source: `wiki/spec/language/state_contracts.md` lines 1–281 (read).

## Rules

rule id | the exact sentence, quoted | the heading it is under | what must be checked, in plain words | when it must hold | lane
--- | --- | --- | --- | --- | ---
STATE_CONTRACTS1 | "`where` is retired from Omega source syntax." | Requirement clauses | Reject source using `where` | not stated | 0
STATE_CONTRACTS2 | "`requires` introduces predicates on data declarations, named domains, generic declarations and machines, and introduces structural or nominal static-callable requirements." | Requirement clauses | Accept `requires` only in those declaration contexts with the stated roles | not stated | SHARED
STATE_CONTRACTS3 | "Assembly bodies carry their contract clauses directly after `asm`, without a grouping keyword." | Requirement clauses | Assembly contracts appear immediately after `asm` with no grouping keyword | not stated | 10
STATE_CONTRACTS4 | "There is no compatibility spelling or second proof system for the retired word." | Requirement clauses | Reject compatibility spellings or alternate proof systems for retired `where` | not stated | 0
STATE_CONTRACTS5 | "Data declaration | The complete value's default-domain validity obligations; construction, consumption and invariant windows retain their existing rules." | Requirement clauses | On data declarations, check complete-value default-domain validity plus existing construction/consumption/invariant window rules | not stated | 1
STATE_CONTRACTS6 | "Named domain | Membership obligations for the exact qualified subject, without changing establishment or access routes." | Requirement clauses | On named domains, check membership for exact qualified subject; do not alter establishment or access routes | not stated | 5
STATE_CONTRACTS7 | "Generic declaration | Obligations on the admitted argument tuple, including static type equations and complete callable contracts." | Requirement clauses | On generic declarations, check obligations on admitted argument tuple, static type equations, and complete callable contracts | not stated | 7
STATE_CONTRACTS8 | "Machine | Entry obligations established at every invocation under exact actual-argument substitution." | Requirement clauses | At every machine invocation, establish entry obligations under exact actual-argument substitution | at a call | 10
STATE_CONTRACTS9 | "Inline assembly or local assembly machine | Incoming-edge assertions over the exact modeled machine state." | Requirement clauses | On inline/local assembly, check incoming-edge assertions over exact modeled machine state | on every edge | 10
STATE_CONTRACTS10 | "Callable assembly machines retain the ordinary machine contract." | Requirement clauses | Callable assembly machines use ordinary machine contract checking | not stated | 10
STATE_CONTRACTS11 | "Data and domain predicates retain their logical `self` subject." | Requirement clauses | Data and domain `requires` predicates use logical `self` subject | not stated | 1
STATE_CONTRACTS12 | "This does not introduce a machine receiver, implicit argument, borrow or authority." | Requirement clauses | Do not treat data/domain `requires` as introducing machine receiver, implicit argument, borrow, or authority | not stated | 2
STATE_CONTRACTS13 | "Generic constraints retain binder kinds, static/runtime distinctions, type-equation inference and specialization rules;" | Requirement clauses | Preserve binder kinds, static/runtime distinctions, type-equation inference, and specialization rules on generic constraints | not stated | 7
STATE_CONTRACTS14 | "an entry predicate does not make an unknown value static." | Requirement clauses | Do not classify an unknown value as static solely because an entry predicate mentions it | not stated | 7
STATE_CONTRACTS15 | "Repeated predicate clauses in one scope conjoin their obligations." | Requirement clauses | Conjoin obligations from repeated predicate clauses in one scope | not stated | 0
STATE_CONTRACTS16 | "`ensures` remains the distinct normal-completion guarantee and proof obligation." | Requirement clauses | Keep `ensures` separate as normal-completion guarantee and proof obligation | at normal return | 0
STATE_CONTRACTS17 | "`requires machine Step(...)` declares a structural callable requirement;" | Requirement clauses | Treat `requires machine Step(...)` as structural callable requirement | not stated | 3
STATE_CONTRACTS18 | "`requires machine Handler satisfies Trait::requirement` declares a nominal one." | Requirement clauses | Treat `requires machine Handler satisfies Trait::requirement` as nominal callable requirement | not stated | 3
STATE_CONTRACTS19 | "They declare originating contracts." | Requirement clauses | Structural and nominal machine requirements declare originating contracts | not stated | 3
STATE_CONTRACTS20 | "A wrapper with an explicitly kinded machine binder may instead infer the exact requirements it forwards" | Requirement clauses | When permitted by generics forwarding rules, infer exact forwarded requirements from explicitly kinded machine binder | not stated | 7
STATE_CONTRACTS21 | "direct invocation shape does not invent a contract." | Requirement clauses | Do not invent a contract from direct invocation shape alone | at a call | 3
STATE_CONTRACTS22 | "A nested callable's own `requires`, `ensures` and operational clauses belong to that callable." | Requirement clauses | Attribute nested callable's `requires`, `ensures`, and operational clauses to that callable | not stated | 3
STATE_CONTRACTS23 | "Its declaration-ending semicolon closes that complete contract before subsequent clauses resume the enclosing declaration." | Requirement clauses | Declaration-ending semicolon closes nested callable contract; subsequent clauses belong to enclosing declaration | not stated | 3
STATE_CONTRACTS24 | "There are no clause-ending semicolons;" | Requirement clauses | Reject semicolons that terminate individual header clauses | not stated | 10
STATE_CONTRACTS25 | "Nested premises are not flattened into the enclosing machine's entry requirements." | Requirement clauses | Do not fold nested callable premises into enclosing machine entry requirements | not stated | 10
STATE_CONTRACTS26 | "Assembly removes the grouping word rather than replacing it with another `requires`: `asm requires ready ...` has one entry requirement, and `asm clobbers none { ... }` needs no predicate clause." | Requirement clauses | Assembly: one entry requirement when `asm requires` present; no predicate required when `asm clobbers none` only | at entry | 10
STATE_CONTRACTS27 | "Body-level `requires`, `ensures` and `clobbers` retain their checking boundaries." | Requirement clauses | Enforce separate checking boundaries for body-level `requires`, `ensures`, and `clobbers` | not stated | 10
STATE_CONTRACTS28 | "Nested `asm machine` blocks carry entry/exit assertions under checked assembly;" | Requirement clauses | Check entry/exit assertions on nested `asm machine` per checked assembly rules | not stated | 10
STATE_CONTRACTS29 | "an enclosing callable machine's postcondition still applies at normal completion." | Requirement clauses | Apply enclosing callable machine postcondition at normal completion of nested assembly | at normal return | 3
STATE_CONTRACTS30 | "No assertion becomes an admission or overrides the instruction catalog." | Requirement clauses | Assembly assertions must not admit instructions or override instruction catalog | not stated | 10
STATE_CONTRACTS31 | "Machine header clauses have no trailing semicolons." | Machine header delimiters | Reject trailing semicolons on machine header clauses | not stated | 10
STATE_CONTRACTS32 | "This applies uniformly to `requires`, `ensures`, `reaches`, `invokes`, `suspends`, `blocks`, `crashes`, `terminates` (including `terminates by`), and other machine header clauses." | Machine header delimiters | Uniform no-trailing-semicolon rule for listed and other machine header clauses | not stated | 10
STATE_CONTRACTS33 | "The next clause keyword or the implementation body delimits a clause; a newline does not change its owner." | Machine header delimiters | Delimit clauses by next keyword or body; newline does not reassign clause ownership | not stated | 10
STATE_CONTRACTS34 | "A checked machine proceeds directly from its last clause to its body." | Machine header delimiters | Checked machine body follows last header clause without intervening clause terminator | not stated | 10
STATE_CONTRACTS35 | "A semicolon before that body does not terminate a clause and then resume the same declaration." | Machine header delimiters | Semicolon immediately before body must not be parsed as clause end plus declaration resume | not stated | 10
STATE_CONTRACTS36 | "A bodyless machine declaration or requirement ends with one semicolon after its complete header." | Machine header delimiters | Bodyless machine/requirement ends with exactly one semicolon after full header | not stated | 10
STATE_CONTRACTS37 | "That terminator belongs to the declaration, not whichever clause happens to come last." | Machine header delimiters | Attribute bodyless terminator to declaration, not to last header clause | not stated | 10
STATE_CONTRACTS38 | "A nested `requires machine` is such a declaration: its semicolon closes the nested contract and subsequent clauses belong to the enclosing declaration." | Machine header delimiters | Nested `requires machine` semicolon closes nested contract; following clauses belong to enclosing declaration | not stated | 10
STATE_CONTRACTS39 | "Here `terminates` belongs to `Step`, and `crashes Trap` belongs to `invoke`." | Machine header delimiters | Assign header clauses to the nested or enclosing machine per declaration nesting (illustrated rule) | not stated | 10
STATE_CONTRACTS40 | "To put a crash ceiling on `Step`, put that clause before its declaration terminator." | Machine header delimiters | Clauses for nested bodyless requirement must appear before that requirement's declaration terminator | not stated | 10
STATE_CONTRACTS41 | "An additional same-level terminator rejects; it is not a compatibility spelling." | Machine header delimiters | Reject extra same-level declaration terminator; not a compatibility spelling | not stated | 10
STATE_CONTRACTS42 | "closing an inner requirement and then its enclosing bodyless requirement legitimately needs two declaration terminators." | Machine header delimiters | Allow two declaration terminators when closing inner then enclosing bodyless requirements | not stated | 10
STATE_CONTRACTS43 | "Neither is a clause terminator." | Machine header delimiters | Declaration terminators must not be treated as clause terminators | not stated | 10
STATE_CONTRACTS44 | "Statement, data-field, and other declaration punctuation is unchanged by this rule." | Machine header delimiters | Do not alter statement, data-field, or other declaration punctuation per machine delimiter rule | not stated | 1
STATE_CONTRACTS45 | "A machine enters its own executable body; there is no separately authored entry member." | Bindings and arrivals | Machine execution starts in its own body; reject separately authored entry member | at entry | 10
STATE_CONTRACTS46 | "Trailing nested declarations scope helper names, not runtime data." | Bindings and arrivals | Trailing nested declarations scope only helper names, not runtime data | not stated | 10
STATE_CONTRACTS47 | "Both ordinary and terminal calls invoke the selected machine with ordinary explicit parameters." | Bindings and arrivals | Invoke machines with ordinary explicit parameters for ordinary and terminal calls | at a call | 3
STATE_CONTRACTS48 | "Context records and their borrows are supplied like other arguments; no receiver or enclosing activation is retained or rebound implicitly." | Bindings and arrivals | Supply context and borrows as explicit arguments; no implicit receiver or rebound enclosing activation | at a call | 2
STATE_CONTRACTS49 | "An ordinary terminal value, explicit return, or permitted implicit Unit completion ends the invocation and owes its declared result/return contract." | Bindings and arrivals | On those completion forms, end invocation and check declared result/return contract | at normal return | 0
STATE_CONTRACTS50 | "A transition explicitly supplies the target machine's arguments." | Bindings and arrivals | Transition must explicitly supply target machine arguments | at the operation | 0
STATE_CONTRACTS51 | "Executable uses name the exact invocation's parameters or preceding local bindings; arrival contracts use the target's parameters." | Bindings and arrivals | Executable uses use invocation parameters or preceding locals; arrival contracts use target formals | not stated | 10
STATE_CONTRACTS52 | "Enclosing and sibling runtime bindings cannot be recovered by matching spelling." | Bindings and arrivals | Reject recovering enclosing/sibling bindings by name spelling alone | not stated | 0
STATE_CONTRACTS53 | "Nominal member and case selection retains the declared owner, including payload projections rooted at an explicit context argument." | Bindings and arrivals | Member/case selection keeps declared owner including context-rooted payload projections | not stated | 1
STATE_CONTRACTS54 | "Machine `requires` applies at every invocation, including terminal and recursive calls." | Bindings and arrivals | Check machine `requires` at every invocation including terminal and recursive | at a call | 10
STATE_CONTRACTS55 | "Every arrival establishes the target's requirements and declared domains under actual argument substitution." | Bindings and arrivals | On every arrival, establish target requirements and domains under actual-argument substitution | on every edge | 10
STATE_CONTRACTS56 | "Nesting does not assume the parent's entry requirements again, import its return guarantees, or supply facts invalidated by mutation." | Bindings and arrivals | Nested arrival must not re-assume parent entry, import parent return guarantees, or use facts invalidated by mutation | at a call | 10
STATE_CONTRACTS57 | "Live caller facts can discharge a nested helper's requirements;" | Bindings and arrivals | Allow discharging nested helper requirements with live caller facts when valid | at a call | 8
STATE_CONTRACTS58 | "body inference must justify any inferred private requirements on every arrival, not grant them from lexical ancestry." | Bindings and arrivals | Justify inferred private requirements on each arrival; do not grant from lexical ancestry alone | at a call | 8
STATE_CONTRACTS59 | "Every invocation retains both sides of its own contract." | Bindings and arrivals | Each invocation keeps both sides of its own contract | at a call | 3
STATE_CONTRACTS60 | "A recursive contract is not its own proof; induction follows the well-founded obligations in termination." | Bindings and arrivals | Reject using recursive contract as self-proof; require well-founded termination obligations for induction | not stated | 4
STATE_CONTRACTS61 | "Arrival substitution is simultaneous and identity-based." | Bindings and arrivals | Perform arrival substitution simultaneously and identity-based | on every edge | 10
STATE_CONTRACTS62 | "Scalar arguments retain their evaluation snapshots; facts about referenced storage must survive writes from later arguments." | Bindings and arrivals | Retain scalar evaluation snapshots; referenced-storage facts must survive later-argument writes | at a call | 2
STATE_CONTRACTS63 | "A same-named formal cannot alias an unrelated caller value." | Bindings and arrivals | Reject same-named formal aliasing unrelated caller value | at a call | 0
STATE_CONTRACTS64 | "Every reachable predecessor participates in a join." | Bindings and arrivals | Include every reachable predecessor in join | on every edge | 8
STATE_CONTRACTS65 | "Unknown input cannot be omitted to preserve a convenient invariant." | Bindings and arrivals | Do not omit unknown input to preserve an invariant | on every edge | 8
STATE_CONTRACTS66 | "A join merges facts; predecessors supplying one value agree on its exact declared domain set under literals and expected types." | Bindings and arrivals | At join, merged facts require agreeing exact declared domain sets per literals/expected types | on every edge | 5
STATE_CONTRACTS67 | "A minimum collection length is not an exact extent, and declaration initializers are not recurring loop invariants." | Bindings and arrivals | Do not treat minimum collection length as exact extent or declaration initializers as loop invariants | not stated | 6
STATE_CONTRACTS68 | "A checked contradiction between valid premises can establish that a particular arrival is impossible." | Unreachable branches | Allow proving arrival impossible from checked contradiction of valid premises | on every edge | 8
STATE_CONTRACTS69 | "Obligations on that arrival may close by constructive contradiction elimination." | Unreachable branches | Close obligations on impossible arrival via constructive contradiction elimination when proved | on every edge | 8
STATE_CONTRACTS70 | "The contradiction must retain exact subjects, storage versions, scope and premise dependencies." | Unreachable branches | Contradiction proof must keep exact subjects, storage versions, scope, premise dependencies | not stated | 8
STATE_CONTRACTS71 | "Failure to prove an obligation is not evidence that its arrival is impossible;" | Unreachable branches | Do not treat unproved obligation as evidence of impossible arrival | not stated | 8
STATE_CONTRACTS72 | "a dead branch cannot supply facts to a reachable predecessor." | Unreachable branches | Dead branch must not supply facts to reachable predecessor | on every edge | 8
STATE_CONTRACTS73 | "Source and operand well-formedness remain required." | Unreachable branches | Require source and operand well-formedness on unreachable branches | not stated | SHARED
STATE_CONTRACTS74 | "By default, issue an unreachable-branch warning when an authored branch contradicts its machine's entry contract or established source facts independent of specialization." | Unreachable branches | Default: emit unreachable-branch warning when branch contradicts entry contract or established facts independent of specialization | not stated | SHARED
STATE_CONTRACTS75 | "The diagnostic identifies the branch condition and the conflicting premises, with their source locations where available." | Unreachable branches | Warning diagnostic names branch condition, conflicting premises, and locations when available | not stated | SHARED
STATE_CONTRACTS76 | "Do not warn merely because ordinary generic specialization or compile-time selection removes an alternative;" | Unreachable branches | Do not warn solely because specialization removes an alternative | not stated | 7
STATE_CONTRACTS77 | "a generic branch that already contradicts its own contract is not exempt." | Unreachable branches | Warn on generic branch that contradicts its own contract even if specialization would remove it | not stated | 7
STATE_CONTRACTS78 | "Proof-internal impossible cases do not each create source warnings." | Unreachable branches | Do not emit source warning for each proof-internal impossible case | not stated | 8
STATE_CONTRACTS79 | "This is an advisory diagnostic, not an inherent language error." | Unreachable branches | Unreachable-branch warning is advisory; not required hard error | not stated | SHARED
STATE_CONTRACTS80 | "Proving the path impossible, pruning its emitted code and issuing a warning are separate decisions." | Unreachable branches | Treat impossibility proof, code pruning, and warning as independent decisions | not stated | SHARED
STATE_CONTRACTS81 | "Proof soundness does not depend on pruning or on warning emission." | Unreachable branches | Soundness must not depend on pruning or warning emission | not stated | 8
STATE_CONTRACTS82 | "Resolve every called target as a machine, nested or otherwise." | Terminal calls in transition arms | Resolve called targets to machines | at a call | 3
STATE_CONTRACTS83 | "A terminal call is an invocation whose normal result completes the current machine, including Unit completion." | Terminal calls in transition arms | Classify terminal call as invocation whose normal result completes current machine | at normal return | 3
STATE_CONTRACTS84 | "Argument admission, result compatibility, machine postconditions, cleanup, crash and no-normal-return behavior are the ordinary call-and-completion rules." | Terminal calls in transition arms | Apply ordinary call rules for admission, compatibility, postconditions, cleanup, crash, no-normal-return | at a call | 3
STATE_CONTRACTS85 | "Only the selected arm executes, with ordinary once-only operand evaluation." | Terminal calls in transition arms | Execute only selected transition arm; evaluate operands once | at the operation | 0
STATE_CONTRACTS86 | "Terminal machine calls accept the same operational acknowledgements as other calls:" | Terminal calls in transition arms | Terminal calls require same operational acknowledgements as non-terminal calls | at a call | 3
STATE_CONTRACTS87 | "This invokes `Storage::read` in the current activation and forwards its normal result; it does not start or detach a task." | Terminal calls in transition arms | Suspend/block terminal call runs in current activation; must not start or detach task | at a call | 3
STATE_CONTRACTS88 | "If the call parks, completion of the enclosing machine remains pending." | Terminal calls in transition arms | When call parks, enclosing machine completion stays pending | at a call | 3
STATE_CONTRACTS89 | "No return, cleanup or storage reclamation occurs merely because the call is syntactically final or becomes suspended." | Terminal calls in transition arms | Do not complete return/cleanup/reclamation solely because call is terminal or suspended | at a call | 2
STATE_CONTRACTS90 | "Ordinary cleanup-bearing completion edges still discharge their obligations." | Terminal calls in transition arms | Discharge obligations on ordinary cleanup-bearing completion edges | on every edge | 2
STATE_CONTRACTS91 | "Tail-call lowering may remove unnecessary control state only while preserving all live backing, loans, custody and completion obligations." | Terminal calls in transition arms | Tail-call lowering must preserve live backing, loans, custody, completion obligations | not stated | 2
STATE_CONTRACTS92 | "Recursive runtime calls still obey the no-accumulated-frames rule." | Terminal calls in transition arms | Enforce no-accumulated-frames on recursive runtime calls | at a call | 4
STATE_CONTRACTS93 | "Nested machine calls have no marker exemption." | Terminal calls in transition arms | Nested machine calls require acknowledgements even in terminal position | at a call | 3
STATE_CONTRACTS94 | "Their checked call envelope determines the required `suspend` and `block` acknowledgements, including in terminal position, even when lowering later implements the call with a jump." | Terminal calls in transition arms | Derive required suspend/block markers from checked call envelope including terminal position | at a call | 3
STATE_CONTRACTS95 | "Markers do not redirect name resolution." | Terminal calls in transition arms | Suspend/block markers must not change name resolution | at a call | 3
STATE_CONTRACTS96 | "Missing or partial acknowledgements reject;" | Terminal calls in transition arms | Reject missing or partial suspend/block acknowledgements | at a call | 3
STATE_CONTRACTS97 | "redundant markers remain permitted under effects." | Terminal calls in transition arms | Permit redundant markers per effects call-site acknowledgements | at a call | 3
STATE_CONTRACTS98 | "The same borrow/carry requirements apply in a terminal arm as at any other suspending call." | Terminal calls in transition arms | Apply borrow/carry across suspension rules in terminal arms same as other suspending calls | at a call | 2
STATE_CONTRACTS99 | "Facts apply to their exact subjects and program points." | Mutation and subject identity | Track facts at exact subjects and program points | at the operation | 8
STATE_CONTRACTS100 | "Assignments first capture their destination place, including its selectors and bounds at that program point." | Mutation and subject identity | Before assignment effects, capture destination place with selectors and bounds at that point | at the operation | 0
STATE_CONTRACTS101 | "They then evaluate the right-hand side and its effects, capture transferable evidence, and finally invalidate overlapping destination facts and establish the new value at the captured place." | Mutation and subject identity | Evaluate RHS, capture evidence, invalidate overlapping destination facts, establish new value at captured place | at the operation | 0
STATE_CONTRACTS102 | "Right-hand-side mutations invalidate their own overlapping facts when they occur;" | Mutation and subject identity | RHS mutations invalidate overlapping facts when they occur | at the operation | 8
STATE_CONTRACTS103 | "the captured selector is not a fact that the selector's live binding still has its earlier value." | Mutation and subject identity | Do not treat captured selector as fact that binding retains pre-assignment value | at the operation | 8
STATE_CONTRACTS104 | "Uninitialized storage has no assumed zero." | Mutation and subject identity | Do not assume zero for uninitialized storage | not stated | 2
STATE_CONTRACTS105 | "Copies transport facts about the captured value, not source expressions to be reevaluated after mutation." | Mutation and subject identity | Copy facts follow captured value, not reevaluated source expressions after mutation | at the operation | 8
STATE_CONTRACTS106 | "A reference identifies live storage, not its former contents." | Mutation and subject identity | Reference subject is live storage, not former contents | not stated | 2
STATE_CONTRACTS107 | "Replacing a local reference binding changes that binding without redirecting aliases made from its previous value." | Mutation and subject identity | Rebinding local reference does not redirect aliases from previous value | at the operation | 2
STATE_CONTRACTS108 | "A write through the reference changes the referent, not the binding." | Mutation and subject identity | Dereference write updates referent, not the reference binding | at the operation | 2
STATE_CONTRACTS109 | "Stored reference leaves retain their captured origins until replacement or exposure invalidates that relation." | Mutation and subject identity | Stored reference leaves keep captured origins until replacement or exposure invalidates | not stated | 2
STATE_CONTRACTS110 | "Type or lifetime correspondence alone does not establish a returned reference's storage origin." | Mutation and subject identity | Reject establishing returned reference storage origin from type/lifetime correspondence alone | at normal return | 2
STATE_CONTRACTS111 | "Caller-visible mutation summaries include operand evaluation and callee writes." | Mutation and subject identity | Mutation summaries visible to caller include operand evaluation and callee writes | at a call | 3
STATE_CONTRACTS112 | "A complete empty summary preserves facts; an unknown summary proves no non-interference." | Mutation and subject identity | Empty complete summary preserves facts; unknown summary proves no non-interference claim | at a call | 8
STATE_CONTRACTS113 | "Fact invalidation uses storage footprints, while borrow authorization uses the access route." | Mutation and subject identity | Invalidate facts by storage footprints; authorize borrows by access route | not stated | 2
STATE_CONTRACTS114 | "Neither may-write summaries nor origin recovery grant loans, permit moves, or discharge linear claims." | Mutation and subject identity | May-write summaries and origin recovery must not grant loans, moves, or discharge linear claims | not stated | 2
STATE_CONTRACTS115 | "Saving an integer from a valid ordinary storage place captures its current value." | Restoring a saved scalar value | Saving integer from valid ordinary place captures current value | at the operation | 5
STATE_CONTRACTS116 | "A later overwrite invalidates facts about that storage, not facts about the saved copy." | Restoring a saved scalar value | Overwrite invalidates storage facts, not saved-copy facts | at the operation | 8
STATE_CONTRACTS117 | "Writing the saved value back establishes a new equality between the current contents of that exact destination and the saved value." | Restoring a saved scalar value | Restore write establishes new equality between exact destination contents and saved value | at the operation | 5
STATE_CONTRACTS118 | "A subsequent guard on the copy may therefore justify a requirement on the restored contents." | Restoring a saved scalar value | Allow guard on saved copy to justify requirement on restored contents when equality established | at the operation | 8
STATE_CONTRACTS119 | "This is new evidence from the restoring write, not resurrection of invalidated storage facts." | Restoring a saved scalar value | Treat as new evidence from restore write, not resurrection of old storage facts | at the operation | 8
STATE_CONTRACTS120 | "Checking must establish the captured place, index identity, saved value and absence of intervening invalidating effects at the use." | Restoring a saved scalar value | At use, prove captured place, index identity, saved value, no intervening invalidating effects | at the operation | 8
STATE_CONTRACTS121 | "A different saved value, changed selector, possible aliasing write or unknown call effect supplies no unproved equality." | Restoring a saved scalar value | Do not grant unproved equality when saved value, selector, aliasing write, or unknown call differs | at the operation | 8
STATE_CONTRACTS122 | "Entry storage may supply an initialized reaching value without an earlier store in the body; no zero value or initialization is inferred for uninitialized storage." | Restoring a saved scalar value | Entry may reach initialized value without body store; do not infer zero/init for uninitialized storage | at entry | 2
STATE_CONTRACTS123 | "Analysis may bound its value/storage tracking and conservatively lose a relation; it then needs fresh evidence for the obligation." | Restoring a saved scalar value | After conservative loss of relation, require fresh evidence for obligation | not stated | 8
STATE_CONTRACTS124 | "This decision neither requires an unlimited write history nor permits weakening a proved relation solely because a write preceded the restoration." | Restoring a saved scalar value | Need not keep unlimited write history; must not weaken proved relation solely because write preceded restoration | not stated | 8
STATE_CONTRACTS125 | "Normal returns owe applicable postconditions for both value and Unit results." | Returns and proof use | Check applicable postconditions on normal returns for value and Unit | at normal return | 0
STATE_CONTRACTS126 | "An implicit return occurs after the executable portion's final statement, not after executing trailing declarations." | Returns and proof use | Implicit return point is after final executable statement, before trailing declarations | at normal return | 0
STATE_CONTRACTS127 | "A selected returning arm uses that arm's facts." | Returns and proof use | Return proof uses facts from selected returning arm | at normal return | 8
STATE_CONTRACTS128 | "A terminal call owes both the callee's result and the caller's completion contract; lexical ancestry adds no further return targets." | Returns and proof use | Terminal call must satisfy callee result and caller completion; no extra return targets from ancestry | at normal return | 3
STATE_CONTRACTS129 | "Crash edges are not normal returns." | Returns and proof use | Do not treat crash edges as normal returns | on every edge | 3
STATE_CONTRACTS130 | "Synthetic `result` denotes the exact returned value; output-reference parameters retain their actual reference origins." | Returns and proof use | `result` is exact returned value; output-reference params keep actual reference origins | at normal return | 0
STATE_CONTRACTS131 | "Exit proof consumes live facts, not the conclusion being proved." | Returns and proof use | Exit proof may use only live facts, not the conclusion under proof | at normal return | 8
STATE_CONTRACTS132 | "Reference-subject transport across calls or loop edges does not transport the truth of an expired entry assumption." | Returns and proof use | Do not transport truth of expired entry assumption with reference-subject transport across call/loop edges | on every edge | 8
STATE_CONTRACTS133 | "Declaration contexts cannot receive a sibling invocation's facts." | Returns and proof use | Reject importing sibling invocation facts into declaration contexts | not stated | 8
STATE_CONTRACTS134 | "Outcome-specific guarantees follow the matching nominal result case and remain separate from unconditional guarantees, as specified in proof availability." | Returns and proof use | Apply outcome-specific guarantees per matching result case; keep separate from unconditional guarantees | at normal return | 8
STATE_CONTRACTS135 | "a successor's checked `ensures` may establish the enclosing machine's completion promise on the exact borrowed subject." | Returns and proof use | Under contract-based repair, successor `ensures` may establish enclosing completion on exact borrowed subject | at normal return | 8
STATE_CONTRACTS136 | "Its `requires` and parameter/default domains must hold before entry; its postconditions become usable only for the matching completed outcome." | Returns and proof use | Repair successor: check requires and domains before entry; postconditions only for matching completed outcome | at entry | 8
STATE_CONTRACTS137 | "The lender's qualification and restoration duty remain attached to the original storage through intermediate transfers." | Returns and proof use | Keep lender qualification and restoration on original storage through transfers | not stated | 2
STATE_CONTRACTS138 | "Neither weakening the displayed view nor composing a future guarantee makes the qualification true during repair." | Returns and proof use | Display weakening or future guarantee composition does not make qualification true during repair | not stated | 8
STATE_CONTRACTS139 | "Existing well-founded recursion rules govern cyclic compositions; a promise cannot prove itself." | Returns and proof use | Apply well-founded recursion to cyclic compositions; reject self-proving promise | not stated | 4
STATE_CONTRACTS140 | "A call used as a total predicate term requires termination and absence of crash routes." | Returns and proof use | Total predicate call requires proved termination and no crash routes | at a call | 4
STATE_CONTRACTS141 | "Equality conditional on normal runtime return does not establish total denotation." | Returns and proof use | Do not infer total denotation from equality conditioned on normal runtime return | not stated | 8
STATE_CONTRACTS142 | "Repeated calls need effect- and observation-free semantics plus exact targets and inputs; an empty write summary alone does not prove repeatability." | Returns and proof use | Repeatable calls require effect/observation-free semantics and exact targets/inputs; empty write summary insufficient | at a call | 3
STATE_CONTRACTS143 | "Source proof search may be incomplete." | Returns and proof use | Compiler need not complete source proof search | not stated | 8
STATE_CONTRACTS144 | "Failure to infer an invariant, normalize a term, or prove a supported implication grants no fact and is not a refutation." | Returns and proof use | On proof failure, grant no fact and do not treat as refutation | not stated | 8

## Ambiguous sentences

Sentence | Reading A | Reading B
--- | --- | ---
"These are source-language obligations, not a list of patterns recognized by the Rust compiler." | This chapter defines Omega source obligations only; Rust reference implementation matching is out of scope here. | Rust compiler may still implement overlapping checks, but this text does not enumerate Rust patterns. |
"A wrapper with an explicitly kinded machine binder may instead infer the exact requirements it forwards" | Inference is optional when forwarding rules apply; explicit requirements may still be required when inference is absent or fails. | Wrapper must use inference whenever a kinded machine binder is present; explicit `requires machine` on the wrapper is never needed. |
"Live caller facts can discharge a nested helper's requirements;" | Any live caller fact that proves the nested requirement may discharge it. | Only caller facts that are still valid at the nested call site after accounting for invalidation may discharge requirements. |
"By default, issue an unreachable-branch warning" | Warning emission is the default diagnostic policy but may be disabled by tooling configuration not stated here. | Warning is always emitted unless the contradiction is proof-internal only. |
"This is an advisory diagnostic, not an inherent language error." | Programs with unreachable branches may compile successfully while emitting the warning. | Programs may compile without the warning even when a branch is unreachable if warning is not issued. |
"If the call parks, completion of the enclosing machine remains pending." | Enclosing machine has no normal return until the parked call completes. | Other parts of the enclosing machine may still make progress while completion obligation is deferred. |
"Tail-call lowering may remove unnecessary control state only while preserving all live backing, loans, custody and completion obligations." | Tail-call optimization is permitted when listed obligations are preserved. | Tail-call lowering is mandatory whenever control state can be removed while preserving obligations. |
"A complete empty summary preserves facts; an unknown summary proves no non-interference." | Unknown summary means caller cannot assume non-interference from callee. | Unknown summary is a hard error unless caller proves non-interference another way. |
"Entry storage may supply an initialized reaching value without an earlier store in the body;" | Parameters or entry facts may initialize storage reachably without body store. | Only machine entry bindings count as entry storage for this rule; other contexts unstated. |
"For contract-based repair, a successor's checked `ensures` may establish the enclosing machine's completion promise on the exact borrowed subject." | Establishing enclosing completion via successor ensures is optional when repair pattern matches. | Successor must use `ensures` this way whenever contract-based repair is used. |
"Source proof search may be incomplete." | Compiler may stop proof search early without rejecting the program. | Incomplete search must never reject programs that would be accepted with complete search. |

## Terms used without definition in this chapter

- source-language obligations
- mathematical proof contracts / theorem meaning / assumptions / bundles / evidence
- Terminal fact validity / emitted product obligations
- requires-clause-unification
- structural or nominal static-callable requirement
- default-domain validity obligations / construction, consumption and invariant windows
- establishment or access routes
- admitted argument tuple / static type equations / complete callable contracts
- actual-argument substitution
- incoming-edge assertions / modeled machine state
- logical `self` subject
- binder kinds / static/runtime distinctions / type-equation inference / specialization rules
- originating contracts / forwarded-callback-requirements
- operational clauses / header delimiters
- checked assembly / instruction catalog / clobbers
- machine-clause-delimiters
- `reaches`, `invokes`, `suspends`, `blocks`, `crashes`, `terminates by`
- checked machine / bodyless machine declaration
- nested callable families
- executable body / entry member
- nested machines / visibility / declaration order / flattening
- ordinary terminal value / permitted implicit Unit completion
- transition / arrival contracts
- nominal member and case selection / payload projections / context argument
- lexical ancestry / private requirements
- well-founded obligations / termination
- evaluation snapshots
- join / declared domain set / literals and expected types
- minimum collection length / declaration initializers / loop invariants
- scoped-contradiction-elimination / contradiction elimination / classicality
- valid premises / storage versions / premise dependencies
- unreachable-branch warning / specialization / compile-time selection
- proof-internal impossible cases / pruning
- terminal call / Unit completion
- operational acknowledgements / suspend block
- parks / enclosing activation / detach a task
- cleanup-bearing completion edges
- tail-call lowering / live backing / custody
- no-accumulated-frames rule
- checked call envelope / markers / name resolution / effects call-site acknowledgements
- borrow/carry requirements / borrows across suspension
- destination place / selectors and bounds / evaluation schedule
- transferable evidence / overlapping destination facts
- uninitialized storage / assumed zero
- live storage / referent / binding
- stored reference leaves / captured origins / exposure
- mutation summaries / may-write summaries / origin recovery / access route / linear claims
- restored-scalar-value-facts / Terminal fact rules / restored scalar values
- index identity / intervening invalidating effects / entry storage
- unlimited write history
- implicit return / executable portion / trailing declarations
- crash edges / synthetic `result` / output-reference parameters
- exit proof / entry assumption
- declaration contexts / sibling invocation
- outcome-specific guarantees / unconditional guarantees / identity availability and erasure
- contract-based repair / dependent values
- borrowed subject / parameter/default domains / matching completed outcome
- lender's qualification / restoration duty / displayed view
- cyclic compositions / total predicate term / total denotation
- effect- and observation-free semantics / empty write summary / repeatability
- supported implication / refutation

CHAPTER COMPLETE
