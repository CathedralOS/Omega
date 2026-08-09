# Design Brief: Terminal Psi, Fuel, And Resource Provisioning

Status: canonical Psi architecture settled 2026-08-02. The hard-root accounting
precursor is schedule-keyed and uses logical-fuel provisions. The first
terminal-Psi schedule, interpreter meter, and acyclic maximum-path fixed-entry
and safe-point checkers are live; build-time migration, response outcomes, and
native metering remain implementation work. The current
TypedTrees evaluator-step schedule is
telemetry precursor evidence, not canonical-Psi fuel. The implementation cut
and migration are detailed in
[`terminal_psi.md`](../architecture/pipeline/terminal_psi.md).

Foundation checkpoint (2026-08-02): Psi-owned core and proof-kernel crates now
carry stable semantic identities, typed scalar propositions checked against a
module value table, total primitive judgments, explicit structural proof
certificates, and sealed admission evidence bound to an exact authorized site,
authority identity, evidence identity, and installation-profile decision. The
kernel rejects admission for a primitively derivable proposition. That
foundation was the prerequisite evidence substrate; the executable checkpoints
below extend it.

Executable checkpoint (2026-08-02): the in-memory terminal semantic module now
carries stable machine/block/value/operation/edge identities, representable
integer constants, v2 Boolean constants, v3 exact-width wrapping integer
addition, v4 exact-width saturating integer addition, v5 exact-width
wrapping integer subtraction, v6 exact-width saturating integer subtraction,
v7 exact-width wrapping integer multiplication, and v8 exact-width saturating
integer multiplication, unconditional jump/return control, and bodyful
contracts. Semantic v9 adds proof-only structural places and content
conservation, v10 adds identity-preserving claim reshuffles, v11 adds stable
sum-case content-path segments, v12 adds exact authored-partition substitution
rows, and v13 adds structural Boolean conditional control without adding an
executable operation. V14 adds independent dense entry-claim bindings without
adding an executable operation or proposition. V15 adds total Boolean logical
negation as an operation and proposition term. V16 adds canonical nominal
proposition declarations and normalized application identity without adding an
executable operation. V17 adds total Boolean equality, v18 adds total equality
over two values of one exact integer type, v19 adds signedness-aware integer
less-than and less-or-equal, v20 adds total integer bitwise AND, OR, and XOR,
v21 adds wrapping left and signedness-aware right shifts, v22 adds an explicit
no-successor crash terminator with a closed cause, nominal damage scope, and
machine-local abandoned-frontier lower bound, v23 separates the body-derived
damage minimum from the selected published containment demand, v24 adds
canonical sparse per-cause context maxima to machine contracts, v25 adds total
fixed-width integer bitwise complement, v26 adds universally total fixed-width
`i*`/`u*` widening whose target contains the complete source range, v27
distinguishes the address carrier from an ordinary same-width unsigned integer
while retaining its current 64-bit representation, v28 adds proof-gated exact
fixed-integer casts, v29 and v30 add proof-gated Exact right and left shifts,
v31 adds proof-gated Exact fixed-integer addition, v32 adds proof-gated Exact
fixed-integer subtraction, v33 adds proof-gated Exact fixed-integer
multiplication, v34 adds proof-gated Exact fixed-integer division, v35 adds
proof-gated Exact fixed-integer remainder, v36 adds proof-gated Wrapping
fixed-integer division, and current v37 adds proof-gated Wrapping fixed-integer
remainder.
Shift counts retain their own integer type and reduce by Euclidean modulo of
the shifted value's width. The verifier
reconstructs operation, edge-binding, and return-binding axioms guaranteed on
every return path, rejects
unreachable fact sources and out-of-scope contract
values, and requires evidence for every `ensures`; the proof kernel checks
semantic-axiom citations, equality composition, and closed integer relations
over the complete current arithmetic vocabulary. Wrapping addition reduces
modulo the declared 1–128-bit width and interprets signed reduced bits as two's
complement;
saturating addition clamps at the declared signed or unsigned bounds. Wrapping
and saturating subtraction apply the same policies to `left - right`;
wrapping multiplication reduces the product at the declared width, while
saturating multiplication clamps it at the declared bounds. All six are total
and create no overflow obligation. Omega's interpreter executes the same
verified module object and rejects out-of-range integer arguments before
execution. Reaching a verified crash reports a distinct terminal outcome after
charging the crash edge once; resumption cannot replay it. Canonical encoding,
validation, and fuel cover the complete v37 row. Artifact-root native lowering
now carries an unconditional crash-only row through Omega target selection and
assignment and emits `ud2` on x86-64 or `brk #0` on AArch64. Recursive Boolean-
and integer-result target control carries the same crash leaf, allowing the
current direct, computed, and short-circuit acyclic guarded graphs to preserve
both their return and crash arms through native emission.
Versions 22–24 record an earlier containment model; their scope and context
fields remain part of those frozen encodings but do not prove survivor safety.
The next crash-schema revision removes them from current production while
preserving validated backward decoding.

The source producer selects a same-cause route from the checked
machine-contract crash plan. The current implementation still groups legacy
rows by cause and containment demand; the settled form groups by cause alone,
removes duplicate guards, treats explicit `true` as unconditional, and feeds
public contract fingerprinting and reporting without reinterpreting typed
syntax.
The checked crash plan also retains a separate implementation-evidence row for
each explicit body site, keyed by state plus state-local statement ordinal and
carrying its derived cause. Terminal production joins the crash statement to
that checked row; the site rows never enter the published contract fingerprint.
Canonical published buckets have dense plan-local identities. The checked row
cites unconditional same-cause buckets as structurally proved guard coverage,
and terminal production consumes this relation rather than searching the
contract. The reconstructed frontier remains independent implementation
evidence and is explicitly only a lower bound on abandoned obligations. Legacy
v23 rows retain a damage minimum and published demand for backward decoding;
neither field licenses survivors. Checker-proved
incoming-path guarded crash branches now lower in the acyclic integer-control
slice, including nested-negation implication, comparison operand reversal, and
negated equality/inequality. Ordered-comparison negation is admitted only for
checked integer operands; integer strict order also yields its non-strict bound
and inequality, while integer equality yields both non-strict bounds. Positive
path conjunctions also close checked-integer order transitively; a derived chain
is strict exactly when at least one link is strict. Two opposed nonstrict
integer relations derive equality by antisymmetry; one-sided bounds do not.
A nonstrict integer relation plus endpoint disequality derives its strict form.
Unknown, user-defined, and float operands remain opaque so unordered values
remain sound. Exact-type
integer comparisons now also lower as executable control
guards over the established recursive integer-expression vocabulary. Greater
forms swap operands into canonical less operations and inequality composes
terminal Boolean negation. Broader guard entailment and source shapes remain.

The first Psi-owned checked-tree producer, `psi-checked-trees-to-terminal`,
lowers a closed set of scalar closed-contract source forms: a recursively nested Boolean
expression over literals, exact named parameters, builtin negation, builtin
equality/inequality, and short-circuit `&&`/`||` from ordinary Boolean
parameters; direct builtin equality/inequality or ordering between two
recursive primitive-integer expressions of one exact type; a recursively nested
expression over exact parameter/literal operands using builtin
wrapping/saturating add, subtract, or multiply, plus bitwise AND, OR, XOR,
unary complement, and wrapping left/right shifts,
from a nonempty sequence of
ordinary primitive-integer parameters; or an
integer-constant/unconditional-jump whose return is the matching literal or a
builtin parameter-plus-literal wrapping/saturating add, subtract, or multiply;
or a rooted acyclic scalar-result graph whose blocks return, jump
unconditionally, or select ordered positive-Boolean/fallback successors.
Selection guards may be Boolean parameters or exact-type builtin integer
comparisons over recursive integer expressions, composed with recursive
short-circuit `&&`/`||`. Short-circuit tests lower into reserved decision blocks
and charge only the executed path.
Same-carrier integer value casts may explicitly select or erase a closed
arithmetic policy inside those expressions. The checked scalar plan retains
the operand and selected policy without retaining a typed-expression handle;
terminal production uses that policy to choose an enclosing operation and
otherwise erases the cast. Because the primitive carrier is unchanged, there
is no terminal operation, proof term, or fuel charge. Strict-width casts whose
target contains the complete source range retain an `IntegerWiden` operation,
preserve the mathematical value at the wider carrier, and cost one operation
unit. This includes unsigned-to-signed widening when the target is wider.
Terminal Psi v28 admits proof-gated exact conversions between distinct fixed
integer carriers when the target does not contain the complete source range.
The checked plan retains the accepted occurrence interval from the existing
validation range engine. Terminal production emits `IntegerExactCast` with a
dedicated obligation; the artifact verifier ignores the retained producer
interval and independently reconstructs the stricter target bound or bounds
from the carrier types and path facts. The first live route derives a true-edge
exact-type integer comparison, substitutes terminal constants, and rewrites
the fact through successor parameters. The operation costs one unit. More
complex range proofs reject if that independent reconstruction cannot derive
the obligation. A compile-known exact fixed-integer cast whose literal is
representable in the target re-lands as the existing target-typed constant and
therefore adds neither an operation nor fuel. Terminal Psi v27 retains `addr`
distinctly from `u64` through declarations, scalar terms,
comparisons, artifacts, and realization; cross-carrier conversions to or from
`addr` remain rejected.
Terminal Psi v29 admits proof-gated Exact integer right shift when the checked
count range lies within `[0, value_width)`. The artifact operation carries a
dedicated obligation, and the verifier independently rebuilds the required
lower/upper count proposition from the exact carriers and path facts. The
operation costs one unit. Terminal Psi v30 adds proof-gated Exact integer left
shift with one operation-owned conjunction that separately proves count
validity and value representability; proof format v21 carries its recursive
term, and the operation also costs one unit.
Terminal Psi v31 adds proof-gated Exact fixed-integer addition. The verifier
uses terminal literals/equalities to identify one constant addend and derives
the exact carrier upper or lower bound required of the other addend. It does
not trust the checked interval that admitted the source expression. Proof
format v22 carries the recursive exact-add term, the operation costs one unit,
and verified Omega lowering uses the ordinary fixed-width target add. Two
unrelated runtime addends remain fail-closed until the terminal proposition
surface can express and prove their joint relation.
Terminal Psi v32 adds proof-gated Exact fixed-integer subtraction. The verifier
uses terminal literals/equalities to identify a constant right operand and
derives the exact carrier lower or upper bound required of the left operand. It
does not trust the checked interval that admitted the source expression. Proof
format v23 carries the recursive exact-subtract term, the operation costs one
unit, and verified Omega lowering uses the ordinary fixed-width target
subtract. An unknown right operand and other two-runtime relational shapes
remain fail-closed until the terminal proposition surface can prove them.
Terminal Psi v33 adds proof-gated Exact fixed-integer multiplication. The
verifier uses terminal literals/equalities to identify either constant factor
and derives the exact carrier interval required of the other factor, including
negative signed factors and the signed-minimum negation edge. It does not trust
the checked interval that admitted the source expression. Proof format v24
carries the recursive exact-multiply term, the operation costs one unit, and
verified Omega lowering uses the ordinary fixed-width target multiply. Two
unrelated runtime factors remain fail-closed.
Terminal Psi v34 adds proof-gated Exact fixed-integer division. The operation
owns one obligation, and the verifier resolves a terminal-known right operand
without trusting the producer's range result. Nonzero unsigned divisors and
signed divisors other than negative one are total. Signed negative one requires
`MIN + 1 <= dividend`; zero and a runtime-unknown divisor fail closed. Two
known operands reduce to truth only when truncating-toward-zero division is
defined and admitted. Proof format v25, canonical semantics, exact fuel,
artifact interpretation, and both native targets carry the complete row.
Terminal Psi v35 adds proof-gated Exact fixed-integer remainder. The operation
owns one obligation, and the verifier reconstructs the same known-divisor
definedness boundary as division. Nonzero unsigned divisors and signed divisors
other than negative one are total; signed negative one requires
`MIN + 1 <= dividend`. Zero and runtime-unknown divisors fail closed. Two known
operands reduce to truth only when truncating remainder is defined and admitted.
Proof format v26, canonical semantics, exact fuel, artifact interpretation, and
both native targets carry the complete row. Signed `%` keeps the dividend's
sign and is distinct from Euclidean modulo used for wrapping shift counts.
Terminal Psi v36 adds proof-gated Wrapping fixed-integer division. A
terminal-known nonzero divisor reconstructs truth, including signed negative
one because the mathematical quotient overflow reduces to `MIN` at the declared
width. Zero and runtime-unknown divisors fail closed. Proof format v27,
canonical semantics, exact fuel, artifact interpretation, and both native
targets carry the complete row. The x86-64 realization guards the `-1` divisor
before `idiv`; AArch64's `sdiv` already produces the required wrapped result.
Terminal Psi v37 adds proof-gated Wrapping fixed-integer remainder. A
terminal-known nonzero divisor reconstructs truth, including signed negative
one because `MIN % -1` is zero. Zero and runtime-unknown divisors fail closed.
Proof format v28, canonical semantics, exact fuel, artifact interpretation, and
both native targets carry the complete row. The x86-64 realization returns zero
before `idiv` for divisor `-1`; AArch64 derives the result with `sdiv` and
`msub`.
Declared semantic-domain casts remain rejected until their own executable
vocabulary exists.
Unary integer negation follows the parser's settled `0 - value` lowering. The
checked scalar plan contextually lands the compiler-generated anonymous zero at
the validated operand carrier, so terminal production emits an ordinary
constant plus Wrapping or Saturating subtraction rather than a parallel
negation operation.
Unconditional jumps may compute recursive exact-typed integer bindings;
Boolean targets additionally accept recursive non-short-circuit literal,
negation, equality/inequality, and exact-type integer-comparison bindings. An
integer-result graph may carry such a value through an ordinary Boolean block
parameter and use its recursive expression as native control on both targets.
Mixed-scalar short-circuit tuples use typed left-to-right stages, carry original
parameters and prior results to each later stage, and converge once at the
authored target. Conditional-edge stages remain arm-local, so the unselected
tuple is neither executed nor charged. A pure unconditional mixed-scalar graph
enters this typed path directly rather than requiring an artificial source
selector. All Boolean- and integer-result shapes now share the same general
typed DAG producer, including contract-free all-crash graphs and one-state
returns. Boolean results may retain mixed-scalar bindings, short-circuit
returns, and checked crash leaves. The duplicate direct-parameter, comparison,
Boolean-return, integer-chain, three-state conditional, Boolean-chain, and Boolean-DAG
and crash-only producers are retired. All-crash graphs carry no return
obligation or proof evidence.
Computed conditional-edge bindings use synthesized arm-local blocks so only
the selected expression executes and consumes fuel. Nested selections, linear
prefixes, and convergent tails use the same terminal block and edge
vocabulary. Compile-known integer evaluation follows lowered comparison
selectors and recursive bindings, including typed Boolean values carried into
later integer-result control. It meets scalar facts conservatively at joins,
rejects an unrelated closed integer contract after a known Boolean selection,
and reports no total result when a crash exit remains reachable. The ordered
Boolean-result form supports ordinary Boolean entry/branch parameters with
recursive short-circuit guards and branch returns.
The general Boolean-result form supports rooted acyclic nested selections and
convergent tails with recursive unconditional bindings and short-circuit
returns. Short-circuit jumps converge at value-producing decision leaves;
ordered multi-value tuples use left-to-right stages that carry already-produced
values through each later decision tree before one final target jump. The
general path accepts pure unconditional multi-value graphs without a synthetic
selector.
Conditional successors use those same arm-local binding blocks, so recursive
and short-circuit payloads execute only after their edge is selected.
Compile-known Boolean facts propagate through the lowered DAG and meet at
joins; a closed result rejects a reflexive contract naming the other literal.
Checked contract plans retain the accepted closed Boolean/integer
requires/ensures equality as a source-handle-free carrier. Terminal production
consumes that carrier and fails closed instead of reopening typed contract
facts; contract-free all-crash graphs carry no value clause.
Checked proof facts also retain nominal proposition declarations and normalized
applications after transparent aliases and source handles are eliminated, so
terminal production assigns its dense proposition identities without reopening
typed proof facts.
Checked value facts retain the accepted executable scalar expression tree for
each return, guard, and transition argument under a stable state/statement-role
location. Operator selection, primitive types, landed literals, arithmetic
domains, comparison normalization, and positive-guard normalization are fixed
before terminal production; the producer no longer derives any of that meaning
from typed expression nodes. Typed statement topology and debug spans remain a
temporary presentation input only: checked flow facts now retain ordered state
identity, primitive signatures, terminator shape, stable successors, and
argument arity for semantic production. Stable checked machine-name and
signature-eligibility rows also drive selection. Replaceable debug-map
attachment consumes an optional checked presentation plan of stable spans and
source-file metadata. The terminal producer has no typed-tree input dependency;
the complete typed root can be discarded before semantic, proof, and debug
production, and dropping only the presentation plan simply omits the debug map.
It emits the semantic module and proof bundle separately and fails closed on
all other shapes. Its canaries drop the frontend trees before terminal
verification and interpretation; ninth-parameter `bool` and `u8` machines
additionally cross the selected host incoming-stack ABI, while runtime wrapping
add combines its ninth stack argument with its first register argument and a
nested add-then-multiply expression reaches the same native lane. A three-state
companion also carries two independently computed bindings across each
unconditional edge. A selected-edge companion computes distinct add/multiply
bindings in synthesized true/false blocks and reaches both native targets. A
Boolean companion selects either a staged short-circuit tuple or an ordinary
computed tuple without executing the other arm. An integer-result companion
computes Boolean inequality on an unconditional binding, carries it across the
terminal edge, and uses it as emitted AArch64/x86-64 control. Two mixed-scalar
companions stage `&&` with integer payloads on unconditional and selected
conditional edges, preserving bypass fuel and reaching the same native lanes.
Their compile-known wrong-contract companion follows `true && false` through a
Boolean block parameter, selects integer `9`, and rejects a contract naming
`8`. A no-selector companion carries a staged Boolean and an integer through a
pure unconditional graph while retaining bypass fuel. The source control
canary executes each nested path after
frontend disposal, meters only selected edges, and retains every successor and
shared-tail edge at the Omega abstract boundary. Parsing
through checked semantics and this first terminal producer are now Psi-owned;
general terminal vocabulary must extend the same direction. The same producer
independently revalidates checked content
conservation fingerprints, exact claim-preserving reshuffles, and direct
partition-composition substitutions before emitting their canonical terminal
v9-v12 evidence rows; the executable canary remains content-free. A
source-independent Omega abstract-operation consumer accepts only the verified
module and emits owned scalar-materialization, wrapping-add, saturating-add,
wrapping-subtract, saturating-subtract, wrapping-multiply,
saturating-multiply, Boolean-not, Boolean-equality, integer-equality,
integer-less-than, integer-less-or-equal,
integer-bitwise-not, integer-bitwise-AND, integer-bitwise-OR, integer-bitwise-XOR,
wrapping-shift-left, wrapping-shift-right,
jump-binding, structural conditional, and return requirements
with stable Psi provenance and no source
handles. The clean
target continuation resolves the current compile-known stream to a
provenance-retaining immediate return, emits AArch64 and x86-64 machine code,
and executes the emitted host entry in a linker harness with the same result as
terminal interpretation. The first runtime-parameter continuation now retains
declared parameter/result types, selects their AAPCS64, System V AMD64, or
Microsoft x64 register/incoming-stack locations through the established call
planner, and emits direct scalar parameter returns on both architectures. A
nine-`u8` canary forces a stack argument and agrees with interpretation at 77.
The next continuation lowers recursive parameter-fed wrapping/saturating
integer expressions and emits signed or unsigned 8/16/32/64-bit native
arithmetic on both architectures. A nested `u8` register/stack canary and a
signed `i64` two-bound canary agree with interpretation through real C ABI
calls. An explicit assigned-target stage validates exact target register homes
and assigns stable aligned AArch64 plus scratch-conflicting x86-64 input spills
before terminal emission. Broader liveness allocation, spill reuse, and
non-scalar homes remain implementation work.
Semantic v5 and proof format v4 add recursive wrapping-subtract vocabulary;
semantic v6 and proof format v5 add recursive saturating-subtract vocabulary;
semantic v7 and proof format v6 add recursive wrapping-multiply vocabulary;
semantic v8 and proof format v7 add recursive saturating-multiply vocabulary;
semantic v15 and proof format v10 add recursive Boolean-negation vocabulary;
semantic v16 adds proposition declarations without executable vocabulary; and
semantic v17 plus proof format v11 add recursive Boolean-equality vocabulary;
semantic v18 plus proof format v12 add recursive integer-equality vocabulary;
semantic v19 plus proof format v13 add recursive integer-ordering vocabulary;
semantic v20 plus proof format v14 add recursive integer-bitwise vocabulary;
semantic v21 plus proof format v15 add recursive wrapping-shift vocabulary;
semantic v25 plus proof format v16 add recursive integer-bitwise-complement
vocabulary; semantic v26 plus proof format v17 add recursive exact-typed
integer-widening vocabulary; and semantic v27 plus proof format v18 distinguish
address-typed values and proof terms from ordinary unsigned integers, without
changing fuel schedule v1. Parameter-fed
canaries round-trip, verify, cost two units, and agree with native execution: wrapping
`u8` computes 5-10 = 251, while signed `i64` saturating subtraction reaches
both bounds and wrapping `u8` multiplication computes 20*13 = 4.
The signed `i64` saturating-multiply canary reaches both bounds and covers the
`MIN * -1` edge plus an ordinary negative product.

The vocabulary has canonical semantic bytes and a domain-separated semantic
fingerprint; the source, wrapping, and saturating canaries round-trip after
discarding producer state. Proof format v1 retains its frozen original bytes,
minimal format v2 adds recursive wrapping-add terms, minimal format v3 adds
recursive saturating-add terms, minimal format v4 adds recursive wrapping-subtract
terms, minimal format v5 adds recursive saturating-subtract terms, and minimal
format v6 adds recursive wrapping-multiply terms, minimal format v7 adds
recursive saturating-multiply terms, minimal format v8 adds content-conservation
terms, minimal format v9 adds sum-case structural paths, minimal format v10 adds
recursive Boolean-negation terms, minimal format v11 adds recursive
Boolean-equality terms, minimal format v12 adds recursive integer-equality
terms, minimal format v13 adds recursive integer-ordering terms, and minimal
format v14 adds recursive integer-bitwise terms, minimal format v15 adds
recursive wrapping-shift terms, minimal format v16 adds recursive integer
bitwise-complement terms, and minimal format v17 adds recursive integer-widening
terms with exact source and target types. The proof section has its own
golden fingerprint, and a role-separated manifest binds semantic, proof,
installation, and debug sections without folding replaceable evidence into
program identity. The clean terminal lane owns a semantic-identity-bound object
artifact, emits the compatibility object container and standalone images for
the four supported architecture/format pairs, and validates exact
relocation-free text plus complete executable-region coverage. The macOS
canaries execute the emitted Mach-O image directly after producer state is
dropped. A canonical typed installation payload separately binds the terminal
identity, exact target facts, PE subsystem, profile decision, selected
provider-plan set, complete image digest, and compiler text-validation evidence;
its exact bytes enter the installation role of the artifact manifest. The
provider-free scalar canaries use an empty selected set. This metadata does not
replace the native executable admission/placement state machine. Canonical
typed debug-map v1 now binds the exact semantic identity to ordered source-file
metadata and bounded spans for stable terminal subjects, rejects unknown
subjects and wrong-module attachment, and remains replaceable presentation
evidence. The checked-source producer fills retained declaration spans and the
real-source canary round-trips the manifested debug bytes after frontend state
is dropped. Generated operations and values retain their exact source-expression
spans; explicit transitions retain their arrow site, and implicit returns retain
the returned expression rather than the enclosing state declaration. General
register assignment remains on the legacy backend.

Semantic v1 integer, v2 Boolean, v3 wrapping-add, v4 saturating-add, v5
wrapping-subtract, v6 saturating-subtract, v7 wrapping-multiply, v8
saturating-multiply, v9 content, v10 reshuffle, v11 case-path, v12 partition,
v13 conditional, and v14 entry-claim modules retain their frozen bytes and
execution semantics; explicit migration produces a new current-v25 fingerprint
and derives dense entry bindings from any validated archived reshuffles. The
v15 Boolean-negation slice round-trips, verifies, costs one operation plus one
return edge, interprets, and returns the complemented canonical Boolean through
the clean Omega lowering and native C-ABI lane. The v17 Boolean-equality slice
round-trips, verifies, charges one operation, and interprets equality between
two defined Boolean operands. Its checked-source canary compares a parameter
with `false`; clean Omega lowering folds that literal comparison to the existing
canonical Boolean target forms and agrees with native C-ABI execution. A second
checked-source canary compares two runtime Boolean parameters; recursive target
and assigned Boolean expressions preserve both ABI inputs and emit canonical
equality results on AArch64 and x86-64. The v18 integer-equality slice compares
two runtime `u64` parameters, reconstructs its exact logical axiom, costs one
operation plus the return edge, and agrees across canonical round-trip,
verification, interpretation, typed target lowering, and AArch64/x86-64
emission. Proof format v12 separately carries that exact term in replaceable
certificates. The v19 ordering slice retains signedness through proof terms,
interpretation, target assignment, and native condition selection; its `u64 <`
and `i64 <=` canaries cost one operation plus the return edge, and proof format
v13 carries the exact relations in replaceable certificates. The v20 bitwise
slice retains exact integer types through proof reconstruction, interpretation,
target assignment, and native emission; its AND, OR, and XOR canaries cost one
operation plus the return edge, and proof format v14 carries the exact result
terms in replaceable certificates.
The v21 wrapping-shift slice retains the value and count integer types
independently through proof reconstruction, interpretation, target assignment,
and native emission. Left shift wraps at the value width; right shift selects
logical or arithmetic behavior from that value type. Counts reduce by
Euclidean modulo of the value width, so negative signed counts and counts at or
above the width remain total. Its canaries cost one operation plus the return
edge, and proof format v15 carries the exact result terms in replaceable
certificates. The v25 bitwise-complement slice retains an exact integer carrier
through source checking, canonical artifact decode, proof reconstruction,
interpretation, fuel, target assignment, and native emission on both target
architectures. Proof format v16 carries the exact recursive complement term in
replaceable certificates. The v26 integer-widening slice retains exact source
and target carriers through source checking, canonical artifact decode, proof
reconstruction, interpretation, fuel, target assignment, and sign- or
zero-extending native emission on both target architectures. Direct canaries
cost one operation plus the return edge; a nested wrapping-add companion costs
one widening, one constant, one addition, and one edge. Proof format v17
carries the exact recursive widening term. The same-carrier policy-cast canary
selects wrapping addition through explicit source casts and costs only that
operation plus its return edge; a direct policy erasure remains an ordinary
parameter return and costs only the edge. Wrapping/Saturating unary negation
costs one zero constant, one subtraction, and one return edge. These source
forms reuse existing terminal terms, so none changes semantic or proof format
after its owning vocabulary lands. The v27 address canary retains an `addr`
parameter through canonical decode and exact-type equality, returns Boolean
true under artifact-root interpretation and both native targets for a
full-width input, and costs one comparison plus the return edge. Address
identity changes no fuel rule; proof format v18 is selected only when a carried
certificate contains an address-typed term. The exact-literal narrowing canary
re-lands `127u64 as u8` before terminal production, crosses canonical artifact
interpretation and both native targets as an ordinary `u8` constant, and costs
the existing one constant plus one return edge. The v28 guarded narrowing
canary admits `value as u8` only on the `value <= 255u64` arm. Its independently
reconstructed operation obligation is mandatory, proof format v19 carries
exact-cast terms, both selected paths cost six schedule-v1 units, and the cast
site itself costs one operation unit. Artifact interpretation and emitted
x86-64/AArch64 code return 255 on the proved boundary and zero on the fallback
path. The guarded Exact-right-shift canary admits `value >> count` only on the
`count <= 63u64` arm. Semantic v29 and proof format v20 carry the operation and
its mandatory independently reconstructed obligation; both paths cost six
schedule-v1 units, and the shift costs one operation unit. Artifact
interpretation and emitted x86-64/AArch64 code return one for
`(1u64 << 63) >> 63` and zero on the count-64 fallback. The v3
wrapping slice
round-trips, verifies,
meters, lowers, emits,
and executes `u8` 200+100 as 44. The v4 saturating slice traverses the
same path and clamps that sum to 255. Semantic v13 conditionals round-trip,
validate, execute both arms, charge only the selected successor, and retain both
ordered successors through Omega's abstract boundary.
Omega's exact target continuation executes one runtime Boolean conditional
whose integer-returning arms may cross acyclic computed jump chains. A
compile-known conditional encountered within either arm folds to its selected
successor and excludes the untaken operations and edges from emitted
provenance. Runtime-nested acyclic conditionals retain recursive successor
control through target selection and register assignment, then emit every
integer-returning leaf on x86-64 and AArch64. AArch64 tests branch on the
assigned Boolean register, or an exact byte loaded from the incoming stack,
without clobbering an unrelated integer input in `x0`. Entry jump prefixes,
including computed scalar bindings, now enter that same recursive lowering and
retain their fuel edges and canonical provenance. Boolean-result CFGs retain
the same recursive control with canonical immediate or ABI-parameter leaves
and emit on both architectures. Cyclic semantics, reusable native block
layout, and operations beyond the current scalar terminal vocabulary remain
fail-closed.
The Psi checked-source producer exercises both integer- and Boolean-result
conditional forms. Its integer canary carries two runtime selections into a
shared tail, and the Boolean canary preserves short-circuit control; both
survive frontend disposal and agree across verification, fixed fuel,
interpretation, assignment, and native emission.
`psi-terminal-fuel` defines schedule v1 as one unit per executed terminal
operation and one unit per taken terminal edge. The verified interpreter returns
exact schedule-keyed usage attributed to stable operation/edge identities; a
finite sponsor allowance fails atomically before an unpaid site. Explicit
in-memory execution state resumes at that exact site after checked allowance
replenishment without replaying prior work. The acyclic subset has an exact
maximum entry-to-terminal-exit certificate keyed by semantic identity, entry, and fuel
schedule; consumers recompute every field without trusting the producer. A
memoized CFG walk takes the maximum successor cost at each conditional rather
than summing mutually exclusive arms. The same checker derives exact selected
block-to-edge segment certificates, including the endpoint charge, so adjacent
segments neither omit nor double-charge a jump. The current-vocabulary semantic
safe-point selector now returns the complete canonical graph partition at every
explicit jump, conditional, or return edge; validation rejects omitted or
reordered segments. Build-time migration, loop certificates, reusable native
block layout, and native metering remain.
Direct-return Boolean `&&`/`||` lower to terminal conditional trees rather than
eager operations. A deciding left operand bypasses the right subtree; measured
usage is three units on that path versus four when the right operand is
evaluated. Recursive Boolean expressions can drive the resulting control nodes
or be returned from their leaves, so equality operands in a short-circuit
expression preserve the same metered semantics through native AArch64/x86-64
control. The same construction composes with linear Boolean state chains:
decision leaves carry canonical Boolean values through ordinary jump bindings,
and deciding paths bypass the unused subtree before converging on the next
source state. Multi-value jump bindings repeat that construction in authored
tuple order, threading prior values through each stage so evaluation order and
fuel provenance remain explicit. Explicit Boolean conditionals use terminal
test edges from a short-circuit guard directly to the selected branch, while
each selected arm may independently contain the same decision form. Equality
and inequality may also consume short-circuit operands: value-producing leaves
retain the `BooleanEqual` operation and, for inequality, its canonical
`BooleanNot`, so schedule-v1 still charges those explicit semantic sites.
Attributed response reporting additionally waits on executable terminal
wait/foreign-edge variants carrying their response-contract status. The current
total operation plus unconditional jump/return vocabulary can close a bounded
report, but it cannot recompute `NoFiniteGuarantee(edge)` from semantics; a
producer-authored edge attribution would not satisfy the verification model.

## Context

WCSU proves a spatial fact: a closed activation needs at most a derived amount
of stack. Logical-work accounting has three distinct customers:

- deterministic metering of work that actually executes;
- a restricted theorem for paths that must fit a fixed logical budget; and
- attributed response reporting for waits and edges with no finite guarantee.

General parametric work functions, arbitrary recurrence solving, and WCET are
not prerequisites for those facilities.

## Terminal Psi

Psi operates on Omega-branded source files and owns the complete target-neutral
pipeline: parsing, resolution, typing, semantic checking, proof and obligation
construction, expression lowering, and canonicalization. Its terminal product
is the one versioned portable execution representation consumed by Omega.
Omega begins with terminal Psi and owns provider installation, target
realization, optimization, ABI lowering, native emission, and execution.

```text
Omega files
    -> Psi parse / resolve / type / check / lower / canonicalize
    -> terminal Psi
    -> Omega interpret or realize for a target
```

There is no Omega-to-Psi-to-Omega pipeline and no separate public source
language called Psi. The names mark an implementation and trust boundary:
Omega is the user-facing language and platform brand; Psi owns its checked
portable semantics.

Terminal Psi is distinct from mutable compiler optimization representations.
The reference oracle executes it directly; native code is an acceleration
lowered from the same module. Terminal artifacts are concrete and
post-instantiation. Generic parsing, checking, and instantiation may occur in
nonterminal Psi forms, but the interpreter, verifier, and Omega lowering do not
need generic execution semantics.

Psi semantics and accounting are independently versioned:

```text
TerminalPsiIdentity {
    semantic_version;
    program_fingerprint;
}

FuelScheduleIdentity {
    schedule_version;
}
```

Changing the fuel schedule changes accounting, not program meaning. Cached
semantic results therefore key on Psi semantics and program identity; cost
records additionally key on the fuel schedule.

### The representation cut

Terminal Psi is live for the closed executable and proof vocabulary described
above. `CheckedTrees`, `StateGraph`, and `ControlFlowPlan` still retain
`TypedTrees` expression tables and
`ExpressionHandle` as executable content. `StateGraph` and `ControlFlowPlan`
provide useful machine, state, transition, contract, borrow, and ownership
topology, but the latter is presently mostly a remap of the former rather than
expression lowering. `AbstractOperations` is already an Omega representation:
runtime storage regions, calling conventions, ABI aggregate classes, native
offsets, and related target-realization choices are its job.

Terminal Psi therefore replaces the hollow state-graph/control-flow pair with
one self-contained form at that altitude. It keeps their semantic skeleton but
replaces every source-tree reference with lowered values, operations,
predicates, typed places, blocks, and obligation-carrying edges. Short-circuit
evaluation, calls, guards, cleanup, suspension, and fallible operations may
create blocks not present in today's source-derived state segmentation; the
current arena topology is not the public schema.

The boundary is based on provenance, not on whether a number resembles an
offset. Author-declared device offsets, transfer widths, alignment demands,
and placement schema are semantic and remain in Psi. Target-selected native
field offsets, stack slots, register assignments, ABI classes, and concrete
storage regions belong to Omega.

### Semantic module

The fingerprinted semantic module contains:

- concrete machines, states, typed block parameters, values, calls,
  transitions, and terminals;
- a closed semantic operation vocabulary and statically visible variants for
  choices that change execution or generated obligations;
- structural places rooted in ordinary or provider-backed storage, including
  field, index-by-value, dereference, and range/subextent projections;
- machine-local entry-claim bindings that give each content claim a dense
  identity, projection, algebra, and entry structural place without asserting
  equality to an output;
- contracts, author-declared premises, generated structural obligations,
  cleanup/transfer actions, conservation equations, work attribution, trust
  classes, and authorized admission sites;
- nominal proposition families with their binder telescopes,
  fact-only/witness-bearing classification, and normalized carrierless
  evidence interface; changing that interface changes the semantic module,
  while transparent proposition definitions expand before this boundary and
  remain only in debug maps;
- relation-local left/right carrier index packs, selected heterogeneous
  constructor lifts, dependency-ordered field relations, and checked
  proposition-transport evidence; carrier declarations contribute no global
  relation-role row;
- erased bindings with their semantic type, multiplicity, validity,
  conservation, and provenance rows but no executable storage or cleanup;
  runtime layouts consume the erased-stripped form;
- exact witness-evidence term identities distinct from nominal proposition
  applications and derivation provenance, plus machine-derived nominal output
  packages whose named proof fields erase and whose guarded fields occur only
  in matching outcome variants;
- target-neutral provider requirements and scoped ordering operations; and
- stable identities shared by execution, propositions, proof evidence, fuel,
  diagnostics, and lowering provenance.

A choice that changes execution semantics or generated obligations must be
distinguishable without constant propagation. For example, trapping,
wrapping, and saturating integer addition are closed instruction variants, not
an ordinary runtime policy value. Several sound proof lemmas of differing
precision may describe one transition; lemma selection and later proof-library
improvements do not change operation or program identity.

The proposition vocabulary may differ from the executable vocabulary, but both
refer to the same canonical values, places, operations, and edges. Each
operation definition owns one normative execution transition and one generated
obligation schema. Its logical rules must be proven sound with respect to that
transition under those obligations. These per-operation proofs, plus the
global soundness obligations for control-flow composition, place algebra,
admission binding, canonical decoding, and ordering, form an enumerable Psi
language-verification backlog rather than an amorphous trusted compiler.

Conditional control flow is one terminator over an already-defined Boolean
value with ordered true/false successor edge records. Each successor retains
its own edge identity, typed bindings, actions, and fuel charge. This makes
mutual exclusion and exhaustiveness structural rather than generating a proof
obligation solely to repair the representation. Proposition terms are not
executable guards, and only the selected successor contributes path fuel.

The normative Psi semantics is not whatever the current interpreter happens to
do. The interpreter and native lowering implement the versioned operation
semantics. Differential execution compares those two implementations; it does
not replace the per-operation soundness work or prove that both implementations
did not share a mistaken reading.

### Obligations, evidence, and admission

The verifier reconstructs the required obligation set from executable Psi and
its fingerprinted contracts. A proof bundle cannot omit an inconvenient
obligation, weaken a contract, or relabel a derivable fact as admitted.
Admission is legal only at sealed positions whose truth cannot be structurally
derived, such as a foreign boundary, provider fact, or checked assembly claim.
The verifier validates each admission's kind, provider/evidence identity, and
profile acceptance even though it cannot prove the admitted fact true.

Every accepted fact follows exactly one route:

```text
kernel-derived       a specified total judgment re-decided by the verifier
certificate-derived  explicit evidence checked by the proof kernel
admitted             an authorized unverifiable assertion accepted by policy
```

`requires` and published guarantees are program semantics and remain in the
module. Call sites must establish requirements. A bodyful guarantee must be
derived; only a bodyless or foreign guarantee at an authorized site may depend
on admission.

Primitive kernel judgments are minimized. Each is a normative, total,
specified decision procedure with its own soundness obligation. Other solvers
remain outside the trusted base by producing certificates checked by the small
kernel. A total, guaranteed certificate reconstruction may run locally in a
consumer; any search that may time out, return unknown, or otherwise fail must
ship its certificate when portable verification is required. An external
non-certifying answer is admitted evidence, not derived proof.

Terminal Psi plus its content-addressed semantic dependencies is sufficient to
state and check replacement evidence without source or the producing compiler.
That makes evidence replacement possible, not necessarily cheap: proprietary
or expensive proof search may still be required to find a new certificate.

### Artifact sections and identity

One installed execution selects one complete semantic version. Translating an
older module creates a new module and fingerprint; a verifier may not approve
one representation and execute another. A distribution container may carry
several separately fingerprinted variants, but selection occurs before
verification and execution.

The artifact separates four concerns:

```text
semantic module       executable Psi, contracts, obligations, admissions
proof bundle          replaceable derivations and carried certificates
installation record   selected providers, target facts, profile decisions
debug/source maps      presentation and diagnostics
```

The semantic fingerprint covers only the semantic module. The containing
artifact manifest hashes every attached section so evidence or installation
records cannot be silently replaced. Improving a proof changes the proof-bundle
and container identities, not the program's semantic identity. Supply-chain
attestation may separately state which producer created a module; it grants no
semantic authority to the verifier.

Canonical decoding rejects alternate encodings rather than silently
normalizing them. Numbering, ordering, algebraic normal forms, and serialization
are deterministic, so byte identity and semantic-module identity coincide for
one Psi version. Proof-system and evidence versions are separate from Psi
semantics. A bug in a Psi operation definition requires a semantic-version
response; a bug in a proof checker or trusted decision procedure revokes that
evidence version and may allow a replacement proof bundle over the unchanged
semantic module.

### Vocabulary construction

The proposition IR, proof kernel, and their extension/versioning discipline are
established before operations depend on them, then vocabulary grows through
vertical slices. For each operation class, specify together:

1. canonical encoding and typed operands/results;
2. execution transition;
3. generated obligations and authorized admissions;
4. proof rule plus its soundness obligation;
5. interpreter behavior and Omega lowering requirement; and
6. fuel identity under a separately versioned schedule.

Two operations require distinct static identities when their execution
semantics or generated obligations differ. Proof-lemma choice is not an
operation distinction. Proposition expressiveness is not limited to automatic
decidability: closed total fragments may discharge automatically, while richer
claims require explicit checkable evidence. Unsupported entailment refuses; it
never triggers unbounded proof search during verification.

The canonical operation vocabulary must retain scoped ordering events rather
than flattening them into opaque calls or one universal fence. CPU atomic
fences, same-context compiler/interruption fences, DMA publication, device
acquisition, MMIO completion, cache maintenance, and checked ISA barriers name
different participants and guarantees. A cross-device event retains its exact
range, mapping, observer/device instance, and ordering scope so the verifier can
check composition and target lowering can discharge the same requirement.

Erased proof evidence does not create runtime ordering. For example, a DMA
publication result may authorize a later doorbell in source, but the
publication operation itself must contribute the Psi event that forbids sinking
covered writes past publication or hoisting notification before it. On a
coherent target the verified realization may emit no instruction; on another
target it may expand into bounded cache maintenance, barriers, or an admitted
OS provider call. Those realizations retain distinct work and trust evidence
while implementing the same scoped semantic event.

## Logical fuel

The fuel schedule assigns deterministic logical cost to terminal Psi
instructions or normalized blocks. Fuel is not native instruction count,
cycles, energy, or wall-clock time.

The execution sponsor supplies a budget. Executed code cannot inspect its
remaining fuel, branch on budget policy, catch exhaustion as a machine result,
or distinguish interpreted from natively metered execution. Exhaustion is a
sponsor event: the host may replenish and resume, cancel, or terminate
according to installation policy.

The same denomination serves:

- build-time evaluation by executing terminal Psi in the evaluator;
- portable interpreted artifacts through direct metering; and
- native realizations whose trusted lowering inserts counters that charge the
  corresponding terminal-Psi blocks.

Optimization may reduce physical work without reducing logical fuel. A
compiler release may not silently change budget behavior merely because its
native lowering improved.

Build usage remains deterministic for the concrete invocation, target
description, evaluator/Psi semantics, and fuel schedule. It never depends on
host load or elapsed time. Long terminating builds remain legal; progress,
warnings, cache accounting, and optional root-selected ceilings consume the
meter without making the ceiling program semantics.

## Restricted fixed-work checking

A restricted checker may prove:

> Entry `E`, under preconditions `P`, executes at most `K` units under fuel
> schedule `S`.

The supported fragment has constant-bounded iteration, bounded call
multiplicity, acyclic or explicitly measured call structure, and no unresolved
blocking or foreign-completion edge. The checker applies to a whole hard-root
entry or to a selected path segment ending at the next semantic safe point.

The public certificate keys terminal Psi, the entry, relevant preconditions,
fuel schedule, and scalar ceiling. Private proof or optional diagnostic
evidence may retain the maximizing path; it has no semantic identity and does
not seed target WCET analysis. An edge without a finite response contract
retains the exact attribution that prevented closure.

Static premises may be discharged at installation. Invocation-dependent
premises are ordinary call obligations and must hold at each meter-free call.

A sponsor may execute a fixed-work entry natively without runtime metering when
trusted lowering and installation establish that the executing bytes came
from the certified Psi module and the proved ceiling fits the granted fuel. Psi
without such a theorem remains safely executable under interpreter metering or
trusted inserted native metering. A certificate that arbitrary native bytes
refine terminal Psi is a separate future proof-carrying-code chain.

Provider-local `FixedFuelProviderSummary` and `LogicalFuelResourceColumn` are
the current implementation precursor for hard roots. Each summary and
provision now names the `psi-core`-owned nonzero `FuelScheduleIdentity` directly;
composition rejects mixed schedules, and the external-root artifact publishes
the schedule version, provision, ceiling, and composed units. A summary's local
evidence now distinguishes a sealed recomputable terminal-Psi entry/segment
certificate from an admitted opaque-provider unit claim. Certificate-backed
units derive from the certificate, contribute no provider-validation receipt,
and retain exact terminal identity in the external-root artifact. The real
source canary composes its four-unit entry certificate through this path after
the generic installation ladder freezes and validates its code. The sealed
binding checks terminal semantic identity, architecture, exact
relocation-free artifact/frozen bytes, installed-code context, and selected
function offset. External-root installation rechecks the whole-entry
certificate against the exact root code and stub; a segment certificate alone
cannot authorize a whole hard-root entry. Opaque provider leaves remain
admitted summaries, and the Cathedral hard-root graph still needs migration.
This precursor does not grow into general symbolic complexity analysis.

## Response and physical time

Logical compute and response are separate:

```text
pure fixed path       work: Bounded(K)   response: finite under a timing model
block mutex.lock()    work: local bound  response: NoFiniteGuarantee(mutex.lock)
suspend io.read()     work: local bound  response: NoFiniteGuarantee(io.read)
```

A selected-point report has three honest outcomes:

- `Bounded(K, evidence)` when restricted fixed-work checking closes;
- `Unknown(reason)` when the checker cannot prove a bound; and
- `NoFiniteGuarantee(edge)` when a reachable wait or foreign edge publishes no
  finite response contract.

A hard-control profile requiring bounded response rejects `Unknown` and
`NoFiniteGuarantee` at its roots. Force-terminating a blocked holder is not a
substitute for a response theorem.

A monotonic clock or performance counter may report one observed execution
under a target provider. Observation is not a future guarantee. Converting a
logical or target-work ceiling to a statement such as `<= 850 us` requires a
separate derived or admitted worst-case timing model.

Fuel and target WCET optimize different cost functions, so their maximizing
paths may differ. A future real-time analysis re-searches target paths. It may
reuse structural enabling facts from the Psi certificate, but lowering must
also show that helper calls, expansions, and other target realization choices
introduce no unbounded structure.

A strict real-time profile needs analyzable evidence for every dependency:
terminal Psi, a separately verifiable native WCET certificate, or an admitted
target-specific summary when policy permits one. Terminal Psi is the preferred
distribution form, not the only mathematically possible evidence source.

## Spatial resources are provisioned

Omega does not add one flat `memory_budget` meter. Allocation and storage
already require authority. A sponsor provisions the concrete resources an
execution may use:

- independently sized `Extent` or allocator capabilities;
- WCSU-derived stacks and activation-stack pools;
- static image/code storage admitted at installation; and
- qualified extents for pinned, shared, physical, DMA-visible, persistent, or
  other provider-defined memory.

Multiple heaps are multiple allocator or `Extent` values. A component receives
bounded child storage authority instead of ambient access to a global allocator.
External retained storage remains ordinary claim and custody accounting.

Infallible allocation in the package-level bump canary is the first concrete
customer for a `CountedQuantity<Bytes>` content algebra. Allocation consumes
normalized size, alignment padding, and metadata from a proof-level natural
residual magnitude keyed by the `Bytes` unit identity. The residual tail
`Extent` supplies placement; released extents leave the allocation's live
custody frontier but do not restore bump capacity until reset recomposes the
original backing. A scalar
free-byte count does not prove placement in a fragmented general heap. Such
allocators remain fallible or require an exact free-extent/reservation theorem.

## Contracts, installation, and proof-carrying code

Fuel and spatial provisions normally belong to an execution sponsor or
installation profile, not API/ABI identity. A replacement may require more
fuel or provision while remaining semantically compatible; installation
rejects or reprovisions it. A deadline or fixed resource ceiling enters the
interface contract only when an API deliberately promises it.

The proof-carrying-code scope in this brief is terminal Psi. Its verifier may
check memory safety, ownership and resource conservation, reach, termination,
and fixed-fuel certificates without trusting the producing compiler. Native
lowering/refinement certificates have a different subject and TCB and remain a
separate future lane.

Terminal-Psi verification has two compositional judgments. The artifact-aware
judgment canonical-decodes the module and reconstructs the complete obligation
set from its operations, edges, contracts, and authorized admission sites. The
generic proof-kernel judgment checks derivations of the resulting propositions.
The module determines what must be proved; a proof bundle only discharges that
set and is rejected for missing, extra, mismatched, or differently bound
evidence.

The semantic split is settled even though final implementation placement is
deliberately deferred. The Psi-aware judgment may gain a low-rung reference
implementation, emit a reconstruction derivation checked by the low kernel, or
remain an explicitly named trusted component. A Psi-hosted implementation of the
generic kernel is useful for speed or a further independent diamond, but cannot
by itself establish that the right obligations were reconstructed.

## Implementation sequence

1. Establish the proposition IR, small proof kernel, closed total judgments,
   evidence envelope, and authorized admission taxonomy.
2. Replace the current state-graph/control-flow remap with terminal Psi vertical
   slices, lowering expressions and predicates together and retaining
   structural places and edge obligations.
3. Re-root the reference interpreter on terminal Psi. During migration, keep
   the old interpreter only as comparison evidence; the established oracle
   claim is rebuilt rather than assumed to survive the change.
4. Re-root abstract-operation construction on terminal Psi and remove its
   dependency on `CheckedTrees` expression substitution. This moves
   monomorphization-shaped substitution above the boundary and unblocks generic
   backend work on explicit instantiated values.
5. Define deterministic serialization, semantic/module fingerprints, proof and
   installation section hashes, and version migration tests.
   Bind every evidence row to the exact semantic and reconstructed-obligation
   identities; a certificate-provided proposition is never authoritative.
6. Define the separately versioned logical fuel schedule and interpreter meter.
7. Feed build-time evaluation usage, progress, warnings, and optional policy
   from that meter.
8. Migrate provider-local fixed-work summaries to Psi fuel and generalize them
   to selected safe-point segments.
9. Preserve `Bounded`, `Unknown`, and attributed no-finite-guarantee outcomes
   in artifacts and diagnostics.
10. Add trusted native block metering while preserving accounting provenance
   through optimization; canonical block topology itself need not survive.
   Defer a separate Psi-to-native PCC chain.
11. Connect the canonical entry/current content vocabulary to sealed
   introduction and custody-exit frontier rows. Terminal Psi already carries
   canonical `IntervalSet<CoordinateSpace>`, partial n-ary separation, and
   canonical residual difference.
12. Add `CountedQuantity<Bytes>` with the package-level bump-allocation canary;
   retain exact tail placement and keep general fragmented allocators fallible
   unless they supply placement/reservation evidence.
