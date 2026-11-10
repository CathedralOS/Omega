# Checked assembly

Source: `wiki/spec/language/assembly.md` lines 1–422 (read).

## Rules

| rule id | the exact sentence, quoted | the heading it is under | what must be checked, in plain words | when it must hold | lane |
| --- | --- | --- | --- | --- | --- |
| ASSEMBLY1 | "Assembly machines have an explicit assembly body language and ordinary checked machine contracts." | Checked assembly | Assembly machines must use the assembly body language and must carry ordinary checked machine contracts | not stated | 10 |
| ASSEMBLY2 | "Nested assembly machines name the basic blocks of one checked control-flow graph." | Checked assembly | Nested `asm machine` declarations must correspond to basic blocks in a single checked CFG | not stated | 10 |
| ASSEMBLY3 | "colon labels and implicit inter-block fall-through are retired source forms." | Checked assembly | Colon labels and implicit inter-block fall-through must be rejected | not stated | 10 |
| ASSEMBLY4 | "Assembly has no weaker, asm-only proof system." | Checked assembly | Assembly must use the same checked proof obligations as non-assembly code, not a weaker asm-only system | not stated | 8 |
| ASSEMBLY5 | "`asm machine` declares an assembly-bodied machine." | Checked assembly | Assembly-bodied machines must be declared with `asm machine` | not stated | 10 |
| ASSEMBLY6 | "`asm { ... }` embeds a checked assembly region in ordinary Omega code." | Checked assembly | Inline assembly regions must use `asm { ... }` and remain checked | not stated | 10 |
| ASSEMBLY7 | "Both are parsed for the selected target, not treated as opaque text or bytes." | Checked assembly | `asm machine` and `asm { }` must be parsed for the selected target, not as opaque text or bytes | not stated | 9 |
| ASSEMBLY8 | "Every accepted instruction has a compiler-owned contract;" | Checked assembly | Each accepted instruction must have a compiler-owned catalog contract | not stated | 10 |
| ASSEMBLY9 | "unknown instructions and raw emitted bytes reject." | Checked assembly | Unknown instructions and raw emitted bytes must be rejected | not stated | 10 |
| ASSEMBLY10 | "A prebuilt blob is a foreign provider artifact under admission, not inline assembly." | Checked assembly | Prebuilt blobs must be admitted as foreign provider artifacts, not as inline assembly | not stated | 9 |
| ASSEMBLY11 | "There is no compatibility `ForeignBinding::Instruction` source form." | Checked assembly | Programs must not use a `ForeignBinding::Instruction` compatibility source form | not stated | 9 |
| ASSEMBLY12 | "supported instructions execute their catalog-defined semantics against ordinary values/storage and an explicitly supplied target environment." | Checked assembly | Supported instructions must be modeled with catalog semantics over ordinary values/storage and explicit target environment | not stated | 10 |
| ASSEMBLY13 | "Neither an alternate body nor a wrapper per block is required." | Checked assembly | Interpretation must not require an alternate body or per-block wrapper to execute catalog semantics | not stated | 10 |
| ASSEMBLY14 | "This grants no execution support to an unmodeled instruction." | Checked assembly | Unmodeled instructions must have no interpreted execution support | not stated | 10 |
| ASSEMBLY15 | "Every assembly-bodied declaration spells `asm machine`, including a declaration nested inside another assembly machine." | Machine bodies and checked entry | Every assembly-bodied declaration, including nested ones, must spell `asm machine` | not stated | 10 |
| ASSEMBLY16 | "An unmarked `machine` does not silently switch body language because of its parent." | Machine bodies and checked entry | An unmarked `machine` nested under assembly must not switch to assembly body language | not stated | 10 |
| ASSEMBLY17 | "Executable Omega statements are not mixed into the instruction stream;" | Machine bodies and checked entry | Executable Omega statements must not appear inside the instruction stream | not stated | 10 |
| ASSEMBLY18 | "the structural `transition` form below and machine contracts are checked control-flow metadata." | Machine bodies and checked entry | `transition` forms and machine contracts must be checked as control-flow metadata, not as mixed executable statements | not stated | 10 |
| ASSEMBLY19 | "An assembly machine can implement an ordinary machine requirement, including a member of a named implementation." | Machine bodies and checked entry | An `asm machine` may satisfy an ordinary machine requirement or named implementation member | not stated | 10 |
| ASSEMBLY20 | "The body must satisfy the requirement's full signature, propositions, ownership, crash, reach, termination and applicable calling/state plan." | Machine bodies and checked entry | Assembly body must satisfy full requirement signature, propositions, ownership, crash, reach, termination, and applicable calling/state plans | not stated | SHARED |
| ASSEMBLY21 | "`asm` selects a body language, not an unchecked provider or a distinct satisfaction relation." | Machine bodies and checked entry | `asm` must only select body language; it must not opt into unchecked provider or alternate satisfaction | not stated | 10 |
| ASSEMBLY22 | "Within one assembly region, nested assembly machines are local continuations, not fresh ABI entries." | Machine bodies and checked entry | Nested assembly machines in one region must be local continuations, not new ABI entries | not stated | 10 |
| ASSEMBLY23 | "Entering one through a local edge neither allocates a frame nor repeats argument setup, register initialization or a prologue." | Machine bodies and checked entry | Local-edge entry into nested block must not allocate frame or repeat argument setup, register init, or prologue | on every edge | 10 |
| ASSEMBLY24 | "Nesting scopes names, not runtime storage or authority." | Machine bodies and checked entry | Nesting must affect names only, not imply extra runtime storage or authority | not stated | 10 |
| ASSEMBLY25 | "Separately callable entries still owe the ordinary explicit-argument and calling-plan rules." | Machine bodies and checked entry | Separately callable assembly entries must meet explicit-argument and calling-plan rules | at a call | 3 |
| ASSEMBLY26 | "Each assembly machine's executable body is one basic block." | Explicit control flow | Each `asm machine` body must be exactly one basic block | not stated | 10 |
| ASSEMBLY27 | "Nested declarations do not execute merely because the preceding source ends." | Explicit control flow | Nested `asm machine` declarations must not execute solely due to preceding source ending | not stated | 10 |
| ASSEMBLY28 | "Instructions use `;` separators." | Explicit control flow | Instructions must be separated with `;` | not stated | 10 |
| ASSEMBLY29 | "`transition -> next;`" (table: Checked transfer to the physically adjacent local block, with no emitted transfer instruction.) | Explicit control flow | `transition -> next` must denote checked transfer to physically adjacent block with no emitted transfer instruction | at the operation | 10 |
| ASSEMBLY30 | "`jmp target;`" (table: Actual direct branch to a visible local continuation, including sibling edges and backedges.) | Explicit control flow | `jmp` must branch to a visible local continuation including siblings and backedges | at the operation | 10 |
| ASSEMBLY31 | "Conditional jump followed by `transition -> next;`" (table: One conditional terminator: the instruction names the taken edge; the transition names the adjacent not-taken edge.) | Explicit control flow | Conditional jump plus following `transition -> next` must form one terminator naming taken and adjacent not-taken edges | at the operation | 10 |
| ASSEMBLY32 | "A catalog-defined return or other exit" (table: Checked completion under the enclosing entry/exit plan, or the exact contracted environment exit.) | Explicit control flow | Catalog return or exit must complete under enclosing entry/exit plan or contracted environment exit | at normal return | 10 |
| ASSEMBLY33 | "A conditional jump is the final native instruction of its block." | Explicit control flow | Conditional jump must be the last native instruction in its block | at the operation | 10 |
| ASSEMBLY34 | "Its immediately following `transition` completes that terminator;" | Explicit control flow | The instruction immediately after a conditional jump must be the completing `transition` | at the operation | 10 |
| ASSEMBLY35 | "no other executable statement may intervene or follow." | Explicit control flow | No executable statement may appear between conditional jump and its `transition`, or after that pair except as allowed by other terminator rules | at the operation | 10 |
| ASSEMBLY36 | "An unconditional transfer or return terminates immediately." | Explicit control flow | Unconditional transfer or return must terminate the block immediately | at the operation | 10 |
| ASSEMBLY37 | "There is no `else` instruction suffix or implicit missing successor." | Explicit control flow | Programs must not use `else` instruction suffix or rely on implicit missing successor | not stated | 10 |
| ASSEMBLY38 | "The initial transition selects the instruction entry after one-time setup;" | Explicit control flow | Initial `transition` must select instruction entry point after one-time setup | at entry | 10 |
| ASSEMBLY39 | "it does not require a trampoline." | Explicit control flow | Initial transition must not require a trampoline | at entry | 10 |
| ASSEMBLY40 | "A later jump to `check` skips that setup." | Explicit control flow | Later `jmp` to a block must skip one-time setup applicable only to initial transition | on every edge | 10 |
| ASSEMBLY41 | "Transitions may connect a parent to its first nested block or a block to its next laid-out sibling." | Explicit control flow | `transition` edges must only connect parent to first nested block or block to next laid-out sibling | on every edge | 10 |
| ASSEMBLY42 | "Lexical nesting alone does not establish adjacency." | Explicit control flow | Adjacency for `transition` must not be inferred from lexical nesting alone | not stated | 10 |
| ASSEMBLY43 | "An intervening instruction range, incompatible adjacency constraints or an unrealizable layout rejects;" | Explicit control flow | Intervening instructions, incompatible adjacency, or unrealizable layout must reject | not stated | 10 |
| ASSEMBLY44 | "the compiler cannot silently turn `transition` into a jump." | Explicit control flow | Compiler must not lower `transition` to `jmp` when source requires zero-instruction adjacent transfer | not stated | 10 |
| ASSEMBLY45 | "No register moves, prologue or other hidden setup may be inserted on that edge." | Explicit control flow | `transition` edges must have no inserted register moves, prologue, or hidden setup | on every edge | 10 |
| ASSEMBLY46 | "An actual `jmp` may be relaxed or elided where its semantics permit;" | Explicit control flow | `jmp` may be optimized only where catalog semantics still hold | not stated | 10 |
| ASSEMBLY47 | "that does not weaken the source transition's zero-instruction guarantee." | Explicit control flow | `jmp` elision/relaxation must not weaken `transition`'s zero-instruction guarantee | not stated | 10 |
| ASSEMBLY48 | "An inline `asm` region with no explicit control transfer may complete at its checked body end and resume ordinary Omega code." | Explicit control flow | Inline `asm` without explicit transfer may fall through to checked body end and resume surrounding code | at normal return | 10 |
| ASSEMBLY49 | "That exit is not implicit entry into a nested declaration." | Explicit control flow | Fall-through exit from inline `asm` must not implicitly enter a nested `asm machine` | at normal return | 10 |
| ASSEMBLY50 | "An assembly machine must complete through its checked exit rather than accidentally running past its body." | Explicit control flow | `asm machine` must complete only via checked exit, not fall-through past body | at normal return | 10 |
| ASSEMBLY51 | "A callable machine retains its ordinary contract over the complete invocation: its `ensures` is proved at normal completion, not at an initial transfer into its CFG." | Contracts and clobbers | Callable assembly machine `ensures` must be proved at normal completion, not at initial CFG transfer | at normal return | 3 |
| ASSEMBLY52 | "Every incoming edge proves the destination block's `requires` against its exact register, flag, memory, place, value, authority and ownership state." | Contracts and clobbers | Each incoming CFG edge must prove destination block `requires` against exact register, flag, memory, place, value, authority, and ownership | on every edge | 2 |
| ASSEMBLY53 | "Each outgoing edge proves that local block's `ensures` after its own instructions and before executing its successor." | Contracts and clobbers | Each outgoing edge must prove block `ensures` after block instructions and before successor | on every edge | 2 |
| ASSEMBLY54 | "Those assertions do not recursively execute a successor's body." | Contracts and clobbers | Edge proofs must not require executing successor bodies | on every edge | 8 |
| ASSEMBLY55 | "A loop backedge still owes the enclosing machine's ordinary ranked-decrease obligation when termination is required." | Contracts and clobbers | Loop backedges must discharge enclosing machine ranked-decrease when termination is required | on every edge | 4 |
| ASSEMBLY56 | "The enclosing checked control point proves the initial entry obligations." | Contracts and clobbers | Enclosing checked control point must prove initial entry obligations | at entry | 8 |
| ASSEMBLY57 | "Lexical nesting imports neither unproved parent predicates nor stale facts." | Contracts and clobbers | Nested blocks must not inherit unproved parent predicates or stale facts from lexical nesting | at entry | 8 |
| ASSEMBLY58 | "Instruction requirements generate proof goals; an instruction's presence does not establish its own premises." | Contracts and clobbers | Instruction catalog premises must be proved separately; emitting an instruction does not establish its premises | at the operation | 8 |
| ASSEMBLY59 | "every actual incoming edge must justify any inferred requirement." | Contracts and clobbers | Every incoming edge must justify any inferred block `requires` | on every edge | 8 |
| ASSEMBLY60 | "Public contracts cannot silently strengthen from one implementation's body." | Contracts and clobbers | Public contracts must not strengthen silently based on one implementation body | not stated | 8 |
| ASSEMBLY61 | "Block propositions observe the symbolic assembly state; they emit no instruction and grant no runtime access." | Contracts and clobbers | Block propositions must be symbolic only; they must emit no instructions and grant no runtime access | not stated | 8 |
| ASSEMBLY62 | "A modeled memory proposition such as `[eax] == 1` is a proof query, while a load or store separately owes access, provenance, alignment and authority." | Contracts and clobbers | Memory propositions are proof-only; loads and stores must separately prove access, provenance, alignment, and authority | at the operation | 2 |
| ASSEMBLY63 | "An ambiguous width, address space or backing rejects." | Contracts and clobbers | Ambiguous width, address space, or backing in memory propositions must reject | not stated | 2 |
| ASSEMBLY64 | "These are checked assertions, not admissions or extra facts." | Contracts and clobbers | Assembly `requires`/`ensures` must be checked assertions, not admissions or unconstrained extra facts | not stated | 8 |
| ASSEMBLY65 | "Body-level inline `ensures` checks the ordinary body exit; a named block's `ensures` checks each outgoing edge, including a jump." | Contracts and clobbers | Inline body `ensures` applies at body exit; named block `ensures` applies on each outgoing edge including jumps | on every edge | 8 |
| ASSEMBLY66 | "Neither overrides the instruction catalog." | Contracts and clobbers | Contract clauses must not override instruction catalog requirements | not stated | 10 |
| ASSEMBLY67 | "Here `context` is an explicit ordinary value in scope, not an implicit receiver." | Contracts and clobbers | Names in inline `asm` contracts such as `context` must refer to explicit values in scope, not an implicit receiver | not stated | 1 |
| ASSEMBLY68 | "An authored clobber set must equal the union of the realized instruction contracts' changed registers." | Contracts and clobbers | Authored `clobbers` must exactly equal union of registers changed per realized instruction contracts | not stated | 10 |
| ASSEMBLY69 | "Omitted changes and invented changes both reject;" | Contracts and clobbers | Clobber sets with omitted or invented register changes must reject | not stated | 10 |
| ASSEMBLY70 | "order and duplicates have no meaning." | Contracts and clobbers | Clobber set order and duplicates must be ignored for checking equality | not stated | 10 |
| ASSEMBLY71 | "An explicitly empty set is `clobbers none`." | Contracts and clobbers | Empty clobber annotation must use `clobbers none` | not stated | 10 |
| ASSEMBLY72 | "Omitting the clause never omits the catalog's effects." | Contracts and clobbers | Missing `clobbers` clause must still account for catalog register effects | not stated | 10 |
| ASSEMBLY73 | "For a nested block, the set covers its own instructions up to outgoing edges, not the union of every reachable successor." | Contracts and clobbers | Nested block clobbers must cover only that block's instructions through outgoing edges | not stated | 10 |
| ASSEMBLY74 | "The enclosing region accounts for the complete graph and one-time setup." | Contracts and clobbers | Enclosing assembly region must account for full CFG and one-time setup effects | not stated | 10 |
| ASSEMBLY75 | "Implicit instruction writes, flag effects and overlapping register views remain authoritative even when not spelled as destinations." | Contracts and clobbers | Implicit writes, flag effects, and overlapping register views from catalog must remain enforced | at the operation | 10 |
| ASSEMBLY76 | "Declaring a clobber neither initializes a register nor saves it." | Contracts and clobbers | `clobbers` must not be treated as register initialization or save | not stated | 10 |
| ASSEMBLY77 | "Writing then restoring a register still contributes a write footprint; proving ABI preservation of its entry value is a separate obligation." | Contracts and clobbers | Write-then-restore still counts as write footprint; ABI entry-value preservation needs separate proof | at the operation | 2 |
| ASSEMBLY78 | "The right sides are checked, pure entry expressions over explicit inputs and authorized projections, not arbitrary executable Omega bodies." | Register initialization and calling plans | `registers` clause right-hand sides must be checked pure entry expressions, not arbitrary executable bodies | at entry | 0 |
| ASSEMBLY79 | "All right sides refer to the incoming values;" | Register initialization and calling plans | Register initializer expressions must refer only to incoming values | at entry | 0 |
| ASSEMBLY80 | "initialization preserves that meaning when source and destination registers overlap." | Register initialization and calling plans | Overlapping source and destination in `registers` init must preserve incoming value semantics | at entry | 10 |
| ASSEMBLY81 | "The compiler must not sequentially destroy an input needed by a later binding." | Register initialization and calling plans | Register binding order must not destroy inputs needed by later bindings | at entry | 10 |
| ASSEMBLY82 | "Boolean conditions bind their target register representation, retaining the relationship to their propositions for subsequent branch checking." | Register initialization and calling plans | Boolean register bindings must retain proposition relationships for branch checking | at entry | 8 |
| ASSEMBLY83 | "Bindings are initialization, not a declaration of where the caller supplied arguments." | Register initialization and calling plans | `registers` bindings are initialization only and must not redefine native argument placement | at entry | 3 |
| ASSEMBLY84 | "they must prove indexing, address formation and other partial-expression obligations before the body begins." | Register initialization and calling plans | Initializer side effects must prove indexing, address formation, and partial-expression obligations before body entry | at entry | 0 |
| ASSEMBLY85 | "Nested CFG continuations instead receive the exact incoming register state; there is no automatic rebinding on an edge." | Register initialization and calling plans | Nested CFG edges must pass exact incoming register state without automatic rebinding | on every edge | 10 |
| ASSEMBLY86 | "Proof assertions may name the region's explicit input identities while their relationships remain live; this does not capture an enclosing Omega activation." | Register initialization and calling plans | Proof assertions may reference input identities while live but must not capture enclosing Omega activation | not stated | 8 |
| ASSEMBLY87 | "boundary requirements select `Calling<Policy>`, whose validated plan governs both callers and checked assembly implementations." | Register initialization and calling plans | Boundary assembly must use validated `Calling<Policy>` plan governing callers and implementations | at a call | 3 |
| ASSEMBLY88 | "Typed symbols remain available as operands under their ordinary value/place rules;" | Register initialization and calling plans | Typed symbols in assembly must obey ordinary value and place rules | at the operation | 1 |
| ASSEMBLY89 | "an input name is not an immutable promise that a particular physical register still contains that input after a write." | Register initialization and calling plans | Input names must not be assumed to remain in fixed physical registers after writes | after writes | 2 |
| ASSEMBLY90 | "Explicit working-register choices must respect instruction and state constraints." | Register initialization and calling plans | Working-register choices must satisfy instruction and machine-state constraints | at the operation | 10 |
| ASSEMBLY91 | "There is no separate `outputs result = register` clause." | Register initialization and calling plans | Assembly signatures must not use `outputs result = register` | not stated | 10 |
| ASSEMBLY92 | "The signature defines the logical result, and the calling plan defines all its physical locations." | Register initialization and calling plans | Logical results come from signature; physical result locations come from calling plan | not stated | 3 |
| ASSEMBLY93 | "At a checked `ret`, the implementation proves initialization and validity of every result component, postconditions, ownership dispositions, stack state and required preservation." | Register initialization and calling plans | Checked `ret` must prove all result components valid, postconditions, ownership dispositions, stack state, and required preservation | at normal return | 2 |
| ASSEMBLY94 | "Authorized mutations through parameters remain separate effects." | Register initialization and calling plans | Parameter mutations authorized by signature must be tracked as separate effects | at the operation | 2 |
| ASSEMBLY95 | "The policy does not create access authority, and an arbitrary integer in `r9` proves nothing." | Register initialization and calling plans | Calling-plan register placement must not confer access authority; arbitrary register integers prove no storage facts | at the operation | 2 |
| ASSEMBLY96 | "Initialization and return of the destination pointer in `rax` must satisfy the same plan." | Register initialization and calling plans | Result pointer initialization and return in `rax` (or plan-selected location) must satisfy calling plan | at normal return | 3 |
| ASSEMBLY97 | "Exact-control entries must account for their complete validated entry, stack, restoration and exit contract; no uncontracted prologue, spill or epilogue is permitted." | Register initialization and calling plans | Exact-control entries must include full validated entry/stack/restoration/exit contract with no uncontracted prologue, spill, or epilogue | at entry | 3 |
| ASSEMBLY98 | "An inline region does not acquire control of its enclosing machine's ABI merely by using physical registers." | Register initialization and calling plans | Inline `asm` using physical registers must not acquire enclosing machine's ABI control | not stated | 3 |
| ASSEMBLY99 | "Assembly machines use ordinary type, value, const, static-machine and named implementation binders." | Generics, slices and tracked storage | Assembly machines must use ordinary binders for type, value, const, static-machine, and named implementation | not stated | 7 |
| ASSEMBLY100 | "Nested declarations retain exact enclosing static applications, not runtime closures." | Generics, slices and tracked storage | Nested assembly must retain exact enclosing static applications, not runtime closures | not stated | 7 |
| ASSEMBLY101 | "Every admitted specialization must justify its instructions, operand widths, layouts, multiplicity and target features." | Generics, slices and tracked storage | Each admitted specialization must justify instructions, operand widths, layouts, multiplicity, and target features | not stated | 7 |
| ASSEMBLY102 | "Equal byte size alone does not authorize copying or relocating arbitrary `T`." | Generics, slices and tracked storage | Equal byte size must not alone authorize copy or relocate of arbitrary `T` | at the operation | 2 |
| ASSEMBLY103 | "An assembly machine may satisfy a generic machine parameter under the same contract as an ordinary machine;" | Generics, slices and tracked storage | Generic machine parameters may be satisfied by assembly under same contract as ordinary machines | not stated | 7 |
| ASSEMBLY104 | "That composition neither injects source instructions nor guarantees inlining or compile-time unrolling." | Generics, slices and tracked storage | Generic assembly composition must not inject instructions or guarantee inlining/unrolling | not stated | 7 |
| ASSEMBLY105 | "Ordinary `&[T]` and `&mut [T]` parameters retain length, backing, lifetime and access obligations in assembly." | Generics, slices and tracked storage | Slice parameters in assembly must retain length, backing, lifetime, and access obligations | not stated | 6 |
| ASSEMBLY106 | "A native boundary policy must explicitly admit their foreign shape or use a checked pointer/length adapter;" | Generics, slices and tracked storage | Native boundary assembly with slices must use explicitly admitted foreign shape or checked pointer/length adapter | at a call | 3 |
| ASSEMBLY107 | "that ABI restriction does not prohibit slices under Omega's internal convention." | Generics, slices and tracked storage | Internal-convention assembly may use slices despite native boundary restrictions | not stated | 6 |
| ASSEMBLY108 | "Each access still proves its current extent, offset, initialization, alignment and permission." | Generics, slices and tracked storage | Each storage access in assembly must prove extent, offset, initialization, alignment, and permission | at the operation | 2 |
| ASSEMBLY109 | "Clobbering a pointer, losing the relation at a join, changing the backing or ending its loan invalidates the affected evidence." | Generics, slices and tracked storage | Pointer clobber, lost join relation, backing change, or ended loan must invalidate dependent evidence | at the operation | 2 |
| ASSEMBLY110 | "Neither numeric address exposure nor tracking metadata creates a loan or extends one." | Generics, slices and tracked storage | Address exposure and tracking metadata must not create or extend loans | not stated | 2 |
| ASSEMBLY111 | "A body has no implicit general-register or scratch-flag inputs from surrounding compiled code." | Assembly state and explicit communication | Assembly body must not take implicit GPR or scratch-flag inputs from surrounding code | at entry | 10 |
| ASSEMBLY112 | "Such inputs must come from its declared operands or a catalog-defined state observation." | Assembly state and explicit communication | GPR/flag inputs must come from declared operands or catalog state observation | at entry | 10 |
| ASSEMBLY113 | "Within one body, register and flag versions flow through explicit machine edges and are part of each block contract;" | Assembly state and explicit communication | Register and flag versions must flow only through explicit edges and appear in block contracts | on every edge | 10 |
| ASSEMBLY114 | "separate `asm` statements share no hidden scratch state." | Assembly state and explicit communication | Separate `asm` statements must not share hidden scratch register or flag state | not stated | 10 |
| ASSEMBLY115 | "Register allocation and incidental preservation are not source contracts." | Assembly state and explicit communication | Register allocation and incidental preservation must not appear as source-level contracts | not stated | 10 |
| ASSEMBLY116 | "Clobbers account for changes; they do not supply initial values." | Assembly state and explicit communication | Clobbers document changes only; they must not supply initial register values | not stated | 10 |
| ASSEMBLY117 | "Communication between separate bodies uses ordinary Omega values/places, authorized memory effects, or explicitly contracted environment state." | Assembly state and explicit communication | Inter-body communication must use Omega values/places, authorized memory, or contracted environment state | not stated | 2 |
| ASSEMBLY118 | "Here `saved` is an ordinary typed place, not a retained register." | Assembly state and explicit communication | Values moved between `asm` regions via Omega names must be ordinary typed places, not retained registers | not stated | 2 |
| ASSEMBLY119 | "interrupt-mask changes, device writes and memory updates do not reset when a label or body exits." | Assembly state and explicit communication | Persistent environment effects must survive block/label exit | across a pause | 10 |
| ASSEMBLY120 | "System registers and architectural control flags explicitly read or changed by catalog operations belong to that persistent state." | Assembly state and explicit communication | Catalog-read/changed system registers and architectural flags belong to persistent environment state | at the operation | 10 |
| ASSEMBLY121 | "Labels and bodies are not transactions, isolation domains, or rollback points." | Assembly state and explicit communication | Assembly labels and bodies must not be treated as transactions, isolation, or rollback | not stated | 10 |
| ASSEMBLY122 | "Exact operand constraints do not silently narrow wider integer places." | Instruction obligations | Instruction operand constraints must not silently narrow wider integer places | at the operation | 5 |
| ASSEMBLY123 | "Each requirement must be discharged from checked facts and held authority or through the permitted admission route." | Instruction obligations | Every instruction catalog requirement must be discharged from checked facts, authority, or permitted admission | at the operation | 8 |
| ASSEMBLY124 | "Moving an instruction into a helper cannot remove its requirements." | Instruction obligations | Refactoring instructions into helpers must retain all catalog requirements | not stated | 8 |
| ASSEMBLY125 | "Direct assembly and a boundary operation for the same mechanism contribute the same normalized reach and authority demand." | Instruction obligations | Assembly and boundary operations for same mechanism must contribute same normalized reach and authority | not stated | 3 |
| ASSEMBLY126 | "Assembly does not automatically make its enclosing machine a provider." | Instruction obligations | Enclosing machine must not automatically become a provider solely because it contains assembly | not stated | 9 |
| ASSEMBLY127 | "Admission never leaves an emitted obligation open." | Instruction obligations | Admitted evidence must close all emitted obligations | not stated | 8 |
| ASSEMBLY128 | "A checked machine that implements a consuming boundary-trait requirement may terminally dispose of a linear argument only when the assembly CFG proves the requirement's effect and every normal exit carries the corresponding ownership disposition." | Instruction obligations | Terminal linear disposal requires CFG proof of requirement effect and ownership disposition on every normal exit | at normal return | 2 |
| ASSEMBLY129 | "A `nop`, a write to the wrong modeled place, missing ordering or a path bypassing the effect leaves the value live and rejects." | Instruction obligations | Incomplete effect paths (`nop`, wrong place, missing ordering, bypass) must leave linear value live and reject | not stated | 2 |
| ASSEMBLY130 | "No native binding or assembly-specific trust assertion substitutes for this proof." | Instruction obligations | Native bindings and asm-specific trust assertions must not substitute for ownership-effect proof | not stated | 8 |
| ASSEMBLY131 | "It cannot exempt an assembly block from the instruction catalog, reinterpret an assembly contract as an assumption, admit raw emitted bytes, or suppress instruction obligations." | Instruction obligations | Assumed binding contracts must not exempt assembly from catalog, reinterpret contracts, admit raw bytes, or suppress instruction obligations | not stated | 8 |
| ASSEMBLY132 | "The checked body consumes authority under its own contract, just like non-assembly code;" | Instruction obligations | Assembly body must consume authority under its own checked contract | at the operation | 8 |
| ASSEMBLY133 | "Low-level instructions alone do not justify this escape hatch." | Instruction obligations | Low-level instructions alone must not justify assumed binding escape | not stated | 8 |
| ASSEMBLY134 | "do not introduce an assumed binding to silence that failure." | Instruction obligations | Checker failures in assembly must not be silenced by introducing assumed bindings | not stated | 8 |
| ASSEMBLY135 | "Interpret the checked operations, not native instruction bytes or source text reparsed at runtime." | Interpreted execution | Interpretation must use checked operations, not reparsed source or raw bytes at runtime | not stated | 10 |
| ASSEMBLY136 | "Otherwise retain the exact catalog operation, target/features, inputs/outputs, authority, state effects and exits through the ordinary pipeline." | Interpreted execution | Non-ordinary operations must retain full catalog operation metadata through the ordinary pipeline | not stated | 10 |
| ASSEMBLY137 | "No second asm-only interpreter pipeline or opcode per spelling is required." | Interpreted execution | Implementation must not require a second asm-only interpreter or per-spelling opcode table beyond ordinary pipeline | not stated | 10 |
| ASSEMBLY138 | "A clobber/effect summary alone is not executable semantics:" | Interpreted execution | Clobber/effect summaries must not stand in for executable semantics | not stated | 10 |
| ASSEMBLY139 | "each supported operation needs its actual result and state transition, including the catalog's permitted nondeterminism and faults." | Interpreted execution | Each supported operation must model actual results and state transitions including catalog nondeterminism and faults | not stated | 10 |
| ASSEMBLY140 | "Scratch registers and flags" (table: Block-local state only where needed to realize the operation.) | Interpreted execution | Scratch registers and flags in interpretation must be block-local only as needed | not stated | 10 |
| ASSEMBLY141 | "Memory access" (table: Existing authorized storage/views, with the same bounds, layout and custody.) | Interpreted execution | Interpreted memory access must use authorized storage/views with same bounds, layout, and custody | at the operation | 2 |
| ASSEMBLY142 | "Control transfer" (table: Existing checked transitions or the exact modeled external-control exit, never arbitrary native jumps.) | Interpreted execution | Interpreted control transfer must use checked transitions or modeled external exit, not arbitrary native jumps | at the operation | 10 |
| ASSEMBLY143 | "Use ordinary checked interfaces/bindings for external mechanisms, not an author-supplied callback that may redefine a mnemonic." | Interpreted execution | External mechanisms must use checked interfaces/bindings, not author callbacks that redefine mnemonics | not stated | 9 |
| ASSEMBLY144 | "Modeled state is not the host machine's state." | Interpreted execution | Interpreted target environment state must not be equated to host machine state | not stated | 9 |
| ASSEMBLY145 | "Machine-owner authority for a modeled processor cannot disable host interrupts or acquire host physical memory." | Interpreted execution | Modeled processor authority must not affect host interrupts or host physical memory | not stated | 9 |
| ASSEMBLY146 | "Live hardware access needs its separately admitted provider and actual host authority." | Interpreted execution | Live hardware access requires separately admitted provider and actual host authority | not stated | 9 |
| ASSEMBLY147 | "Preserve persistent effects across blocks, helpers, normal code and work-budget pauses." | Interpreted execution | Persistent environment effects must be preserved across blocks, helpers, normal code, and work-budget pauses | across a pause | 10 |
| ASSEMBLY148 | "Halt is the modeled processor state/control effect, not interpreter work-budget exhaustion or a successful Unit return." | Interpreted execution | `halt` semantics must be modeled processor effect, not work-budget exhaustion or successful `Unit` return | not stated | 10 |
| ASSEMBLY149 | "If a work-budget pause occurs inside a block, retain its live scratch and progress too; completed instructions/effects are not replayed." | Interpreted execution | Work-budget pause inside block must retain scratch and progress without replaying completed instructions/effects | across a pause | 10 |
| ASSEMBLY150 | "External events may occur only at the model's admitted points, not merely because control returned to the host." | Interpreted execution | External events must occur only at model-admitted points | not stated | 10 |
| ASSEMBLY151 | "A block is not implicitly atomic." | Interpreted execution | Assembly basic blocks must not be treated as implicitly atomic | not stated | 10 |
| ASSEMBLY152 | "Before starting a selected executable closure, validate its demanded operation coverage and required environment supply, including callees/providers." | Interpreted execution | Executable closure start must validate operation coverage and environment supply including callees/providers | at entry | 9 |
| ASSEMBLY153 | "Repeat that admission for dynamically installed components." | Interpreted execution | Dynamically installed components must repeat operation/environment admission | not stated | 9 |
| ASSEMBLY154 | "Missing semantics or an unsupported environment rejects explicitly, never as a fabricated zero, Unit success, skipped effect or implicit native/JIT fallback." | Interpreted execution | Missing semantics or unsupported environment must reject explicitly, not fake success or JIT fallback | not stated | 9 |
| ASSEMBLY155 | "A no-op is valid only where the catalog permits it and all remaining obligations are preserved." | Interpreted execution | No-op interpretation is allowed only when catalog permits and obligations remain satisfied | at the operation | 10 |
| ASSEMBLY156 | "Native-only availability is not interpreted support." | Interpreted execution | Native-only instruction availability must not count as interpreted support | not stated | 9 |
| ASSEMBLY157 | "Operations requiring address translation or physical geometry need an explicit admitted mapping; never reinterpret an entry/storage ID as a host pointer." | Interpreted execution | Address translation/physical geometry requires admitted mapping; entry/storage IDs must not become host pointers | at the operation | 2 |
| ASSEMBLY158 | "Pure operations remain subject to [semantic-evaluation admission](evaluation.md);" | Interpreted execution | Pure assembly operations remain subject to semantic-evaluation admission rules | not stated | 8 |
| ASSEMBLY159 | "build/test environments retain their existing authority limits." | Interpreted execution | Build/test environments keep existing authority limits for assembly interpretation | not stated | 9 |
| ASSEMBLY160 | "neither supply missing instruction semantics nor become build-admissible." | Interpreted execution | Runtime-only assumed bindings must not supply missing instruction semantics or become build-admissible | not stated | 8 |
| ASSEMBLY161 | "Catalog availability distinguishes user-checked instructions from deriver-only entry/exit operations." | Control flow and machine state | Instructions available to user-checked assembly must be distinguished from deriver-only entry/exit operations | not stated | 10 |
| ASSEMBLY162 | "Jumps within a body target declared assembly machines;" | Control flow and machine state | Intra-body jumps must target declared assembly machines | at the operation | 10 |
| ASSEMBLY163 | "jumps leaving the body map to explicit checked Omega terminal transfers to visible machines." | Control flow and machine state | Jumps leaving assembly body must map to checked Omega terminal transfers to visible machines | at the operation | 10 |
| ASSEMBLY164 | "Source nesting does not grant a raw jump into another body or bypass its contract/acknowledgements." | Control flow and machine state | Nesting must not authorize raw jump into another body or bypass its contracts/acknowledgements | at the operation | 10 |
| ASSEMBLY165 | "Both forms retain arguments, invariants and live obligations." | Control flow and machine state | Intra- and inter-body jumps must retain arguments, invariants, and live obligations | on every edge | 2 |
| ASSEMBLY166 | "Undeclared targets, uncontracted returns, unwinds and unmodeled exits reject." | Control flow and machine state | Undeclared targets, uncontracted returns, unwinds, and unmodeled exits must reject | not stated | 10 |
| ASSEMBLY167 | "an inline region cannot return past its enclosing machine." | Control flow and machine state | Inline `asm` must not return past its enclosing machine | at normal return | 10 |
| ASSEMBLY168 | "Return-from-interrupt cannot bypass an entry contract." | Control flow and machine state | Return-from-interrupt must not bypass entry contract | at normal return | 3 |
| ASSEMBLY169 | "Direct targets remain final-artifact-validated;" | Control flow and machine state | Direct jump/call targets must remain valid in final artifact | not stated | 9 |
| ASSEMBLY170 | "indirect calls/tail calls require sealed requirement-compatible entry references, not numeric addresses." | Control flow and machine state | Indirect calls/tail calls must use sealed requirement-compatible entry references, not numeric addresses | at a call | 3 |
| ASSEMBLY171 | "Return and continuation state is compiler-owned, not ordinary addressable data." | Control flow and machine state | Return/continuation state must be compiler-owned, not ordinary addressable data | not stated | 10 |
| ASSEMBLY172 | "Every stack/control mutation must be modeled;" | Control flow and machine state | Every stack and control mutation must be modeled in catalog or deriver | at the operation | 10 |
| ASSEMBLY173 | "an instruction unable to preserve a modeled exit is deriver-only or rejected." | Control flow and machine state | Instructions that cannot preserve modeled exit must be deriver-only or rejected | not stated | 10 |
| ASSEMBLY174 | "Forward-edge entry identity is an independent obligation." | Control flow and machine state | Forward-edge entry identity must be proved as an independent obligation | on every edge | 3 |
| ASSEMBLY175 | "Regime-changing instructions require their prior regime and establish the next." | Control flow and machine state | Regime-changing instructions must document prior regime and establish next | at the operation | 10 |
| ASSEMBLY176 | "The complete boundary `StatePlan` constrains transitive use after optimization, allocation and callees;" | Control flow and machine state | Boundary `StatePlan` must constrain transitive machine-state use through optimization, allocation, and callees | not stated | 10 |
| ASSEMBLY177 | "source appearance cannot prove SIMD absence." | Control flow and machine state | Source must not be used to prove absence of SIMD or other state; evidence must come from plans/artifacts | not stated | 8 |
| ASSEMBLY178 | "Missing or plan-drifted evidence rejects." | Control flow and machine state | Missing or plan-drifted final machine-state evidence must reject | not stated | 8 |
| ASSEMBLY179 | "Floating instructions require canonical semantic controls, excluding sticky status flags." | Control flow and machine state | Floating-point instructions must use canonical semantic controls without sticky status flags | at the operation | 5 |
| ASSEMBLY180 | "Assembly changing them must establish the matching state transition and restore the [canonical configuration](numeric_values.md#target-realization-and-control-state) before ordinary checked code resumes." | Control flow and machine state | FP control changes in assembly must restore canonical FP configuration before resuming ordinary checked code | at normal return | 5 |
| ASSEMBLY181 | "An integer address grants no memory authority." | Hardware memory | Integer addresses alone must not grant memory authority | at the operation | 2 |
| ASSEMBLY182 | "Loads/stores require an authorized extent/view or a specific provider grant." | Hardware memory | Loads and stores must prove authorized extent/view or provider grant | at the operation | 2 |
| ASSEMBLY183 | "Plan-derived MMIO access follows the same rules." | Hardware memory | MMIO access from plans must follow same authority rules as ordinary memory | at the operation | 2 |
| ASSEMBLY184 | "Cache maintenance, TLB invalidation/shootdown, DMA completion and cross-core instruction-fetch visibility require their target-specific contracted sequences." | Hardware memory | Cache, TLB, DMA, and cross-core visibility operations must meet target-specific contracted sequences | at the operation | 10 |
| ASSEMBLY185 | "Executable mapping additionally requires admitted-artifact provenance, scoped installation authority and reach; ordinary bytes cannot supply them." | Hardware memory | Executable mappings require admitted-artifact provenance, installation authority, and reach; ordinary bytes are insufficient | at the operation | 9 |
| ASSEMBLY186 | "The instruction catalog owns `lidt` requirements, not an OS's table carrier or lifecycle." | Hardware memory | `lidt` requirements belong to instruction catalog, not OS table carrier/lifecycle | not stated | 10 |
| ASSEMBLY187 | "The build proposes the target environment and its providers; the receiving host admits and supplies their scoped state and authority." | Interpreted execution | Build must propose target environment/providers; host must admit and supply scoped state and authority | not stated | 9 |
| ASSEMBLY188 | "The compiler owns instruction meaning and its realization obligations;" | Interpreted execution | Compiler must own instruction meaning and realization obligations | not stated | 10 |
| ASSEMBLY189 | "No single universal CPU/device object or complete ISA emulation is required." | Interpreted execution | Implementation must not require universal CPU/device object or full ISA emulation | not stated | 9 |
| ASSEMBLY190 | "Provide the state needed by the supported operations, with explicit sharing between execution contexts where required." | Interpreted execution | Interpreter must supply only state needed by supported ops with explicit cross-context sharing where required | not stated | 10 |
| ASSEMBLY191 | "Simulation establishes behavior under its stated model, not native timing or hardware correctness." | Interpreted execution | Simulation must follow stated model only, not native timing or hardware correctness | not stated | 9 |
| ASSEMBLY192 | "Ordinary provider evidence/trust policy applies;" | Interpreted execution | Provider evidence and trust policy for simulation/hardware must follow ordinary rules | not stated | 8 |
| ASSEMBLY193 | "a successful callback or matching signature proves no correspondence." | Interpreted execution | Matching callback or signature must not alone prove hardware correspondence | not stated | 8 |
| ASSEMBLY194 | "Interrupt recognition delays, synchronization, ordering and external-entry conditions follow the exact target contract;" | Interpreted execution | Interrupt, sync, ordering, and external-entry behavior must follow exact target contract | not stated | 10 |
| ASSEMBLY195 | "Resume/external entry follows its contracted route and retains installation, loan and acknowledgement custody." | Interpreted execution | Resume/external entry must follow contracted route and retain installation, loan, and acknowledgement custody | at entry | 2 |
| ASSEMBLY196 | "Interpretability alone grants no compile-time execution permission." | Interpreted execution | Interpretable assembly ops must not gain compile-time execution solely from interpretability | not stated | 8 |
| ASSEMBLY197 | "Scratch isolation does not remove that requirement." | Interpreted execution | Scratch isolation must not waive explicit admitted mapping for address translation/physical geometry | not stated | 2 |
| ASSEMBLY198 | "Ordinary authored provider alternatives remain possible; they are not mandatory replacements for asm." | Interpreted execution | Providers may replace asm but must not be mandatory replacements for all asm uses | not stated | 9 |
| ASSEMBLY199 | "A mode transition is not an exotic calling convention." | Control flow and machine state | Machine-mode transitions must not be modeled as exotic calling conventions | not stated | 3 |
| ASSEMBLY200 | "[Final machine-state evidence](../build/machine_state_evidence.md) checks the actual artifact and exact return-control/restored-state set." | Control flow and machine state | Final machine-state evidence must verify artifact return-control and restored-state set | not stated | 8 |
| ASSEMBLY201 | "Opaque execution requires the admitted plan evidence or adequate isolation under a root-reported receipt." | Control flow and machine state | Opaque execution paths require admitted plan evidence or adequate isolation with root-reported receipt | not stated | 8 |
| ASSEMBLY202 | "Memory safety and [stack provisioning](../resources/storage.md#compiler-owned-stack-accesses) protect that state under their contracts." | Control flow and machine state | Return/continuation state protection must follow memory safety and stack provisioning contracts | not stated | 2 |
| ASSEMBLY203 | "Independent final-byte transfer certificates and CET/PAC/shadow-stack hardening remain separate proof/trust-reduction work, not additional source properties." | Control flow and machine state | CET/PAC/shadow-stack and final-byte certificates are separate from asm source properties | not stated | 8 |

## Ambiguous sentences

| Sentence | Reading A | Reading B |
| --- | --- | --- |
| "Nested assembly machines name the basic blocks of one checked control-flow graph." | Each nested `asm machine` is exactly one CFG node in the parent region's graph. | Nesting is syntactic grouping; basic-block identity is established only by layout/adjacency, not by nesting alone. |
| "This grants no execution support to an unmodeled instruction." | Unmodeled mnemonics fail at compile time and have no interpreter case. | Compile may accept with catalog stub but runtime interpretation still has no semantics for unmodeled ops. |
| "The structural `transition` form below and machine contracts are checked control-flow metadata." | `transition` is never emitted as an instruction and is verified only as CFG metadata. | `transition` may lower to nothing but still participates in proof obligations distinct from `jmp` metadata. |
| "asm selects a body language, not an unchecked provider or a distinct satisfaction relation." | Satisfaction of requirements uses the same relation as ordinary machines. | Body language selection is orthogonal to whether a separate provider models foreign entry. |
| "Transitions may connect a parent to its first nested block or a block to its next laid-out sibling." | Only those two transition patterns are legal at source level. | Other adjacency may exist after layout if still physically adjacent and proven; "may" lists allowed cases, not an exhaustive grammar. |
| "Existing flow facts and admissible private contract inference can discharge those goals without repeated annotations, but every actual incoming edge must justify any inferred requirement." | Inference is edge-local; each edge must carry justification facts. | Global inference may discharge once if all edges share the same symbolic state evidence. |
| "Public contracts cannot silently strengthen from one implementation's body." | Public API contracts are fixed at declaration; bodies cannot add ensures/requires visible to callers. | Strengthening through separate compilation units is forbidden; within one module, whether inferred public types may sharpen is not stated. |
| "For a nested block, the set covers its own instructions up to outgoing edges, not the union of every reachable successor." | Nested `clobbers` omit successor effects by design. | Nested clobbers must still include effects through terminator instructions on outgoing edges within the block. |
| "Whole-machine assembly does not by itself promise the absence of all generated entry/exit code." | Authors may still get compiler-generated prologue/epilogue unless they opt into exact-control contracts. | "Whole-machine assembly" might include boundary implementations that always get wrapper code; exact-control is a separate author annotation not named here. |
| "Equal byte size alone does not authorize copying or relocating arbitrary `T`." | Layout/size equality is insufficient for `memcpy`-style generic relocation in asm. | Byte equality blocks only unchecked blits; proven layout-compatible copies might still be allowed via separate rules. |
| "Register allocation and incidental preservation are not source contracts." | Authors need not spell callee-saved behavior; compiler owns it. | Incidental preservation cannot be assumed by proof unless clobber/catalog covers it; "not source contracts" means authors don't write them, not that proofs skip them. |
| "Labels and bodies are not transactions, isolation domains, or rollback points." | Environment state persists across block exits without automatic undo. | Optimizer may still elide redundant env writes; "not rollback points" forbids language-level undo, not peephole cleanup. |
| "Assembly does not automatically make its enclosing machine a provider." | Containing `machine` is not a provider merely due to asm contents. | Provider status still requires explicit provider conformance elsewhere; asm blocks inside providers follow same rule. |
| "A checked block may separately establish a conformance for callers when its instruction model proves it." | Callee-side proof may discharge caller conformance obligations. | Conformance establishment is optional reporting only and does not affect caller checking unless linked by separate build rules. |
| "Neither an alternate body nor a wrapper per block is required." | Embedding interpretation uses catalog ops directly. | Blocks may still be lowered with wrappers in codegen as long as semantics match; requirement is on semantic model not codegen shape. |
| "Scratch isolation does not remove that requirement." (paired with address translation) | Scratch memory isolation does not replace admitted address-translation mapping. | Scratch isolation is sufficient for some ops but not those needing physical geometry; which ops fall in which class is not enumerated here. |
| "Catalog availability distinguishes user-checked instructions from deriver-only entry/exit operations." | Users may only write user-checked mnemonics in source; deriver-only ops appear only in compiler output. | Distinction affects checking obligations; deriver-only ops might appear in source in advanced modes not described here. |
| "Opaque execution requires the admitted plan evidence or adequate isolation under a root-reported receipt." | Some paths allow isolation instead of full machine-state replay evidence. | "Adequate isolation" is defined only in linked machine-state evidence chapter; threshold is external to this chapter. |
| "Independent final-byte transfer certificates and CET/PAC/shadow-stack hardening remain separate proof/trust-reduction work, not additional source properties." | Source asm need not spell CET/PAC properties. | Compiler must still emit certificates elsewhere; "separate" means not asm syntax, not optional for shipping. |

## Terms used without definition in this chapter

- checked-assembly-machine-bodies (named decision)
- checked-assembly-label-contracts
- assembly body language
- checked control-flow graph
- compiler-owned contract (instruction)
- foreign provider artifact
- admission (provider)
- embedded interpretation
- target environment
- interpreted execution
- structural `transition` form
- named implementation
- satisfaction relation
- local continuation
- fresh ABI entry
- physically adjacent local block
- visible local continuation
- catalog-defined return
- contracted environment exit
- one-time setup
- laid-out sibling
- adjacency constraints
- unrealizable layout
- zero-instruction guarantee
- ranked-decrease obligation
- admissible private contract inference
- block proposition
- symbolic assembly state
- realized instruction contracts
- overlapping register views
- write footprint
- `registers` clause
- pure entry expressions
- authorized projections
- partial-expression obligations
- compiler-owned internal convention
- `Calling<Policy>`
- validated plan
- logical result / physical locations
- hidden pointer (calling plan example)
- exact-control entries
- static-machine binders
- named implementation binders
- runtime closures
- target features
- checked pointer/length adapter
- Omega's internal convention
- register dataflow / join
- numeric address exposure
- tracking metadata
- catalog-defined state observation
- scratch-flag
- execution environment
- instruction catalog
- service reach
- normalized reach and authority demand
- consuming boundary-trait requirement
- ownership disposition
- assumed binding contract
- exceptional-use rule
- root-selected boundary
- Terminal value, place and transition operations
- catalog operation
- permitted nondeterminism and faults
- observable transition (interpretation)
- target environment providers
- Cathedral (OS ownership)
- machine-owner authority
- work-budget pause
- semantic-evaluation admission
- runtime-only assumed binding contracts
- deriver-only entry/exit operations
- sealed requirement-compatible entry references
- compiler-owned stack accesses (linked)
- `StatePlan`
- final machine-state evidence
- plan-drifted evidence
- canonical semantic controls
- sticky status flags
- canonical configuration (FP)
- CET/PAC/shadow-stack hardening
- final-byte transfer certificates
- MMIO
- admitted-artifact provenance
- scoped installation authority
- hardware materialization
- `lidt`

CHAPTER COMPLETE
