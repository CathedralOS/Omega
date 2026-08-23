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
in this phase. The root composition below establishes their reachability and
combined 187-byte prefix.

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
These leaves remain separate theorem interfaces for the following composition.

`bc-skip-ws-summary.alpha` composes those interfaces through both whitespace
procedures. For `skip_ws_step`, cbyte's selected return flows through the exact
local-`c` store/load, the argument push/pop into `is_space`, and the checked
comparison edges. Whitespace advances once and returns one; an ordinary
nonspace/nonsemicolon value, including numeric zero, leaves `CUR` unchanged and
returns zero. A semicolon comment must execute one initial same-cursor
`cbyte;cbyte;adv` iteration that consumes its opener. Each later loop position
partitions into LF, zero, or a nonzero/non-LF continuation. LF returns one with
the LF unconsumed; logical end or in-range NUL returns zero; only the third case
calls bounded `adv` and takes the backedge. The two cbyte calls before that
decision share an unchanged versioned cursor, so determinism gives the same
byte. `LEN-CUR` is natural and decreases by one on every inner backedge. The
successor cursor and smaller rank are required together, capture-renamed to the
new cutpoint variables, and rechecked against the domain invariant.

The outer `skip_ws` fixed point imports all four step clauses. Its backedge is
selected only for whitespace and newline-ended comments, both of which strictly
increase `CUR`; therefore the same rank strictly decreases until the first
ordinary byte, logical end, or in-range NUL. Each result-one successor and
strictly smaller rank is likewise required, capture-renamed, and rechecked at
the outer cutpoint. The procedure always returns zero.
The phase rejoins exact targets and nine-byte continuations for all seven calls,
all selected source events/transitions/locals/primitives/pushes, and every
explicit or synthetic epilogue. Whole-table censuses and decoded region scans
exclude hidden selected-procedure memory/I/O/trap actions. Input position,
output, `SRC[0:LEN]`, `LEN`, and all compiler globals except `CUR` are preserved;
the prior call-cut/frame theorem restores both stacks, while scratch registers
and reclaimed frame bytes remain excluded. Sixteen isolated teeth cover the call
continuation, local/argument flow, same-cursor fact, LF/zero terminal results,
both ranks, forbidden result-zero backedge, event undercount, preserved cursor
domain, opening-semicolon provenance, rank premises, and both successor
renamings. The informal
source comment “1 if it did” is not authoritative for a zero-ended comment,
which consumes at least `;` but correctly returns zero. Composition from
`main.ready` through the fixed prefix and this cursor fixed point is discharged
by the next phase.

`bc-main-ready-summary.alpha` advances the successful root path from
`main.ready` at PC 51226 to `main.loop` at PC 51262. The bridge first publishes
a distinct ready clause only after its successful slurp facts have been
rechecked. The new phase then rejoins ready block 351 and loop block 352 under
main procedure 69, events 605..607, transition 287, zero arities, zero ambient
temporary heights, and the exact three call targets and nine-byte
continuations. A decoded scan of `[51226,51262)` plus the exact block-effect
census excludes hidden ready actions.

The three calls import the already-published `emit_prelude`, `emit_write_str`,
and `skip_ws` theorems in source order. Their composition appends exactly the
55-byte prelude and 132-byte write helper, proves that whitespace appends
epsilon, preserves external input and the successful `SRC[0:LEN]` relation,
normalizes `CUR` to the first ordinary byte, logical end, or in-range NUL, and
restores the active main frame before taking the sole jump at PC 51253. Thus
the loop cutpoint has the exact ordered 187-byte prefix. Seven isolated teeth
sever the bridge clause, first continuation, second theorem import, prefix
total, epsilon ordering, loop target, or exclusive event row; each rejects only
in this phase. The next blockwise frontier is the `main.loop` cbyte/token split,
not the independently blocked typed-exhaustion projection.

`bc-main-loop-entry-summary.alpha` closes that token split without entering
`parse_proc`. From the published root cutpoint, event 608 is the exact
zero-argument/zero-ambient `cbyte` call at PC 51262 with continuation 51271.
Push row 233 stages its return, literal row 810 supplies zero, comparison row
809 is exactly `!=`, and guarded transition 288 selects `main.body` at PC
51405. Literal row 811 plus the exact main epilogue supplies the other path,
returning through the root call continuation to `halt r0` at PC 29.

The relational proof keeps all three cbyte cases explicit. An in-range NUL and
the miss case—equal to logical end under the carried `CUR<=LEN` domain—both
produce canonical `Halt(0)` with the ordered 187-byte prefix and restored root
pair. An in-range nonzero byte reaches `main.body` without changing `CUR`.
Every case preserves external input, `SRC[0:LEN]`, the normalized cursor, and
compiler globals; the body case retains the active main frame. A whole-table
block census closes primitive rows 809..811 and push row 233 in addition to the
existing local/memory/transition/event census, while the decoded block scan
finds exactly one call and one return and excludes direct I/O, halt, and
arithmetic traps. Twelve isolated teeth sever one continuation, comparison,
guard target, cbyte-case association, selected result, halt payload, body
cutpoint, row census, generic theorem import, or concrete source-relation
bridge; each rejects only here. The next simulation frontier is the root-
reachable `parse_proc` call and its procedure summary.

The loop phase additionally publishes `MLSP`, a trace-parametric version of the
same cbyte split. It assumes an arbitrary ordered trace, successful source
segment, bounded normalized cursor, and active main frame, then preserves that
trace on both outcomes. The concrete `MLHZ`/`MLBD` clauses are root
instantiations of this interface. MLSP itself creates no reachability facts:
its explicit instantiation bridge checks caller-owned loop-PC, trace, cursor,
source/input, and active-frame tokens before deriving a concrete split. This
matters after one procedure has emitted code: a later `main.loop` visit must not
be tied back to the initial 187-byte prefix or silently assume `parse_proc`'s
postconditions. `bc-summary-combinators.alpha` separately owns the reusable
ordinary call join and the whole-table primitive/push census, including
disjoint binary, argument, and store-address push intervals; semantic modules
no longer clone those table walks.

`bc-classifier-shape.alpha` and `bc-classifier-summary.alpha` close the first
missing dependency beneath `read_ident` while keeping artifact custody separate
from relational meaning. The shape phase rejoins procedures 6..8 through exact
blocks, transitions, slot-zero loads, comparison/literal rows, binary and call-
argument pushes, lexical events, call targets/continuations, epilogues, 16-byte
frames, and decoded quiet regions. The split push-family census is important:
`is_alnum` owns binary row 15 but argument rows 236..237.

The meaning phase is deliberately byte-scoped because every dynamic caller
supplies a zero-extended cbyte value and Alpha comparisons are signed. It
exhausts all 256 values, evaluating each source branch order and an independent
interval specification. The resulting theorems are exact:
`is_digit` recognizes `[48,58)`, `is_alpha` recognizes underscore plus
`[65,91)` and `[97,123)`, and `is_alnum` is their union. Both alnum calls carry
the same slot-zero value; its false path returns `is_digit`'s result unchanged.
All three terminate, emit nothing, preserve input/source/cursor/compiler state,
and restore the caller frame. Ten isolated teeth cover domain, independent
bounds, source opcode/boundary, argument/call association, and exhaustive row
custody. Shared joins, loop meaning, classifier shape, and classifier meaning
are each below 20 KB rather than accumulating another monolithic checker file.

`bc-read-ident-shape.alpha` and `bc-read-ident-summary.alpha` compose those
classifiers through procedure 12. The shape module checks all four blocks and
three transitions, the lexically inverted cbyte/is_alnum event rows and exact
continuations, the adv call, both epilogues, its 8-byte frame, five fixed-global
word sites, nine primitives, five split-family pushes, and exhaustive row and
decoded-region censuses. The semantic theorem `RIDS` does not require a hidden
“starts with alpha” premise: for every cursor in the successful source segment
it returns the maximal (possibly empty) alnum prefix. It stores entry CUR in
IDOFF, stops before the first non-alnum/logical-end/in-range-NUL byte, stores
exit CUR minus IDOFF in IDLEN, returns zero, and restores the frame. Alnum-true
implies cbyte hit an in-range nonzero byte, so adv decreases the natural
`LEN-CUR` rank on the only backedge. Source/input/output and other compiler
globals remain unchanged. Twelve isolated teeth cover calls and argument flow,
fixed addresses and subtraction, row closure, rank, renaming, and stop meaning;
the two new modules are 7.3 KB and 9.5 KB.

`bc-expect-shape.alpha` and `bc-expect-summary.alpha` close the next parsing
leaf. The shape phase rejoins procedure 24's three blocks, guarded match edge,
skip_ws/cbyte/adv calls and exact continuations, slot-zero `ch` load, equality
and return literals, binary push, two explicit plus one synthetic epilogue,
16-byte frame, and exhaustive quiet-region/table ownership. The conditional
`EXPS` meaning is deliberately scoped to `1<=ch<=255`: skip_ws first terminates
at a normalized cursor; mismatch—including logical end or in-range NUL—leaves
that cursor unchanged, while a nonzero match proves `CUR<LEN` and adv consumes
exactly one byte. Both branches return zero, restore the frame, and preserve
source/input/output and other compiler globals. Eleven isolated teeth exercise
the calls, slot/comparison, censuses, delimiter premise, match-range fact, and
both cursor outcomes. The modules are 4.3 KB and 6.7 KB.

`bc-declare-shape.alpha` and `bc-declare-summary.alpha` reuse the earlier
ranged-store/NLOC and identifier-slice work to close procedure 34. The shape
module pins its three blocks, room guard, local `s` snapshot, seven word-memory
sites, nineteen primitives, binary/store-address pushes, both explicit and the
synthetic epilogue, 16-byte frame, and exhaustive quiet-region/table ownership.
The conditional `DCLS` theorem snapshots actual/source NLOC as `s` in
`[0,1024]`. With room it stores IDOFF/IDLEN in NAMEOFF[s]/NAMELEN[s], advances
NLOC to `s+1`, preserves RESOURCE_FAIL, and returns `s`. At capacity the domain
forces `s=1024`; tables and NLOC stay unchanged, numeric RESOURCE_FAIL becomes
252, and the result is zero. The theorem deliberately does not assign a typed
resource kind to that shared numeric status. Both paths terminate, emit
nothing, preserve source/input/CUR/identifier globals and other compiler state,
and restore the frame. Twelve isolated teeth cover shape, capacity/status and
payload values, row closure, and both branch relations. The modules are 9.3 KB
and 6.6 KB.

`bc-let-keyword-shape.alpha` and `bc-let-keyword-summary.alpha` close the exact
keyword predicate needed by `count_lets`. The shape phase covers `id_char` and
`is_let` procedures 13..14: blocks/guards, the indexed byte load, three
argument calls and continuations, every return/epilogue, frames, and exhaustive
local/memory/primitive/push/event/decoded ownership. Conditional `IDCH` admits
only `0<=k<IDLEN` and returns `SRC[IDOFF+k]`. `ILET` uses its exact IDLEN==3
guard before any byte access, then proves all length, `l`, `e`, `t`, and success
short-circuit cases; it returns one exactly for the identifier `let`. Both are
quiet and restore callers. Twelve isolated teeth cover bounds/addressing,
calls/arguments/constants/censuses, and branch meaning. The modules are 11.3 KB
and 9.5 KB, keeping artifact and relational responsibilities separate.

`bc-literal-skip-shape.alpha` and `bc-literal-skip-summary.alpha` close the two
literal-aware cursor helpers needed by `count_lets`. Exact shape covers
`skip_char_lit`/`skip_str_lit` procedures 37..38, all blocks/transitions,
thirteen calls, returns and synthetic epilogues, frames, primitives, pushes,
and exhaustive decoded ownership. Their conditional relational summaries do
not assume well-formed input: the character helper blindly advances three
bytes, or four after a backslash, and can finish at LEN+2; the string helper
partitions quote, zero/NUL/end, ordinary, and escape bytes and can finish at
LEN+1 after a trailing escape. Its natural rank is LEN+1-CUR. A narrowly
bounded exact-body `ADVX` consequence justifies the out-of-range increments
without widening the ordinary ADVE theorem. Fourteen isolated teeth cover
shape, calls, constants, censuses, bounds, deltas, zero-tail preservation, rank,
and backedge renaming. The modules are 11.6 KB and 18.6 KB.

The `bc-count-lets-*` family closes the outer scan without rebuilding another
monolith. Control/effect shape (8.0 KB) and data/expression shape (10.9 KB)
rejoin procedure 39 exactly. The one-iteration layer (16.0 KB) derives a narrow
SWSX clause for CUR in `(LEN,LEN+2]` and exhausts zero in/out of range, both
literal kinds, braces, alpha let/non-let, and ordinary bytes. The 15.8 KB fixed
point ranks live iterations by `2*(LEN+2-CUR)+live`, handles the depth-one
matching-close exit separately, increments only for exact `let`, retains the
last scanned IDOFF/IDLEN state, restores entry CUR, and returns the exact count.
Twenty-four negative canaries are isolated in a 6.7 KB shell harness and sever
shape, SWSX, malformed bounds, depth/count/rank joins, identifier carry,
restoration, or result. The focused gate carries a 180,738-byte Alpha tape.

`bc-parse-params-control-shape.alpha` and
`bc-parse-params-data-shape.alpha` split procedure 68's parameter/capacity
artifact slice by responsibility. The 6.8 KB control module binds blocks
339..344 (and all procedure block PCs), transitions 274..281, ten calls, three
pre-output returns/epilogues, the 48-byte frame, and exhaustive effect/decoded
ownership. The 7.9 KB data module binds its locals, fixed memory sites,
primitives, split push families, and exhaustive expression rows. The separate
13.8 KB `PLOP` theorem begins at the checked post-`expect('('); skip_ws()`
cutpoint with the successful source segment, NLOC zero, status zero, and a
normalized cursor. Each non-close room iteration composes `RIDS` and the room
clause of `DCLS`, then the exact skip_ws/cbyte and optional comma adv/skip_ws
sequence. Even an empty identifier or absent comma increments NLOC, so
`4-NLOC` strictly decreases on every backedge. A close remains unconsumed and
exits with 0..4 exact parameter-table entries; a fifth non-close writes numeric
252 and returns zero without output. The declaration-failure edge is
unreachable under the stronger four-parameter guard. Eighteen negative
canaries live in a separate 5.5 KB harness and sever exact shape, rooted source
custody, terminal retention, separator normalization, status, rank, or
renaming. Independent review found the correspondence clean, and the complete
gate carries a 188,250-byte Alpha tape. The opening delimiter and earlier name
scan are not manufactured by this conditional theorem; the deterministic
procedure-prefix composition must establish that cutpoint.

`bc-parse-capacity-summary.alpha` keeps the adjacent pdone meaning separate
from the parameter fixed point. Its conditional `PCAP` clause selects PLOP's
successful close outcome, composes exact close consumption, snapshots
`0<=nparams<=4`, instantiates `EXPS` for `{`, and invokes `CNTS` at event 594's
checked ambient height one. The exact let count is bounded by LEN, so
`nslots=nparams+count<=1048580` cannot wrap and stays in Alpha's nonnegative
signed-comparison domain. The checked `nslots<=1024` edge reaches slotsready
with status zero, prior output unchanged, and the parse frame active. The
complementary edge writes numeric 252 and returns zero through the exact
pre-output epilogue with the caller frame restored. Both preserve source/input,
the parameter-table prefix, count_lets' restored body cursor, and its carried
identifier state. Seventeen isolated canaries in a separate 5.1 KB harness
exercise exact artifact rows, ambient height, imported close/expect/count
relations, count and addition bounds, prefix status, guard polarity, and both
terminal outcomes. The 9.6 KB theorem passed independent review and the full
189,911-byte checker gate. The gate now executes its canonical proof before
assembling the historical mutation matrix, failing invalid theorem integrations
quickly without removing any final canary.

`bc-emit-ident-shape.alpha` and `bc-emit-ident-summary.alpha` close the first
dynamic-output leaf needed after slotsready. Exact procedure-45 shape binds its
three blocks, `k<len` guard and backedge, direct `write_byte`, explicit and
synthetic returns, 32-byte frame, locals, indexed source load, primitives,
pushes, and exhaustive effect/expression/decoded ownership. Because this leaf
deliberately writes a byte, its decoded census separately requires exactly one
direct output instead of misusing the shared quiet-region scanner. Conditional
`EIDS` starts with the successful source segment and
`0<=off<=off+len<=LEN`, carries an arbitrary prior trace plus
`SRC[off:off+k]`, and treats the positive and false `k<len` outcomes as
explicit premises. The byte branch appends the exact next source byte and
strictly decreases `len-k`; the stop branch derives `k=len`, returns zero, and
restores the caller while preserving source/input/CUR/compiler globals.
Seventeen isolated canaries cover exact artifact joins, direct-write count,
slice/guard premises, byte and trace extension, rank/backedge renaming, and the
terminal result. Shape, meaning, and teeth remain separate files below 9 KB.

`bc-emit-dec-shape.alpha` and `bc-emit-dec-summary.alpha` close the bounded
decimal-output leaf used by procedure prologues and parameter stores. The exact
shape binds procedure 40's four blocks, `n>=10` split, sole recursive call and
argument, post-child continuation, direct digit write, explicit/synthetic
returns, 16-byte frame, locals, division/remainder/addition, pushes, and full
effect/expression/decoded censuses. The decoded scan requires exactly one call,
one write, two returns, one division, one remainder, and seven target stores.
Conditional `DECS` is deliberately limited to `0<=n<=8192`: the checker
executes all 8,193 inputs and validates `q=n/10`, `r=n%10`,
`n=10*q+r`, digit byte `48+r`, and the decimal-phase decrease. A four-phase
induction selects the base/recursive guard outcomes, passes exact q through the
checked argument push, consumes the immediately preceding child phase, and
requires child output before the current digit. Thus it appends canonical
decimal bytes without a leading zero, returns zero, restores the caller, and
preserves compiler state apart from output. Twenty-three isolated canaries live
in a 6.7 KB harness; shape and meaning remain 9.9 KB and 11.9 KB. Seven
induction-only canaries seed the separately tested arithmetic certificate rather
than replaying all 8,193 rows; canonical and arithmetic variants retain the
real sweep. Independent shape/meaning review was clean, and the complete gate
passes with a 198,975-byte checker tape. BCT9's conservative full-word resource
ceiling is reused only for global stack safety, not misrepresented as decimal
value semantics.

`bc-fixed-decimal-emitters-shape.alpha` and
`bc-fixed-decimal-emitters-summary.alpha` consume `WSTR` and `DECS` for the two
bounded output leaves immediately below `parse_proc.slotsready`. Exact shape
rejoins procedures 42..43, blocks 209..212, the sole `nslots>0` transition,
events 316..333, locals 121..124, primitives 532..541, split pushes 152..155
and 316..318, both 16-byte frames, every source/synthetic epilogue, and decoded
inventories of sixteen calls, four returns, and eleven target stores. `EPRO`
checks every `0<=nslots<=1024`: it emits the mandatory 46-byte frame prefix and,
only when positive, `imm r5,` followed by canonical `DECS(8*nslots)` and the
fixed allocation suffix. `EPAR` checks every `0<=k<4` and emits the exact
parameter-store fragment with `DECS(8+8*k)` before `DECS(k)`. Both clauses
extend an arbitrary prior trace, otherwise preserve compiler state, and restore
the caller. `EPRO`'s explicit returns materialize zero; `EPAR`'s unused
fallthrough result remains caller-clobbered rather than being overclaimed.
Nineteen isolated canaries in a 5.5 KB harness sever
shape, bounded arithmetic, child value/order, fixed-byte totals, store census,
or terminal restoration; the 12.2 KB shape and 12.8 KB meaning modules pass in
the complete 205,641-byte checker.

`bc-parse-output-prefix-shape.alpha` and
`bc-parse-output-prefix-summary.alpha` consume those leaves and close the
deterministic `parse_proc` prefix from PCAP's successful room clause through
`genbody` PC 50945. Exact blocks 345..348, transitions 282..285, events
596..599, locals 231..239, primitives 801..804, split pushes 230..231 and
365..368, the 48-byte frame, exhaustive table slices, and the decoded four-call
/ eight-store quiet region bind the machine path. `PFXS` passes PCAP's saved
name slice to `EIDS`, appends exact `":\n"`, passes exact `nslots` to `EPRO`,
and initializes `k=0`. A complete fifteen-state sweep of
`0<=k<=nparams<=4` closes the `EPAR(k)` backedge with strict
`nparams-k` decrease and source-ordered output; the synthetic EPAR result is
dead before the exact `k` reload. The exit trace is
`prior || name || ":\n" || prologue(nslots) || concat(param_store(k),
k=0..nparams-1)` with the active parse frame, status-zero room clause,
source/input/CUR, and parameter table retained. Eighteen isolated canaries in a
5.3 KB harness sever exact joins/censuses, the bounded domain, saved-name
custody, literal length, EPAR argument/dead-result/successor facts, or exit
state. The 8.4 KB shape and 12.0 KB meaning modules pass in the complete
210,239-byte checker. This clause remains conditional on PCAP's room outcome
and assigns no typed resource kind to numeric 252.

`bc-gen-stmts-boundary-shape.alpha` and
`bc-gen-stmts-boundary-summary.alpha` close procedure 62's root-independent
immediate boundary without pretending to solve its recursive child. Exact
blocks 308..314, transitions 252..257, events 498..507, local/memory/primitive
and push intervals, the 16-byte frame, every epilogue, exhaustive table slices,
and a decoded inventory of six calls, five returns, and twenty stores bind the
artifact. `GSBD` checks every entry `D=0..64` against the existing
`remaining=64-live-depth` counter/potential bridge. At `D=64`, the exact false
guard stores numeric 252 and returns zero without changing BLOCKDEPTH. At
`D<64`, exact increment and terminating `SWSQ` reach the loop with depth `D+1`;
the resource, close-brace/`ADVE`, and zero-byte exits each restore `D` and return
zero, while a remaining nonzero/non-`}` byte reaches the exact `gen_stmt` call
PC 44956 unconsumed with the proc62 frame active and arbitrary prior output
unchanged. Twenty-four isolated canaries in a 6.3 KB harness cover exact joins,
censuses, all 65 depths, context selection, counter actions, SKIP/ADV, byte
classes, child PC, and active-frame custody. The 16.1 KB shape and 16.9 KB
meaning modules produce a 219,443-byte checker. The child call is not executed;
no child outcome, totality, recursive fixed point, or typed status-252 meaning
is claimed.

`bc-parse-number-shape.alpha` and `bc-parse-number-summary.alpha` close the
finite lexical/value leaf for procedure 33. Exact shape binds blocks 167..169,
both loop transitions, the two `cbyte` calls and same-value `is_digit` handoff,
the body `adv`, explicit/synthetic epilogues, one local slot, wrapping
multiply/add/subtract lowering, split push families, exhaustive table slices,
and a decoded four-call/two-return/eight-store quiet region. Conditional `PNUM`
starts at arbitrary `0<=i<=LEN`, carries the exact digit slice `SRC[i:j]`, and
relates slot zero to its left-to-right fold modulo 2^64. Classifier false
returns that word at the unconsumed cursor. Classifier true proves `j<LEN`,
requires the body's second observation at the same cursor and byte, applies
`V'=(10*V+(byte-48)) mod 2^64`, and composes one `ADVE` with strict `LEN-j`
decrease before capture-avoiding backedge renaming. A complete ten-byte digit
offset sweep and two concrete wrap/high-bit probes prevent a nonwrapping or
signed strengthening. Twenty-four isolated canaries live in a 6.4 KB harness;
shape and meaning remain 9.0 KB and 14.4 KB, and the focused checker is 225,147
bytes. This does not prove canonical decimal literals, full-word decimal
output, or the expression SCC.

`bc-parse-char-shape.alpha`, `bc-parse-char-cases.alpha`, and
`bc-parse-char-summary.alpha` close the finite acyclic lexical/value leaf for
procedure 56. Exact shape binds blocks 256..266, transitions 197..210, events
414..420, locals 155..166, primitives 608..621, binary pushes 178..182, empty
argument/store-address intervals, explicit/synthetic epilogues, and a decoded
six-call/two-return/twelve-store quiet region. The responsibility-specific
cases module exhausts all 256 first bytes and independently checks the unique
backslash discriminator, then exhausts the ordered `n/t/r/0/default` mapping
against an independent direct specification. Conditional `PCHR` starts at an
in-range opening quote: an ordinary byte is returned unchanged at exact final
cursor `i+3`; a backslash selects a second in-range observation, maps
`n/t/r/0` to `10/9/13/0`, preserves any other byte, and returns at `i+4`.
The two final advances use reusable `ADVX`, not `ADVE`, and no closing quote is
read or required. Separate clauses preserve opening-at-end, payload/no-close,
trailing-backslash, and escaped-byte/no-close outcomes, including distinct
boundary-zero versus in-range-NUL provenance at both observations and exact
`LEN+1`/`LEN+2` cursor classes. Outcomes publish only after the selected
observation, branch, result path, fin-cursor, and terminal-bound joins have
been consumed. Thirty-eight isolated canaries live in a 9.2 KB harness; shape,
finite cases, and path summary remain 14.0 KB, 4.7 KB, and 19.9 KB, and the
focused checker is 235,255 bytes. This does not prove
canonical character literal syntax, the remaining lexical/value leaves, or
the expression SCC.

`bc-operator-classifier-shape.alpha` and
`bc-operator-classifier-summary.alpha` close procedures 53..54. Exact shape
binds blocks 234..242, transitions 178..185, events 379..382, locals 144..148,
primitives 572..585, binary pushes 167..171, both 16-byte frames, all explicit
and synthetic epilogues, and the decoded zero-call/six-return/nine-store quiet
region at PCs 34977..35880. The decoded scanner formerly private to parse_char
is now responsibility-neutral and shared by both leaves. `OMUL` and `OADD`
evaluate source branch order and independently ordered membership tests for all
256 bytes, requiring exact accepted/complement counts 3/253 and 2/254. The
shape-fixed full-word `==` operations compare only literals below 256, so a
separate checked complement proves every remaining Word returns zero. Thus
`is_muldiv(c)` is one exactly for `*`, `/`, or `%`, and `is_addsub(c)` is one
exactly for `+` or `-`; both otherwise return zero, emit nothing, preserve
compiler state, and restore their one-slot frames. Twenty-four isolated
canaries live in a 6.9 KB harness; shape and meaning are 10.0 KB and 9.6 KB,
and `BC_BLOCK_FOCUS=operator-classifier` passes with a 239,992-byte checker.

The eventual `parse_proc` theorem must be maximal, not universally terminating.
For malformed input, an unrecognized body byte such as `@` can survive both
`gen_stmt` and the number fallback without cursor progress while `gen_stmts`
keeps emitting. The honest contract is therefore Return-or-Diverge with exact
finite/infinite output behavior. The next engineering milestone is the body
Return-or-Diverge relation; the existing typed status-252 projection is the
only language-design blocker in this area.
