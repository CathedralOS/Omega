# Beta refinement reconstruction

This directory owns untrusted reconstruction of the Beta source-to-Alpha
artifact refinement obligation. `beta_symbolic.py` derives source meaning;
`alpha_symbolic.py` derives the meaning of the compiled Alpha tape;
`alpha_refinement_check.py` independently pins both derivations and asks the
low-rung proof kernel to check their equivalence. The curated
`refinement-samples/` and deterministic generators exercise that cross-rung
edge. See [`REFINEMENT.md`](REFINEMENT.md) for its exact claim and limits.

`symbolic_loop_check.py` remains the focused source-side check that pins Beta
loop summaries to executable reference meaning over concrete input grids. It is
refinement support, not Beta's canonical interpreter and not Alpha opcode
conformance.

The shared parser and concrete interpreter remain under
`bootstrap/rungs/beta/reference/`. Reconstruction may consume that meaning
surface, but it neither compiles Beta nor grants an artifact authority.
Support binaries are compiled with the persisted lattice-built `bc.tape`
through `bootstrap/rungs/beta/artifact_env.sh`; the refinement owner does not
rebuild or depend on the disposable Rust Beta producer.

Run `ownership-test.sh`, `symbolic-loops.sh`, `refinement.sh`, and
`refinement-cert-diamond.sh` from any working directory.

`bc-artifact-structure.alpha` is the first whole-artifact obligation checker
rooted below `bc`. It walks the reachable control-flow graph of the persisted
Alpha tape, permits jump-skipped inline data, and rejects unknown/truncated or
overlapping instructions plus invalid direct targets. Its focused gate includes
mutated negative controls and the exact tape-hole payload boundary. It also
reconstructs ordered procedure regions from direct-call targets and proves the
static call/return shape: entry zero is the unique root, only that root halts,
every callee region has a return, call continuations stay in the caller, and
non-call edges cannot cross procedure boundaries. This closes instruction
framing, direct-control-target reconstruction, and static call/return nesting.
Dynamic call-depth bounds, data-stack and memory bounds, complete stream
semantics, cyclic progress, and terminal-class correspondence remain open.

`bc-block-control.alpha` and `bc-effect-sites.alpha` form the next whole-artifact
slice. The gate concatenates those responsibility-specific modules into one
Alpha checker, so the effect checks reuse the exact source scan, block table,
tape bytes, and decoded-instruction coverage rather than trusting a second
private reconstruction. The checker resolves the source's procedure-local
entry/state/`to`/`when` graph and validates canonical block/transition locations
and successor shapes. It also binds 613 lexical source effect sites—310 ordinary
calls, two reads, five writes, 113 fixed-string emits, and 183 explicit
returns—to their exact artifact instructions. The emit check decodes and compares
all 829 literal bytes and validates the exact jump/address/length/helper macro.
Compiler-synthesized prelude, helper, and 70 fallthrough epilogues account for
the remainder, so every one of the artifact's 424 calls, two reads, six writes,
254 returns, and sole halt has exactly one owner.

The mapper supplies locations only; `bc-block-control.sh` packages the exact
repository source and artifact itself. Structurally valid branch/call retargets,
I/O register/opcode mutations, helper mutations, unreachable literal changes,
emit pointer/length changes, and malformed witnesses all reject. This proves
static effect-site custody and the fixed-emit macro when reached. Argument and
composed-expression values, frame contents and dynamic call depth, return values,
non-literal I/O values, global trace order/reachability, terminal classes,
memory/stack bounds, and cyclic progress remain open.

`bc-frame-shape.alpha`, concatenated into that same checker, derives 27
parameters and 51 function-scoped `let`s directly from the source. It validates
all 70 base prologues, the 47 nonempty frame allocations covering 78 slots, and
all 27 ordered register-to-parameter-slot stores. Each of the 310 ordinary call
sites must match its source callee's arity and its immediate lowering must pop
the exact 134 staged arguments into `r0..r1` in reverse stack order. Frame-size,
saved/base-fp register, parameter offset/register, pop-order, and pop-step
mutations retain valid Alpha framing and reject here. This establishes static
frame shape and parameter handoff conditional on the staged values. Dynamic
callee-frame contents, absolute live stack depth, and carried return values
remain open.

`bc-local-access.alpha` participates in the canonical BCT8 witness while
keeping name resolution authoritative in Alpha. It records all 27 parameters
and 51 `let` declarations with their function-scoped slots, distinguishes
assignment targets from comparison operands and calls, and binds 169 source
variable reads plus 73 `let`/assignment writes to exact 19-byte fp-relative
macros. Valid alternate-slot offsets, `r14` replacement, same-width load/store
swaps, duplicate locations, and reordered source witnesses reject. This closes
static local-slot selection and opcode custody, not the values carried through
those slots, definite assignment, expression evaluation, or dynamic aliasing.

`bc-memory-sites.alpha` independently classifies matching source brackets and
binds all 61 raw loads (56 word, five byte) and 34 raw stores (33 word, one byte)
to exact Alpha width/opcode/register sites. Every store also checks the immediate
16-byte address pop. Same-width load/store-width substitutions, register
changes, pop-step changes, duplicate sites, and reordered BCT8 records retain
valid Alpha framing and reject. The reduced phases below close the Beta-source
address intervals and transfer the three selected address operands under a
valid callee entry frame. IDOFF/IDLEN and name-table payload values, other raw
loads, general address correspondence, and global establishment of the selected
frame/machine-memory preconditions remain open.

`bc-raw-load-families.alpha` consumes a new grammar-derived classification of
all 61 loads: 54 aligned literal compiler-global words, five indexed SRC bytes,
and two indexed name-table words. It exhaustively rejoins all 95 memory rows,
requires store rows to have no load class, and checks every fixed load's exact
adjacent `imm r0,address; load r0,r0` artifact bytes. The literal window
`[2097064,2097152)` is aligned, inside 64 MiB, and disjoint from the source and
the already-bounded stacks. A missing fixed owner rejects in this phase. This
closes address selection and bounds for the 54 fixed loads when reached; the
five SRC and two table addresses deliberately remain indexed span obligations.

`bc-expr-primitives.alpha` extends that same source scan and BCT8 witness with
all 581 decimal/character literals and all 55 arithmetic operators (`+`, `-`,
`*`, `/`, `%`). Each literal must be the exact `imm r0,value` instruction; each
operator must be the exact 22-byte left-value pop and arithmetic macro with the
source-selected opcode. An independent artifact inventory classifies all 180
comparison-result pairs and 113 emit-address immediates as compiler-synthetic,
then requires owners for every remaining `imm r0` and arithmetic macro. Literal
value/register changes, same-valued retargeting to a synthetic comparison
result, arithmetic opcode/pop-step/register changes, duplicate locations, and
reordered records all retain valid Alpha framing and reject.

The BCT8 phase additionally binds all 180 comparison operators to the exact
59-byte lowering selected by the source operator: signed `jlt` versus full-word
`jeq`, operand order, left-value pop, branch-taken target, done target, and the
complementary 0/1 materialization. Same-width branch-opcode, operand-order,
valid-boundary target, materialized-result, and pop-step mutations reject while
retaining Alpha framing. This proves the static comparison macro conditional on
its staged operand values. Carried local/memory/callee values, arithmetic traps,
and comparison reachability remain forward-simulation obligations rather than
claims of this flat-custody phase.
At this flat phase, identical same-valued primitives within one source block
remain mutually swappable; it proves block-local multiset/shape custody, not
unique per-occurrence provenance.

`bc-stack-pushes.alpha` reconstructs all 403 source-required data-stack pushes
from the primitive, call-arity, and raw-store tables: 235 binary-left pushes,
134 left-to-right ordinary-call argument pushes, and 34 store-address pushes.
It checks every exact `imm r2,8; sub r15,r2; store r15,r0` macro and independently
inventories every decoded artifact occurrence. Stack-step/register/value/opcode,
duplicate-location, and cross-block witness mutations retain valid Alpha framing
and reject. Because every push has the same bytes, this is block-local
multiset/shape custody; recursive value association, ordering among identical
same-block pushes, and live stack bounds remain open at this phase.

`bc-expr-composition.alpha` then reparses all 70 procedures and 355 blocks with
Beta's exact expression precedence and statement boundaries. It consumes every
flat source table in lexical order while requiring its already-owned Alpha PCs
to form the recursive lowering sequence: left/push/right/operator, nested raw
loads, left-to-right call arguments followed by their pushes and reverse pops,
address/push/value/store, expression/local-store, guarded branches, and return
epilogues. Every complete statement expression restores its entry-relative
`r15`; an independently reconstructed symbolic high-water mark pins two live
temporary words for exact `bc.beta`. Same-valued literal, argument-push, and
store/binary-push permutations are accepted by the preceding flat-custody
projection but reject here. This closes syntax-directed code composition and
relative temporary balance. It does not yet establish absolute `B_bc1` stack
bounds, dynamic procedure-frame contents, raw-memory bounds/values, callee
summaries, reachability, or terminal/trace correspondence.
Identical complete same-block statements/effects can still be mutually
swappable when every owned macro is byte-for-byte identical; cross-statement
artifact order is part of the remaining blockwise simulation.

The same grammar pass now pins the complete spelling of the seven indexed
raw-load expressions and parses every other load as an aligned fixed-global
literal. The following exhaustive load-family phase, rather than lexical row
position alone, consumes that classification.

The BC11 composition pass also classifies all 34 raw-store address expressions
without trusting the mapper: 31 are aligned fixed compiler-global words in
`[2097064, 2097145)`, one is exactly `2097152 + n`, and the two local-name-table
stores are exactly `3145728 + s * 8` and `3153920 + s * 8`. This static
source-family result is consumed by the following value phase.

`bc-ranged-store-bounds.alpha` preserves those parser-derived classes for all
95 memory rows and exhaustively checks 31 fixed stores plus the three ranged
families. It pins the exact `slurp`, `declare`, and `parse_proc` reset schemas,
their eight blocks and five transitions, rejects any additional decoded branch
predecessor, then checks Beta-source inductive intervals for `n`, `s`, and
global `NLOC`. The guarded paths establish the exact SRC, NAMEOFF, and NAMELEN
byte extents without wrapping and their numeric disjointness from the reserved
global, explicit-stack, and hidden-return regions. Three coherent
source/artifact/witness mutations pass a projection ending immediately before
this schema/induction phase and fail the full checker; an independent
underreported-loop mutation reaches and fails backedge closure. This is a
source-semantic lemma plus static artifact macro custody; its machine transfer
is the following phase. At this point the stored `c`/IDOFF/IDLEN values, every
raw-load bound, whole-artifact live-frame invariant, and general local contents
remain open.

`bc-ranged-store-transfer.alpha` then rejoins only the selected local, memory,
primitive, push, frame, temporary-peak, and address-class rows. Exact lowering
chains carry `slurp.n` to the SRC store operand, snapshot machine `NLOC` into
`declare.s`, carry it to both name-table operands, and write `s+1` back to
machine `NLOC`; the two zero roots are joined when reached. A two-cell executable
tag/interval stack derives the selected `+`, `*`, staged addresses, store pops,
and exact operand ranges rather than relying only on row custody. An independent
decoded-CFG fixed point over the two call-free procedures propagates exact relative `r15`,
current/caller `r14`, and saved-fp facts, rejects nonidentical merges, pins a
32-byte maximum, and requires all selected returns to restore the entry pair.
Consequently the three Alpha address operands obey the source intervals
conditional on an aligned entry `(r15,r14)=(S,F)` with
`524320 <= S <= F <= 1048576`; `declare` additionally assumes actual machine
`NLOC` equals source `NLOC` in `[0,1024]`. Wrong-row, wrong-value-tag, and
shallow-frame checker teeth reject. This does not yet prove that every
whole-compiler invocation establishes those preconditions by itself; the later
counter/frame/potential lift now supplies actual `NLOC`, depth-counter, and
saved-frame transfer across all calls and both reset paths. The later
cursor-zero `slurp` summary closes its stored `c`/input-prefix relation;
IDOFF/IDLEN, the seven indexed raw loads, and their span bounds remain open.

`bc-counter-transfer.alpha` adds a witness-free selected-value premise for the
two bounded recursion counters and resource status. It rejoins the exact Alpha
rows for the `slurp` zero roots, `gen_expr`/`gen_stmts` frame snapshots and
signed `< 64` guards, accepted `+ 1` writes, every `- 1` exit, and all seven
`RESOURCE_FAIL` writers. Executable interval steps check the admitted
`[0,63] -> [1,64] -> [0,63]` chains, the rejected-depth-65 value, `NLOC`'s
preceding zero/`s+1` closure, and the `{0,252}` resource domain. An independent
scan of all 95 raw-memory rows requires exactly three `NLOC`, four
`EXPRDEPTH`, five `BLOCKDEPTH`, and seven `RESOURCE_FAIL` fixed writers.
An executable 64-row context bridge then assigns each BCS9 phase index
`remaining` the machine meaning `depth = 64 - remaining`, checks recursive
selection of `remaining - 1` alongside `depth + 1`, binds row zero to both
checked rejected-probe procedures, and binds the root's accepted `0 -> 1` step
to row 63.
The focused gate changes the checked counter/context relation and undercounts a
protected writer with all earlier inputs unchanged; both reject in this phase.
This is still a machine-value premise, not whole-call induction: it does not
yet carry the selected globals or saved frame words through arbitrary callees,
prove reachability of each exit, or make the BCT9 root stack bound absolute.

`bc_call_bounds.py` emits the untrusted compact BCS9 potential tables consumed
by `bc-call-bounds.alpha`. The Alpha phase independently resolves all 310
ordinary calls among the 70 source procedures, derives frame bytes from the
already checked prologues, and reuses grammar-reconstructed per-call temporary
heights and per-procedure peaks. It computes stopped reachability around the
only three recursive cutpoints, checks the exact `gen_expr`/`gen_stmts` counter
schemas and `emit_dec` signed-positive `/ 10` schema, charges the rejected 65th
probe including its guard comparison temporary, and validates every potential
equation. The conservative root summary is at most 12,720 explicit-stack bytes
and 662 hidden returns, comfortably inside the `B_bc1` extents; underreported
probe and root witnesses reject. This proves the finite call recurrence and its
numeric margin conditional on isolation of the depth counters and saved-frame
words. The later selected-callee, protected-counter, all-store/frame-summary,
and potential-lift phases establish those relations globally and close the
absolute stack obligation. General local/return values, reachability, and
terminal/trace correspondence remain open.

`bc-stack-register-custody.alpha` then transfers the earlier responsibility-
specific checks into one fresh per-PC owner map. It derives owners from the
fixed prelude, checked entry-block/frame tables, explicit and synthesized
epilogues, expression primitives, ordinary-call arities, raw stores, and all
403 push roots; duplicate owners reject. An independent decoded-instruction
scan requires that map to equal exactly the 2,630 starts which write `r14` or
`r15` or address memory through `r15`: 324 writes to `r14`, 1,430 writes to
`r15`, and 1,129 stack-addressed memory accesses, with 253 saved-frame loads in
both the first and third totals. This closes exhaustive static custody of the
artifact's explicit-stack effects. The following phases prove the ranged source
intervals and selected Alpha operands, then close whole-artifact carried
stack/frame and depth-counter bounds through the all-procedure frame summaries
and potential lift.

`bc-frame-summary.alpha` extends that conditional frame result to all 70 source
procedures without inlining calls. It independently partitions every decoded
artifact store into exactly 70 saved-fp stores, 403 expression-stack pushes,
73 local stores, 34 raw stores, and 27 parameter stores—607 stores total—then
uses the already checked frame offsets and raw-address separation to protect
each procedure's saved-fp word. A decoded-CFG fixed point propagates exact
relative `r15`, current/caller `r14`, and saved-fp-token states, rejects
nonidentical merges, matches every reached ordinary or emit-helper call to its
source event and checked ambient temporary height, and requires every reached
return to restore the symbolic entry pair. Its measured high-water mark must
equal the checked frame bytes plus that procedure's grammar-derived temporary
peak. An omitted store owner and an underreported local peak reject only in
this phase. Calls are deliberately treated as identity assumptions: this is a
finite per-procedure ABI summary, not the interprocedural induction that
consumes the BCS9 potentials, bounds recursion, establishes absolute `B_bc1`
addresses, or proves that a reached callee returns.

`bc-stack-potential-lift.alpha` composes those premises rather than introducing
a new producer witness. The counter phase's 64-row bridge selects the exact
BCS9 context at both bounded recursive cutpoints; the checked potential equation
then turns each call-cut frame summary into an induction rule for its callee and
continuation. The final phase rechecks the exact `r15=r14=1048576; call main`
prelude, all 9,030 explicit/hidden context pairs, and the main root of 12,720
explicit bytes plus 662 hidden returns. The resulting explicit and hidden
low-water marks are 1,035,856 and 67,103,568, respectively, so every defined
state after the checked two-instruction stack/frame initialization satisfies
`524288 <= r15 <= r14 <= 1048576`; saved frames and both stacks remain disjoint
from the compiler's checked raw-store regions. A
wrong counter/context relation, protected-writer undercount, missing store owner,
underreported procedure peak, and underreported final root each reject in their
own phase. This closes absolute `B_bc1` stack safety, not general raw-load/local/
return values, dynamic reachability, termination, or terminal/trace
correspondence.

`bc-slurp-summary.alpha` is the first blockwise relational value proof. It is a
cursor-zero conditional summary: both meanings enter `slurp` with cursor zero,
the same finite input, an empty output trace, and a compatible checked frame.
The phase rejoins all five blocks, four transitions, ten local actions, eight
stores, two reads, two explicit returns, and the exact selected value macros.
Its executable induction carries `n`, `Input(n)` versus EOF, the common cursor,
and one compact segment token `SRC[0:n] = input[0:n]`. The sole extension rule
requires the exact `SRC+n` endpoint and `Input(n)` payload at PC 538 before it
mints the `n+1` segment; the local increment and second read close both
backedge outcomes through an explicit capture-avoiding tag rename.
`1048576-n` decreases on every cycle, and an exhaustive
effect scan proves the procedure emits no bytes and has no ordinary calls.

Consequently inputs of at most 1 MiB return 1 with the complete input copied,
LEN set, and CUR/NLOC/LBL/EXPRDEPTH/RESOURCE_FAIL/BLOCKDEPTH zero. Larger inputs
consume exactly 1 MiB+1 bytes, return 0 with the first 1 MiB copied, and do not
execute those resets. Both exits preserve other regions, restore the caller
pair, avoid traps/out-of-bounds access, emit nothing, and terminate. Checker
teeth derive the endpoint payload from the wrong local, erase the rank
decrement, break the backedge rename, or feed zero rather than n to LEN; each
rejects only in this phase. This is a conditional procedure summary, not proof
that `main` establishes its entry relation, the status-253 composition, or a
typed `Exhaust(SourceBytes,...)` observation.

`bc-main-slurp-bridge.alpha` discharges the first two of those remaining root
obligations without changing the observation model. It rejoins the exact
prelude, main/slurp call at PC 51045, zero-ambient checked frame, `storage_ok`
store/load, equality macro, guarded transition, failure epilogue, and prelude
halt. The slurp phase publishes separately keyed success and oversize theorem
clauses only after rechecking each clause's input-length partition, cursor,
segment, reset-store count, selected result, and output invariant; the bridge
must import those clauses rather than supplying a raw 0/1 result. The only
dynamically executed prefix before slurp is effect-free, so the
canonical cursor-zero/epsilon-output initial relation is the procedure's actual
entry relation. The selected-value proof then carries slurp's 0/1 result through
local slot zero: one reaches `main.ready` at PC 51226 with the successful slurp
footprint, while zero materializes 253, restores both caller pairs, and reaches
`halt r0` at PC 29 with empty output. The selected suffix has no input/output,
nested call, division, or remainder. Wrong local provenance, reversed equality,
status relabeling, and cross-clause import reject only in this phase. This proves the concrete
`Halt(253)` projection for source oversize; whether that projection is exposed
as typed `Exhaust(SourceBytes,1048576,1048577)` is still a design ruling.

`bc-write-str-summary.alpha` gives the synthesized helper at PCs 31..82 one
reusable relational meaning. For natural `len` and an admitted nonwrapping
interval `[p,p+len)`, its loop carries `r0=p+k`, `r1=len-k`, and the exact
appended slice `M[p:p+k]`; `len-k` decreases on every backedge. The zero edge
and positive step establish termination after `8*len+3` instructions, output
`M[p:p+len]`, no input or memory mutation, the exact final argument registers,
and return to the saved continuation while preserving `r4..r15`. A direct scan
then instantiates that conditional theorem at all 113 kind-four event rows,
rejoining each checked pointer, decoded length, helper call, inline-data jump,
and in-tape interval; their lengths total the already compared 829 literal
bytes. Wrong loaded-byte provenance, zero rank delta, broken backedge renaming,
an underreported byte total, and a non-start cost-path step reject only in this
phase. This supplies per-event trace clauses, not global reachability or output
order.

`bc-fixed-emitter-summary.alpha` consumes those clauses for the two fixed-output
procedures reached at the start of `main.ready`. It chains every inline-literal
jump from the preceding helper-call continuation, requires the final
continuation to begin the exact epilogue, and concatenates event rows 311..315
into `emit_prelude`'s 55 bytes and rows 221..232 into `emit_write_str`'s 132
bytes. Independent region and source-table scans exclude extra calls, direct
I/O, halts, trap operations, transitions, locals, and raw-memory actions. Both
procedures therefore terminate, preserve the input cursor and compiler
heap/raw state, restore `r14`/`r15` and
the caller frame, and append exactly their Beta literals in order;
caller-clobbered `r0..r3` and reclaimed stack bytes are not preserved. Each
supplied end is the next canonical block PC, and an all-block scan proves each
procedure owns exactly one block. A wrong first row, eight-byte call
continuation, underreported prelude total, or wrong procedure end rejects only
in this phase. Reachability from `main.ready` and the combined 187-byte prefix
are the next composition step.

`bc-cursor-leaf-summary.alpha` gives the three cursor leaves reusable
conditional meanings before the whitespace loops are composed. It imports the
successful `slurp` clause for `LEN=L<=1048576` and the exact source segment,
then carries the explicit caller premise `0<=CUR<2^63`. `cbyte` follows its
checked signed comparison: `CUR<LEN` returns the zero-extended byte at the exact
`SRC+CUR` address, while `LEN<=CUR` returns zero without reading the source.
Consequently an in-range NUL and logical end both produce numeric zero; the
theorem does not pretend they are distinct observations. `adv` is intentionally
narrower: under `CUR<LEN`, its wrapping addition is proved nonwrapping and its
only compiler-global effect is `CUR'=CUR+1<=LEN`. `is_space` uses the same
checked parameter slot in all four equality blocks and exhausts the mutually
exclusive patterns for 32, 9, 10, 13, and their complement, returning one only
for the first four.

The phase rejoins procedures 4, 5, and 9 through their exact blocks,
transitions, local/raw-memory rows, primitives, pushes, returns, and epilogues.
Independent scans of all 242 local rows, 95 memory rows, 291 transitions, and
613 source events exclude hidden selected-procedure actions; decoded region
scans exclude input/output, calls, halts, and arithmetic traps. Thus all three
terminate, preserve external input/output and source memory, and restore their
private frames; `cbyte` and `is_space` have no compiler-global mutation, while
`adv` changes only `CUR`. Caller-clobbered result registers and reclaimed stack
bytes remain outside the preservation claim. Six phase-isolated variants reject
wrong source-index provenance, the wrong cbyte boundary partition, zero `adv`
progress, omitted CR, a whitespace complement, and an effect undercount.
`skip_ws_step`, the inner comment rank, and the outer `skip_ws` fixed point are
the next relational composition rather than being inlined into these leaves.
