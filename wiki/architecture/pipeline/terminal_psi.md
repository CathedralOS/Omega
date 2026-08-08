# Terminal Psi Architecture

[Pipeline](pipeline.md)

Status: target architecture settled 2026-08-02; implementation checkpoint
updated 2026-08-08. This document records the implementation cut and migration
from the current bootstrap pipeline. The
semantic and evidence contract is owned by
[`canonical_ir_fuel_and_resource_provisioning.md`](../../design_briefs/canonical_ir_fuel_and_resource_provisioning.md).

Implementation status (2026-08-02): `compiler/psi-rs` is the Psi-owned
workspace root. The first source-facing ownership slice is live:
`psi-source` owns loaded-source records/maps, identities, byte spans, and
source-backed text;
`psi-tokens` owns token streams; and
`psi-source-files-to-tokens` owns the Omega lexer without depending on any
Omega crate. The old Omega-named source-to-checked pipeline packages are
retired; every workspace harness invokes the Psi stages directly. Some
former Omega-named source representation adapters are retired; legacy
consumers depend on the Psi owners directly. `omega-compiler` invokes the Psi-owned lexer, parser,
resolver, typer, checker, and source representations directly, although its
legacy backend still consumes checked semantics until general terminal
production replaces that early cut. `psi-core` provides nonzero stable semantic identities, the
typed scalar proposition vocabulary, and a module-owned value-typing context.
`psi-proof-kernel` provides total primitive judgments, structural proof
checking (including semantic-axiom citation and typed equality transitivity),
versioned certificate envelopes, and exact profile-authorized admission
validation. Admission cannot replace a primitive derivation, and architecture
tests reject any Psi dependency on Omega.

The first in-memory executable slice is also live. `psi-terminal` defines a
versioned module with stable machines, blocks, values, operations, edges, and
bodyful contracts. Frozen semantic v1 contains representable integer constants;
v2 adds Boolean constants; v3 adds exact-width wrapping integer addition; v4
adds exact-width saturating integer addition; v5 adds exact-width
wrapping integer subtraction; v6 adds exact-width saturating integer
subtraction; v7 adds exact-width wrapping integer multiplication; v8 adds
exact-width saturating integer multiplication; v9 adds proof-only
structural-place declarations and content-conservation propositions; v10 adds
canonical identity-preserving claim reshuffles; v11 adds distinct stable
sum-case segments to structural content paths; v12 adds exact authored-
partition substitution witnesses; v13 adds one structural Boolean conditional
with ordered true/false successors; v14 adds canonical machine-local
entry-claim bindings independently of output equality; v15 adds total Boolean
logical negation; v16 adds canonical nominal proposition declarations and
normalized application identities; v17 adds total Boolean equality; v18 adds
total equality over two values of one exact integer type; v19 adds
signedness-aware integer less-than and less-or-equal; v20 adds total bitwise
AND, OR, and XOR over one exact integer type; v21 adds wrapping left and
signedness-aware right shifts; v22 adds an explicit no-successor crash
terminator carrying a closed cause, one nominal damage scope, and the
machine-local claim frontier known to be abandoned; v23 separates that scope
into a body-derived damage minimum and selected published containment demand;
v24 adds a canonical sparse per-cause context maximum to each machine
contract; v25 adds total fixed-width integer bitwise complement; v26 adds
universally total fixed-width `i*`/`u*` widening whose target contains the
complete source range; and current v27 distinguishes the target-selected
address carrier from an ordinary same-width unsigned integer while retaining
its current 64-bit representation.
A wrapping shift
retains the shifted value's exact result type and the count operand's
independent integer type;
the count reduces by Euclidean modulo of the shifted value's width.
None of v9-v14 or v16 adds an executable operation. The conditional is control vocabulary rather than an
operation, and an entry-claim binding is identity metadata rather than a
proposition. The current crash row is the first representation slice: the
verifier requires its frontier to equal every still-live entry claim (terminal
Psi has no claim-consuming operation yet), direct interpretation reports a
distinct terminal crash outcome and never replays it after resume, canonical
bytes and semantic identity cover every field, and fuel charges its edge.
Omega native lowering rejects the row until target crash plans are represented;
it never silently treats a crash as a return or ordinary terminal transition.
The source frontend now retains fingerprinted `crashes Cause Scope` buckets and
explicit `crash Cause;` exits. Source production accepts exactly one prechecked
covering bucket, emits its selected containment demand and the site's derived
damage minimum on the crash terminator, and fails closed for absent or
ambiguous coverage. This covers crash-only unconditional machines and
checker-proved incoming-path guards in the acyclic integer-control slice,
including exact-type integer comparison guards, nested-negation implication,
and portable comparison equivalences. Integer guard operands may use the same
recursive scalar-expression vocabulary as integer results; equality and order
retain terminal operations, inequality composes `BooleanNot`, and greater forms
swap operands into the canonical less relation.
Route facts are already
restricted to Boolean expressions. Public contract and generic-template
identities already merge exact `(cause, scope)` buckets, discard duplicate
routes, and let a route-less or explicit-`true` route subsume guarded
alternatives. Checked machine-contract plans retain that published set as
source-handle-free buckets; the public fingerprint and contract manifest
consume the same carrier, and terminal lowering selects prechecked coverage
from it rather than re-reading typed clauses. An independent checked body layer
retains every explicit crash site's state-local location and cause; these rows
are reported as implementation evidence, do not enter the public fingerprint,
and are required by terminal production. Canonical published buckets receive
dense plan-local identities. A site already records every unconditional
same-cause bucket whose guard coverage follows structurally, and terminal
lowering consumes that checked relation rather than searching routes itself.
Exact retained incoming guards, including the negations accumulated by later
dispatch arms, now join to identical canonical published predicates without
entering public contract identity. Positive conjunctions also imply each
conjunct, negated disjunctions imply each negated disjunct, and nested logical
negation flips polarity. Boolean comparisons with a literal normalize
`x == true`, `x == false`, `x != true`, and `x != false`, including negated
fallthrough edges, to the operand polarity they establish; converse
implications remain rejected. Comparison predicates also retain their
operand-reversed equivalent, and negated equality/inequality retains the
opposite relation. Negated ordered comparisons use the complement relation only
when both operands have checked integer types. Unknown, user-defined, and float
operands stay opaque because unordered values invalidate the usual complement
law. For checked integers, strict order also entails its non-strict bound and
inequality, while equality entails both non-strict bounds. Integer order chains
compose across positive path conjunctions: any strict link makes the derived
endpoint relation strict, while an all-nonstrict chain remains nonstrict.
Opposed nonstrict integer relations apply antisymmetry and retain the resulting
endpoint equality for both explicit-site and checked-call coverage. A one-sided
bound does not imply equality. A nonstrict integer relation paired with
endpoint disequality sharpens to the corresponding strict relation; this rule
also feeds explicit sites and checked calls.
Canonical operand identities join the links; unrelated endpoints and unordered
float relations do not compose. Checked sites
retain their exact incoming-predicate conjunction separately from these
coverage consequences. Checked calls likewise retain invocation coordinates,
the exact target contract fingerprint, the incoming path conjunction, a
separate source-independent structural consequence set, and every surviving
substituted route. Caller-ceiling coverage consumes the consequence set while
preserving the exact conjunction as distinct evidence. Same-unit private bodies
are summarized over the viable call graph while typed expressions are still
present. A
temporary canonical predicate tree carries positional parameters through every
nonrecursive private edge, so guarded routes survive arbitrarily deep acyclic
wrappers and concrete outer arguments can still disprove them. Recursive SCC
edges widen to unconditional cause/scope buckets: this is the finite
conservative top for cycles whose argument transformations could otherwise
create an unbounded predicate family. Only final source-handle-free predicate
identities enter the checked plans.
Checked ownership also reconstructs a canonical lower bound of stable claim
identities that are definitely live and non-conditional at the site. A crash
abandons those claims without cleanup or consumption. Terminal production maps
them through the dense source-claim table and rejects an unmapped identity
rather than silently weakening the frontier. Conditional sum claims join that
lower bound only when canonical symbol-rooted membership evidence proves every
case segment on the nested claim path; rendered source labels, dynamic indexes,
and partial outer-case proofs do not suffice. Checked sites now also retain the
intrinsic cause minimum: `Trap` requires at least `Activation`, while `Abort`
requires `ExecutionDomain`. Exact nominal identity and the permanent
`ExecutionDomain` top provide the first conservative scope order. If a crash
abandons an open default-domain invariant window, the checked site retains the
invariant-bearing data identity and widens its damage minimum to
`ExecutionDomain`. This is the conservative portable top until finer custody
evidence can select an intermediate nominal scope. Reports
separate guard-covering buckets from the subset whose containment demand also
covers the minimum; terminal production consumes that two-dimensional subset,
rejects narrower authored demand, and emits both the derived minimum and
selected published demand into v23. Archived v22 bytes decode conservatively
with both in-memory fields equal to their single encoded scope. Conditional
frontier membership, finer custody-based minimum widening, broader guarded
production beyond the scalar acyclic slice, narrower supervisor/task context
production, and installation
realization remain the rest of CRASH-CONTRACT. Terminal Psi v24 stores each
effective sparse per-cause context maximum in the fingerprinted machine
contract and the verifier enforces `containment demand <= context
maximum[cause]`. Artifact-root crash production supplies `ExecutionDomain` for
both closed causes. A sibling checked-to-terminal entry point accepts an
already selected canonical sparse context for narrower activation, task, or
supervisor composition and validates the completed semantic module before
artifact production. Build/provider selection still needs to supply that input.
Archived modules migrate only causes used by their crash terminators to the
legacy root maximum. Declared intermediate nominal scope
ordering is separately blocked on `OWNER_QUESTIONS.md` Q2.
`psi-terminal-verifier` rejects malformed identities, types, contract scopes,
cycles, unreachable fact sources, and missing/extra evidence, reconstructs the
exact operation/edge/return axioms, and checks every `ensures` from a separate
proof bundle. `omega-interpreter` exposes an artifact-root entry that accepts
only canonical semantic/proof section bytes, decodes both sections, invokes
that verifier under an explicit admission profile, and executes only the
resulting `VerifiedTerminalModule`. Omega's terminal-Psi-to-abstract-operation
builder exposes the parallel artifact-root entry and constructs realization
requirements only after the same decode and verification sequence. Source,
checked trees, producer-owned modules, and prevalidated Rust objects do not
cross either entry. Installation and debug sections remain separately
manifest-bound metadata and do not affect interpretation or semantic lowering.

Checked proof facts retain nominal proposition declarations and normalized
applications after transparent aliases and source handles have been removed.
Terminal production assigns dense terminal identities from that checked
vocabulary; it no longer walks typed proposition declarations or typed
proof-fact applications.

Checked value facts likewise retain the complete accepted executable scalar
expression vocabulary after operator selection and type checking. Recursive
return values, positive guards, and transition arguments are keyed by stable
state identity, state-local statement ordinal, and semantic role rather than
`ExpressionHandle`. Terminal production fails closed when that carrier is
absent and does not reopen typed expression nodes. Checked flow facts retain
the companion control plan: ordered stable state identities, primitive
parameter/result types, return/crash/jump/conditional terminators, stable
successors, and argument arity. Terminal semantic and proof production no
longer reads typed statement or transition records. Typed source records remain
only for the replaceable debug map: checked flow facts now also retain stable
machine names and the bootstrap signature-eligibility classification used by
semantic selection. An optional checked debug plan separately retains stable
subject spans and cloned source-file presentation metadata. The terminal
producer itself imports no typed-tree vocabulary: replacing the complete typed
frontend root with an empty value after checking preserves its semantic module,
proof bundle, and debug map, while omitting only the debug plan yields the same
artifact semantics without presentation metadata.

The first control-flow slice is live in v13. One conditional terminator reads
an already-defined Boolean value and owns ordered true and false successor edge
records, each with its own stable `EdgeId`, target, typed block-parameter
bindings, scalar binding actions, and fuel site. Mutual exclusion and
exhaustiveness are structural properties of the terminator, not propositions
reconstructed from two arbitrary guards. Predicate terms never appear as
executable guards. The verifier validates an acyclic CFG and dominance of every
value use; proof reconstruction retains only facts common to every return
path. The canonical codec round-trips both ordered successors, the interpreter
executes and charges only the selected edge, and Omega's source-independent
abstract plan retains canonical block entries and both successor records.
The checked-source producer lowers scalar integer-result acyclic control graphs
whose blocks return, jump unconditionally, or select one ordered
positive-Boolean/fallback successor pair. Unconditional jumps may compute
recursive exact-typed integer expressions for integer bindings and recursive
Boolean expressions for Boolean bindings. Non-short forms include literal,
negation, Boolean equality/inequality, and exact-type integer comparison;
short-circuit forms use value-producing decision blocks inside typed
left-to-right tuple stages. Either may feed a later integer-result selector
through an ordinary terminal block parameter. A computed
conditional successor binding lowers through a synthesized arm-local block:
the conditional passes its current parameters into only the selected block,
that block computes the recursive exact-typed arguments, and an ordinary jump
binds the authored target. The unselected arm is neither evaluated nor charged.
Recursive `&&`/`||` guards lower into reserved decision blocks over the same
ordered conditional vocabulary. The authored source block passes its current
parameters into the decision entry, only the selected tests execute, and the
chosen leaf targets either the authored successor or its arm-local binding
block. This also makes checker-proved transitive integer-conjunction crash
routes executable through terminal verification and direct interpretation.
Compile-known integer values propagate through the lowered DAG, including
recursive arithmetic bindings, exact-type comparison selectors, and recursive
Boolean values carried through Boolean block parameters. The known-value
lattice retains scalar type, follows a Boolean binding into its selected
integer-result arm, and meets conservatively at joins; a reachable crash exit
prevents claiming a total result, while a known selector may exclude an untaken
crash arm.
Nested selections and convergent tails retain
their authored blocks and edges in terminal Psi; proof reconstruction
intersects facts at joins, and the fixed-work checker derives the maximum
entry-to-return cost. Omega recursively realizes the graph on x86-64 and
AArch64, duplicating a pure shared tail where the current native tree form
requires it while preserving canonical Psi provenance. The Boolean-result
companion now accepts the same rooted acyclic topology over ordinary Boolean
parameters: nested selections, convergent tails, recursive non-short-circuit
unconditional bindings, recursive short-circuit returns, and short-circuit
guards all reach verification, exact fuel, interpretation, and both native
targets. Short-circuit return leaves use reserved value-producing decision
blocks. Unconditional jumps use the same leaves to bind authored targets;
multi-value tuples evaluate each element left-to-right in a separate stage,
carry prior results through later decision trees, and converge once before the
authored target. Pure unconditional multi-value graphs enter this general
lowering directly; they do not require a conditional source block. Conditional
successors route computed Boolean payloads through the same arm-local blocks
and decision trees, so an unselected expression has no operations or fuel
charge. Compile-known evaluation runs over this lowered acyclic graph, follows a
known selector, meets parameter facts conservatively at joins, and supplies the
same fail-closed contract check for every returning scalar graph. Loops remain
a later slice.

The checked-frontend migration also keeps the ownership firewall explicit:
`psi-checked-trees` now owns the target-neutral checked representation and
the legacy state/control representations and transforms, artifact/backend
orchestration, interpreter, and backend leaf consumers depend on it directly.
The unused former Omega compatibility package is retired.
Target-neutral facts and effect summaries are likewise Psi-owned, while concrete selected provider
plans and target/layout-specific task activation plans are Omega-owned and
travel as orchestration sidecars. `CheckedTrees` does not embed that
target/provider realization state. `psi-typed-trees-to-checked-trees` now owns
semantic checking and checked-fact construction. Its validation and proof
dependencies live in `psi-validation` and `psi-proof`; their unused Omega
compatibility packages are retired, with cross-owner validation/provider tests
retained in the architecture harness. Provider installation and approval remain
Omega concerns, and Omega orchestration runs that admission explicitly after
the Psi check.

The first Psi-owned terminal source producer is live as
`psi-checked-trees-to-terminal`. It accepts a closed set of scalar free-machine forms. A
Boolean machine may return a literal, exact named parameter, or a recursively
nested expression over builtin logical negation, Boolean equality/inequality,
and short-circuit `&&`/`||` from any sequence of ordinary Boolean parameters,
either directly or through a
nonempty linear sequence of unconditional state bindings. Every non-entry
Boolean state has one ordinary Boolean parameter, and each jump carries the
result of the same recursively nested Boolean expression vocabulary from its
source state. Short-circuit binding leaves converge on the next state through
ordinary Boolean block-parameter arguments; a final-state short-circuit return
uses the same terminal decision form. When equality or inequality contains a
short-circuit operand, value-producing decision leaves retain the explicit
`BooleanEqual` operation and the canonical `BooleanNot` composition rather
than replacing their proof and fuel sites with a truth-table branch.
Compile-known Boolean bindings likewise must match the closed reflexive
contract. Checked contract plans retain that accepted Boolean/integer equality
as a source-handle-free value carrier, and terminal production fails closed on
an absent or unrecognized carrier instead of reopening typed contract facts.
Checked value facts separately retain every executable return, guard, and
successor argument in this scalar vocabulary, so terminal production no longer
uses a typed expression handle to recover executable meaning. Checked flow
facts retain the matching scalar control topology, so the producer also does
not recover executable structure from typed statements or transitions. Stable
checked selection rows supply machine name and signature eligibility without
reopening typed machine declarations. Debug attachment consumes only the
optional checked presentation plan. A
single-state Boolean-result machine over ordinary primitive-integer parameters
may compare two recursively nested integer
expressions of one exact type with builtin `==`, `!=`, `<`, `<=`, `>`, or `>=`.
The operands use the same parameter/literal arithmetic, bitwise, and wrapping
shift vocabulary as integer-result machines. Equality retains
an `IntegerEqual` operation and inequality composes its canonical
`BooleanNot`. Ordered comparisons normalize to `IntegerLessThan` or
`IntegerLessOrEqual`; greater forms swap operands. Signedness remains in the
operand type rather than becoming a source-only operator detail. A single-state
integer machine may declare any sequence of ordinary primitive-integer
parameters, including none, and return one exact named parameter, one landed
literal, or a recursively nested
expression over parameters and landed literals using the six builtin
add/subtract/multiply operations in the settled Wrapping or Saturating domains,
plus builtin bitwise AND, OR, XOR, and unary complement without an
arithmetic-policy choice and wrapping left/right shifts. A shift's value
operand and result share one exact
integer type; its count may have a different integer type and carries no
arithmetic-domain weight. An explicit value cast may retag an integer with or
without one of the closed arithmetic policies when its primitive carrier is
unchanged. Checked retention removes that static retag after using it to select
any enclosing operation; it emits no terminal operation and consumes no
operation fuel. A strict-width primitive cast whose target contains the
complete source range retains an `IntegerWiden` operation: its exact result is
the same mathematical integer at the wider carrier, and it costs one operation
unit. This includes unsigned-to-signed widening when the signed target has a
strictly larger width. Narrowing, same-width signedness changes,
signed-to-unsigned casts, and conversions whose totality depends
on occurrence range evidence remain outside this scalar slice rather than
being mistaken for identities. Terminal Psi v27 retains `addr` distinctly from
`u64` across declarations, scalar terms, comparisons, artifacts, and Omega
realization. Cross-carrier conversions to or from `addr` remain outside rather
than being mistaken for representation identities.
Declared semantic-domain casts remain outside the slice as well.
Source unary integer negation retains its parser-defined `0 - value` meaning.
Because the generated zero has no authored suffix, checked retention lands that
anonymous zero at the already-validated operand carrier before producing the
existing Wrapping or Saturating subtraction; terminal Psi gains no separate
negation operation.
The linear integer form may declare any sequence of ordinary primitive-integer
machine parameters, including none, and any sequence of at least two states.
It computes a recursively nested parameter/literal add/subtract/multiply
expression for each unconditional jump argument and binds the complete ordered
sequence of ordinary integer parameters in every non-entry state. Each argument
must exactly match its target parameter type; later expressions may use any
same-typed bound parameter and landed literals. When the whole chain is
compile-known, the producer independently recomputes its result and rejects an
unrelated reflexive contract. Integer operations use the settled Wrapping or
Saturating domains. The general multi-state scalar form admits a rooted
acyclic graph of source states with a Boolean or integer result. Each control
block owns one ordered
positive-Boolean/fallback pair, each linear block owns one unconditional
successor, and leaves return a recursively nested Boolean or integer
expression. Successors bind ordered already-defined
Boolean or integer parameters with exact target types; an unconditional jump
may instead compute recursively nested parameter/literal integer expressions
or Boolean expressions before binding its target. Mixed-scalar tuples that
contain short-circuit Boolean expressions evaluate every authored argument
left-to-right in typed stages, carrying original parameters and earlier results
until one convergence jump binds the authored target. The same staging is
arm-local for conditional-edge payloads, so an unselected tuple has no executed
operations, edges, or fuel charge. Joins are explicit block-parameter merges.
Every scalar-result machine uses this general typed DAG producer, including
contract-free all-crash graphs, one-state returns, pure unconditional graphs,
and three-state conditionals. A Boolean-result graph may carry and compute mixed
Boolean/integer bindings, retain recursive short-circuit returns, or end a
checked branch in an explicit crash. The former direct-parameter, comparison,
Boolean-return, integer-chain, three-state conditional, Boolean-chain, and
Boolean-DAG and crash-only lowerers/builders are retired; source shape no
longer selects a parallel terminal producer. All-crash graphs establish no
ordinary return value, so they carry no return obligation or proof evidence.
Short-circuit guards use explicit decision blocks, and computed conditional
edge bindings use selected-arm blocks because terminal edges do not own
operations. This preserves source ordering and keeps each branch's computation
local to its selected path. The
Boolean results accept the recursive Boolean vocabulary in positive guards and
branch returns. A short-circuit guard sends its
terminal test edges directly to the selected branch with the authored entry
arguments; computed successor payloads remain arm-local decision trees and only
the selected arm is charged. All accepted returning forms require a matching
closed `requires`/`ensures` pair; contract-free all-crash graphs carry neither
value clause. The producer rejects all other checked-tree shapes,
including selected domain-owned operator meanings. The source canary lowers
all six versioned integer-policy operations in both constant-fed and
runtime-parameter forms, Boolean literal, negation, Boolean
equality/inequality, integer equality and ordering, and
nested same-carrier arithmetic-policy casts plus a direct policy erasure,
narrow signed Wrapping/Saturating unary negation,
ninth-parameter returns, a
three-state Boolean chain carrying its ninth parameter, a closed three-state
integer chain, a direct zero-parameter integer literal, plus a nine-parameter
integer direct return, and a six-block integer graph with nested runtime
selection and a three-way convergent tail. An integer-result companion computes
Boolean inequality on an unconditional edge, carries it through a Boolean block
parameter, and selects the returned integer with that value. These canaries
also stage an `&&` binding beside integer payloads on unconditional and
conditional edges; the latter bypasses the entire tuple on its unselected arm.
They discard `CheckedTrees`, then verify and execute the produced semantic
modules. A wrong-contract companion stages compile-known `true && false`,
follows the resulting Boolean parameter to the integer `9`, and rejects a
closed contract naming `8`. A pure unconditional companion carries a staged
Boolean plus an integer through two source jumps and preserves short-circuit
fuel without adding an authored selector.
Direct-return `&&`/`||` expressions lower to acyclic terminal conditional
trees. Each evaluated operand owns its selected conditional edge; the deciding
left operand bypasses the right subtree, and Boolean leaf constants plus return
edges remain explicit and metered. No eager logical operation is introduced.
The source conditional likewise survives frontend disposal, selects only its
taken path, crosses Omega's abstract boundary with every successor intact, and
executes through emitted native code. Nested runtime conditionals retain their
branch expressions and operation provenance through independent assigned
frames and native emission. A compile-known Boolean literal selects its
exact arm during Omega target lowering: only that arm's operations, structural
edge, and return edge reach emitted provenance, while terminal interpretation
still validates and meters the original two-successor graph. The computed
two-binding canary paths and the literal-selector canary each have a five-unit
fixed-work certificate. The target continuation also follows each successor
through an acyclic chain of unconditional blocks, substitutes computed jump
bindings, and permits arms to converge on one shared tail. Shared operations
and edges appear once in canonical Psi provenance; the current native tree
realization may duplicate the pure tail on distinct paths. Cyclic target
programs remain fail-closed.
Constant-fed wrapping add and the Boolean literal reach emitted host machine
code; direct ninth `bool` and `u8` returns cross the host incoming-stack ABI;
and runtime wrapping add
combines the first register argument with the ninth stack argument. A nested
wrapping add-then-multiply source expression also crosses those ABI locations
and reaches emitted host code. A two-state runtime canary now computes from the
first register and ninth stack arguments, binds that result through an
unconditional edge, and continues arithmetic from the block parameter; its
five-unit fixed-fuel certificate, interpretation, and emitted host result agree.
The three-state companion carries that runtime value through a second computed
jump and agrees at an eight-unit ceiling. A multi-binding three-state companion
carries two independently computed values across each edge, costs ten units,
and agrees across fixed-fuel derivation, interpretation, and native execution.
The
artifacts therefore have no frontend lifetime dependency.
This is the correct ownership direction, but the accepted expression grammar
remains this deliberately narrow integer/control/contract slice. An
architecture test keeps one fail-closed `lower_machine` entry. General terminal
production must extend this Psi stage rather than reintroduce an Omega-to-Psi
bridge. The stage now independently revalidates and lowers checked content
conservation, identity reshuffles, direct partition compositions, and exact
entry-claim bindings into the current v9-v14 terminal vocabulary. Those
evidence translators retain stable semantic paths, dense claim identities,
source theorem fingerprints, and exact place substitutions. The current
executable source canaries remain content-free.
The current legacy exit prover also cannot establish an ordinary
`result == literal` contract, so the bootstrap canary carries the closed typed
fact `7i32 == 7i32` and asserts the executed result separately. An Omega
source-independent consumer is also live:
`omega-terminal-psi-to-abstract-operations` accepts only a
`VerifiedTerminalModule` and produces an owned stream of scalar materialization,
wrapping-add, saturating-add, wrapping-subtract, saturating-subtract,
wrapping-multiply, saturating-multiply, Boolean-not, Boolean-equality,
integer-equality, jump-binding, and return requirements with stable Psi
provenance. Its function
records also retain declared runtime parameters and the result pseudo-value
with exact scalar types; the real checked-source producer now exercises that
path through a ninth stack argument. Neither it nor
`omega-terminal-abstract-operations` depends on
checked/typed trees, `ExpressionHandle`, or the legacy source-shaped abstract
operation plan.

The first target/native realization is live on the same clean lane.
`omega-terminal-abstract-operations-to-target-operations` resolves the verified
compile-known scalar operations and jump bindings into a target immediate
return while retaining every contributing Psi operation and edge identity. It
also uses the established native call planner to select AAPCS64, System V
AMD64, or Microsoft x64 register/incoming-stack locations for runtime scalar
parameters. Direct parameter returns stay explicit; parameter-fed wrapping and
saturating addition plus wrapping and saturating subtraction and wrapping and
saturating multiplication lower to recursive, exact-width
target expressions. Runtime Boolean equality lowers to a recursive Boolean
target expression with canonical immediate/parameter, negation, and equality
nodes; assigned AArch64 and x86-64 emission preserves both inputs and produces
canonical zero/one results. Runtime integer equality similarly retains the
operand integer type and two recursive integer expressions; assignment and
emission compare their normalized exact-width representations on both native
architectures. Runtime integer ordering follows the same lane and selects
signed or unsigned `<`/`<=` conditions from the retained `IntegerType`.
Runtime integer bitwise AND, OR, XOR, and unary complement retain the same
exact integer type and lower to one native logical instruction after
recursively evaluating their operands. Complement negates exactly the retained
carrier width; native normalization preserves narrow signed and unsigned
results.
Same-carrier arithmetic-policy casts add no target expression node: an
enclosing operation already carries the selected Wrapping or Saturating
meaning, while a direct policy erasure remains the existing parameter return.
Unary integer negation likewise selects the existing target subtraction after
materializing its exact-width zero; it does not create a target-only negation
meaning.
Runtime wrapping shifts retain the count operand's independent integer type,
reduce that value modulo the shifted width, and select logical or arithmetic
right shift from the shifted value's signedness. Current native source widths
are powers of two, so x86-64 and AArch64 realize the modulo with `width - 1`
before the variable shift.
Recursive Boolean expressions may also serve as
target control conditions for either Boolean- or integer-result control and as
Boolean return leaves; assignment gives each expression
its own frame, and emission tears that frame down before entering either arm.
This lets short-circuit terminal trees branch on nested equality expressions
without introducing eager Boolean opcodes. For the exact conditional form, a
Boolean ABI parameter retains both emitted arms, while a compile-known Boolean
constant selects one arm and omits the unreachable arm from target bytes and
provenance. Each arm may now cross an acyclic sequence of computed
unconditional bindings before return, including convergence on a shared tail;
target lowering reduces each path to its exact runtime expression without
erasing terminal operation or edge identity.
`omega-terminal-target-operations-to-assigned-target-operations` is the next
explicit boundary. Its first correctness-oriented rung validates every selected
parameter register against the target, freezes repeated parameter locations,
and assigns stable aligned AArch64 frame spills before any expression scratch
register may overwrite an incoming argument. The resulting
`omega-terminal-assigned-target-operations` plan is the only expression-home
input accepted by terminal machine emission. X86-64 register and incoming-stack
homes remain explicit when the scratch discipline preserves them; a selected
input in `rax`, `r10`, or `r11` receives an assigned frame spill, while `rsp`
cannot be an expression-parameter home. Broader liveness-based allocation,
spill reuse, and non-scalar homes remain later work.
`omega-terminal-machine-emission` emits ordinary scalar-return code for
AArch64 and x86-64 and rejects non-native integer widths.
`omega-terminal-image-emission` then constructs an owned, canonical-order
object artifact whose function spans retain terminal-Psi provenance and exact
semantic identity. It emits the compatibility Omega object container and
standalone ELF/AArch64, ELF/x86-64, Mach-O/AArch64, and PE/x86-64 images through
the shared image model and writers. Parameter-return emission supports Boolean
and 8/16/32/64-bit integers in selected native registers or incoming stack
slots on both architectures. Runtime wrapping/saturating addition,
wrapping/saturating subtraction, and wrapping/saturating multiplication support
signed and unsigned 8/16/32/64-bit operands, recursive expressions, and mixed
immediate/register/stack leaves. Runtime Boolean equality uses the same assigned
frame discipline and supports recursive Boolean expressions on both native
architectures. Runtime integer equality and ordering use the same frame
discipline for two exact-width integer expressions and emit canonical zero/one
results; ordering selects signedness-aware conditions on both architectures. The
assigned plan preserves every referenced
AArch64 argument register in an aligned local spill frame before evaluation
into `x0`; both emitters compensate incoming-stack addresses for their assigned
frame and expression stack.
The relocation-free slice requires exact
final text, complete provenance-bearing compiler regions, and no unclassified
executable gaps. The source, Boolean, wrapping-add, and saturating-add canaries
drop all producing semantic and lowering state before artifact emission; the
host linker harness executes the retained entry bytes, and the macOS host
canary also executes the emitted Mach-O image directly. General value liveness,
spill reuse, non-scalar assignment, and migration of the legacy backend remain
outside this checkpoint.

Canonical semantic serialization and identity are now live for this initial
vocabulary in `psi-terminal-codec`. The real-source canary encodes the semantic
module, records its identity, discards the source and producing module, decodes
a fresh module and proof bundle, validates their section manifest, and then
drives verification, interpretation, and native realization. The v3 wrapping
canary independently round-trips, verifies, meters, lowers, emits, and executes
`u8` 200+100 as 44 after producer state is discarded; the v4 saturating
canary follows the same path and clamps that sum to 255. A frozen-v1
nine-parameter canary forces its returned `u8` through the host incoming-stack
ABI and matches interpretation at 77. A v4 nested runtime canary wraps
a register and ninth-argument stack `u8`, then saturates with another register
to 255; a signed `i64` canary exercises both saturation bounds. Both agree with
interpretation through real C ABI calls. Canonical typed debug-map v1 now binds
an exact semantic identity to strictly ordered source files and stable terminal
machine, block, operation, edge, value, contract, obligation, place, and
machine-local claim subjects. Source rows retain origin, presentation path,
byte length, and a domain-separated content digest; sites retain bounded byte
spans. Decoding rejects wrong-module attachment, unknown subjects/files,
alternate ordering, invalid spans, unknown tags, truncation, and trailing
bytes. The exact checked-source producer populates honest declaration spans
retained by the Psi symbol/source tables for machines, states, parameters, and
the terminal subjects derived from them. Psi expression tables now retain
authored integer/Boolean literal and operator-token spans through syntax,
resolution, typing, and checking; terminal operations and their result values
therefore point to their exact authored expression sites. Authored transition
arrows likewise survive into terminal jump-edge sites; a synthesized return
edge retains the exact returned-expression site. Terminal contract
and obligation subjects point to the exact authored `ensures` fact site rather
than the enclosing machine declaration. The real-source
canary encodes and manifests the debug section,
drops checked trees, and decodes it against the reconstructed semantic module.
Reusable native block layout, general register assignment, build-time fuel
migration, and native fuel metering remain next.
The v5 wrapping-subtract canary independently round-trips, verifies,
costs one operation plus one return edge, lowers, and executes parameter-fed
`u8` 5-10 as 251 through a real C ABI call.
The v6 saturating-subtract canary follows the same path and exercises
both signed `i64` bounds through real C ABI calls.
The v7 wrapping-multiply canary round-trips, verifies, costs one
operation plus one return edge, and executes parameter-fed `u8` 20*13 as 4
through a real C ABI call.
The v8 saturating-multiply canary follows the same two-unit path and
executes parameter-fed signed `i64` multiplication through real C ABI calls,
covering positive overflow, negative overflow, `MIN * -1`, and an ordinary
negative product.

## Boundary

Psi operates on Omega-branded source files and owns every target-neutral stage
through one canonical terminal representation. Omega consumes terminal Psi; it
does not feed source-shaped data back into Psi.

```text
Psi
    source files
    -> tokens -> syntax -> resolved -> typed -> checked
    -> lowered expressions, predicates, places, blocks, and edges
    -> terminal Psi

Omega
    terminal Psi
    -> abstract operations -> target operations
    -> assigned instructions -> bytes -> installed image
```

Parsing therefore belongs to Psi. “Omega files” is the language and product
branding; Psi is the frontend, semantic verifier input, and portable execution
representation.

## Why no existing stage is the cut

The current implementation has no expression-lowering pass before instruction
selection:

- `CheckedTrees` embeds `TypedTrees` plus checked fact tables;
- `StateGraphCode` copies the typed expression table, and operations and
  transitions retain `ExpressionHandle`;
- `ControlFlowCode` clones the same expression table and mostly remaps the
  graph topology and semantic arenas; and
- abstract-operation construction and instruction selection still inspect and
  substitute tree expressions directly.

`StateGraph` and `ControlFlowPlan` are therefore useful topology and evidence
scaffolds, not self-contained executable representations. Conversely,
`AbstractOperations` already owns runtime storage regions, calling-convention
classes, ABI aggregate distinctions, and other Omega realization concerns.
Removing those fields would not reveal a hidden portable IR.

The missing pass is the boundary: merge the useful state-graph/control-flow
shape and fill it with lowered semantic content. This is not serialization of
today's `StateGraph`, purification of `AbstractOperations`, or a second similar
block IR placed beside `ControlFlowPlan`.

## Terminal requirements

Terminal Psi is immutable and self-contained. It contains no arena handle that
requires `TypedTrees`, source syntax, the producer compiler, or instruction
selection to interpret its meaning. It contains:

- concrete machines and instantiated types;
- explicit typed blocks, block parameters, values, calls, transitions,
  continuations, and terminals;
- lowered predicates over the same stable value/place identities as execution;
- typed structural places, including ordinary and provider-backed roots plus
  field, dynamic-index, dereference, and range/subextent projection;
- explicit cleanup, transfer, conservation, invalidation, suspension, and
  boundary actions on edges;
- closed semantic operation variants, including scoped CPU/device ordering
  events; and
- fingerprinted contracts, obligation schemas, authorized admission sites,
  trust attribution, and work identities.

Nominal proposition declarations retain their binder telescopes,
fact-only/witness-bearing classification, and any normalized carrierless
evidence interface in this fingerprinted vocabulary. Changing that interface
is a semantic proof-API revision even though the proposition keeps its nominal
symbol. Transparent proposition definitions expand before terminal production,
have no independent semantic identity, and retain their source names only in
debug maps.

Relation applications retain their independently bound left and right carrier
index packs; no global carrier-parameter role is serialized. Selected
constructor lifts, dependency-ordered field relations, and every required
proposition-transport proof enter the semantic rows that justified a lifted
operation. Callable argument telescopes use positional identity, with source
parameter names confined to debug metadata.

An erased binding remains in typed semantic and proof rows with its
multiplicity, validity scope, conservation obligations, and provenance. It has
no executable storage place or cleanup action. Runtime layout and operation
encoding consume the erased-stripped form, while semantic fingerprints retain
the binding and its type.

Author-declared hardware geometry is semantic and may contain offsets, widths,
and alignment. Omega begins where the target chooses native layout, stack and
register placement, ABI classes, concrete storage regions, instructions, and
relocations.

## Psi operation definition

Every operation enters the vocabulary as one reviewed vertical slice:

```text
operation identity and canonical encoding
execution transition
generated obligations and authorized admissions
proof rule / logical interpretation
soundness proof of that rule against the transition
interpreter realization
Omega lowering requirement
fuel identity
```

Operations are statically distinct when execution semantics or generated
obligations differ. Obligation-affecting policy is a closed instruction variant,
not an ordinary value that requires constant folding before verification.
Additional sound proof lemmas may be published without changing operation or
program identity.

The proof kernel, proposition representation, total primitive judgments,
certificate envelope, and admission taxonomy land before an operation depends
on them. Concrete proposition and operation vocabularies are then co-designed
in vertical slices; the proof language is not speculated in isolation.

### Content-conservation proposition slice

The content slice extends structural-place terms with an entry/current version;
it does not add a general historical-expression modality. It carries the exact
owner-unique content-projection identity, canonical
`IntervalSet<CoordinateSpace>` and `CountedQuantity<Unit>` terms, variadic
partial `separate(...)`, containment and equality, and canonical interval-set
residual difference. Sealed claim-frontier rows record content introduced into
or transferred out of checked custody.

The verifier infers identity-preserving reshuffles. A primitive that changes a
partition carries an authored theorem, and checked wrappers compose those
theorems. At a bodyless partial boundary, Psi derives the kept content and
residual and permits the provider to admit only acceptance of custody for that
exact residual—not the partition arithmetic. External root correspondence and
fresh issuance remain scoped admitted hypotheses with provenance; downstream
conservation remains derived.

### Crash-control slice

Terminal Psi represents `Trap` and `Abort` as closed crash causes attached to
distinct no-successor terminators. A crash terminator is not an ordinary
terminal transition and does not encode abandonment by omitting a cleanup list.
It carries the path-conditioned site guard, derived damage-minimum scope,
covering published route buckets, and the statically known local frontier as an
explicit lower bound. The exact dynamically abandoned frontier is not claimed
to be edge-enumerable.

Published crash buckets are fingerprinted semantic content. Each bucket has
one cause, one nominal containment demand, and a canonical disjunction of route
predicates over the same lowered values and structural places as executable
Psi. Buckets normalize only when both cause and scope match. Omitted scope has
already elaborated to the permanent portable top `ExecutionDomain`; an
unconditional clause contains the canonical `true` predicate.

The verifier independently reconstructs every crash site and checks:

```text
site_guard implies
    OR(covering_guard
       && site_damage_minimum <= covering_containment_demand)
```

Call composition substitutes arguments and caller path facts into published
routes. Disproving every route removes the corresponding crash edge from the
caller's semantic frontier; disproving only wide-scope routes narrows the
remaining containment demands. Evidence derived from a callee body is usable
only when that body is within the same fingerprinted verification unit.
Otherwise the verifier consumes the imported published ceiling and its
certificate.

Psi also checks every surviving route demand against the enclosing
fingerprinted per-cause context maximum. It does not interpret scopes as
processes, matrices, partitions, or machines. Omega binds those nominal tokens
to a selected fault plan and records evidence that the realized target scope is
at least the route's damage demand and no wider than the context permits.

The reference interpreter does not return a crash as data. Reaching a crash
terminator yields a distinct interpreter outcome carrying its cause and
semantic site identity. Build-time evaluation rejects any invocation with a
surviving crash route; a concrete invocation that disproves all routes remains
admissible. Native lowering may retain a physical check even when a caller has
proved its semantic edge unreachable, unless specialization makes erasure
valid.

Implementation checkpoint (2026-08-02): the source-to-checked precursor and
the first terminal proposition slice are live. Exact owner-projection calls,
entry/current structural-place versions, and flattened canonical
`separate(...)` equations retain one schema-stable fingerprint per callable /
algebra in checked facts and proof/debug artifacts. Terminal semantic v9
declares proof-visible parameter/result roots and carries the exact algebra,
semantic domain, projection fingerprint, versioned stable place path, and
canonical equation without any Omega arena identity. Canonical semantic bytes
and minimal proof format v8 are golden-pinned; verifier checks restrict content
propositions to `ensures`, reject invalid roots and `entry(result)`, and accept
replaceable certificates. Identity-preserving reshuffle inference has a
Psi-checked producer: exact input-relative outcome maps derive one fingerprinted
entry/current equality per preserved claim, retaining its claim identity and
both structural paths. The derivation requires the same terminal projection
identity and algebra on both places, accepts type or ordinary contract
qualification, and never synthesizes separated composition across independent
claims. Fresh establishments, mismatched projections, and runtime indices infer
nothing. Terminal semantic v10 carries field/fixed-index rows in canonical
machine-local claim order; semantic v11 adds distinct stable sum-case path
segments, and proof format v9 carries those segments in certificates. The
verifier revalidates one-to-one, non-overlapping
parameter-entry/result-current paths, exact projection and algebra identity,
and reconstructs one content-equality semantic axiom per projection for
replaceable certificates. Terminal semantic v12 adds exact direct-wrapper
partition composition: canonical rows retain the source theorem and
fingerprint, dense participating claims, total structural-place substitution,
and derived theorem. Validation requires an authored separation tree, checks
source and wrapper place roles, and mechanically replays the substitution
before exposing the derived proposition as a semantic axiom. Archived v12-v13
modules bind every derived entry projection through the listed identity row;
v14 instead resolves it through the independently encoded entry-claim
binding. Existing proof format v9 already represents the resulting proposition.
Composition through surrounding non-direct
rewrites, sealed introduction and custody-exit frontier rows, and the general
frontier theorem remain to land.

The first real-source integration attempt exposed and has now closed one
missing semantic row. A
direct partition wrapper has checked entry claim identities and an exact
partition substitution, but correctly has no one-to-one identity reshuffles:
aggregate conservation does not establish that either input equals a particular
output. Terminal semantic v14 adds a fingerprinted machine-local entry-claim
binding containing dense claim identity, projection, algebra, and entry
structural place, with no output and no equality assertion. Partition
compositions reference those bindings; `ContentIdentityReshuffle` remains the
one-to-one equality case. Validation requires one canonical binding per claim
identity, rejects duplicate or overlapping entry subjects, and permits content
axioms to reference the same semantic subject. The proof adapter does not turn
the binding itself into an axiom. The checked producer retains exact
claim-to-entry-place rows, emits dense v14 bindings for reshuffle and
partition-only claims, and remains fail-closed on ambiguous bindings.

Correction checkpoint (2026-08-03): checked-to-terminal content production now
lives in `psi-checked-trees-to-terminal`. It consumes Psi-owned checked facts;
the v9-v14 terminal vocabulary, canonical codec, and verifier remain Psi-owned
and source-independent. The deleted Omega-to-Psi translator must not return.

These normalized obligations are semantic and fingerprinted. Their proof
derivations remain replaceable proof-bundle material.

## Verification boundary

The artifact verifier, proof kernel, and proof producer have distinct jobs:

```text
producer            emits canonical terminal Psi plus candidate evidence
artifact verifier   derives what that exact module is required to prove
proof kernel        checks derivations of those required propositions
```

The artifact verifier canonical-decodes terminal Psi, validates its structure,
derives structural obligations from every operation and edge, and retains the
fingerprinted author contracts. It matches evidence only after reconstructing
the complete obligation set. The proof bundle is not an obligation manifest and
cannot choose what is sufficient. Missing obligations, extra evidence, changed
propositions, wrong module/obligation identities, and unauthorized admission all
reject.

Every accepted fact is:

- re-decided by a specified total kernel judgment;
- discharged by a checked certificate, carried or reconstructed by a total
  certifying procedure; or
- admitted at a sealed site and accepted by the consuming profile.

Admission cannot replace a derivable obligation. Search that may time out or
return unknown must carry its certificate for portable verification. Primitive
trusted judgments are minimized and each joins the enumerable language
soundness audit.

The semantic module, proof bundle, installation record, and debug/source maps
remain separate. Proof improvements do not change semantic identity; provider
selection and attached evidence do change their own section and container
identities. One execution verifies and runs one complete Psi semantic version.

`psi-terminal-verifier` is the current Rust implementation of the artifact-aware
judgment; `psi-proof-kernel` implements its current proof checks. That is an
implementation milestone, not the final trust placement. Before terminal-Psi
PCC becomes the deployment authority, the project must choose and implement one
of these auditable closures:

- a low-rung reference artifact verifier that reconstructs the same obligations;
- a Psi verifier that emits an obligation-reconstruction derivation accepted by
  the low-rung proof kernel; or
- an explicitly trusted Psi artifact verifier, named as such in the trust ledger.

A future Psi-hosted proof-kernel implementation may accelerate or independently
cross-check certificate validation. It does not by itself discharge the separate
obligation-reconstruction trust question.

## Canonical semantic bytes (format v1)

`psi-terminal-codec` owns the canonical encoding of the supported in-memory
vocabularies. Wire format v1 begins with `PSITERM\0`, a little-endian `u16`
format version, and the terminal semantic version. Counts are fixed-width
little-endian `u32`, stable identities are nonzero little-endian `u64`, integer
payloads occupy the full signed or unsigned 128-bit field, and every sum type
uses a closed one-byte tag. This intentionally favors one simple auditable
encoding over density.

Machines, blocks, and v9 structural-place declarations are strictly ordered by
their stable identities; ensures are strictly ordered by obligation identity. Requirements and flattened
conjunction members are strictly ordered by their canonical encoded bytes,
duplicates are rejected, and symmetric equality operands use that same wire
ordering. Content equations order their symmetric sides canonically;
`separate(...)` is flat, sorted, duplicate-free, and exact projection/domain,
entry/current place, field, fixed-index, and v11 sum-case identities are encoded. Nested
conjunctions, proposition nesting, recursive scalar terms, and content terms
deeper than 256 edges are rejected. Execution-significant vectors—parameters, operations, and
jump arguments—retain their declared order.

Decoding fails on unknown versions or tags, zero identities, invalid booleans,
noncanonical ordering/forms, malformed or verifier-invalid modules, truncated
input, and trailing bytes. A successfully decoded module is re-encoded and the
bytes must match exactly; the decoder never normalizes an alternate encoding.
The semantic fingerprint is SHA-256 over a v1 domain separator, the canonical
byte length, and those exact bytes. `TerminalPsiIdentity` contains only the
semantic version and this fingerprint: proof bundles, installation records,
and debug maps are deliberately absent and remain replaceable.

Semantic version 1 is frozen with `IntegerConstant`; version 2 adds
`BooleanConstant`; version 3 adds `WrappingIntegerAdd`; version 4 adds
`SaturatingIntegerAdd`; version 5 adds `WrappingIntegerSubtract`; version 6
adds `SaturatingIntegerSubtract`; version 7 adds `WrappingIntegerMultiply`;
version 8 adds `SaturatingIntegerMultiply`; version 9 adds proof-only
structural places and content-conservation propositions; version 10 adds
canonical identity-preserving claim reshuffles; version 11 adds stable sum-case
content-path segments; version 12 adds exact authored-partition substitution
rows; version 13 adds the ordered Boolean conditional terminator; version 14
adds canonical machine-local entry-claim bindings without asserting an output
equality; version 15 adds total `BooleanNot` operations and scalar terms;
version 16 adds self-contained nominal proposition declarations and normalized
applications without adding an operation; version 17 adds total `BooleanEqual`
operations and recursive scalar terms; version 18 adds total `IntegerEqual`
operations and recursive scalar terms; version 19 adds distinct
`IntegerLessThan` and `IntegerLessOrEqual` operations and recursive scalar
terms; version 20 adds distinct `IntegerBitwiseAnd`,
`IntegerBitwiseOr`, and `IntegerBitwiseXor` operations and recursive scalar
terms; version 21 adds total wrapping left and right shifts with an independent
integer count type; version 22 adds the explicit `Crash` terminator, closed
`Trap`/`Abort` cause, nominal damage scope, and canonical machine-local
frontier lower bound; version 23 separates the body-derived damage minimum
from the selected published containment demand; version 24 adds canonical
sparse per-cause crash-context maxima; version 25 adds total
`IntegerBitwiseNot` operations and recursive scalar terms; version 26 adds
total range-contained `IntegerWiden` operations and recursive scalar terms; and
current version 27 adds a distinct address-carrier integer-type tag without a
new executable operation.
The arithmetic operations require two already defined operands of the exact
result integer type and have distinct canonical recursive proposition terms for
their exact logical results. Boolean equality requires two already defined
Boolean operands and reconstructs their exact equality result. Integer
equality requires two already defined values of one exact integer type and
reconstructs a Boolean result equating their representations. Integer ordering
has the same operand/result discipline and reconstructs the exact signedness-
aware relation. Bitwise operations require and return one exact integer type
and reconstruct the exact representation-level result. Integer widening
requires the target to contain the complete source range and reconstructs the
unchanged mathematical value at the result type. Validation and
execution continue to accept valid v1 through v26 modules under their original
meaning, while an older module
cannot claim a later operation, control form, or evidence row.
`migrate_module_to_current` is an explicit validated older-to-v27 translation.
For v10-v13 content rows it derives the new entry bindings from the already
validated reshuffles and remaps claim references into dense machine-local IDs;
it otherwise preserves the graph and obligations. Migration creates new
canonical bytes and a new semantic fingerprint. An unchanged proof bundle
retains its separate bytes and identity but is verified again against the
migrated module. Golden tests retain archived v1 through v24 identities and
independently freeze the v25 integer-bitwise-complement fixture, the v26
integer-widening fixture, and the current v27 address-carrier fixture, plus the
v10 identity-reshuffle fixture, v11 sum-case
fixture, v12 partition-composition fixture, v14
entry-claim fixture, v15 Boolean-negation fixture, v16 proposition-vocabulary
fixture, v17 Boolean-equality fixture, v18 integer-equality fixture, the
distinct v19 integer-ordering fixtures, the distinct v20 integer-bitwise
fixtures, and the distinct v21 wrapping-shift fixtures.

The same codec gives proof bundles their own canonical `PSIPRF` bytes and golden
fingerprint. Proof format v1 remains the minimal frozen encoding for the
original proposition vocabulary. Format v2 adds the recursive wrapping-add
scalar term; format v3 adds the recursive saturating-add scalar term; format v4
adds the recursive wrapping-subtract scalar term; format v5 adds the recursive
saturating-subtract scalar term; format v6 adds the recursive wrapping-multiply
scalar term; format v7 adds the recursive saturating-multiply scalar term;
format v8 adds content-conservation propositions and field/fixed-index
structural-place terms; format v9 adds sum-case path segments; format v10 adds
recursive Boolean-negation terms; format v11 adds recursive Boolean-equality
terms; format v12 adds recursive integer-equality terms; format v13 adds
recursive integer less-than and less-or-equal terms; and format v14 adds
recursive integer bitwise AND, OR, and XOR terms; format v15 adds recursive
wrapping left/right shift terms with independent value and count integer
types; format v16 adds recursive integer bitwise-complement terms; format v17
adds recursive integer-widening terms with exact source and target types; and
format v18 distinguishes address-typed terms from ordinary same-width unsigned
integer terms.
The encoder selects the minimal format needed by a carried proof tree, and the
decoder rejects a bundle encoded with a newer format than its proof tree needs.
Evidence entries are strictly ordered by `ObligationId`; the
closed encoding covers kernel judgments, separately versioned recursive proof
trees, and exact admission site/authority/evidence/profile identities. Unknown
tags, zero identities or proof versions, alternate evidence ordering,
truncation/trailing data, malformed propositions, and proof/proposition nesting
beyond the v1 bounds reject. Proof-tree propositions retain their exact rule
direction rather than being normalized as semantic contracts, because a proof
section is replaceable evidence and its cited axiom direction is significant.

`TerminalArtifactManifest` binds the canonical semantic and proof identities
plus optional installation and debug section hashes. Each role has a separate
SHA-256 domain, and absent differs from a present empty section. Replacing a
valid proof, installation record, or debug map changes that section and the
container identity while preserving `TerminalPsiIdentity`; validation
recomputes the complete manifest from attached bytes.

The first typed installation payload is live in
`omega-terminal-image-emission`. Wire format v1 begins with `PSIINST\0` and
binds the terminal semantic identity, architecture, object format, pointer
size/alignment, PE subsystem when present, exact profile-decision identity,
strictly ordered selected-provider-plan identities, a domain-separated SHA-256
of the complete emitted image, and the compiler text-validation evidence. Its
decoder rejects unknown versions/tags, zero identities, invalid target facts,
alternate provider order, nonzero reserved fields, truncation, and trailing
bytes, then reproduces the canonical bytes. Validation recomputes the image
binding from the sealed `TerminalExecutableImage`. The scalar canaries carry an
empty provider set because they contain no calls or boundaries; later vertical
slices populate that set from actual selected plans. The record is manifest
metadata, not executable authority and not a replacement for the separate
`omega-executable-installation` admission/placement ladder. Typed debug-map v1
is independently live in `psi-terminal-codec`; it is replaceable presentation
metadata bound to one exact semantic identity. The checked-source producer
populates retained declaration spans plus exact integer/Boolean-literal and
operator sites for terminal operations and their result values. Authored jump
edges use their exact transition-arrow sites; synthesized return edges retain
the exact returned-expression site.

## Logical-fuel v1

`psi-terminal-fuel` owns the accounting identity independently from terminal
semantic versioning. Schedule v1 charges one logical unit for every executed
operation in the current closed terminal vocabulary and one for every taken
terminal edge, including conditional successors and `Crash`. The cost table
matches the closed operation/terminator enums exhaustively, so a new vocabulary
variant cannot compile without making its schedule treatment explicit. A
schedule revision changes accounting identity, never terminal semantic bytes or
the program fingerprint.

The interpreter charges before executing each semantic site and returns a
deterministic `TerminalFuelUsage`: total units plus execution count and units
aggregated under stable `OperationId`/`EdgeId` attribution. Its sponsor-owned
meter may be unbounded or carry a finite allowance. Insufficient allowance is a
host result before the unpaid site, leaves usage unchanged, and is not visible
or catchable as a terminal-Psi machine result. The serialized real-source
canary costs four v1 units—two constants and two edges—and retains the same
semantic identity before and after accounting. `TerminalExecution` retains the
exact block/operation cursor and values across that sponsor event; checked
replenishment resumes at the unpaid site without replaying or double-charging
earlier work, including in the serialized real-source/native canary. Build-time
migration, attributed response outcomes, and trusted native block metering
remain later IRFUEL slices.

A verified crash consumes its one edge unit before producing a distinct
terminal outcome. Repeated resume reports the same outcome without charging or
executing the edge again.

The v3 wrapping canary also costs four v1 units: two constants, one addition,
and one return edge. Semantic-version migration therefore does not imply a fuel
schedule change. The v4 saturating canary has the same four-unit shape;
each newly admitted operation is reviewed against the closed schedule table.
The v5 parameter-fed wrapping-subtract canary costs two units: one
subtraction and one return edge. It retains schedule v1 because the existing
per-operation rule already determines that cost.
The v6 parameter-fed saturating-subtract canary has the same two-unit
shape and independently reaches both signed `i64` bounds.
The v7 parameter-fed wrapping-multiply canary also costs two units and
computes `u8` 20*13 as 4.
The v8 parameter-fed saturating-multiply canary costs two units and
reaches both signed `i64` bounds.
The explicit same-carrier policy-cast canary likewise costs two units—one
selected wrapping addition and one return edge—while a direct policy erasure
costs only its return edge. Neither static retag changes semantic or proof
format versions because neither introduces terminal vocabulary.
The unary-negation canaries cost three units: one exact-width zero constant, one
Wrapping or Saturating subtraction, and one return edge. They reuse existing
semantic/proof vocabulary and therefore require no format-version change.
The direct integer-widening canaries cost two units: one widening operation and
one return edge. The nested wrapping-add companion costs four units: one
widening, one constant, one addition, and one return edge. Artifact-root tests
round-trip canonical sections, interpret exact signed and unsigned results,
and execute full-width host comparisons after both native selectors emit.

`psi-terminal-fixed-fuel` provides the first restricted checker over this same
schedule. It derives the maximum entry-to-terminal-exit cost over the verified
acyclic CFG with no additional precondition assumptions, memoizing shared tails and
taking the greater successor cost at a conditional rather than summing mutually
exclusive arms. The certificate keys the canonical terminal-Psi identity,
entry machine, schedule identity, and ceiling.
Validation recomputes every field from the verified decoded module; changing
program semantics invalidates an old certificate even when the numeric cost is
unchanged, and a verified but noncanonical module cannot acquire semantic
identity. The source canary's exact four-unit certificate equals measured
execution after source and producer state are discarded. Exact machine-local
block-to-edge segment certificates now reuse the same canonical identity and
schedule, include their selected jump, conditional, or return edge, and reject
an endpoint that is not reached before return. Every explicit edge is a
semantic safe point; the checker derives and validates the complete reachable
graph partition in canonical block/edge order so no branch segment can be
omitted or reordered. Crossing a conditional within one unselected segment
still fails closed. Loop outcomes, relevant-precondition subsets, and Cathedral
hard-root migration remain later slices.

Omega external-root composition now accepts those sealed entry and segment
certificates as a distinct local-evidence form beside admitted opaque-provider
summaries. It derives local units and schedule from the certificate, retains no
provider receipt for recomputable Psi evidence, and reports the terminal
semantic identity and exact entry/segment endpoint. A sealed Omega binding now
checks that terminal artifact text is exactly the relocation-free frozen
installed bytes, that architectures match, and that the selected entry stub
names the certified function offset. External-root installation rechecks the
whole-entry certificate against the exact root code context and stub; a
segment-only root fails closed. The real-source canary crosses the complete
generic installation ladder. Migrating the Cathedral hard-root graph remains.

## Migration plan

1. **Ownership migration complete for source-to-checked packages:** the
   target-neutral parsing-through-checking crates and representations live
   under Psi ownership, and their former Omega compatibility packages are
   retired. No parser or semantic checker remains on an Omega-to-Psi path.
2. Extend the live stable Psi value, proposition, proof, and place identities
   into the first terminal semantic module without changing the current backend.
   **Initial scalar subsets complete:** frozen v1 integer constants, v2 Boolean
   constants, v3 wrapping integer addition, v4 saturating integer addition, v5
   wrapping integer subtraction, v6 saturating integer subtraction, v7
   wrapping integer multiplication, and v8 saturating integer multiplication
   have verifier, direct-interpreter, canonical-codec, fuel, Omega-lowering,
   and native-return coverage. The runtime-parameter slice covers direct returns plus recursive
   wrapping/saturating addition, subtraction, and multiplication
   expressions over native register and incoming-stack ABI locations. The v15,
   v17, v18, and v19 Boolean-negation, Boolean-equality, integer-equality, and
   integer-ordering slices likewise cross validation, canonical encoding,
   fuel, interpretation, and native lowering. The v9
   content slice has canonical semantic/proof bytes, checked-plan translation,
   and certificate verification; v10 carries and revalidates checked
   identity-reshuffle rows as semantic axioms, while sealed frontier rows remain.
   Executable storage places, general register assignment,
   and the other arithmetic variants remain later slices.
3. Lower the live integer/control/contract slice from Psi checked semantics
   into terminal Psi, add its Omega abstract-operation consumer, and
   compare interpreted/native behavior before broadening the vocabulary.
   **Initial vertical slice complete through native comparison:** the
   fail-closed Psi terminal producer and real source canaries now verify and
   execute after checked trees are dropped, then lower the verified module into
   an owned, source-independent Omega requirement stream, a target
   return-immediate, host machine code, an owned object artifact, and a direct
   host image whose execution matches interpretation. A nine-parameter source
   machine also returns its ninth `u8` through the selected host incoming-stack
   ABI, and a runtime wrapping-add source machine combines that stack argument
   with the first register argument. A recursive add-then-multiply source
   expression reaches the same interpreted/native result. A parameterized
   two-state source machine carries a register-plus-stack expression through a
   jump binding, continues from the block parameter, and matches the same
   interpreted/native result with a five-unit fixed-fuel certificate. A
   three-state companion repeats the binding/computation step and matches with
   an eight-unit certificate. A two-binding companion carries the complete
   ordered pair across both edges and matches with a ten-unit certificate. The same exact-text image
   boundary is structurally exercised for all four currently supported
   architecture/format pairs.
4. Add the remaining arithmetic variants, calls, continuations, cleanup,
   conservation inference/frontiers, boundary operations, suspension, and scoped ordering as
   reviewed vertical slices.
5. Move binding substitution and concrete instantiation above terminal Psi so
   no Omega pass consumes source expressions.
6. Re-root the reference interpreter, rebuilding differential-oracle evidence
   during the transition.
7. Re-root abstract-operation construction on terminal Psi, then retire the
   redundant state-graph/control-flow representation and adapters.
8. Freeze canonical serialization and semantic fingerprints only after the
   in-memory vocabulary has passed interpreter and lowering canaries.
   **Initial vocabulary complete:** canonical semantic bytes and identity now
   round-trip through the real-source interpreter/native canary. Canonical
   proof bytes and role-separated semantic/proof/install/debug manifest hashes
   are also live. Semantic migration is exercised: archived v1 and v2 bytes
   retain their identities and migrate explicitly into separately fingerprinted
   current-v25 modules; archived v3 wrapping-add, v4 saturating-add, v5
   wrapping-subtract, v6 saturating-subtract, and v7 wrapping-multiply
   identities plus the v8 saturating-multiply identity are frozen as well. Typed
   installation records, the canonical typed debug/source-map schema, and
   declaration-span population from the checked-source producer are live;
   exact operation and authored transition-arrow sites are retained; broader
   source-provenance coverage grows with later executable slices.

The migration may keep old and new paths temporarily for comparison. That is a
testing bridge, not a permanent two-semantics architecture.
