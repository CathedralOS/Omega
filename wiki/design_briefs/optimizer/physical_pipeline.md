# Optimizer Physical Pipeline

This brief owns the lowering-to-publication path. The architecture entrance is
[optimizer_architecture.md](../optimizer_architecture.md).

## Selected lowering

Abstract-to-target lowering now enters through a small settlement and
installation-evidence coordinator. It descends first by function result
family, then through explicit scalar setup, special-form, conditional, and
straight-line routes or structural direct-call and return routes. Unit,
boundary-settlement, cleanup, and structural-layout mechanics remain named
sibling responsibilities rather than hidden branches in one lowering file.

The adjacent 51-line translation-validation entrance is independent of
those producer routes. It first binds Psi identity, requested target, entry,
function count/order, machine, and attachment, then descends into exact family
replay. Its function catalog reconstructs parameterless straight-line
Unit return, one exact PortWrite followed by Unit return, one exact Unit call
followed by Unit return, one exact trivial affine local establishment followed
by Unit return and its discard cleanup, exact byte-sequence, integer, or raw-bit
IEEE literal establishment, including ordered sequences of at least two
integer or IEEE literals, a heterogeneous integer/IEEE literal sequence, or
three IEEE literals consumed by one settled
nearest-even fused multiply-add, followed by Unit return, integer and Boolean literal
returns, scalar `Crash`, direct integer
and Boolean parameter returns, Boolean negation of a parameter, and equality of
two Boolean parameters, equality of two same-type integer parameters, or strict/inclusive
ordering of two same-type integer parameters, plus integer bitwise-not and
integer-widen or proof-bearing integer exact-cast of one parameter, and integer
bitwise-AND, bitwise-OR, bitwise-XOR, proof-bearing fixed-integer exact-add,
exact-subtract, exact-multiply, exact-divide, or exact-remainder, plus
wrapping-divide, wrapping-remainder, saturating-add, wrapping-add,
saturating-divide, saturating-remainder, saturating-subtract, wrapping-subtract,
wrapping-multiply, or saturating-multiply of two same-type integer parameters,
plus wrapping or proof-bearing exact shift-left and shift-right of an
independently typed integer value and count,
without calling
`lowering`, `KnownScalar`, or the scalar-return helper. The distinct parameter
families share governed source-envelope and whole-roster ABI replay rungs,
which independently apply the target's calling policy to prove every incoming
register or stack location.
The Unit-return leaf independently requires the exact claim-free, service-free,
single-block source shape and reconstructs the target's empty native call plan,
empty parameter and operation provenance, exact return edge, and empty cleanup
roster. Whole-plan admission separately binds every Unit body's canonical
structural-type roster to the source plan so a function-local family validator
cannot silently accept reordered, substituted, missing, or injected global
declarations.
The adjacent PortWrite leaf requires the exact parameterless, claim-free,
single-block `PortWrite; ReturnUnit` shape and a singleton published-service
ceiling. Independent replay retains the operation, service, port, byte,
provenance, empty native call plan, return edge, and cleanup roster on every
native target; optimized custody consumes the typed family receipt rather than
reclassifying the target body.
The Unit-call sibling admits a separate parameterless Unit-return callee and an
exact claim-free, structurally parameterless `CallUnit; ReturnUnit` caller.
Independent replay binds the callee, arbitrary exact requirement-obligation and
crash-continuation rosters, empty structural arguments and claim transfers,
empty native call plan, target provenance, cleanup, and return edge on every
native target. Exact operation rosters keep it disjoint from return-only and
PortWrite families.
The trivial-affine-local sibling requires one empty-record declaration at
ordinal zero, no construction, and the exact `DiscardRoot` cleanup before Unit
return. Its independent replay reconstructs the establishment, local/type
identity, native Unit call plan, return edge, cleanup, and provenance on every
native target; its two-operation roster keeps it disjoint from the other Unit
families.
The three literal-plus-Unit siblings retain their distinct source and target
operation grammars. Byte-sequence establishment binds the borrowed-view place,
structural type, and exact bytes. Integer establishment binds the scalar type
and admitted mathematical value. IEEE establishment binds raw Binary32 or
Binary64 bits directly, including signed zero and NaN payloads, without a host
float conversion. All three independently replay their identities,
provenance, empty native Unit call plan, return edge, cleanup, and global
structural roster across every native target; deleting an unused literal sends
the transformed plan through the separate return-only family.
The IEEE-sequence sibling admits two or more consecutive constants and retains
each operation/result identity, each raw Binary32/Binary64 bit pattern, exact
order and provenance, the return edge, empty native Unit call plan, cleanup,
and global structural roster. Independent replay therefore preserves signed
zero and NaN payloads without host-float conversion while the operation-count
grammar keeps the sequence disjoint from both the singleton IEEE and
return-only families.
The integer-sequence sibling likewise admits two or more consecutive constants
before Unit return, but retains each exact integer type and mathematical value
rather than raw IEEE bits. Independent replay binds every operation/result
identity, order, provenance, return edge, empty native Unit call plan, cleanup,
and global structural roster across all five native targets. Its count-based
grammar is disjoint from the singleton integer-literal and return-only routes,
and optimized custody preserves the typed sequence receipt.
The heterogeneous sequence sibling admits only consecutive integer and raw-bit
IEEE constants before Unit return and requires at least one member of each
kind. Independent replay preserves the exact typed payload and identity of each
member in source order, along with provenance, return, cleanup, native-call,
and structural-roster custody across all five targets. That mixed-membership
predicate keeps it disjoint from both homogeneous sequence siblings, and
optimized custody retains its distinct receipt.
The x86 FMA sibling has the exact five-operation grammar of three raw-bit
Binary32 or Binary64 constants, one nearest-even fused multiply-add consuming
those values in order, and Unit return. Independent replay binds each
definition and operand, the fused result, provenance, native Unit call plan,
and the complete admitted settlement: canonical provider/target identity,
scalar FMA slot, selected requirement and exact compiler-intrinsic realization,
plus both the provider-plan report coordinate and strong digest. Whole-plan
settlement admission rejects missing, duplicate, and unknown rows before family
replay; Linux, Windows, and UEFI x86-64 are the exact applicable target set,
while Arm fails with a typed family applicability error. Optimized custody
passes the settlement through lowering and validation rather than reclassifying
the fused operation or trusting target-carried metadata alone.
Parameter replay descends through explicit direct, unary, arithmetic, bitwise,
and comparison rungs. The arithmetic model join owns only the common ordered
operand/result carrier and sends obligation-retaining policies through its named
`proof_bearing` leaf. Arithmetic catalog dispatch and independent source/target
reconstruction send the complete exact, wrapping, and saturating
division/remainder set through named `quotient` leaves; the arithmetic parents
retain the add/subtract/multiply families instead of accumulating one flat
switchboard. Those joins bind operand/result identity and exact
operation/edge provenance; Boolean equality, typed integer equality, and strict
or inclusive integer ordering retain ordered and identical operands through
recursive `ReturnBooleanExpression` receipts, while integer bitwise-not,
bitwise-AND, bitwise-OR, bitwise-XOR, exact-add, exact-subtract, exact-multiply,
exact-divide, exact-remainder, wrapping-divide, wrapping-remainder, wrapping-add,
saturating-add, saturating-divide, saturating-remainder, saturating-subtract,
wrapping-subtract, wrapping-multiply, and saturating-multiply retain exact-width
operands,
integer-widen retains distinct source/target types, and exact-cast additionally
retains its proof obligation through `ReturnIntegerExpression`.
The three constant-unary siblings remain distinct from those parameter
families. Constant widen validates `[IntegerConstant, IntegerWiden, Return]`
and replays the materialized widened immediate. Proof-bearing constant exact
cast validates `[IntegerConstant, IntegerExactCast, Return]` and faithfully
replays `IntegerExactCast(Immediate)`, retaining both operation identities,
the obligation, source/target types, and the independently computed exact cast
value. Its 38 legal native fixed-integer relations cross two representability
boundaries and all five targets; no cast-elimination authority is inferred.
Constant bitwise-not validates only
`[IntegerConstant, IntegerBitwiseNot, Return]`, independently computes the
exact width-aware complement, and requires `ReturnIntegerImmediate` with both
source operations retained in provenance. Its signed/unsigned 8/16/32/64 and
address64 carriers cross both boundaries and all five targets; it cannot be
reclassified as either a plain immediate return or parameter bitwise-not.
Constant Boolean-not independently validates only
`[BooleanConstant, BooleanNot, Return]`, computes the complemented truth value,
and requires `ReturnBooleanImmediate` with both source operations retained in
provenance. Both truth values cross all five targets; the family cannot be
reclassified as either a plain Boolean-immediate return or parameter
Boolean-not.
Constant Boolean equality is a separate four-operation sibling. It validates
two ordered Boolean constants, `BooleanEqual`, and `Return`; independently
reconstructs all three definitions and the exact equality result; and requires
`ReturnBooleanImmediate`. All four ordered truth pairs cross direct and public
optimized custody on all five native targets. The grammar cannot be
reclassified as plain Boolean immediate, constant Boolean-not, or parameter
Boolean equality.
Constant integer equality is another distinct four-operation sibling. It
validates two ordered, same-type native integer constants, `IntegerEqual`, and
`Return`; independently reconstructs both mathematical operands, all three
definitions, and the exact Boolean result; and requires
`ReturnBooleanImmediate`. Signed and unsigned fixed 8/16/32/64 plus address64
cross four ordered boundary pairs at both direct and public optimized custody
on all five native targets. It does not share the parameter integer-equality
carrier and cannot be reclassified as a plain Boolean immediate family.
Constant integer less-than follows as a separate ordered-comparison sibling.
It validates two same-type native integer constants, `IntegerLessThan`, and
`Return`, compares their exact signed or unsigned mathematical values without
host-width conversion, and requires `ReturnBooleanImmediate`. Equal endpoints
and both minimum/maximum directions for fixed 8/16/32/64 and address64 cross
direct and public optimized custody on all five native targets. Its catalog row
is disjoint from constant equality and parameter ordering.
Constant integer less-or-equal is the inclusive ordered-comparison sibling. It
validates two same-type native integer constants, `IntegerLessOrEqual`, and
`Return`, compares their exact signed or unsigned mathematical values without
host-width conversion, and requires `ReturnBooleanImmediate`. Equal minimum
and maximum endpoints plus both ordered endpoint directions cross direct and
public optimized custody for fixed 8/16/32/64 and address64 on all five native
targets. Its catalog row and replay remain disjoint from strict ordering,
constant equality, and parameter inclusive ordering.
Constant integer bitwise-AND is a distinct four-operation materialization
family. It validates two ordered, same-type native integer constants,
`IntegerBitwiseAnd`, and `Return`; independently computes the exact typed result
with `IntegerType::bitwise_and`; and requires `ReturnIntegerImmediate` with all
three source operations retained in order. Signed/unsigned fixed 8/16/32/64
and address64 cross four ordered boundary pairs at direct and public optimized
custody on all five targets. Its catalog row is disjoint from bitwise-not,
bitwise-OR/XOR, plain immediate, and parameter AND.
Constant integer bitwise-OR is an independent sibling rather than an AND-mode
inside one broad rule. It validates two ordered same-type native integer
constants, `IntegerBitwiseOr`, and `Return`; computes the exact typed result
through `IntegerType::bitwise_or`; and independently requires one
`ReturnIntegerImmediate` with all source operations retained in order. The nine
fixed/address carriers, four ordered boundary pairs, and five targets produce
180 direct plus 180 optimized-custody cases. Plain immediate, bitwise-not,
AND/XOR, parameter OR, and runtime-expression substitution all fail closed.
Constant integer bitwise-XOR is the third independent binary bitwise sibling.
It validates the exact ordered
`[IntegerConstant, IntegerConstant, IntegerBitwiseXor, Return]` grammar,
computes the declared carrier through `IntegerType::bitwise_xor`, and requires
one `ReturnIntegerImmediate` with all source operations retained in order. The
nine fixed/address carriers, four ordered boundary pairs, and five targets
produce 180 direct plus 180 optimized-custody cases. Plain immediate,
bitwise-not, AND/OR, parameter XOR, and runtime-expression substitution all
fail closed.
Constant wrapping integer add is a separate exact four-operation family. It
validates two ordered same-type constants, `WrappingIntegerAdd`, and `Return`,
computes modulo the declared width through `IntegerType::wrapping_add`, and
requires one exact `ReturnIntegerImmediate` with ordered provenance. Signed/
unsigned fixed 8/16/32/64 and address64 each cross four ordered wrap boundaries
at direct and optimized custody on all five targets. Exact/saturating policy,
subtraction/multiplication, plain-immediate, and parameter-family substitution
fail closed.
Constant wrapping integer subtract is the ordered arithmetic sibling. It
validates two same-type constants, `WrappingIntegerSubtract`, and `Return`,
computes modulo the declared width through `IntegerType::wrapping_sub`,
and requires one exact `ReturnIntegerImmediate` with ordered provenance.
Signed/unsigned fixed 8/16/32/64 and address64 each cross four ordered wrap
boundaries at direct and optimized custody on all five targets. Exact or
saturating policy, addition/multiplication, plain-immediate, and parameter-
family substitution fail closed; operand order is never treated as
commutative.
Constant wrapping integer multiply is a separate exact arithmetic sibling. It
validates two same-type constants, `WrappingIntegerMultiply`, and `Return`,
computes modulo the declared width through `IntegerType::wrapping_mul`, and
requires one exact `ReturnIntegerImmediate` with ordered provenance. Signed/
unsigned fixed 8/16/32/64 and address64 each cross four ordered wrap boundaries
at direct and optimized custody on all five targets. Exact or saturating
policy, addition/subtraction, plain-immediate, and parameter-family
substitution fail closed.
The sibling shift rung owns distinct value/count types, values, parameter
indices, and ABI locations rather than forcing them through arithmetic's
same-type carrier. Both wrapping directions admit fixed or address64 carriers
independently for value and count and reduce signed negative counts with
Euclidean modulo by the value width. Wrapping right shift separately preserves
unsigned fixed/address zero-fill and signed fixed sign-fill. Neither
direction's specific family identity can be substituted with its sibling,
either proof-bearing exact shift, bitwise, or arithmetic expressions even when
an individual runtime value agrees.
Exact shift-right admits only fixed value/count carriers and retains the
operation's canonical `ExactShiftCount` obligation. That goal proves the
unmodified count is in `0 <= count < value_width`; it does not require discarded
bits to be zero. Signed values sign-fill and unsigned values zero-fill. Replay
rejects wrapping or left-shift substitution, address carriers, operand drift,
and independent obligation drift.
Exact shift-left uses the same fixed-carrier and independent-type custody, but
retains the stronger canonical `ExactShiftLeftRepresentable` obligation. Its
goal conjoins the unmodified count range with mathematical-result bounds for the
value carrier. Replay rejects wrapping or right-shift substitution, address
carriers, operand drift, and independent obligation drift.
Exact-add independently retains its range-obligation identity, rejects address
carriers, and rejects substitution with wrapping or saturating addition.
Saturating-add independently rejects both wrapping and proof-bearing exact-add
substitutions. Saturating-subtract likewise rejects wrapping and proof-bearing
exact subtraction without treating operand order as commutative.
Saturating-multiply independently rejects wrapping, proof-bearing exact,
addition, and subtraction policy substitution. Exact-subtract and
exact-multiply retain their exact range-obligation identities; exact-divide and
exact-remainder retain defined-division obligations covering a nonzero divisor
and a representable quotient. All four reject address carriers and
arithmetic-policy substitution and never treat ordered-operand custody as
commutative. Exact remainder follows truncation-toward-zero division and keeps
the dividend's sign; signed `MIN % -1` remains undefined because the
corresponding exact quotient is not representable. Wrapping divide and
wrapping remainder instead retain only nonzero-divisor obligations. The former
maps signed `MIN / -1` to `MIN`; the latter returns zero while otherwise keeping
the truncating remainder's dividend sign. Saturating divide also retains only a
nonzero-divisor obligation, but maps signed `MIN / -1` to `MAX`; saturating
remainder retains the same goal and returns zero for signed `MIN % -1`, while
otherwise keeping the dividend's sign. The optimized
custody tests construct canonical exact-cast representability, exact-subtract
range, exact-product range, exact-division-defined quotient and remainder
goals, and wrapping plus saturating quotient/remainder nonzero-divisor goals as
machine preconditions, then discharge them with real Terminal proof
certificates. Replay never substitutes fresh proof search for obligation
identity. Its adjacent ordered catalog is the sole enable/disable inventory.
Each descriptor joins one source classifier to one typed replay adapter; the
separate selection leaf makes zero matches explicitly uncovered, retains one
match on that function's roster row, and fails closed on ambiguity.
The 51-line `validation/mod.rs` entrance is only the module map and public
validation surface. `whole_plan.rs` owns Psi/target/function/structural-roster
coordination and exact FMA-settlement admission, while catalog selection and
each family leaf own their narrower classifier and independent semantic replay.
Error and receipt publication follow the same descent: their small entrances
map immediate, parameter, terminal, and whole-roster responsibilities. Named
`error/family` and `receipt/function_translation` leaves own the closed tagged
carriers; `receipt/function_translation/arithmetic` names their arithmetic
payload join without widening that carrier. The separate `receipt/family` leaf
owns the exact receipt-to-family projection, while `model/family` owns the
stable family IDs themselves.
The optimized target carrier retains this receipt. Its family-row roster is
deliberately partial and therefore cannot overstate validation of other target
operations while those rows are still being added.

Projected structural qualification custody has a separate plan-family route
because its invariant spans caller and callee. A 54-line producer fence points
to one named `structural_call_return` leaf and rejects every other nonempty
projected-roster shape. Independent validation descends through a plan catalog,
source/layout/target replay, and local caller/callee leaves. The exact admitted
shape is one owned linear parameter crossing `CallStructural;
ReturnStructural` to a callee that returns its parameter; all six target roster
locations, both native call plans, placements, claims/transfers, provenance,
and structural layout are replayed on every native target. Public optimized
custody continues through one exact identity-legalization family. A distinct
atomic result-bearing carrier retains both target functions, entry blocks, the
canonical projected rosters, and each optimizer node's fuel, effect, and
ownership custody. Independent legalization replay reconstructs the source,
target, ABI, roster, optimizer-node, and machine shape on all five targets and
publishes a typed family receipt. Instruction selection now admits only that
same atomic closure. Its selected carrier binds eight direct integer fragment
placements, the exact ordinary-call and return constraint rows, fixed operand
views/classes/access, complete implicit uses/defs/clobbers, and each
target-dependent transfer. X86-64 retains the full fixed-view `copy_i64` row
where argument and return registers differ; AArch64 retains an explicit
same-view/no-copy decision. Construction descends through projection,
constraint, and transfer leaves, while independent replay reconstructs source,
catalog, view, effect, and transfer custody on all five targets. Nonempty
projected custody uses selected identity V13; empty plans retain V12
byte-for-byte. Liveness and pre-allocation machine-effect entrances reject the
new carrier explicitly, so allocation, machine realization, encoding, and
publication still gain no projected-qualification authority.

Target legalization and instruction selection produce explicit selected forms
over virtual registers. Fixed operands are constraints; they do not preassign
the entire program. Selected rules may fold exact incoming immediates or choose
equivalent target forms, but must preserve operation, edge, trap, provenance,
and fuel identities.

Ordinary, Unit, structural-scalar, and structural-result calls retain their
ordered requirement-obligation and crash-continuation rosters from Terminal
projection through abstract, target, and temporary assigned operations. The
structural Unit selected route carries the same rows through legalized V10 and
selected V12 identities and independent replay. Machine emission consumes the
semantic carrier but does not duplicate it into runtime effects or machine
bytes; selected instruction identity and source-operation custody bind the last
semantic boundary.

The composed-Unit rollout canary carries one qualification-free owned linear
whole-root parameter through a Boolean three-block graph. Both successor edges
transfer distinct checked state-entry aliases; each bodyless attached-Unit leaf
consumes its alias with an exact completion receipt. Checked-to-Terminal
lowering independently replays those events and binds the aliases to one
Terminal claim before verifier and codec publication.

A disjoint composed leaf family admits parameterless internal Unit calls. Each
leaf rejoins the exact checked target state, contract fingerprint, service
reach, and retained ordinary Unit plan. Lowering emits repeated calls to one
deduplicated qualification-free empty-body target machine; it does not copy the
target into each branch or treat a missing transitive plan as an empty body.
That target closure now has a second exact rung: the shared leaf target may
contain one parameterless internal Unit call before its return. Independent
admission rejoins the nested flow coordinate, state, contract fingerprint, and
service reach, then retains and deduplicates the transitive empty target.
Lowering emits one root, one relay, and one empty target machine; a missing or
altered transitive plan rejects the whole route. The admission walk is
recursive rather than depth-coded: a depth-two relay canary produces one root,
two relays, and one empty target while retaining the same one-call node shape
and rejecting cycles through its active closure.

The first larger acyclic composed family prefixes that conditional frontier
with a finite ordered chain of exact scalar-only jumps. Every checked edge
records Boolean source and target parameter position zero and targets the next
state in source order. Lowering independently reconstructs one distinct block
parameter per non-entry control state, allocates root blocks dynamically, and
places any internal target closure after the complete root graph. One- and
two-prefix canaries cover boundary and internal-call leaves. Admission rejects
changed scalar maps, target states, attachments, contracts, custody, or leaf
effects rather than silently routing the graph through a shorter family.

A disjoint nested-control family retains a general finite acyclic Boolean
control graph. Controls may target any retained control or effect leaf and may
forward any exact ordered Boolean parameter map required by the target;
multiple edges may converge on one leaf. Producer and consumer independently
walk the graph by checked state identity, reject cycles or unreachable states,
and rejoin every scalar argument expression and structural-cleanup target.
Lowering derives block/value identities and block-parameter arity from the
admitted graph before publishing boundary leaves or one deduplicated internal
target closure. Right-deep depth-three and balanced three-control canaries
cover multi-value handoffs, argument-bearing true and false arms, and a shared
leaf emitted once. Scalar-map reordering, target corruption, and forged
convergence reject the route before verifier or codec publication.

The first effectful-control widening admits a finite ordered sequence of
qualification-free, parameterless internal Unit calls before either transition
in a control state. Calls remain checked state operations rather than synthetic
blocks; therefore their source coordinates shift the guard and successor
statement ordinals. Independent admission rejoins every call coordinate,
target state, contract, service reach, and finite target closure, then emission
places the calls in source order before the conditional terminator. A dedicated
`operations` rung on each side owns this sequence. Missing targets, coordinate
drift, operation reordering, and custody-bearing calls reject before
publication.

The same operation rung admits an exact parameterless boundary call with no
scalar or structural arguments. Producer assembly includes control states in
boundary/provider discovery; consumer admission rejoins the boundary call and
emission records its source-call occurrence while appending it to the existing
control block before the guard. Provider-backed control prefixes cross this
route by joining executable operations to flow calls at exact source
coordinates; named-transition call facts remain graph topology. Checked scalar
facts omit the implicit `self` parameter but key every explicit transition
argument by its raw target position, while executable plans use separate dense
source and target scalar indices. Producer and consumer independently rejoin
those coordinates, and missing scalar facts or provider requirements reject.

Selected-plan construction has one 52-line roster entrance over scalar, plain
Unit, and structural Unit results. Scalar construction reconstructs common
condition context and selects exactly one row from its adjacent ten-row
catalog. Immediate, entry-parameter, direct and widened exact-add/subtract, and
the three active-resident exact-add-chain leaves each return their whole
virtual-register and block body, eliminating the former duplicated
source-family matches.
Structural selection separately joins ABI layout, optional whole-root call,
and return. Catalog omission rejects and ambiguity names both conflicting
families; neither path falls back to transitional assignment.

Independent selected-block validation enters through a 39-line join that
checks the three-block source roster, replays entry control, and then routes
the exact return family. It descends into immediate, parameter, exact-binary,
active-resident exact-add-chain, and shared instruction-projection leaves.
Those leaves reconstruct custody without calling construction helpers and
retain the existing mismatch precedence, instruction IDs, register schedule,
successor order, provenance, effects, and fuel rejection order.

The disjoint unoptimized ranked-`u32` lane now reaches ordinary object custody
without borrowing its machine-code producer as a validator. Each ISA owns an
opaque decoder result for its exact countdown body; the image boundary joins
that decoded layout to independent rank, fixed-fuel, ABI, affine-frontier,
cleanup, provenance, and nine-row fuel replay. The complete ranked record is
retained on the object function.

Semantic-code attribution is emitted and replayed with the ordinary function
body. It maps semantic operations and edges to exact byte intervals without
inserting instructions or rebasing control flow.
At executable-image publication, ranked replay checks the ordinary final text,
semantic proof, rank, ABI, frontier, and semantic-code attribution without a
parallel runtime image.

The mandatory lowering crate has two explicit entrances. `legalization/mod.rs`
joins canonical source projection to independent whole-plan replay;
`selection/mod.rs` joins selected-plan construction to independent validation.
Construction then has its own meaningful roster entrance and scalar catalog
rung; it is not a forwarding wall. Structural, scalar-function,
leaf-expression, constraint, identity, and fixture mechanics descend below
those joins. The crate-level `lib.rs` is only the responsibility map between
the two stages, not a hidden third coordinator.

Immediately below the legalization entrance, `catalog.rs` is the sole ordered
inventory for all twenty forms: fourteen scalar, one scalar-call Unit, one
plain Unit, and four structural Unit. Each row names its typed recipe, producer
matcher kind, exact source-shape constraints, non-authoritative structural
cost, and independent validator kind. `source/matchers/` walks that catalog to
recognize a form; `replay/validators/` reconstructs membership without calling
producer code. Removing a row disables the form, and missing or ambiguous
recipe lookup fails closed. The Unit recipe families are retained in the
current legalized-plan identity. Structural selected-form validation
separately reconstructs ABI layout and call constraints without importing
selection construction helpers.

The runtime comparison vertical is deliberately narrower than the recursive
target-expression vocabulary. Four exact candidates are three-block
unsigned-`U64` functions with two distinct entry parameters and either
`[IntegerEqual, Conditional]`, `[IntegerLessThan, Conditional]`, or
`[IntegerLessOrEqual, Conditional]`, or the composite
`[IntegerEqual, BooleanNot, Conditional]` in the entry block. A fifth exact
candidate has two signed-`I64` entry parameters and
`[IntegerLessThan, Conditional]`. Every candidate has one `U64` immediate
return in each leaf. Legalization represents unsigned and signed ordering as
distinct closed condition variants rather than inferring signedness from
machine bits. Each simple comparison leaf retains its operation, result
definition, exact parameter values, types, locations and definitions,
provenance, and fuel through mirrored producer and replay rungs. The inequality
leaf additionally retains the equality result consumed by Boolean-not, both
result-definition sites, and independent operation, provenance, and fuel
custody for both source operations.

Selected construction lowers each comparison to one two-register compare
and retains four virtual registers. Equality reuses the existing nonzero
branch with not-equal taken to the source false leaf and equal fallthrough to
the source true leaf. Strict less-than instead owns
`ConditionalBranchU64LessThan`: less/source-true is taken and
not-less/source-false is fallthrough. Inclusive less-or-equal reuses that exact
predicate by canonicalizing `left <= right` to `!(right < left)`: construction
reverses the compare operands, maps less/source-false to the taken edge, and
keeps not-less/source-true as fallthrough. It therefore needs no `JBE`/`B.LS`
instruction family. Integer inequality eliminates the materialized Boolean-not
result: the ordinary ordered compare feeds the existing nonzero branch, with
not-equal/source-true taken and equal/source-false as fallthrough. Compare
provenance remains attributable only to equality and branch provenance only to
Boolean-not. Layout and function-fragment evidence
therefore carry an exact predicate plus taken/fallthrough custody rather than
misnaming every branch as nonzero. For the ordered predicate, x86 baseline
layout emits `JB rel32`; only the explicit branch-relaxation rule may change
its alternative and bytes to `JB rel8`, while AArch64 emits `B.LO`. Equality
and inequality retain the existing x86 `JNE` and AArch64 `B.NE` family with
opposite successor mappings. Signed strict less-than instead owns
`ConditionalBranchI64LessThan`; it retains authored operand order and the same
true/less successor mapping while x86 emits `JL rel32` (or `JL rel8` only
through explicit relaxation) and AArch64 emits `B.LT`. Signed and unsigned
predicates have distinct selected, machine-effect, layout, encoding, and
fragment identities. The compare-zero/CBNZ fusion remains inapplicable to all
two-register forms because its exact producer still requires
`CompareI64Zero` and nonzero control.

Scalar source-leaf construction enters through a sub-100-line `derive_leaf`
coordinator. It admits the common node and return envelope, visibly routes
immediate, entry-parameter, direct exact-binary, widened exact-binary, and
active-resident exact-add forms, and then seals shared return and fuel custody.
The exact-add rung orders the existing resident chain before direct binary
fallback and now admits one distinct bridge chain,
`r + (b + (r + (a + b)))`, whose independent replay keeps `b` live across the
first resident use. This recipe is a legalization-only prerequisite: no
selected-instruction or allocator-pressure authority follows until a separate
public selection family exists. Focused leaves below it retain the existing catalog-family order,
diagnostic precedence, operation roster, proof identity, and provenance order;
they do not merge producer mechanics with `replay/validators/`.

## Register allocation

Allocation computes selected-CFG liveness, live-range fragments,
interference, allowed views, ABI constraints, clobbers, and spill legality.
Home assignment, copy insertion, spilling, coalescing, and bounded
rematerialization are separately validated decisions.

The first fixed/precolored interval boundary consumes validated live ranges and
allocation legality without assigning a home. Its 24-line entrance joins one
direct positional derivation to an independently keyed replay. V1 resolves
each authenticated entry or operand fixed constraint to one canonical
half-open `[point, point + 1)` interval and exact physical view, binds ordinary
and structural roots plus register environment, allocator availability, fuel,
policy, budget, usage, and identity, and refuses early-clobber fixed
definitions explicitly. The dual-target fixture has four exact intervals and
usage `{1, 4, 6, 4, 2}`. This grants factual precoloring evidence only; home,
copy, split, spill, memory, frame, and publication decisions remain separate.

Fixed-view-copy validation descends from one small independent join through
root and copy-constraint custody, work and budget replay, leaf-local or
shared-entry policy reconstruction, and exact application/comparison. Its
validated receipt therefore represents reconstructed copy insertion, not
producer self-attestation.

Its public artifact is V6. V4 remains decode-only and byte-pinned without a
structural roster; V5 retains the structural roster but decodes call proof and
crash rows as empty. V6 encodes those rows canonically through the shared
Terminal crash-route codec and also retains projected qualification rows. Those
rows are bound by the selected and fixed-view
semantic identities and authenticated by the outer envelope; legacy decoders
reconstruct fields absent from their payload versions as empty.

The current transition-free, spill-free home stage is a deterministic
constraint-graph allocator. Distinct use/definition ties form quotient
vertices whose domains are the intersection of every member's legal views.
Interference contributes symmetric storage/write-footprint conflicts and
early-clobber rows contribute directional definition-write/use-storage
conflicts. Placement repeatedly chooses the vertex with the fewest currently
viable views, then greatest remaining constrained degree, earliest live point,
and lowest VReg leader; it chooses the lowest compatible view. Producer and
validator derive the domains, graph, and ordering separately. Exhaustion is a
typed pressure result for the separately governed spill/recovery work, not
permission to invent a home transition.

The first non-rematerializing recovery boundary is a target-neutral logical
spill-operation plan. Its tiny entrance joins validated selected instructions,
live ranges, allocation legality, and deterministic spill choice, then sends
the proposed plan through a separate replay rung. V1 admits only an
active-resident non-address unsigned-U64 instruction result in one block. It
records one logical storage identity, a store immediately before the incoming
definition, a reload immediately before the first strictly later flexible use,
and the complete ordered later-use rewrite suffix. The plan is versioned and
identity-bound, but creates no selected instruction or virtual register and
claims no physical slot, byte offset, alignment, frame, unwind, trap, address
stability, encoding, or publication authority. Incoming victims, entry
parameters, legalization temporaries, fixed or use-def suffixes, and
cross-block ranges fail with typed errors rather than silently receiving a
weaker recipe.

The next target-neutral boundary is validated stack-slot coloring. Its
26-line entrance consumes only the validated logical-spill carrier and joins a
canonical producer to an independent replay validator. V1 derives a closed
block-local lifetime from pressure through the first reload rewrite, sorts by
block/start/end/storage identity, and assigns the lowest available 8-byte slot;
touching endpoints conflict, while disjoint or different-block lifetimes may
reuse a slot. Offsets are relative to an abstract spill-area origin. The
versioned, identity-bound artifact grants no stack-pointer or frame-pointer
offset, stack direction, red-zone, shadow-space, frame, unwind, probing,
instruction, encoding, trap, or publication authority. The current logical
schema carries at most one spill action per function, so cross-target compiler
fixtures reach offset zero while internal interval tests pin the full first-fit
contract.

Abstract spill insertion is the next small join. It consumes the validated
logical operation and slot-coloring receipts and emits a deterministic
per-function schedule containing the abstract store, reload, and complete
later-use rewrite suffix. Its independent replay re-derives the ordering,
register-environment and allocator-availability roots, logical reload class,
and spill-area-relative geometry. It does not create an instruction, choose an
opcode or stack/frame address, or claim memory, trap, unwind, probing,
encoding, emission, or publication authority.

Reload-value home assignment then gives that logical reload a bounded physical
view without pretending the reload is already a real virtual register. V1
reconstructs the original linear-scan prefix, applies exactly the validated
single block-local spill, intersects the victim's legal views across the
reload lifetime, and selects the lowest view compatible with every coexisting
home. The producer uses a sorted linear schedule; independent replay uses a
point-indexed event timeline with explicit original and reload occupants and
separate work reconstruction. Reload pressure and later secondary pressure are
typed failures. Recursive spill recovery, synthetic-register realization,
memory effects, ISA lowering, and frame integration remain later boundaries.

The recursive logical insertion schedule now crosses one further compiler-
private boundary into target-neutral spill pseudos. Its small entrance assigns
dense function-local identities to stores and reloads, retains abstract
spill-area-relative storage, distinguishes an original selected VReg source
from a prior reload-action source, and binds each operand rewrite to the exact
reload pseudo that produces it. Direct traversal and an independently keyed
replay agree before custody is issued. These pseudos are neither selected nor
machine instructions and carry no address, memory-effect, frame, trap, unwind,
encoding, emission, or publication authority; those remain later boundaries.

Synthetic reload-value binding closes only the next namespace seam. Its V1
artifact assigns each validated logical reload/home pair an epoch-zero ordinal
in canonical function and logical-value order, retaining the insertion and
home identities plus lifetime, class, and chosen view. Production uses direct
function traversal; replay independently indexes logical reloads and sorts the
closed binding set. The synthetic identity is deliberately not a selected
`VirtualRegisterId`. Bounded recursive epochs/worklists, spill pseudos, real
instructions, memory effects, and all frame/publication work remain absent.

The bridge-chain recipe now has its own scalar-selection family and independent
selected replay. Its selected shape contains nine virtual registers and twelve
instructions. Under the public two-view availability policy, both x86-64 and
AArch64 traverse liveness, ranges, legality, spill choice, logical spill, slot
coloring, abstract insertion, and reload-home replay before reaching the exact
typed `ReloadPressure { function: 0, result: 0 }` branch. The fixture binds the
pressure point, incoming and victim values, store-before, reload-before, and
complete rewrite suffix without directly constructing any compiler-private
`Validated*` receipt. This establishes the deferred branch as reachable; it
does not itself publish an epoch-one seed. The adjacent bounded spill-recovery
worklist begins only at that independently reproduced failure and emits exactly
one `{epoch: 1, ordinal: 0}` item. V1 retains the source reload, machine, block,
half-open lifetime, reload class, complete canonical candidate domain, and
separately identity-bound trigger/worklist budgets. Production and validation
reconstruct all usage axes independently. The item does not choose a second
victim or assigned view, create a selected virtual register or rewrite, or
grant memory, frame, trap, unwind, encoding, emission, or publication
authority.

The adjacent spill-recovery choice entrance is that first consumer. Its V1
artifact reconstructs the post-first-spill allocation from the validated
worklist and retains the complete active-resident and recoverable-contender
rosters. Production uses a sorted allocation schedule; independent replay uses
a point-indexed event timeline. The bounded policy selects the resident with
the farthest live end, then the highest virtual-register identity. It records a
choice only: no eviction, logical spill, assigned view, selected identity,
rewrite, storage, memory, frame, trap, unwind, encoding, emission, or
publication authority follows from it.

The next 39-line executable entrance is
`allocation/spill_recovery_actions/mod.rs`. Its V1 artifact consumes the
validated work item and second-victim choice, then independently reconstructs
epoch-one logical storage, a store ordered before the source logical reload, a
reload before the victim's first strictly later flexible use, and the complete
later-use rewrite suffix. The namespace, source roots, victim type/origin/range,
current and reclaimed views, anchors, rewrites, and five-axis work usage are
identity-bound. These are target-neutral obligations only: the artifact creates
no virtual register, instruction, slot or offset, memory effect, frame, trap,
unwind, encoding, emission, or publication authority. Generalized insertion
and slot scheduling now enter the adjacent
`allocation/generalized_spill_insertion/mod.rs` join. Its V1 producer recolors
the epoch-zero and epoch-one closed lifetimes by deterministic 8-byte first fit;
independent replay reconstructs keyed sources and occupied-offset sets. One
canonical event stream orders stores before reloads before rewrites at equal
points, and the epoch-one store explicitly names the triggering epoch-zero
reload it precedes. The current fixture's `[9,12]` and `[12,14]` closed
lifetimes conflict at point 12, so they occupy offsets 0 and 8 in a 16-byte
abstract spill area. This remains target-neutral scheduling: it grants no real
register, instruction, memory, frame, trap, unwind, encoding, emission, or
publication authority. Generalized reload-home reanalysis is the adjacent
65-line join. Its producer walks a sorted allocation schedule while independent
replay reconstructs a point-indexed event timeline. The carrier retains one
canonical outcome per generalized action, stopping after the first pressure
because later homes depend on resolving it. In the public two-view fixture,
epoch zero receives a home for `[12,17)`, then epoch one retains exact pressure
at point 14 for `[14,15)`, its complete two-view domain, and both blocking
homes: the epoch-zero reload and original `v5`. The artifact binds every source
root and exact work usage `{3, 4, 18, 1, 3}` but creates no selected VReg,
instruction, memory, frame, trap, unwind, encoding, emission, or publication
authority. Turning that retained pressure into the next bounded recovery item
is the next allocation boundary. The adjacent 25-line
`generalized_spill_recovery_worklist` entrance now performs that projection.
V1 accepts only epoch-one pressure and emits one distinct compiler-private
epoch-two work identity per pressured function. Direct production and keyed
replay separately retain the source pressure action and lineage, block,
`[14,15)` lifetime, class, complete two-view domain, both blocker homes, all
custody roots, and exact usage `{2, 2, 13, 1, 1}`. The item is not a generalized
spill action or selected VReg; it chooses no victim or home and grants no
instruction, memory, frame, trap, unwind, encoding, emission, or publication
authority. It feeds the adjacent epoch-two victim-choice boundary.
That boundary now enters through the 56-line
`generalized_spill_recovery_choice` coordinator. V1 independently reconstructs
the complete blocker roster as typed residents with exact lifetimes and views,
proves which single omissions recover a candidate view, and ranks only those
contenders by farthest live end then highest canonical value. The current
dual-target fixture retains both original `v5` and the epoch-zero reload as
recoverable contenders and selects the latter at `[12,17)`. Direct traversal
and keyed replay bind the work item, source pressure, candidate domain,
residents, contenders, chosen/reclaimed views, every custody root, and exact
usage `{2, 2, 13, 1, 1}`. This remains choice evidence: it performs no
eviction, logical spill action, selected rewrite, memory/frame operation, trap
claim, encoding, emission, or publication. An explicit guarded-original policy
also binds the selected-plan and live-range roots. Separate producer and
point-indexed replay leaves admit an original to ranking only when its selected
role and flexible post-pressure use suffix are independently proven, then rank
eligible originals before reloads. Exact guarded usage is
`{4, 2, 43, 1, 1}`. The fixture's `v5` is used at the pressure point, so it
remains visible as a typed contender but is excluded from guarded ranking;
forging it as selected fails closed. The adjacent 43-line
`generalized_spill_recovery_actions` entrance now converts the selected reload
victim into target-neutral epoch-two obligations. Direct traversal and an
independently keyed replay agree on the victim action, current/reclaimed view,
store-before-pressure-reload anchor, reload-before-first-later-rewrite anchor,
complete later generalized-rewrite suffix, and all custody roots. The public
dual-target fixture stores action `{0,0}` before `{1,0}` at point 14 and
reloads it as `{2,0}` before the point-16 rewrite under exact usage
`{1, 1, 7, 1, 1}`. It creates no selected register or instruction and grants no
physical slot, address, memory, frame, trap, encoding, emission, or publication
authority. The same entrance now exposes a separate V2 original-victim entry.
V1 keeps its original signature and exact identity encoding; V2 alone binds the
selected-plan and live-range roots and retains a closed original-versus-reload
victim type. Direct traversal and independently indexed replay reconstruct the
original U64 definition, one-block lifetime, pressure-point absence, and every
later flexible use before emitting the store/reload/rewrite obligations. The
public dual-target graph stores `v5` before point 14 and reloads it before the
point-16 use under usage `{1, 1, 38, 1, 1}`. The adjacent 26-line
`recursive_spill_insertion` entrance now exposes a separate original-victim V2
policy beside the byte-stable reload-victim V1 policy. Direct projection and
independently keyed replay admit only the matching victim/source kind, recolor
the complete closed-lifetime set, and reconstruct one canonical event stream.
A typed action-source enum retains the original VReg without pretending it is a
compiler-private reload action. In the current dual-target fixture, `[9,12]`,
`[12,14]`, and `[14,16]` receive offsets 0, 8, and 0 in a still-16-byte abstract
spill area; the original-victim schedule contains eleven events under exact
usage `{1, 3, 15, 3, 4}`. The epoch-two store names `Original(v5)`, precedes the
pressured reload at point 14, and the new reload/rewrite occur at point 16.
This remains target-neutral logical scheduling and grants no physical slot,
address, instruction, memory, frame, trap, unwind, encoding, emission, or
publication authority.

The adjacent final recursive-home boundary independently allocates every reload
segment introduced by that schedule. It preserves prior generalized homes,
cuts the spilled original or reload lifetime at each store, reconstructs later
segments and their complete candidate/coexistence rosters, and chooses the
lowest compatible physical view. Sorted production and a separate point-indexed
timeline replay agree for reload- and original-victim chains on x86-64 and
AArch64. This is physical-view custody only: it creates no instruction, stack
address, memory effect, frame, fault claim, encoding, emission, or publication
authority.

Homed spill-pseudo V2 is the next separate boundary. It consumes the validated
V1 pseudo schedule and final recursive homes, preserves every V1 row, and adds
the mandatory physical `destination_view` to reload pseudos. The V2 producer
and independently keyed replay bind the V1, home, register-environment,
allocator-availability, optimization-unit, fuel, budget, and usage roots while
a golden pins V1 identity bytes. It grants no ISA opcode, stack address, memory
effect, frame, fault, encoding, emission, or publication authority.

Abstract spill-memory effects form the next V1 boundary. The artifact maps
each homed store/reload to an exact target-neutral `Write`/`Read` row and retains
the pseudo/action anchors, typed source/result lineage, views/class, and
abstract storage offset/size/alignment. Its direct producer and independently
keyed replay agree on order, roots, identity, and bounded work across both
victim lineages and architectures. The model intentionally has no fault field,
address base, frame coordinate, opcode, encoding, emission, or publication
carrier; executable spill realization remains behind Q1.

Abstract spill-access constraints are the next target-neutral boundary. The V1
policy assigns each effect its dense within-block placement and records exact
`StoredValue`, `DeclaredBeforeReload`, and `OverlappingAbstractSlice`
dependencies while retaining the pseudo, point, instruction anchor, and
relative geometry. Direct construction and a separately keyed replay agree on
six placements, three data edges, two declared barriers, seven overlap edges,
and usage `{7, 15, 33, 18, 22}` for both victim lineages on x86-64 and AArch64.
No row establishes cross-block order, program-memory non-aliasing, an executable
address, frame layout, spill-fault behavior, opcode, encoding, emission, or
publication authority.

Non-authoritative spill-frame requirements form the next pipeline-owned V1
boundary. Its small entrance authenticates those access constraints against the
validated target register environment, then reports only each function's
abstract spill-area byte extent and maximum alignment plus the exact selected
ABI preservation convention, stack alignment, and red-zone capacity. A direct
traversal and independently keyed replay bind both source identities, target,
closed policy, budget, usage, plan identity, and receipt across the supported
x86-64 and AArch64 target matrix. Red-zone capacity is an ABI fact, never a
decision to use it. Independent leaf coverage defines a zero-access row as
extent zero with neutral alignment one; the currently validated upstream route
constructs only spill-bearing access constraints. The carrier cannot choose a
stack/frame base, final offset or size, red-zone or shadow-space placement,
prologue/epilogue, instruction, probing, unwind, fault behavior, executable
access, or publication.

The original-victim canary now reaches this allocation boundary. Its exact
graph is
`r + ((r + (a + b)) + (b + r))`: its middle original remains unused at the
epoch-two pressure point and is therefore eligible for guarded-original
ranking. Canonical Psi proof production now advances that prerequisite with a
bounded first slice: the 94-line `direct_add/mod.rs` entrance preserves the
existing correlated, strict-targeted, and flat strategies before a named
`conjunction/` rung combines two recursively proven affine-chain scalar
endpoints under one shared definition/depth budget; relaxed targeted search
remains last. Completed evidence is immediately re-admitted by the proof kernel
and survives source lowering, Terminal codec round-trip, and independent
verifier replay. One additional bounded rung admits exactly one internal
computed-plus-computed definition per query: two recursively proven
fixed-integer endpoints plus one exact cited definition produce a
kernel-checked `IntegerExactAddDefinitionBound`. The intended graph now crosses
that full proof path. The shared search state separately budgets the one
computed join; a second join exhausts and refuses, so arbitrary exact-add DAG
proof authority is not implied. A distinct appended legalization recipe and
independent replay retain the complete graph, and selected construction emits
exactly 10 virtual registers and 13 instructions. In the public x86-64/AArch64
fixture, guarded choice compares `Original(v5)` at `[13,17)` with the epoch-zero
reload at `[12,19)`, prefers the eligible original under exact usage
`{4, 2, 46, 1, 1}`, and rejects forged choices and cross-target custody. The
following V2 logical-action entry now retains the exact original store, reload,
and rewrite obligations. Its matching V2 recursive-insertion policy carries
those obligations through the abstract schedule and existing spill-pseudo
boundary while keeping every physical claim closed.

Register units model aliasing between views. Flags/predicates, vector lanes,
special registers, ABI reservations, call clobbers, and stack/frame constraints
are explicit target facts. Modulo scratch-register assignment is not an
allocator.

The temporary empty-selection compatibility assignment is navigable without
pretending to be that allocator. Its plan coordinator validates the entry
roster, its function coordinator retains provenance, and one exhaustive
carrier router descends into cleanup, boundary, Unit, structural, scalar,
placement, control, and expression families. This preserves the explicit
replacement boundary while the selected physical conveyor gains full
operation coverage.

## Post-allocation machine stage

This stage consumes physical symbolic instructions, selected liveness, homes,
and the physical register model. Its catalog currently contains exact families
such as:

- AArch64 compare-zero plus branch-nonzero to `CBNZ`;
- AArch64 shortest MOVN-seeded i64 materialization; and
- x86-64 zero i64 materialization via `XOR r64, r64` when every canonical
  RFLAGS unit is dead-out; and
- x86-64 `0..=u32::MAX` i64 materialization via the canonical five- or
  six-byte `MOV r32, imm32` form. Its 32-bit write zero-extends the retained
  semantic 64-bit destination and preserves RFLAGS; and
- x86-64 i64 materialization whose exact bits round-trip through signed i32
  via the canonical seven-byte `REX.W + C7 /0 r64, imm32` form. Its 64-bit
  write sign-extends the immediate and preserves RFLAGS.

All produce variants of one validated post-allocation stage result. The result
contains the original source identity, exact rule identity, validated symbolic
plan, accounting, and custody receipt. Encoding dispatches on that typed plan;
the complete compiler route does not grow a new parallel carrier family for
each rule.

CBNZ is the first rule whose producer consumes a declarative symbolic
instruction-pair descriptor. That descriptor states the exact compare and
branch alternative keys; operand and allocatable `x0..x30` view shape; named `nzcv`
and `pc` unit sets; memory, stack, trap, and control effects; compare-to-branch
liveness continuity; and flags-dead-after eligibility. The shared matcher is
partitioned into instruction, register, and liveness leaves under one small
coordination entrance. The rule's independent validator deliberately retains
its separate replay logic. Other materialization rules remain rule-local
selection until a genuinely shared pattern vocabulary is demonstrated.

The shared matcher now has a second, still-bounded descriptor consumer. The
AArch64 same-view-copy owner matches an exact body-tail/terminator
`CopyI64; ReturnI64` pair, including cross-instruction virtual-register and
physical-view/storage relations, then joins its proposal to an independently
replayed symbolic disposition and canonical codec. Its exact opt-in and sole
machine-catalog row now route a rule-neutral disposition through encoding,
zero-byte layout, whole-function exit custody, realization, fragment/object,
and callable publication. No ordinary lowering currently emits that terminal
pair; fixed-view recovery emits a different shared-entry shape and composition
fails with a typed refusal. Consequently applied positive coverage remains at
the machine-rule boundary. Compiler coverage proves honest zero-action routing,
not arbitrary peephole topology, general copy removal, or a compiler-generated
applied deletion.

The exact `Aarch64ElideSameViewCopyI64BeforeCompareZeroV1` rule proves the
second closed instruction-pair topology. It scans adjacent ordinary-body
`CopyI64; CompareI64Zero` instructions, requires equal copy source/destination
physical view and storage plus an exact destination-to-compare VReg relation,
and records only the copy disposition. Its small owner joins descriptor-based
proposal to independent rule-local replay of footprints, liveness, provenance,
actions, and revisions. The existing copy-elision artifact family advances to
an authenticated V2 policy tag, while the sole new catalog row and exact build
selection use the generic composition, layout, realization, manifest, object,
and callable route. The topology enum admits only body-tail/terminator and
adjacent-body pairs; it grants no generic or arbitrary-length matcher.
Rule-local coverage pins the exact nonzero five-axis work vector and every
representable first-over boundary, independently replayed action corruption,
and authenticated codec failures. Compiler-facing coverage retains the exact
policy through deterministic zero-action selection, all generic publication
custody fields, and Linux/macOS AArch64 object and callable publication. It
does not relabel that honest no-candidate fixture as an applied deletion.

The adjacent `costs/` rung is deliberately non-authoritative. Its V1 model
binds the complete native target and model version into a stable identity, and
projects existing `MachineSizeKnowledge` without converting encoder-resolved
bounds into exact facts. Latency remains explicitly unavailable. The result is
safe input for future ranking and reports only: exact-rule eligibility,
independent replay, and semantic validation cannot consult it, and the source
organization audit enforces that dependency boundary.

Direct homes and homes after selected lowering enter one
`StagedPostAllocationMachineFunctionRelativeRealization`. CBNZ, MOVN,
same-view-copy elision, XOR-zero, MOV-r32-imm32, and MOV-r64-imm32 therefore
share the same encoding, layout, exit, realization, and fragment source route.
The former named CBNZ/MOVN complete-route carriers have been removed; rule-
specific values remain typed leaves borrowed from the shared result.

Selected-lowering realization likewise enters fragment admission through one
`SelectedLowering` carrier whether or not a function-relative layout rule also
ran. Add/subtract folds do not acquire fragment route variants, and rel8 remains
an optional typed leaf of the selected-lowering realization rather than an
admission prerequisite. The fragment manifest records the generic phase source
kind while its selection and realization identities retain the exact rules.

The incoming-u12 add/subtract producer emits an immutable fold plan. Its
validator separately reconstructs source eligibility, register constraints,
the exact action roster, rewritten instructions, provenance and fuel custody,
dense identifiers, work usage, and transformed-plan identity. Validation does
not call the producer's transformation helpers; an architecture dependency
guard enforces that separation.

The adjacent machine catalog is also the architecture-admission point. CBNZ
and MOVN require AArch64; XOR-zero, MOV-r32-imm32, and MOV-r64-imm32 require
x86-64.
Each row joins the exact optimization name, required architecture, and closed
execution kind. The selected row survives physical composition intact; both
source lineages dispatch on its typed kind instead of reconstructing a second
name-to-implementation switch.
Function-relative rel8 relaxation declares x86-64 in its adjacent layout
catalog. Unsupported target selection is rejected with the exact optimization,
required architecture, and actual architecture before rule dispatch; custody
errors preserve this reason instead of converting it to a generic
phase-composition or root mismatch.
Linux, Windows, and UEFI x64 share x86-64 applicability, while Linux and macOS
Arm64 share AArch64 applicability. UEFI applicability does not grant its still
unimplemented publication authority.

## Encoding and layout

ISA crates own canonical form encoding/decoding and reconstructed effects. A
machine rule never hand-assembles bytes. Layout-independent encoding retains
row identity, decoded footprint, effects, provenance, and optimization
disposition.

The encoding entrance joins construction to a separate `validation/` rung.
That rung checks roots and normalized optimization custody, then descends
independently through ordinary rows, structural rows, and aggregate
counts/identity. Row validation consumes candidate bytes only through the
target-owned baseline, MOVN, XOR-zero, MOV-r32-imm32, MOV-r64-imm32, and
structural-call decoders; an
architecture guard forbids imports of producer row/structural encoders. CBNZ
dispositions are reconstructed from the typed optimization plan while its
unresolved branch remains explicit deferred control.

Function-relative layout resolves labels, branch extents, and exact byte
offsets. Its independent admission rung re-derives layout policy, canonical
block order, function/block spans, row offsets, structural call/return spans,
and aggregate identity from admitted pre-layout dispositions. Candidate x86-64
and AArch64 conditional branches are decoded with target-owned validators;
x86 displacement is relative to the branch end, while AArch64 displacement is
relative to the instruction address. Candidate evidence, fused CBNZ register
reads/effects, and structural unresolved-fixup custody must match that replay.
Layout rules such as x86 rel32-to-rel8 relaxation consume a complete baseline
layout and return a validated replacement. Baseline and selected byte counts
remain replayable.

## Exit and publication custody

The pipeline retains one chain:

```text
validated stage result
  -> selected-form encoding receipt
  -> resolved-layout receipt
  -> whole-function exit contract
  -> function-relative realization manifest
  -> fragment/text/object manifests
  -> optimized artifact manifest
  -> ordinary callable-entry manifest
```

Each boundary recomputes child identities and rejects detached, reordered,
truncated, trailing, or cross-source data. Generic artifact layers bind child
identities and do not need a new schema merely because a new exact rule exists.
The genericization changed the data carried at three serialized boundaries:
function-relative realization is v9, while fragment emission and fragment text
placement are v8. Their records retain the exact selected-lowering selection
or post-allocation optimization, not a broad optimization level.

Function-relative V9 persistence enters through a 78-line framing and final-
admission join. Canonical content encoding, ordered decoding, post-allocation
optimization tags and custody, target layout, rendering, errors, and cursor
mechanics descend into named leaves. The split preserves every byte and the
existing trailing-data, conflicting-transformation, then identity-mismatch
rejection order.
Its adjacent test taxonomy independently reauthenticates 32 direct manifest
fields and nine nested post-allocation custody fields, covers stale outer
identity separately, exercises 25 closed/envelope wire axes, and swaps all five
receipt roots. Real x86 rel32-to-rel8 and MOV-r32-imm32 public routes carry the
mutations to replay rather than stopping at codec acceptance.

The Terminal-Psi-to-native stage now exposes its full physical composition as
small owning entrances. Source-entry settlement replays declaration and
calling-plan custody. Native realization then chooses the ordinary or exact
selected input, admits provider executions/installations, emits machine code,
and replays object/image assembly. The optimized ProgramStorage encoding and
wrapper-object boundaries remain separate joins: encoding projects and replays
the target template; object construction joins settlement, semantic contract,
composite object, manifest, and custody. Provider projection, machine routes,
artifact assembly, semantic replay, object validation, models, codecs, and
diagnostics descend into named leaves.

The first non-scalar D29 Terminal lane retains one selected fixed-token
type+const application whose checked caller and specialized realization join
an exact all-affine whole-structural operand permutation and one scalar result.
It emits `CallStructuralScalar` and retains the authored operation coordinate;
private source and substituted-generic type spellings are reconciled only when
their independently checked structural shapes agree. Mixed structural/scalar
arguments, structural results, and native physical-child publication remain
outside this lane.

D32 requires this physical composition to issue one child receipt for every
settled boundary occurrence surviving the validated optimization projection.
The role-tagged `PhysicalChildParent` is either a reference to reconstructible
D29 operator-application coverage or a complete retained-and-replayed D41
boundary-trait settlement. Equal D29 applications may share one semantic
parent, but optimized-operation identities and physical children remain
distinct. Each child binds the domain-separated identities of its parent and
exact optimized operation before retaining selection, assignment, relocation,
and emitted-span custody. Artifact replay derives the survivor set and rejects
missing, duplicate, stale, substituted, padded, or role-swapped children; the
physical pipeline may omit only an occurrence whose verified optimization
proof establishes elimination.

The first admitted-provider child lane is intentionally narrow: an unoptimized,
zero-argument Unit normalized import. Its complete D41 parent binds exact
provider execution, selected plan, locator, boundary-entry plan, and same-stack
admission; its child binds the machine/object/image call, normalized import,
unresolved semantic relocation, all emitted spans, and final image-symbol
identity. Wider call shapes and optimized foreign calls remain fail-closed.

For UEFI, the physical adapter contract is settled but not yet implemented in
this chain. A generated ABI shell invokes one checked bootstrap adapter;
physical-arrival and firmware-service postconditions supply its opaque
premises. The adapter establishes Loaded Image correspondence and independent
initial storage, proves resource composition, crosses
`ProgramStorageEntry::enter`, calls the semantic continuation, reclaims
returning-profile resources, and maps normal Unit return to success. Crash,
trap, and abort remain non-returning. Optimization selection still grants no
firmware image or publication authority until that adapter, native-image
validation, and selected-build publication join the same custody chain.

## Composition policy

During bring-up, phase compositions are deliberately narrow. A route accepts
only exact implemented sets and rejects the rest with a closed error. The
long-term coordinator executes each stage catalog over one typed stage carrier;
it must not encode an optimization name in top-level route variants.

The current policy is centralized at
`coordination/physical_pipeline/routes/composition/mod.rs`. Owning catalogs
first validate each phase selection and target predicate; this entrance alone
admits baseline, selected lowering, one allocation-recovery rule alone, one
post-allocation rule with optional selected lowering, function-relative layout
with optional selected lowering, and four exact cross-phase pairs:
active-resident immediate-U64 multi-use rematerialization followed by AArch64
MOVN, x86 XOR-zero, x86 MOV-r32-imm32, or x86 sign-extending MOV-r64-imm32
selection.
Multiple recovery or machine rules, every other recovery-machine pair, and
machine plus layout reject before route execution.
The canonical post-allocation catalog entry survives composition and its closed
execution kind selects the named leaf; lower leaves independently validate
transformation and custody.

## Required tests

- disabled selection preserves baseline bytes and identities;
- wrong target, form, allocation, or live-out facts reject;
- producer and independent validator disagree on corrupted plans;
- canonical encoders reject alternate or trailing byte forms;
- exact byte deltas and offsets replay after layout;
- every custody boundary rejects one-field identity corruption; and
- direct, selected-lowering-composed, and final artifact paths retain the same
  full selection identity.

The catalog matrix covers all 18 current exact names across all five native
target constructors: 70 admitted cells and 20 typed architecture rejections.
Target-independent Psi, selected-lowering, and allocation-recovery rules are
explicit declarations, not untested fallthrough behavior.

The adjacent composition matrix covers all 136 unordered exact-name pairs on
both x86-64 and AArch64. Its 272 cells contain 140 admitted routes, 72 typed
composition rejections, and 60 target rejections. Every cell also checks the
exact Psi pass projection and proves that overlaying the complete Psi suite
does not change the physical disposition; focused triple cases pin the two
selected-lowering rules with machine and layout routes.

Current XOR-zero coverage proves both direct and selected-lowering routes
and its active-resident zero-rematerialization composition through fragment,
object, and callable publication. The composed route consumes recovery's
recomputed liveness, so the rule's exact dead-RFLAGS predicate is checked after
the selected graph changes. Target-register-environment
coverage selects and corrupts the exact scalar-call ABI row for System V AMD64,
Microsoft x64, AAPCS64, and Darwin AAPCS64 across Linux x64, Windows x64, UEFI
x64, Linux Arm64, and macOS Arm64. It checks argument/result views, implicit
control and stack facts, every individual call clobber, preserved-unit
injection, platform ABI substitution, preservation-convention drift, and the
Microsoft structural-Unit call row. This does not claim general call-crossing
allocation coverage. One exact attached-Unit U64 fork/join chain is now
represented in the selected CFG on Linux System V AMD64 and AAPCS64: two
constants feed two independent calls, and their results feed a third call.
Selection uses explicit copies around fixed argument/result views rather than
precoloring the durable values themselves. The first result is live through
the unrelated second call; liveness retains that call's complete clobber set,
legality removes every aliasing caller-saved home, and deterministic
allocation selects a preserved home without spilling on both ISAs. The legal
and selected representations independently bind the exact three-call grammar,
callee identities, ABI rows, values, provenance, fuel, and target identity.
This stops before machine-effect analysis because the current effect
vocabulary cannot honestly describe x86 call stack/memory behavior. It grants
no callee-save prologue/epilogue, relocation, encoding, emission, publication,
or general scalar-call authority.

Applied selected-lowering publication coverage crosses both exact incoming-u12
rules with every hosted target: Linux x64, Windows x64, Linux Arm64, and macOS
Arm64. Each row retains two literal-fold actions through encoding, fragment,
text, ELF/COFF/Mach-O object construction, and ordinary-callable admission;
repeated runs compare every serialized manifest, record, text span, and object
container. UEFI and unwind are not implied by this matrix. The former lacks
publication authority, while physical frames and unwind carriers remain P5
prerequisites.

Post-allocation publication coverage crosses all seven exact machine rules
with both applicable hosted operating systems: three x86-64 rules on Linux and
Windows, and four AArch64 rules on Linux and macOS. The 14 rows run twice and
compare authenticated realization, fragment, text, object, artifact, and
callable records plus final text and ELF/COFF/Mach-O container bytes. Applied
materialization and fusion fixtures require nonzero actions; the two copy-
elision fixtures retain honest zero-action custody because current lowering
does not emit their exact candidates. Every rule also has one typed wrong-
architecture refusal before execution. This matrix grants no UEFI publication
or unwind claim.

Ranked countdown coverage proves ordinary executable-image and installation
custody on Linux x64 and Linux Arm64, including exact final bytes and semantic-
code attribution.

The ordinary empty-selection compiler route is deliberately outside this
optimizer custody chain. Its four-target byte and artifact-metadata baseline is
locked by the no-selection golden compiler test; only an explicit nonempty
selection constructs the optimizer-side prephysical carrier.

Allocation recovery has one final function-relative carrier. Its closed source
taxonomy has `FixedViewCopies` and `ActiveResidentRematerialization` leaves;
each leaf retains its exact upstream receipt and post-allocation transformation
identity, while the carrier owns the common machine plan, selected-form
encoding, resolved layout, whole-function exit contract, and realization
manifest. Recovery alone returns the physical pipeline's `AllocationRecovery`
variant, and fragment admission consumes one `AllocationRecoveryV1` source
kind. The admitted active-resident-plus-materialization pairs instead move the
same source taxonomy into the generic post-allocation realization's
`AfterAllocationRecovery` leaf. That join independently replays recovery,
machine plan, exact materialization, encoding, layout, and exit custody and
publishes the generic post-allocation source kind. Neither recovery rule owns a
parallel publication vertical. Fragment and fragment-text manifests use schema
v9 because the source-kind tag denotes these generic carriers.
