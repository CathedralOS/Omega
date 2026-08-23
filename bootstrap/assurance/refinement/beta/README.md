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
valid Alpha framing and reject. The reduced ranged-store phase below closes the
Beta-source address/alignment/bounds premise for three stores only; transfer to
the Alpha operands, stored/loaded values, all raw loads, and general address
correspondence remain open.

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
source-semantic lemma plus static artifact macro custody, not yet a transfer of
`n`/`s` through Alpha frame slots. The stored `c`/IDOFF/IDLEN values, every
raw-load bound, live frame values, and general local contents remain open.

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
words. The later source induction shows that the three ranged source families
avoid those locations; their Alpha value transfer, the intended fixed counter
writes, carried depth/frame values, and the absolute stack obligation remain
open, as do local/return values, reachability, and terminal/trace
correspondence.

`bc-stack-register-custody.alpha` then transfers the earlier responsibility-
specific checks into one fresh per-PC owner map. It derives owners from the
fixed prelude, checked entry-block/frame tables, explicit and synthesized
epilogues, expression primitives, ordinary-call arities, raw stores, and all
403 push roots; duplicate owners reject. An independent decoded-instruction
scan requires that map to equal exactly the 2,630 starts which write `r14` or
`r15` or address memory through `r15`: 324 writes to `r14`, 1,430 writes to
`r15`, and 1,129 stack-addressed memory accesses, with 253 saved-frame loads in
both the first and third totals. This closes exhaustive static custody of the
artifact's explicit-stack effects. The following induction makes the three
ranged source families numerically disjoint from the reserved stack and counter
regions, but their Alpha operand transfer and carried stack/frame values remain
open.
