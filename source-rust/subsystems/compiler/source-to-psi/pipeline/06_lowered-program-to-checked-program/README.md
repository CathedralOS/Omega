# 06 - Lowered program to checked program

This crate is the lowered-form checker: it decides whether a program's
proof obligations hold, and its verdict is the compiler's only verdict.
[Structure-only elaboration](../05_prepared-typed-trees-to-lowered-program/README.md)
(stage 05) writes the obligations into the lowered form (Terminal Psi), and
this crate accepts one only when the proof kernel accepted a certificate for
it. It is the third piece of
[check on the lowered form](../../../../../../wiki/drafts/designs/check_on_lowered_form.md):
every other outcome is a `Refusal` with reason `Unproved`. There is no
interval verdict, no admitted route and no assumption, so a discharged
obligation is one that [stage trust](../../../../../../wiki/drafts/designs/stage_trust.md)
rule 2 lets remove a runtime check. The product driver passes the complete
Terminal checking input to `judge_compilation`; this stage never elaborates
typed source or calls stage 05. [The route's verdict](#the-routes-verdict) is
the compilation verdict, including every demanded generic and conformance form.

## Find the code

Start at [`judge_compilation`](src/verdict.rs) to follow the checks in order. [Publication](src/publication.rs) issues the checked program after acceptance.

- [Obligations](src/obligations/) identifies what operations owe and prepares machines for judgment.
- [Control flow](src/control_flow/) follows edges, joins, and termination rankings.
- [Storage](src/storage/) checks initialized places, borrows, and cleanup.
- [Calls](src/calls/) checks inputs and results against contracts, including selected providers and generic machines.
- [Effects](src/effects/) checks what can crash, wait, or reach a service.
- [Proofs](src/proofs/) derives and reuses evidence and judges laws. The kernel decides the certificates submitted to it.
- [Reports](src/reports/) explains refusals and unreachable branches.

An abstract algebra can cite a law of its supplied record to prove identity of
two operation results. The checker instantiates the law's signature and records
the kernel-checked proof of the requested identity. For a concrete record determined by its fields, the
[automatic discharge rule](../../../../../../wiki/spec/proofs/contracts.md#record-identity-from-field-identities)
requires a kernel proof built from their identities.

A law over data that contains itself is judged apart from every machine.
Stage 05 hands this crate one
[mathematical signature](../../representations/lowered-program/README.md#the-mathematical-signature)
for the compilation. [laws.rs](src/proofs/laws.rs) has the kernel check its
declarations once, then proves each law against the entries before that law's
own statement. For a law that dispatches on a formal it proves one equality
per case, over an arbitrary value of that case, and assembles the cases into
one induction. A case is proved by normalizing both sides with the kernel and
rewriting with the laws the case cites and with the law itself at a field of
the case ([equality.rs](src/proofs/equality.rs), `prove_under`). The kernel checks
the assembled term against the law's statement; the judgment is
`RouteVerdict::Laws`, and a law or case left unproved is one entry of
`RouteVerdict::UnprovedLaws` that names the equality and the arm.

A law's premises are hypotheses of each of its cases. Where a case cites a
law that has premises, the checker proves each at the cited arguments from
the case's other evidence, in whatever order the body wrote its citations; a
citation whose premise cannot be proved refuses the case. Two facts about
constructors come with every equality the case holds. An equality of two
values of one case gives the equality of each pair of fields, so
`Succ(x) = Succ(y)` gives `x = y`. An equality of two different cases proves
the case's goal outright, because no value satisfies it. Both are kernel
terms built from the data's induction principle, checked with the rest of
the proof.

The search rewrites both sides of the goal with the equalities the case
holds, smallest term first, until the two sides reach one term. It never
uses an equality in the direction that puts its own left side inside its
right side (`x` to `plus(x, Zero)`), which would apply to its own result
without end; the other direction, applied from the other side of the goal,
covers what that direction would have. A goal no equality proves therefore
ends as "no proof", not as an exhausted budget.

A law's proof follows its body. A body that dispatches on a formal is an
induction on it; a case that dispatches again, on another formal or on a
field, is a case split inside that case, to any depth (`Proving::split` in
`laws.rs`). Each split states what its subject must satisfy as the goal for
the variables bound after it and binds them again in every case. The
induction binds the earlier formals again as well, so the law is a
hypothesis for every value of them. A case whose premises no value
satisfies needs no argument: the premise that equates two different
constructors refutes it.

A case may also split on a computed value. The goal is then stated for any
value in its place together with a proof that the computed value equals it,
so each case both computes where the value stood and holds the equation as
evidence. A law about a machine defined on fuel is proved for every amount
of fuel by induction on it, the same script in each case, with the law at
one unit less as what a self-citation names; with no fuel left there is no
such hypothesis and the goal must follow without it.

Two classes of a quotient are equal when their representatives are related.
When the search proves no equality of two classes, `Proving::sound` in
`laws.rs` reads the representative out of each side, proves the relation's
formula at the two with the same search and evidence, and applies the
kernel's soundness assumption for quotients to that proof. The kernel's
receipt then counts the assumptions the law rests on: the quotient type, the
class of a value and soundness.

Some laws are not free to state what they like: a machine that satisfies a
trait requirement, a row of the equivalence a quotient selects, and the
congruence and transport theorems of a quotient operation each owe one
statement, which stage 05 records on the law (`DeclaredLaw`). After the
kernel has checked the proof, the law's equality and premises are compared
with the recorded ones by the kernel's conversion. The premises are matched
in any order, because a machine's `requires` clauses arrive in canonical
order and not as written, and there must be no premise more or less. A law
that states something else is refused with the recorded reason, for example
"the proved equality is not the `symmetric` law that the quotient `Sign`
needs of `same_sign`".

A theorem over data indexed by values is proved by induction over the value
(`prove_family` in `laws.rs`). Each case of the data is a goal about
integers: for arbitrary indices and payload values, from what the data's
declaration requires of the case and the theorem at each child the body
cites, the theorem at this index. The integer search of the proof-admission
crate (`SourceIntegerTheory`) finds a proof of each case, and also shows that
what the case's step requires follows from what the data requires. The search
states a goal about named unknowns, so each case declares its indices and
payload values as unknown integers, proves the goal about them, and the
kernel's `abstract_certificate_assumptions` turns them into the binders of the
case's lemma. The kernel's indexed-data builder assembles the lemmas into the
induction and the kernel checks the result once, within the law's budget of
conversion steps. The receipt counts the integer vocabulary and the integer
laws the cases used; the family, its constructors and the induction are
definitions and add nothing. A case with no proof is reported by its name:
"cannot prove ensures contract for `vector_bound` in the case `Vector::Cons`".
The same search shows, for each child the case's step cites the theorem at,
that the index the theorem is ranked on is smaller there. A tree's node has
two children and `left + right + 1` nodes, so `left + 1 <= left + right + 1`
needs `0 <= right`: the search bounds a sum by one part when the other is not
negative, and the case supplies that for each unsigned value.

A value of such data that a running machine builds owes what its case
requires (`prove_integers` in `laws.rs`). Stage 05 states one law per
requirement: for every value of the machine's value parameters that meets
the machine's own `requires`, the requirement holds of the literal's values.
The same integer search proves each one against the vocabulary stage 05
declared, and the kernel checks the proof. A requirement with no proof
refuses the program where the literal is written: "cannot prove what
`Vector::Cons` requires of the value built here in `room`".

A running machine that cites such a theorem does not call it: the
citation is a theorem application that names the theorem's machine, and the
machine imports the theorem's `ensures` after it as from a proof formal
(`theorems.rs`; `terminal_semantics::theorems::named_contract` takes the
implication from a named machine only when its body does nothing). The
theorem's own machine has no statements, so its guarantee has no proof
among the machines. The signature has one: the signature is judged before the
machines, and a machine whose `ensures` clause a proved law states for
every value of its formals (`LawJudgment::theorem`, set by `prove_family`)
takes that law as the clause's proof (`Discharged::Law` in
[lib.rs](src/lib.rs)). When the law is unproved there is no such proof, and
the machine's guarantee is refused as well as the law.

Each proof is tried twice: first with every definition that is not by cases
left as its name, so that a goal and its citations are compared as written,
then with those definitions unfolded. When the search of every order of
rewrites runs out of steps, the cited equalities are instead turned into
rewrite rules from the larger term to the smaller, and both sides of the
goal are rewritten to one normal form (`completed` in `equality.rs`). The
kernel checks whatever proof results, once, on the law's own budget of
65,536 conversion steps; each search has a separate budget for what it
computes on the way.

A hypothesis or citation about some operands of a longer sum is used there
too. `x + y = z` rewrites `x + w + y` to `z + w` when the conformance in
scope lets addition regroup and reorder, and `x * c + w + y * c` to
`z * c + w` when multiplication also distributes over it. The prover puts the
sum in canonical form, rewrites it to hold the rule's left side as one
operand, uses the rule and puts the result in canonical form again; each of
those steps is a law of the conformance or the rule, and all are in the proof
the kernel checks. Without such a conformance a rule applies only where its
left side stands as written.

A law about a record of mathematical values is proved inside the record's
fields: the record is a tuple, each citation rewrites inside one component,
and a tuple of every field of one value is that value.

Normalizing keeps a defined function folded: `plus(a, b)` stays `plus(a, b)`
until the argument it dispatches on is a constructor, and then unfolds one
step. A term is normalized again after each rewrite, since the rewrite may
have put a constructor where a function dispatches. The search also regroups and reorders the operands of an operation
under the signature's licenses, the associativity and commutativity laws the
program's conformance declares and that were stated before the law being
proved. It brings both sides of the goal to one arrangement (nested to the
right, operands in a fixed order where commutativity allows) and puts each
law it used into the proof term, so the certificate's assumptions name it.
Distributivity of Peano multiplication is the customer. From the right, its
successor case is the cited law at `pred` and one regrouping. From the left
it is four addends in another order, which needs commutativity too, and
commutativity needs `plus(a, Zero) = a`, a law that returns its own argument.
The kernel checks each law's proof once, as its certificate: 2,176
conversion steps of a 65,536 budget for associativity, 1,660 and 1,659 for the
two distributivity laws, 1,105 for commutativity. The largest law of
`core/nat.omg` takes 5,628.

When that finds no proof, the search tries again with products expanded over
sums under the distributivity laws the conformance declares, so both sides
reach one polynomial form. A product whose sum is on the side its law states
expands by the law; one whose sum is on the other side is commuted first when
its operation commutes. Each expansion is an application of the law in the
proof term. A product may expand to at most 64 summands (`MAX_SUMMANDS`),
counted before anything is built; a larger one stops the proof as out of
budget, never as wrong. The customer is `mul(add(a, b), add(c, d))` equal to
its four-term expansion over the library's `Nat`, judged in 993 conversion
steps on `mul_distributes`, `mul_comm`, `add_comm` and `add_assoc`
([polynomial_expansion.rs](src/tests/polynomial_expansion.rs)).

When every part is accepted, publication writes one Psi mathematical carrier
row per data declaration into the root module
([publication/mathematics.rs](src/publication/mathematics.rs)). The row names
the data by its source type identity, and its key is the data's type (its
canonical trees) under the declarations up to its own. The laws' certificates
stay in the judgment and are not published, by the owner's decision of
2026-11-23: a receiver trusts the producer and does not replay carried
certificates
([publication and separate consumption](../../../../../../wiki/spec/terminal-psi/product.md#publication-and-separate-consumption)).
Neither existing table fits one in any case. An indexed theorem root row names
an erased evidence formal, and an Id formation row names an erased formal's
Identity type and the source subjects it captures; a law over plain data has
neither.

The checker always uses branch conditions, dispatch bindings and value
relations as premises; no mode compares results with less knowledge. That is
separate from the specified
[automatic-discharge floor](../../../../../../wiki/spec/build/command_line.md#checker-conformance),
which keeps cited lemmas, branches and contracts, and from explicit stage
validators, which add checks without changing which sources are accepted.

Start at [lib.rs](src/lib.rs) (`discharge`). After shared inference settles,
[machine_inputs.rs](src/obligations/machine_inputs.rs) borrows each machine's existing
authored branch rows and associates its requirement role and first carried-proof
offer once. These are inputs, not accepted facts; proof transport and kernel
admission still run for each actual obligation. [minted.rs](src/obligations/minted.rs)
checks that the rows to decide are the ones the lowered form owes;
[graph.rs](src/control_flow/graph.rs) answers dominance between program points;
[places.rs](src/storage/places.rs) says what a place holds when a field read observes
it, from the stores [places/log.rs](src/storage/places/log.rs) derives from the
operations; [prove.rs](src/proofs/prove.rs) is the one prover.
[branch_warnings.rs](src/reports/branch_warnings.rs) reports impossible authored arms
from refutations the kernel accepted and the premises they cite. It shares
results only within one machine's read-only analysis, never retries with a
subset of the assumptions, and decides no obligation.
[joins.rs](src/control_flow/joins.rs) infers and checks join predicates;
[crashes.rs](src/effects/crashes.rs) states crash-site coverage;
[arrivals.rs](src/control_flow/arrivals.rs) admits a loop head's declared invariant.
[rankings.rs](src/control_flow/rankings.rs) admits a loop's ranking and states its
comparisons.
[prove/reach.rs](src/proofs/prove/reach.rs) says which unproved goals the integer
rules could not have decided
([below](#what-the-integer-rules-do-not-reach)).

## Forming declarations before using their facts

A contract clause justifies other code only once it is well defined:
`a / b > 0` means nothing where `b` may be zero.
[formation.rs](src/obligations/formation.rs) checks every partial scalar term with the
same safety goal, prover and proof kernel as an operation. Only clauses
formed on their own supply premises. A pending clause can use a clause formed
later in authored order, but a clause that depends on itself, or a cycle,
supplies nothing. Exact field paths tell entry contents from current
contents; a nested result gets its carrier's facts only after its own
computation forms.

The check covers machine and bodyless contracts, boundary guards and declared
data or scalar domains. An inferred input or result contract is formed before
anything uses it. What the check observes and proves stays private to it: it
adds no runtime operation and no saved evidence table.

`prepare_replay` runs formation and returns an exclusive borrow of the
elaboration. Replay borrows that formed module and checks the existing
certificates without searching for proofs. A declaration changes only through
a function that forms the new contract first, and no caller can change the
module while replay holds it.

## What it decides

| Kind | Goal | Hypotheses available at its point |
| --- | --- | --- |
| `OperationSafety` | Terminal semantics' canonical goal for the operation (`proof_bearing_scalar_leaf_semantics`): representability of an exact add, subtract, multiply or cast, a defined division or remainder, a shift count in range | entry `requires`, result equations of the operations before it, callee `ensures` after earlier calls |
| `CallRequirement` | the callee's `requires` clause at that position, its scalar formals replaced by the call's scalar actuals (embedded where the clause embeds them, see [Clauses over mathematical integers](#clauses-over-mathematical-integers)) and each field of a record formal by the one value that field of the record actual holds at the call | the same, before the call |
| `NormalGuarantee` | this machine's `ensures` clause at that position, with `result` replaced by the value that exit returns | the same, on the exit edge |
| `DataDomain` | one clause of a data type's `requires`, with `self` replaced by the values the place holds once the owing operation completes ([Data `requires`](#data-requires)); when a store or a zero establishment leaves it unproved, the same clause over what the place holds at each consumption point the [invariant window](#invariant-windows) reaches open | the same, before the operation or the consumption point |
| `PublishedGuarantee` | not proved: held when every return edge's `NormalGuarantee` for the clause is discharged | - |
| `ArrivalInvariant` | the destination block's predicate (a loop head's declared invariant, its member's `requires`, or a join's inferred predicate), its parameters replaced by this edge's arguments (`Truth` when the join has none) | the same, on the arrival edge |
| `CrashCoverage` | at a `Crash` terminator, its guard's clauses and the disjunction of this machine's published alternatives for the cause (`Falsehood` when it publishes none), stated over its entry storage; at a call, the callee's route for the cause, stated over the call's actuals with the binding its `requires` uses (a guard over a formal's element reads that element of the actual at the call), implies that disjunction | the same, before the crashing terminator or call |
| `RankComparison` | on an edge inside a ranked cycle, the rank the edge arrives with against the rank of the block it leaves: `<` where the edge claims strict descent, `<=` where it claims to keep the rank ([Rankings](#rankings)) | the same, on that edge |
| `WellFoundedness` | a structural `FiniteIntegerRanking` judgment retaining the component's complete checked rank bindings and finite integer carriers | no prover hypotheses; graph, tuple and carrier admission must pass |

A `Loan` is not proved: it is judged by structure ([Loans](#loans)). Any
other kind is refused as `Unsupported`.

It also makes two structural judgments, with one shared verdict per
operation and terminator that touches storage (`MachineDischarge::uses`),
and follows each invariant window to the points that consume it
(`MachineDischarge::windows`). [Initialization](#initialization) refuses a
use that some path reaches with nothing established.
[Payload access](#payload-access) refuses a case payload named where its case
is not established on every path, and a dispatch with no edge for a declared
case. Both refuse with reason `Unestablished`: like `Unproved`, an error in
the program, not a construct outside the subset.

Hypotheses come only from the lowered form and its tables. `discharge` fills
each machine's `knowledge` table with one row per entry `requires` clause
(`EntryRequirement`, at the entry block; a clause over a field of a record
formal is stated over that field's entry content value, and states nothing
when no operation names the field), per operation result equation
(`OperationSemantics`, on its normal successor), per field read whose place
holds one value on every path to it (`OperationSemantics` of the read; see
below), per `requires` clause of a data type that a read of established
storage can state on its own value (`OperationSemantics` of the read; see
[What a read learns from its type](#what-a-read-learns-from-its-type)), per
callee `ensures` clause (`CalleeGuarantee`, after the call, with the
callee's `result` replaced by the value the call defines;
[calls and outcomes](../../../../../../wiki/spec/terminal-psi/calls_and_outcomes.md#call-contracts):
"Normal guarantees refer to the actual result and appropriate parameter
versions."),
per edge of a conditional (`BranchCondition`: the condition is `true` on the
true edge and `false` on the false edge) and per payload parameter a dispatch
edge binds (`DispatchBinding` of that edge: the parameter equals the one
value its payload leaf holds, when there is one, and holds each clause of
its storage's type that a payload read at the head of the arm could state;
see [What a dispatch binds](#what-a-dispatch-binds)). Each row's proposition
is in `MachineTables::propositions`. The kernel gets a row for an obligation
only when the row's point dominates the obligation's. Requires and guarantees
go in as assumptions and result equations as semantic axioms, which is how
the kernel's definition rules cite them.

## Clauses over mathematical integers

A `requires` or `ensures` may compare mathematical integers,
`embed(rest) + embed(count) * 10 <= 99`
([structure elaboration](../05_prepared-typed-trees-to-lowered-program/README.md#mathematical-integers-in-a-clause)).
The one substitution (`substitute`) states such a clause wherever a scalar
clause is stated: at a call each embedded formal is the actual embedded, in
the actual's carrier, and a literal actual is the integer it denotes
(`bound_mathematical`); on a return the embedded `result` is the returned
value embedded (`returned_for_result`). The arithmetic around the embeddings
is over unbounded integers and is kept as written, so stating the clause
adds no obligation of its own
([mathematical values](../../../../../../wiki/spec/terminal-psi/mathematical_values.md#integer-terms):
"These terms denote unbounded mathematical integers and cannot overflow").

What the prover does with the goal is narrower than what a clause can say:

| Goal | Outcome |
| --- | --- |
| every value it names is fixed to a literal by a result equation (`packed(9, 9)`) | the kernel evaluates the closed comparison (`by_transport`, then `ClosedIntegerRelation`); a false one is `NoDerivation`, which rejects the call |
| `<=` between a literal and one add, subtract or multiply of values of one carrier and literals | bounded from the values' intervals, the rule an exact operation's representability goal uses (`arithmetic_interval`); a failure is `NoDerivation` |
| the same proposition as a hypothesis at that point (a caller that requires the clause it passes on) | taken from the hypothesis |
| a deeper term, operands of two carriers, or `==` or `<` over values that are not fixed | no rule: `NoRuleForGoalForm`, so the route answers `Incomplete`, never accepted and never called wrong |

`Prover::has_rules_for` draws that line from the goal's form with each value
an equation fixes to a literal read as that literal, the reading
`beyond_rules` already uses for the operations behind a goal's values.

## Erased formals at a call

A callee clause may name an `[erased]` scalar formal
([proof contracts](../../../../../../wiki/spec/proofs/contracts.md#explicit-erased-bindings):
the binding "remains in checking and semantic identity"). At a call, each
such formal is replaced by the term the call supplies for it, in the
callee's `requires` (the `CallRequirement` goal) and in its `ensures` (the
`CalleeGuarantee` knowledge), by the same `substitute` that replaces a
runtime formal by its actual. The term is read from the call in the lowered
form (`erased_arguments`), paired by position with the callee's
`MachineContract::erased_scalar_formals`: an invocation row's actual is a
value or a place, and an erased actual is neither. A term is a value of the
caller or a literal; any other form, and a callee with an erased proof
formal, is refused as `Unsupported`. Inside the callee the erased formal is
an ordinary value identity, so its entry `requires` is knowledge there with
no special case. The prover is unchanged.

An erased formal has no runtime lane, so the lowered form does not say where
among the formals it was written; `values_are_declared` takes each erased
formal's position from its row, requires those positions to increase in
contract order, and gives the runtime formals the remaining positions in
their own order. A row that breaks either is `ForeignIdentity`.

## Result summaries

A caller in the same package may rely on what a private helper's body
returns, not only on the `ensures` its author wrote. A helper that widens its
input under `requires narrow >= 300 && narrow <= 500` and writes no guarantee
still returns a value in that range, and its caller may pass the result to a
machine that requires it.
[Local call summaries](../../../../../../wiki/spec/language/state_contracts.md#local-call-summaries):
"An exact checked call inside the same fingerprinted verification unit may
use the callee's written contract plus checked body-derived result and state
facts", and "Derive and check a reusable summary against the body; each call
substitutes its evaluated actual arguments".

[result_summaries.rs](src/calls/result_summaries.rs) runs before anything is
decided. For each machine elaboration lists in `Elaboration::local_results`
(a private machine of the subject package or of a dependency whose body the
compilation reads: only its own package can call it, so no other verification
unit can), callees first, it describes every normal return in the machine's
formals and `result`: each comparison known on that return that names
`result` (what the returned value equals when that is a formal or a literal,
what a callee's guarantee or a guard says of it), and the literal bounds the
prover finds for the returned value. The summary is what every return
establishes: the comparisons all the descriptions hold and the widest of the
returns' bounds. It becomes one more `ensures` clause of the machine only
when the prover and the kernel decide it on every return, and then owes a
`NormalGuarantee` per return and a `PublishedGuarantee` like an authored
clause. Callers read it as `CalleeGuarantee` knowledge with no rule of their
own, and a private helper's inferred input condition (`entry_inputs.rs`,
which runs next) is described from facts that include it.

Summaries run once more after input inference. A helper that omits
`requires` is first summarized under no input, which is already what its
caller's later arrivals need when the body returns it whatever it is given;
what it returns only because of the input its callers establish is a further
clause, added when it is not already in the contract.

Input inference states, beside the facts a caller holds about an actual, the
interval the caller derives for it (`entry_inputs.rs`, `derived`): `step(total)`
after `total = v1 + v2` carries the sum's bounds, which no fact names. A
bound that holds only by cases is found too, for an actual and for a returned
value: under a guard `20 <= v && v <= 60` the limits lie inside the join's
disjunctive fact, so `Prover::literal_bounds` tries each literal such a fact
compares the value with as a goal, on a side that has no bound of its own.

A summary that some return does not establish is not added, and the machine
keeps its written contract; so does a public machine, a dependency, a
requirement, a selected implementation, a Build selection and a machine stage
04 generated. Nothing is assumed around a call cycle, because a summary is in
no contract before it is proved.

Not served: a result that is not a scalar; what a helper stores through a
borrowed formal; a fact that names `result` inside a term
(`result + 1 <= n`), which a return's guarantee does not substitute; and one
alternative per return. A helper that returns its input inside `[20, 60]` and
40 otherwise is known to return a value in `[20, 60]`, not `value` or 40: a
caller's proof splits at most once over a disjunction, and with alternatives
kept, the deleted corpus program
`pass/calls/private_arrival_guarded_helper_range_exit` was rejected where its
caller passes the result to a private helper. Only the module's fingerprint
records which body a caller's proof depended on: the per-fact dependency the
specification asks for ("Using a body-derived fact records a dependency on
that exact checked body") waits for dependency slices.

## The rows are checked against the lowered form

A certificate for every row proves nothing about a row that is missing, that
names the wrong subject or that sits at the wrong point. So before anything
is decided, `follow_the_lowered_form` ([minted.rs](src/obligations/minted.rs)) recomputes
what each machine owes from the lowered form alone and refuses tables that
differ. Nothing is taken from elaboration's word, because the lowered form
already carries every identity:

| Row | What the lowered form says | Must be created at |
| --- | --- | --- |
| `OperationSafety` | each operation Terminal semantics gives a safety goal names its obligation | the point before the operation |
| `CallRequirement` | each call lists one obligation per `requires` clause of its callee, in clause order; the callee's contract gives the count | the point before the call |
| `PublishedGuarantee` | each `ensures` clause of the contract names its published identity | the entry block |
| `NormalGuarantee` | every return terminator owes one per `ensures` clause, naming its own edge and the clause's published identity | that return edge |
| `DataDomain` | counted by `Storage::owed_are_minted` from the writes the place walk finds | the point before the operation |
| `CrashCoverage` | a `Crash` terminator owes one for its cause; an `InvokeNoNormal` terminator, or a `Call` or `CallUnit` with crash continuations, one per cause among them | the point before the terminator or the call |
| `ArrivalInvariant` | a `Jump`, or a `Conditional` successor, that passes scalar or structural arguments owes one for its edge | that edge |
| `WellFoundedness` | each cycle of the machine's `ranked_scc` owes one, named by the identity of its edges | the entry of the cycle's first block |
| `RankComparison` | each edge row of a ranked cycle owes one | that edge |

A row of the first four kinds, or of the last four, that nothing owes is
refused, and so is a row owed twice. The point matters as much as the kind,
because it selects the knowledge a proof may use: a row moved past a guard,
or past its own call where the callee's `ensures` is known, would be proved
from facts that do not hold where the obligation sits.

Two more tables feed the proofs and are checked the same way. An invocation
row must restate one call of the machine: its callee, its scalar actuals in
order and its record actuals as the roots the call passes. Call requirements
and callee guarantees are stated over it, so a row for a call the machine
does not make would be a premise nothing established. An edge row must have
the source and destination of its terminator's edge, because dominance is
computed from the edge table.

The data `requires` clauses are the goal of every `DataDomain` row and the
premise a read of an arrived value learns from its type. The checker reads
them from the module's own type declarations
(`StructuralTypeDeclaration::requires`, part of the encoded module and so of
its identity), type by type and in authored order. No table beside the
module restates them, so there is no second copy that could weaken,
strengthen, drop or move a clause.

A disagreement is a defect of the compiler, not of the program: the refusal
has reason `ForeignIdentity`, which `judge` returns as `Err`, so nothing is
accepted on it.

### The tables that say what storage holds

A goal over a field is stated over the value its leaf holds, so a table that
names another value, another place or another kind of value changes what is
proved without touching an obligation row. None of what storage holds is
taken on elaboration's word.

**The write log is derived.** The walk reads only the log
[places/log.rs](src/storage/places/log.rs) derives from the operation that makes
each store (`Storage::stores`), and, for cleanup actions, the writes
`owned_cleanup::writes` derives from each action's effect:

| Operation | What it stores |
| --- | --- |
| `EstablishRecord` | each operand into its field of the fresh root: a scalar operand as that value, a record operand as what its source held |
| `EstablishScalarCase`, `EstablishStructuralCase` | the fresh root established as `result_case`, then each payload operand as above |
| `EstablishZeroedStructural` | one zero write to the fresh root |
| `StructuralLeafCopy` | what its source held, into the fresh root |
| `StructuralScalarFieldStore`, `WriteOnlyPrimitiveStore` | its value into exactly the leaf it names |
| `EstablishPrimitiveLocal` | its operand, the zero a scalar local is constructed with, into the fresh home |
| `StoreStructuralField` | what its source held, into the field it names |
| a call with a record result | what the callee returned, into the fresh root |
| a read, a case test, an operation over scalar values alone | nothing |

Any other operation is refused as `Unsupported`: it may store somewhere, and
an operation left out of the derivation would be a store nothing reaches.
Elaboration's own write log (`MachineTables::writes`) is its lowering
scratch; the checker never reads it.

Reads of a zeroed aggregate derive the selected leaf’s typed zero from the
root write. Subsequent writes take precedence at the affected places. Each
claimed store must correspond to an operation in the lowered program.

**The place table must be one place per path** (`places_are_interned`). A
table names a place by its identity: an invocation's record actual, the place
of an entry content value, a write row. The walk reads that identity as a
path, so each root's row must be its own, each projection must be listed by
its parent under its own segment, a parent must come before its projections,
and a parent's projections must be sorted with one per segment. Two paths
that share a row would let one field's contents answer for another's.

**The value table must be the lowered form's declarations**
(`values_are_declared` in [minted.rs](src/obligations/minted.rs)). Each formal, the scalar
result, each block parameter and each scalar operation result must have its
declared type and the origin of what declares it. The type is what Terminal
semantics states a goal over and what the kernel bounds a value by; the
origin is what the payload judgment reads to learn which test a condition is
the result of. The one row the lowered form does not declare is the content a
formal's leaf held at entry. Such a value must be of a scalar leaf of a
formal, with that leaf's declared type
(`Storage::entry_contents_are_of_formals`). Since the lowered form declares
none of them, nothing in it says which identity belongs to which leaf, so
they must be in the order of their leaves (`entry_contents` in
[places.rs](src/storage/places.rs)): two rows of one type that exchange their
leaves are refused, and so is a second value for one leaf.

## What a place holds

A read, a whole copy, a dispatch's payload parameters and the two structural
judgments all ask what a place holds on every path to a point: which value a
scalar leaf holds, or which case a sum is established as.
[places.rs](src/storage/places.rs) answers both with one walk (`Storage::reaching`)
over the tables and the lowered form. The two questions differ only in which
rows end a path and in what an edge can establish:

- **The place.** A place is its root and the segments from the root down to
  it: fields, and for a payload a `Case` segment followed by a payload
  field. A read names its fields by `StructuralFieldId` and its case by
  `StructuralCaseId`; a Terminal Psi storage path is resolved by the lowered
  form itself, one field against the prefix before it
  (`runtime_structural_field`). Every `Case` segment is checked against the
  declared cases of the exact sum it is applied to.
- **Overlap.** One function answers whether two places may share storage: a
  place overlaps itself, its ancestors and its descendants; two places that
  part by two different record fields are disjoint; any other pair of sibling
  segments overlaps, and places under different roots do not. Indices and
  referents will extend this function, not add a second one.
- **Reaching.** Walking the graph back from the use, the last store into an
  overlapping place on each path is the one that reaches along it.
  - A store of the value `v` into exactly the leaf supplies `v`.
  - A store into the leaf or above it that moves storage whole (a whole
    copy, a record operand of a literal, a record stored into a field)
    supplies what the matching leaf of its source held just before it, and
    the walk continues there.
  - A call's record result supplies what the callee returned, which has no
    value identity here; any other overlapping store supplies contents with
    no value identity.
  - A path that reaches the machine's entry with no write is supplied by the
    place's entry content when its root is a formal, and by nothing
    otherwise.

- **Leaves.** The leaves beneath a place are its scalars and its sums. A
  sum is a leaf itself, standing for its discriminator, beside the payload
  leaves of every one of its cases: it is established by a whole write,
  whatever case that write selected. A shared byte view is one leaf too,
  whether it is held whole (a formal, a literal, a copy) or stored in a
  record, where it is a byte field of that record
  ([structural access](../../../../../../wiki/spec/terminal-psi/structural_access.md#store-vocabulary):
  "`StructuralByteViewFieldStore` replaces the descriptor of one shared
  borrowed-view field"). The leaf is the descriptor: a view store supplies
  what its source held, and the zero of a record holds the empty shared view
  there. It has no value identity, so nothing is yet known about a stored
  view's extent; a fact about one will have to be ended by a write to the
  field as a scalar leaf's is, because a field, unlike a view formal, can be
  overwritten in place. Exclusive and predicate-bearing view fields are
  refused.

A store inside a block reaches only what that block leads to: a store in a
guarded arm that completes the machine reaches neither a read nor a copy in
the continuation, and a store before the branch reaches both sides.

### The read equation

A read produces a fresh value. It gets the row `read == v` when `v` is the
one value its place holds on every path to it
([fact validity](../../../../../../wiki/spec/terminal-psi/fact_validity.md):
"A primitive read may introduce an equality between its copied SSA result
and an exact reaching stored SSA value. Every actual predecessor must supply
that same value"). Different values on different arrivals, or contents with
no value identity, give none: the read is still well formed and nothing is
known about it. A store never removes a row, so what was known about the
value a leaf held before the store stays known.

### Initialization

Every operation and terminator that reads storage is one use: a field read,
a read of a scalar home, a whole copy, a record operand, a record stored
into a field, a call's record arguments, a record return. Each scalar leaf
at or beneath the place it reads must be supplied with an established value
on every path: by a store into the leaf, by a whole write to a place above
it, by a formal's entry content, or by the zero its declaration established.
If some path supplies nothing, the use's verdict is a refusal with reason
`Unestablished`, concerning that operation or edge.

A local declared without an initializer is zero-constructed
(`local-without-initializer`), so in an elaborated program every path
supplies at least the declaration's zeros, which are a value unless the type
is gated (described next). The sentence "An unestablished local cannot be read"
([structural access](../../../../../../wiki/spec/terminal-psi/structural_access.md#store-vocabulary))
governs declared shared view locals, which belong to the references work and
which this checker refuses; it does not govern a plain scalar or data local.

<a id="zero-establishment"></a>
A data local declared without an initializer is zero-established: its
declaration is a whole write that supplies every leaf, each scalar leaf of
the zero value with a zero constant the read equation can name, and a sum
with its first declared case. The declaration owes the `requires` clauses of
the zero value as `DataDomain` obligations. When one is not proved, the type
is gated
([dependent values](../../../../../../wiki/spec/language/dependent_values.md#default-domains-and-zero-initialization):
"If zero satisfies the default domain, the value is zero-constructible.
Otherwise the type is gated: construction or checked qualification must
establish the domain before observation."). The refusal then sits at the
observation, not at the declaration, because "Zeroing storage is permitted
independently of establishing a value.": the clause's verdict is
`Discharged::Window`, which rejects nothing by itself, and the declaration
opens an initial-establishment window for that clause on the place
([Invariant windows](#invariant-windows)). A store that proves the clause
closes it, so `let mutable g: Both; g.a = 3; g.b = 2; take(g)` is accepted when
each store proves its field's clause. A use that observes a leaf the
declaration zeroed while some clause's window is still open is
`Unestablished`, with the declaration named beside it
(`UnestablishedUse::declared`): `let mutable g: Both; g.a = 3; take(g)` is
refused at `take(g)`, because `g.b` is still the zero `1 <= b` fails on. A
read of a leaf a store wrote observes that store, not zeroed storage, and is
established as before.

A scalar local declared without an initializer is zero-constructed by the
same rule: elaboration logs the zero constant on its home at
`EstablishPrimitiveLocal`, so `let mutable x: u32; f(x)` reads a value that is
provably zero. Every scalar type this route
serves (`i8`..`u64`, `bool`) admits zero, and a qualified or ranged scalar
type is refused by the reader as `type constrained`, so a scalar home owes
no domain clause and no gated scalar occurs in this subset.

Initialization is a structural judgment over the stores and the graph, not
a proposition: it has no obligation row, no certificate and no part in the
prover.

### Data `requires`

<a id="data-requires"></a>
[Data and literals](../../../../../../wiki/spec/language/data_and_literals.md):
"Construction and every restoring write prove the owning default-domain
clauses." Elaboration mints one `DataDomain` obligation per clause an
operation owes; `Storage::owed` recomputes which data-typed places those
are, in the same order, and `discharge` refuses a module in which an owed
clause has no row or two.

The goal is the clause under the one substitution (`substitute`), with the
names of the clause standing for what the place holds at the operation's
normal continuation:

- **A field** becomes the one value the stores say that leaf holds
  there, found by the reaching walk: the operand of a construction, the
  zero constant of a zero establishment, the stored value for the written
  field, and for every other field the value that reached the store. For a
  nested store the same walk serves each enclosing type. A record operand or
  a record stored into a field is followed to its source, as for a read.
- **A case test** `self in S::C` becomes `Truth` or `Falsehood`: whether the
  walk finds the place established as that case there, which a construction
  or a zero establishment decides by structure. A machine clause's test of a
  formal or of its result is decided the same way: a required case is owed
  at each call from what was stored in the argument, a guaranteed case at
  each return from what was stored in the result, and both are then known
  to the callee at entry and to the caller after the call
  ([payload access](#payload-access)). The connective around it is
  simplified: a conjunction with a false conjunct is false, a disjunction
  with a true disjunct is true, and then their other parts are not looked
  at, which is what lets a clause project a payload under the test of its
  case.

So the goal names values and literals only, and it is proved from the
knowledge available before the operation
([structural access](../../../../../../wiki/spec/terminal-psi/structural_access.md#complete-record-construction):
"The obligation binds the initializer or replacement SSA value, never the
old field contents."). The stored value enters the goal from the store the
operation makes, not from a knowledge row, so the completed write contributes no fact to
its own proof. A field with no value identity at that point (what a call
returned, a join of two different stores) or a case no structure decides
leaves the clause unstated: the obligation is `Unproved`, with a detail that
says which, never dropped and never assumed.

A store whose written value is shown to keep its clauses discharges them
there. One that is not shown to keep a clause opens an invariant window,
described next; it is never refused at the store.

### Invariant windows

[Dependent values](../../../../../../wiki/spec/language/dependent_values.md#invariant-windows):
"A write known to preserve the default domain keeps it established.
Otherwise the write opens a window on the place, whose actual new contents
remain available to ordinary flow reasoning. The domain must be proved again
at the next consumption point". So `let mutable s = Span { low: 1, high: 9 };
s.low = 12; s.high = 20; width(s)` is valid: the first store breaks
`low <= high`, the second restores it, and only then is `s` consumed.

- **Opening.** A store's `DataDomain` clause that is not proved over the
  values the place holds after it, or a zero establishment's clause that
  zero does not satisfy, opens one window: on the exact place of the type
  that owns the clause (`s` for `Span`, `w.span` for a `Span` inside a
  `Wrap`, each enclosing type separately), for that one clause. The
  obligation is the window's identity; its verdict is `Discharged::Window`,
  which keeps why the clause was not proved there. A construction makes a
  value and opens nothing: its unproved clause is still `Unproved`.
  A call that lends a place exclusively (`@mutable` or `@write`) is a store
  into every value above that place: the callee may write anything beneath
  it, so `bump(@mutable s.low)` owes `Span`'s clauses for `s` and opens a window
  when they are not proved over what the call left there. The lent place's
  own type owes nothing at the call: it is the callee's formal, which the
  callee proves at handback, as a replaced field arrives formed.
- **Preserving.** A store that writes no storage a clause names (the
  places the one substitution meets in the clause, `Storage::untouched`;
  for a call, every region the write log says it may write) keeps that clause as it was: `Discharged::Preserved`, no window and no
  closing. A store that proves a clause over the place's values is a
  restoring write ("Construction and every restoring write prove the owning
  default-domain clauses"), and closes that clause's window on that place.
  Any other store beneath an open place changes its contents and nothing
  else.
- **Open or closed.** The one reaching walk answers it: going back from a
  point, the last row that is the opening write, a restoring store of the
  same place and clause, or a write that replaces the place whole (to it or
  above it) ends a path. The opening write supplies `Open`, the others
  `Closed`, and a path that reaches the entry `Closed`. A window open on one
  arrival is open at the join.
- **Consumption points.** The list in the specification, mapped to what
  this subset lowers:

  | Specification | Lowered construct |
  | --- | --- |
  | "A read relying on the domain or a dependent coupling." | a whole read of the place or of a place containing it: `StructuralLeafCopy`, a record operand of a literal, a record stored into a field; and a field read or a dispatch's bound payload beneath the place that takes a clause of its storage's type as a premise (`read_premises` or `payload_premises` not empty). A field read or dispatch with no premise, and a case test, rely on nothing and consume nothing. |
  | "Borrow creation or a call consuming the affected domain." | a call's record argument; this route passes records by value, copied first, so it is that copy, and the call itself (next row). |
  | "Transfer, return, scope expiration or loan handback ..." | a record passed on a `Jump` edge; and every `Return`, `ReturnUnit`, `ReturnStructural` and `InvokeNoNormal`, on its edge, which ends the scope of every place of the machine. A `Crash` consumes nothing: "A crash may abandon an open window". An `ExternalCompletion` consumes nothing either: process exit "may abandon outstanding in-domain obligations, including linear obligations, without cleanup or discharge claims" ([process exit](../../../../../../wiki/spec/language/process_exit.md#abandonment-and-survivors)); a record it took as an argument would be read like a `Jump`'s. |
  | "Boundary/capability-carrying calls that may observe or expose the affected storage" | every call operation, for every window open at it. |

  "A call need not close an unrelated domain window when its complete
  access, effect and outcome contracts prove that it cannot observe or
  expose the broken coupling ... Unknown access/effects are treated
  conservatively." The tables hold no callee summary to prove that from, so
  no call is unrelated: `s.low = 12; same(7); s.high = 20; width(s)` is
  rejected at `same(7)`. The exception needs a summary term for a callee's
  access and effects.

  A window a zero establishment opened is consumed by every read under its
  root that still observes a leaf of the window's place the declaration
  zeroed, a field read included, and by no call or exit that does not read
  it (zeroed storage holds no value a callee could observe, and "Failed
  initial establishment instead creates no value"): "A zeroed storage representation can be
  accessed as an established value only after its default-domain
  obligations hold."
- **Proved again.** At each consumption point the window reaches open, the
  clause is stated by the same substitution over what the place holds there
  (`Storage::consumptions`) and decided by the same `decide`, from the
  knowledge dominating the point, under the window's obligation identity.
  One `WindowUse` per (window, consumption point) holds the kernel's
  evidence or the refusal. A consumption on an edge is proved at that edge;
  any other before its operation.
- **Rejected.** An unproved clause at a consumption point rejects the
  program with both sites
  ("An unclosed window rejects with both the opening write or storage cast
  and the failing consumption point."): the `UnprovedObligation` carries the
  opening write's site in `site` and the consumption point in `consumed`,
  and its diagnostic is placed at the consumption. For a window a zero
  establishment opened, the use is `Unestablished` instead and names the
  declaration. Each open window is reported once, at the first consumption
  point in program order that fails; the later ones keep their own
  verdicts.
- **No second window.** A store that does not prove a clause while that
  clause's window on the same place is already open opens no window of its
  own: the window stays open and names the store that opened it. Into zeroed
  storage, a store that does not prove its clause keeps its own window,
  because the leaf it wrote no longer holds the zero.

A window lends nothing: no knowledge row is added for it, a read of a leaf
the machine stored gets no type premise (as before), and a read of an
arrived leaf that does take a premise is itself a consumption point, so it
is accepted only where the open clause is proved again. That premise is of
a clause naming only the one leaf it reads, which no store elsewhere can
break.

A window is a structural judgment plus a proof question, not a conserved
obligation row. `ObligationKind::DomainDebt` names its domain by a
`DomainSemanticId`, and a data `requires` clause has none: it lives in its
type's declaration, by position. Minting `DomainDebt` would also
need a disposition on every edge, which elaboration does not write yet (it
refuses any conserved obligation at a no-successor edge) and this crate does
not check. The opening write's `DataDomain` row already names the type, the
place (through its operation) and the clause, so the window reuses its
identity.

### What a read learns from its type

[Structural access](../../../../../../wiki/spec/terminal-psi/structural_access.md#complete-record-construction):
"A validated field read or selected-case payload copy instantiates that
invariant on its fresh scalar snapshot; other fields and unselected cases
provide no premise. The snapshot survives later writes".

A field read gets one row per `requires` clause of each data type its leaf
lies in, stated on the read's fresh value and justified by the read
(`OperationSemantics`), when two things hold.

- What the leaf holds arrived, on every path, inside a value another machine
  established: a formal's entry content, however it was copied since, or a
  leaf of the root a call returned that this machine never stored over. The
  clause must belong to a type inside that value, on the path from its root
  to the leaf; a type this machine built around it lends nothing
  ([dependent values](../../../../../../wiki/spec/language/dependent_values.md#default-domains-and-zero-initialization):
  "no consumption sees the value outside its required domain", and a call
  and a return each consume the value they pass). A leaf this machine wrote
  gets no such row: its `DataDomain` obligation proved the clause of the
  written value where it was written, and the read equation names that
  value. A store that broke the clause therefore lends it to no read.
- The clause names that one leaf and no other, and any case it tests is
  established at the read by structure: by a construction, by a dominating
  true edge of a membership test, or by the dispatch edge that entered the
  block (the one walk of [Payload access](#payload-access)). The case is
  asked of the place the read observes the value through, the copy when
  the value was copied: a test or dispatch on the copy establishes it
  there, and the place the value arrived in may have been written since.
  A clause testing a case of a type above the observed storage's root is
  not stated. A clause that relates two fields is no premise on one read.

A clause that cannot be stated adds no row; nothing is assumed in its place.
A payload parameter of a dispatch edge gets the same rows, stated on the
parameter ([What a dispatch binds](#what-a-dispatch-binds)).

### Payload access

A case's payload overlays its sum's storage, so naming one is sound only
where the sum holds that case.
[Structural access](../../../../../../wiki/spec/terminal-psi/structural_access.md#store-vocabulary):
"Each `Case` step requires source admission to establish the selected case
for the same root and prefix at the read's observation epoch. A dominating
membership test or dispatch supplies that fact." and "A write to the sum or
any prefix retires these facts".

Every operation and terminator is judged for each `Case` step on the path of
each place it names: a payload read, a copy out of a payload, a store into
one, a case test or dispatch on a sum inside one. The walk asks which case
the sum before that step holds on every path to the use, and the use is
accepted only when the answer is that one case. Going back from the use:

- A write to the sum or to a place above it ends the path. A construction
  (`EstablishScalarCase`, `EstablishStructuralCase`) on exactly the sum
  supplies its `result_case`; a whole copy, a data operand or a data value
  stored into a field continues at the same sum inside its source, before
  the operation; anything else supplies no case. A store beneath the sum
  changes a payload, not the case, and does not end the path.
- An edge of a `StructuralCase` on exactly this sum supplies that edge's
  case. A block several of its edges enter, the wildcard arm, gets none.
- The true edge of a `Conditional` whose condition is the result of a
  `StructuralCaseMembership` on exactly this sum supplies the tested case,
  when the test stands in the branching block and no write to the sum or a
  place above it follows the test there.
- The machine's entry supplies the case the machine's own `requires` states
  outright of that formal (`requires outcome in Outcome::Good`), which every
  caller proved at its call.
- A call's result holds the case the callee's `ensures` states outright of
  it (`ensures result in Result::Ok`), which the callee proved where it
  returns.

"Outright" means the test is a conjunct of the clause. A test beside an
alternative (`(x in S::C) || flag`) may be false while the clause holds, so
it supplies no case (`stated_case_tests` in `places.rs`).

So the accepted shapes are a dominating dispatch edge, a dominating true edge
of a membership test, a construction that is every reaching write, a case the
machine requires of a formal, and a case a callee guarantees of its result. The false edge of a test supplies nothing: "Otherwise the case holds
when every other declared case of that sum is refuted" needs proof about the
other cases and is refused here, not guessed. A refusal has reason
`Unestablished` and concerns the access.

A contract clause can still test a case that none of these decides. Stage
05 names the case of each sum an input formal or a call result holds, one
Boolean per declared case, at the boundaries where it names a scalar leaf's
contents: at entry and at the normal return of the call that wrote it
(`ValueOrigin::StorageCase`). When the walk reaches such a boundary with no
structural answer, it answers with that boundary (`Reached::NamedCase`), and
the clause's test is the proposition `name == true`
(`CaseAnswer::Named`). So `requires (outcome in Outcome::Good) || flag` is
assumed at entry as a disjunction over the entry name, a callee's `ensures
(result in Outcome::Good) || flag` is assumed after the call over the
call's name, and a requirement is owed as a goal over the name where the
argument still holds what it held there. Three kinds of knowledge relate
the names to the program: at each boundary exactly one of a sum's names is
true (`case_name_premises`), and a `StructuralCaseMembership` of a sum that
still holds its boundary case computes that name (`membership_equation`),
so a branch on the test decides the name on both edges. A payload read
still needs one of the structural shapes above; a name proved true does not
open a payload. A block formal's case has no name yet, so a clause over it
stays refused.

A `StructuralCase` terminator is one more use, named by its block. Its
subject must be established, and its roster must list every declared case
of the sum it observes exactly once; a declared case with no edge is
`Unestablished` ([patterns](../../../../../../wiki/spec/language/patterns.md):
"Every dispatch must cover the admitted subject. Missing runtime coverage is
a compile error, not an implicit trap or fallthrough."). Any other fault in
a roster (a case of another sum, a case twice, a payload field that is no
scalar field of its case) refuses the module.

### What a dispatch binds

A dispatch delivers the scalar payload fields of the selected case as the
parameters of its arm's block
([control flow](../../../../../../wiki/spec/terminal-psi/control_flow.md):
"Selected payload fields become scalar parameters on that case's edge before
cleanup."). Each parameter gets the row a payload read would get: it equals
the one value its payload leaf holds on every path to the dispatch, when
there is one. The row is available on that edge and justified as its
`DispatchBinding`. A payload of a case other than the one the sum was
constructed as resolves to no value, because the last write to storage it
overlays was another case's.

Each parameter also gets the rows a payload read at the head of its arm's
block would take from its type
([What a read learns from its type](#what-a-read-learns-from-its-type)):
the same `Storage::premises`, under the same two conditions, stated on the
parameter, available on the edge and justified as its `DispatchBinding`.
Asked at the head of the arm, a case test of the clause is decided by the
edge that entered it: `self in C` is true on the `C` edge and false on any
other, so `(self in Empty) || ((self in Ready) && (self.value <= 50))`
leaves `value <= 50` on the `Ready` edge. An arm several edges enter, the
wildcard, establishes no case, and a clause that tests one is not stated.
A dispatch whose payloads take a premise relies on the domain of every
place those leaves lie in, so it is a consumption point of a window open
there, as such a read is.

The case itself is not a knowledge row: a dispatch has no Boolean value to
state it over, and knowledge names values only. The case is the structural
judgment above and nothing else. No row states a proposition about a place
and no structural `Proposition` variant is used, so the prover has no
field-aware path: it sees one more equation between two values.

A call's actuals replace the callee's formals in its clauses by position
among the scalars: the callee's `k`-th scalar formal becomes the caller's
`k`-th scalar actual. A record actual is a place, and no clause term can name
it, because elaboration refuses a clause that names or projects a record
formal.

## Service reach

A machine that writes `reaches A + B` promises that its body reaches no
other service, and a call that breaks the promise is a compile error at that
call ([effects.md](../../../../wiki/spec/language/effects.md#declarations):
"A derived reach that exceeds its ceiling rejects at the call that
contributes the excess"). `service_reach.rs` judges this on the lowered
module after the proof obligations: every machine publishes a row of
services, a call contributes the row its callee publishes, and

- in a machine whose header wrote a ceiling, each call's contribution must
  lie inside the ceiling, or the call is listed in
  `RouteVerdict::Rejected::exceeded` with the ceiling, the services left out
  and what the call contributes;
- in every machine, each call's contribution must lie inside the row the
  caller itself publishes. A module that breaks this where no ceiling was
  written, or whose rows are unordered, name an undeclared service, lost a
  boundary parent or disagree with the written ceiling or the root reach, is
  refused as a fault of the tables (`ForeignIdentity`), never accepted.

Only the written ceiling comes from elaboration; the rows and the calls are
read from the module. It is a judgment over finite sets and has no
certificate, like a loan conflict. Tests: `tests/service_reach.rs`, each of
which changes rows only and leaves the lowered calls alone.

Limits. The only contributions are written ceilings,
because elaboration refuses boundary operations, requirements that declare a
reach and installation-bound rows; reach through a callable value is handed
back as `Unsupported`. The diagnostic names the call and the callee's row,
not "the identities its requirement declares", since no requirement is
called yet.

## Loans
A call that passes a place to a borrowed formal lends it for the call
([prepared-typed-trees-to-lowered-program](../05_prepared-typed-trees-to-lowered-program/README.md#references)):
the structural argument names the place with the loan's access, and one
`Loan` row stands for it. [places/loans.rs](src/storage/places/loans.rs) recomputes
the rows from the lowered form, as for every other row
([The rows are checked against the lowered form](#the-rows-are-checked-against-the-lowered-form)).
There is one per borrowed structural argument of each call, created at the
point before the call. Its captured place is the invocation row's actual at
that position and the place the argument's storage path names. Its lender is
the incoming loan when the root is a borrowed formal (`Lender::Incoming`), an
earlier loan for a reborrow, the admitted provider permission for a
provider-established view, or else the storage owner. Its access must be the
callee formal's declared access. A row the lowered form does not owe, or owes
differently, is `ForeignIdentity`.

A loan used only during a call ends when it returns, so it is live on no
edge and no edge disposes of it. A returned view can instead keep the input
loans alive until that view's last use. Those dependencies restrict access to
the inputs; they do not make the result's pointer an alias of an input.

A boundary may return a shared or mutable reference to provider storage.
Each scalar field read observes the provider's current value. A mutable
reference can write an ordinary scalar field when every possible return
grants that permission and the result has only shared input dependencies.
Those dependencies keep their original loans alive. The checker requires a
captured address for a write that may reach an exclusive input. Two provider
borrows with overlapping lifetimes still need evidence of disjointness when
either borrows exclusively.

A loan formed for a call with no normal return (an `InvokeNoNormal` or
`ExternalCompletion` terminator) is carried into that transfer. Its edge must
consume exactly those loans, each once (`Fate::Consumed`), and dispose of
nothing else; any other roster is `ForeignIdentity`. Nothing of the machine
runs after such a call, so no later use can meet the loan. An exclusive
loan's call counts as a store: from the call's normal continuation on, the
lent place holds contents its callee established, with no value identity
here (`Stored::Returned`), and nothing outside it changes. A sibling field
keeps what it held, so a read of it still has its read equation; a read of
the lent field has none. A shared loan stores nothing.

Whether a loan may be formed is a resource judgment, not a proposition
([loans](../../../../../../wiki/spec/terminal-psi/loans.md): "Loan descent,
access attenuation, temporal containment and restoration are resource
judgments. Logical facts cannot create, amplify, transfer, extend, return,
consume or duplicate authority."), so it has no certificate. A loan that may
be formed is `Discharged::Lent`; one that may not is refused with
`RefusalReason::Conflict`, a compile error of the program:

- **Two loans of one call that may share storage**, when either is
  exclusive: the later one conflicts. The places' overlap is the one
  relation the place walk uses (`overlap`): two different fields of one
  record are disjoint, anything else beneath one root may share storage.
  ownership.md, "Borrows and aliases": "exclusive loans must not overlap
  incompatibly"; loans.md: "A live exclusive loan does not prove projections
  disjoint."
- **An exclusive reborrow of storage held shared**: a loan beneath a
  borrowed formal declared `@T` with mutable access (loans.md, "Reborrow
  lineage and access": a `Read` parent rejects a mutable child).

`judge` reports each as a `ConflictingUse` in `RouteVerdict::Rejected`
beside the unproved obligations and unestablished uses, naming the callee
and conflicting binding: `state `two` receives `y` as mutable more than once
(conflicting loans)`, `... as both mutable and read-only`, placed at the
call.

## Loops

Elaboration lowers a call cycle as a loop: one head block per machine of the
cycle, the machine's formals as the head's parameters, and a jump to the
head for each call
([structure elaboration](../05_prepared-typed-trees-to-lowered-program/README.md#call-cycles-as-loops)).
A machine's `requires` is its head's invariant, a row of the module's
`scalar_block_invariants`. [arrivals.rs](src/control_flow/arrivals.rs) does three things
with a row, and the prover does the rest unchanged.

- **Admits it.** The predicate may name only machine formals and the head's
  own parameters, the head is not the machine's entry, and every edge into
  the head is one of the row's arrivals exactly once with an
  `ArrivalInvariant` obligation of its own
  ([control flow](../../../../../../wiki/spec/terminal-psi/control_flow.md#safety-and-progress):
  "Missing or duplicate arrivals, wrong scope or carriers, and stale edge
  coordinates reject."). A row that fails refuses the machine: its predicate
  would otherwise be a hypothesis nobody proved.
- **States each arrival's goal:** the predicate with each head parameter
  replaced by the argument that edge carries, all at once (control flow:
  "Every arrival proves the predicate under simultaneous
  destination-parameter to actual-argument substitution, before destination
  hypotheses become available."). It is decided at the edge, so a jump back
  out of the loop has the invariant of the iteration it leaves, and the
  entry's jump has the machine's own `requires`.
- **Makes it known:** one knowledge row, the predicate over the head's
  parameters, available from the head's entry and justified as
  `BlockInvariant`.

That row is all that is known about a head's parameters. A parameter is a
value of its own on every arrival, so what a caller knew about an argument
says nothing about it. This is the specification's "At a cyclic scheduling
cut, only checked invariant predicates enter as induction hypotheses; prefix
facts are discarded" when knowledge names values. A row about a value
evaluated before the loop stays available inside it by dominance, because
that value does not change.

The invariant is used before it is proved, as an induction hypothesis must
be. That is sound only because a closure is accepted whole: `judge` accepts
when every obligation is discharged, every arrival among them
([fact validity](../../../../../../wiki/spec/terminal-psi/fact_validity.md#premise-availability):
"Every arrival must check before the destination predicate can justify
downstream obligations."). A single discharged obligation inside a loop is
not evidence on its own while an arrival of its head is unproved.

An unranked loop proves nothing about ending, and nothing here says it ends.

## Rankings

Elaboration records each loop's private measure in `ranked_scc`, including
measures computed from borrowed slice lengths and transfers between member
heads. [rankings.rs](src/control_flow/rankings.rs) admits the complete claim before any
proof runs (`prepare` calls `admit`). Following
[control flow](../../../../../../wiki/spec/terminal-psi/control_flow.md#safety-and-progress),
it reconstructs the actual cyclic components and checks:

- Exact ordered membership, every internal edge, and exactly one shared
  well-foundedness obligation plus a comparison obligation for each edge.
- Nonempty integer tuples with unchanged arity and carriers. Every value has
  its declared type and is available where compared. Addresses and unresolved
  integer widths cannot supply a finite carrier.
- Each successor tuple equals the target's observations under that edge's
  exact scalar and structural arguments. Computed observations retain their
  constants, exact subtraction and descriptor bindings.
- Removing strict edges leaves no cycle, so every complete cycle must descend.

What `admit` accepted is kept by obligation identity for discharge and
proof reuse. `WellFoundedness` produces `FiniteIntegerRanking`, a structural
judgment containing the checked bindings and finite carriers. It does not ask
the prover for a carrier-floor proposition. Each edge comparison separately
goes through the ordinary prover and kernel: strict edges owe lexicographic
`successor < source`, and preserving edges owe `successor <= source`. A finite
carrier cannot substitute for a missing comparison proof. A transfer can change
the rank expression while preserving its value, for example from a slice length
to a bounded cursor distance. If its strict comparison fails, the checker can
prove the nonstrict comparison instead. It publishes that certificate and the
revised edge only after checking that the remaining strict edges still intersect
every cycle. Replaying the result checks the comparison on the published edge.

Storage observations relate exact captured values. Earlier reads can supply an
equality only while their backing is unchanged and the observation remains in
scope. Copies and block arrivals retain the loop entries crossed while tracing
storage; a saved descriptor cannot acquire endpoint values from a later
iteration. Partial measure operations keep their ordinary safety obligations,
including in acyclic bodies.

A machine publishing `Terminates` with an unranked cycle is refused.
Elaboration also refuses a promise that reaches another unranked call cycle;
this judgment does not establish termination of external callees.

## The prover

One goal-directed prover serves every proved kind. It carries interval bounds as
proof: each integer value gets a best upper and lower literal bound, each with
a node the kernel checks, from the carrier bound of its type, a hypothesis
bounding it or relating it to another value, or, for an exact add or subtract
result, its operands' bounds through `IntegerExactAddDefinitionBound`. An exact
multiply result has no definition rule and needs none: the prover bounds the
mathematical product with `IntegerAffineBound` (the least or greatest of the
four corner products of the operands' bounds), carries that bound through the
result equation with `ValueEqualityTransport`, and restates it in the carrier
with `IntegerOrderSubstitution`. The result of a divide, a remainder, a shift, a
bitwise and or a bitwise or gets its bound from a one-step endpoint chain
([prove/ranges.rs](src/proofs/prove/ranges.rs)): the witness roots at the operand
that is a value, cites the result equation as its only definition and lands
the other operand on the earlier equation that fixes it to a literal, and the
kernel replays the step on the root's bound. This is the range propagation
that [required automatic discharge](../../../../../../wiki/spec/proofs/contracts.md#required-automatic-discharge)
asks of every checker. A cast's result takes its operand's bounds restated
in the result's carrier (`IntegerCastBound`), and an exact cast's
representability goal is the operand's bound, restated over its mathematical
value, against the target carrier's limit. An unsigned remainder is also below a divisor known
positive, whatever the divisor is. A
comparison closes by chaining the two sides' bounds through a closed literal
relation. An arithmetic safety goal closes through `IntegerAffineBound`, whose
image of the operands' bounds the kernel itself computes; an exact left
shift's goal uses the kernel's direct shift form, from the value's bound and
the count's literal or upper bound.

The same prover carries relations between values, with no literal endpoint.
A comparison between two values searches order steps between values:
hypotheses relating two values, and `a < a + d` or `a - d < a` from an exact
result when `0 < d` (`IntegerAddOrder`, `IntegerSubtractOrder`), chained by
order transitivity. An unsigned `a - b` is safe from `b <= a` through the
kernel's correlated unsigned subtract. A goal the direct search misses is
split over a disjunctive fact (a disequality guard transports to `v < k or
k < v`): each alternative proves the goal as a local assumption or
contradicts what is known (`DisjunctionElimination`, contradiction
elimination). Only a disjunction whose atoms share a subject with the goal
is split ([case splits](../../../../../../wiki/spec/proofs/contracts.md#required-automatic-discharge)):
a value, a field read or an application such as `size_of::<u32>()`, where a
value also shares the subjects of its exact definition. `Falsehood` names no
subject, and any disjunction may close it. An arm's own search splits the
remaining candidates, so the first candidate is the only one tried: an arm
that fails fails in a full combination. One obligation's case trees cover at
most 64 arm combinations; a split that fails gives back what it took, and
running out is reported as exhausted search. One goal's whole search, its
branches and the side conditions of its bounds included, has 100,000 steps
(`PROOF_STEP_BUDGET`): a step for each goal started and one for each known
fact a case branch prepares again. The largest proof in the compiler's tests
takes under 27,000. Running out leaves the obligation unproved at code 37,
`proof_search_goal_steps`, never as a proof, so a goal out of reach fails
within seconds however many path facts surround it
(`out_of_reach_bound_search`). Once one machine's checking has spent a full
budget on such goals, each later goal there gets 8,192 steps, so a body of
out-of-reach goals does not pay the full budget once per goal. An implication, the goal at a call that
may crash, is proved by assuming its premise and proving its conclusion
(`ImplicationIntroduction`): under a callee's route `d == 0`, the caller's
`requires d != 0` (`d < 0 or 0 < d`) splits into two contradictions, and
`Falsehood` follows. Last, the prover tries a goal still unproved as one on
an unreachable path, and likewise `Falsehood`, which a crash site owes when
its machine publishes no route for its cause. When an order fact known at the
point contradicts the bounds of its own endpoints (`requires count >= 10`
under the guard `count < 5`), chaining the two gives a closed false relation
between literals, whose denotation is `Falsehood`, and the goal follows
([integer certificates](../../../../../../wiki/spec/terminal-psi/integer_certificates.md#elementary-certificate-rules):
"closes an impossible arrival without pruning that arrival").

Two orders between values with no literal contradict through the guard's own
value instead. After `requires low <= high`, the arm of `high < low` knows
`c == true`, where `c` is the comparison's result. Chaining the two orders
reaches only `high < high`, and the kernel calls a relation false only between
two literals. But `c == false` denotes `low <= high` under `c`'s result
equation, and that is known. So `c == false` is proved
(`ValueEqualityTransport`), `true == c` and `c == false` give `true == false`
(`EqualitySymmetry`, `EqualityTransitivity`), and the denotation of that is
`Falsehood`. The negated guard's denotation may be any goal the direct search
proves, not only a known fact: `requires low < high` rules out `high < low`
by weakening. No kernel rule was added for this, and the mathematical core
judges the whole certificate.

An exact remainder by a divisor that is not a literal takes a bound from its
dividend: it lies between zero and the dividend, so `-7 <= value <= 7` gives
`-7 <= value % divisor <= 7` once `divisor` is proved positive or below
minus one (`remainder_within_dividend`). Like the two rules below, this rests
on a provisional kernel rule: the kernel trusts the statement
(`IntegerRemainderWithinDividend` in
[classicality](../../../../../../wiki/spec/proofs/classicality.md)), and its
mathematical core does not derive it.

An exact quotient is at most its dividend when the dividend is nonnegative and
the divisor positive (`quotient_at_most_dividend`): `value / 2 <= value`, an
order step between two values, which also carries the dividend's upper bound
to the quotient. This too rests on a provisional kernel rule
(`IntegerQuotientAtMostDividend`).

An exact remainder by a nonzero literal divisor (excluding signed `-1`) is
zero when its dividend expands over exact add, subtract, multiply, widen and
exact cast into a polynomial whose every coefficient is divisible by that
divisor (`multiple_remainder_bound`), resting on the provisional kernel rule
`IntegerMultipleRemainder`.

`verify_obligation` then decides the whole certificate against the reconstructed
goal, assumptions and axioms. Search alone supplies no acceptance: a certificate
the kernel refuses remains an unproved obligation.

### What the integer rules do not reach

Each of these is true in the programs that need it and has no derivation in
the rules of [integer certificates](../../../../../../wiki/spec/terminal-psi/integer_certificates.md),
so the obligation is `Unproved`. None is worked around here. The programs
named below were in the deleted `tests/omega/` corpus.

Whether such an obligation rejects the program depends on what it rests on
([prove/reach.rs](src/proofs/prove/reach.rs)). A goal that depends on a value
defined by an operation the rules give no exact interval is unproved whether
or not the program is right, so its cause is `NoRuleForGoalForm` and the
route's verdict is `Incomplete`: the second and third items below, and any
goal on an exclusive or, a bitwise and, or or not, a wrapping or saturating
operation, or a quotient, remainder or right shift whose divisor or count no
literal or earlier equation fixes. The dependence is read from the result
equations of the values the goal names, never from how a search ended, and
it follows an inferred input or result across a call (`Unreached`): a formal
whose inferred condition describes such an actual, and the result of a
private helper whose body returns one. A written contract does not carry it:
an arrival or return that cannot establish a written clause is judged where
the operation is, and a caller that needs more than a written contract
states is rejected. A goal that fails on add, subtract, multiply, a cast, a
left shift, or a divide or right shift by a literal alone is still `Reject`:
the first, fourth and fifth items below are.

One case of the second kind is proved instead: a remainder by a divisor
whose own bounds fix it to one literal (`divisor >= 16 && divisor <= 16`)
takes the kernel's bound of the remainder by that literal spelled in place,
carried onto the result through its equation and the proved `divisor == 16`
(`remainder_by_fixed_divisor`). The kernel has no in-place form for a
quotient or a right shift, so those stay unreached.

- *A contradiction between two order facts when neither is a taken guard.*
  Two bare facts `low <= high` and `high < low` (two `requires`, or a
  split alternative against a requirement) chain to `high < high`, and no
  rule calls that false without a literal. It would take irreflexivity of
  `<` on a value as a kernel rule. None of the programs in
  [route one measures](https://github.com/CathedralOS/Omega/blob/3b24dc9383e37416021ff19b350869a147c57432/wiki/drafts/designs/route_one_measures.md#2-verdict-agreement)
  needs it: a guard that contradicts an order is closed through the
  guard's value, above.
- *A quotient bounded through its divisor's bound.* `value / divisor` with
  `divisor >= 2` is at most half the carrier's maximum. The endpoint chain
  replays a divide only by a literal or landed divisor, and the relation
  `value / divisor <= value` gives only the dividend's own bound. It would
  take endpoint propagation with the divisor's interval
  (`pass/arithmetic/runtime_full_width_unsigned_quotient_exit`).
- *A bitwise operation of two known operands.* A mask bounds its result
  in `[0, mask]`, but `-9 & 15 == 7` needs the operation evaluated, and
  closed evaluation covers add, subtract, multiply and left shift only.
  Exclusive or has no rule at all
  (`pass/arithmetic/bitwise_singleton_signed_and_unsigned_exit`).
- *A relation through a multiply.* `value * value >= value` for
  `value >= 1`; beyond the floor, which propagates ranges only
  (`pass/arithmetic/nonlinear_square_positive_operand_exit`).
- *A quotient or right shift by an operand fixed by its bounds.* The
  endpoint chain lands a divisor or count only on an earlier result
  equation `value == literal`, so a quotient or right shift by
  `divisor >= 3 && divisor <= 3` is unreached. Transport lands such an
  operand (below), but the chain does not consult it.

A value whose two literal bounds meet is that literal ("An operand's bounds
are the tightest literal bounds order reasoning gives it"), and transport
uses it as one. When transport through a goal's definitions fails,
`landed_transport` walks the same definitions and stops at each value whose
bounds meet, replacing its definition by `value == k`, which is the
`IntegerOrderAntisymmetry` of its two bound certificates. In
`(prefix - 496) + a32` with each of `prefix`'s 31 summands fixed by a
requirement, `prefix - 496` lands on 0, so `result <= a32` denotes
`0 + a32 <= a32`. Unfolding `prefix` instead exceeds the kernel's predicate
depth of 64 (`samples/cli/facts/thirtytwo_arg_stack_calling_plan`). The kernel
evaluates what landing closes, including an exclusive or of a landed
operand.

## Why this route and not the arithmetic engine

The code compared here belonged to the checked-tree stages, removed on
2026-11-12. Their certificate
builders (`proof/src/checker.rs`, module `certificate`) took typed-tree
expression handles and a `ProofPlan`, so Terminal values could not drive
them; only the kernel interface (`ProofNode`, `verify_obligation`) could. The
arithmetic engine (`validation/src/contract_entailment.rs`,
`arithmetic_domains.rs`) decided by interval and relational search and
emitted no derivation, and its hypotheses were bound to source shapes. The
kernel already has the rules this subset needs, so the prover emits them
directly.

## Findings

1. **Branch facts reach the prover through the kernel.** A `BranchCondition`
   row states `c == true` (or `false`); the prover transports it through the
   result equations of Boolean values and constants into the comparison it
   stands for (`ValueEqualityTransport`, whose conclusion the kernel
   computes with `check_value_equality_denotation`). Any derived bounds must
   still pass `verify_obligation`; an inferred or transported fact is not
   an independent checking verdict.
2. **Knowledge propositions have a table.** `MachineTables::propositions`
   (machine-local, minted by `push`); `KnowledgeRow::proposition` indexes it.
3. **Callee clauses live on the machine contract.** A callee's `requires`
   and `ensures` live once in its `MachineContract`, identified by machine
   and position, which is what `CallRequirement` and `CalleeGuarantee` use.
   Discharge reads those contracts from the module.
4. **Shift-left representability uses the kernel's direct shift form.**
   The evidence is the value's bound and, for the count, the equation that
   fixes it to a literal or else its upper bound (with `0 <= count` when the
   count is signed); the kernel picks the count endpoint by the sign of the
   value's bound (`map_direct_shift_left_bound`).
5. **The read equation needs no kernel rule.** `read == v` is passed to the
   kernel as a semantic axiom, like every operation equation, and the
   existing equality steps carry it: a bound on `v` bounds the read, and two
   reads of one leaf are related through the value they share. An unsigned
   `p.x - p.x` over a formal discharges with a `Judged` certificate.
6. **No store is on an edge.** Every store is derived from an operation and
   takes effect at its normal continuation, and the backward walk moves
   between blocks. A write row at `ProgramPoint::Edge` or at a block entry
   is one no operation makes and is refused. The slice that stores on an
   edge (a return into the result place, a transfer) has to give the walk
   edges first.
7. **The write log cannot say what the walk needs.** `Content` has no form
   for a source place, for the case a construction established or for a
   call's result, so those stores are all `Overwritten` in the log. The walk
   reads the derived store (`Stored`), which says each of them, and the log
   is only compared with it.
8. **A scalar local with no initializer holds zero.** `let mutable x: u32; f(x)`
   is accepted with `x == 0` known (`local-without-initializer`). The
   establishing operation names the zero as its operand, so the home's store
   is derived from it like any other, and elaboration logs the same row. A
   scalar type that zero does not satisfy would be gated like a data type,
   but none is served: the reader refuses a constrained scalar type.

9. **A case fact is not knowledge.** The lane plan asks for "the
   membership value on each `StructuralCase` edge, as the existing
   `BranchCondition` row". A `StructuralCase` terminator has no membership
   value, and the only proposition that could state the fact
   (`Proposition::StructuralCaseMembership`) names a place, which this
   route's knowledge never does. The case selected on an edge is therefore
   held by the payload judgment alone. What an edge binds is over values and
   is a row, justified as `Justification::DispatchBinding`.
10. **A payload judgment that source programs do not reach yet.** Stage 02
    leaves a directly authored payload member (`s.at`) with no field symbol,
    so elaboration refuses it as a spelling, and the payload members it does
    read are pattern bindings, which lie under their own dispatch edge.
    Only tests over closed bodies exercise the refusing paths of the
    judgment (a read after a write to the sum, on a false edge, of a formal,
    of another case); no source program does yet.

11. **A write that opens a window has a verdict that is neither a
    discharge nor a refusal.** `Discharged::Window` holds why the clause
    was not proved at the store or the zero establishment. The
    specification puts the refusal at the consumption, and an `Unproved`
    verdict at the write would reject `let mutable g: Gated; g = Gated { .. };`
    and `s.low = 12; s.high = 20;` before their later restoration could count.
12. **Invariant windows are judged, not handed back.** See
    [Invariant windows](#invariant-windows). The sentence "Terminal Psi
    opens no invariant window on a field" in structural_access.md is about
    predicate-bearing byte-view fields and does not govern data `requires`.
    Two consequences of the reaching walk: a window closed on one arrival
    and open on another is open at the join, and when the two arrivals hold
    different values the clause has no value to be stated over, so it is
    unproved there. Every arm of this subset completes the machine, so the
    only join is a short-circuit `&&`/`||`, where no store can be written;
    no expressible program is over-refused by it today.
13. **A payload parameter carries the clause of its type through its
    edge.** A dispatch edge binds a payload as a block parameter with no
    read operation, so the clause is not justified by a read's
    `OperationSemantics`: it is filed under the edge's
    `Justification::DispatchBinding`, beside the equation with the value
    the leaf holds. `match b { Byte { value } -> .. }` on a formal, or on a
    copy of one, learns `value <= 255` from `ByteRead`'s `requires`.
14. **`Reached::Returned` tells a call's result from other unknown
    contents.** The walk could already say "contents with no value
    identity"; a read's type premise needs to know those contents arrived
    established. It is the derived store of an operation that is a call of
    the module with a record result.
15. **A requirement over a field of a record actual needs that field to
    hold one named value at the call.** `peek(make())` with
    `requires packet.first == 7` is unproved: what `make` returned has no
    value identity in the caller, and no `ensures` over `result` can supply
    one yet. A callee's implementation cannot substitute for missing
    caller-visible result correspondence.
16. **A clause over a field with no value identity is unproved even when it
    is true of every value.** After `let mutable s = make(); s.low = 0;`, the
    clause `low <= high` is unproved: `high` has no value there. Stating a
    result leaf as a value needs the callee's `ensures` over `result`, which
    this slice does not serve.

## Evidence seals

`Discharge::operation_seals` turns each discharged `OperationSafety`
obligation into a `terminal_codec::ObligationEvidenceSeal`: the obligation's
machine and identity, its goal, the hypotheses and the certificate, and a
SHA-256 fingerprint over their canonical encoding. Only that kind becomes an
accepted fact in Omega 00. The caller hands the seals to Omega 00 with
`lower_module_with_evidence`, the only way to attach them.

Omega 00 checks each seal before it fills the fact's
`proof_bundle_fingerprint`: the fingerprint matches the seal's parts, the
operation is in the module and owns the obligation, and the goal Terminal
semantics states for that operation equals the sealed goal. A seal that fails
a check, or names no accepted fact, refuses the unit. Omega 00 does not re-run
the kernel: that the kernel accepted the certificate under those hypotheses,
and that the hypotheses held where the obligation sits, it takes on faith.

A saved Psi artifact keeps its seals. Publication writes them, with each
complete certificate, into the artifact's publication section as an
operation-evidence record (`PSIOEVD`); reopening the artifact decodes every
certificate and checks each seal's fingerprint and goal against its module.

## The route's verdict

An obligation the prover cannot discharge is a compile error for the program,
never a slower guarded form
([route one](../../../../../../wiki/drafts/designs/check_on_lowered_form.md#route-one-before-anything-else-is-built-on-it)).
`judge` ([verdict.rs](src/verdict.rs)) is the one function callers take the
decision from. It discharges an elaborated closure and returns:

- `Accepted`: every obligation of every machine was discharged by a
  certificate the kernel accepted and every use of storage is established,
  with the operation seals for Omega 00.
- `Rejected`, for a loan: one `ConflictingUse` per loan that may not be
  formed ([Loans](#loans)), with the call's site.
- `Rejected`, for a use of storage: one `UnestablishedUse` per operation or
  edge that some path reaches with nothing stored into what it reads, or
  that observes the zeroed storage of a gated type, with its source site
  and, for the latter, the declaration's. `reject_diagnostic()` gives `unestablished storage:
  <detail>`. One such use rejects the program even when every unproved
  obligation would only be `Incomplete`.
- `Rejected`: one `UnprovedObligation` per obligation that was not, each with
  its kind, its source site from elaboration's correspondence, and its cause
  (`UnprovedCause`). `reject_diagnostic()` turns it into the program error,
  `<statement> (<cause>: <detail>)`, an ordinary `Reject` with no artifact.
  The statement ([describe.rs](src/reports/describe.rs)) says what was not proved,
  using the lowered form
  and elaboration's names table (`MachineNames`): `cannot prove requires
  contract for call bounded from probe: xored >= 1 && xored <= 2 (needs fact
  `value >= 1 && value <= 2` here; establish it before the call via a prior
  contract guarantee, domain membership, or guard) in state `entry`` (the
  callee's clause over the caller's actuals, then the same clause as the
  callee wrote it, and the caller's state), `cannot prove ensures contract
  for exit from shifted at statement 0: result <= value` (the exit's
  statement position retained by the reader), `exact
  arithmetic in machine `tally` state `entry` return value may overflow
  `u64`: ...`, `shift count in ... is not provably below the operand width
  for `u64`` (a left shift names both its conjuncts, the count and the
  shifted value, since the route does not say which failed), `partial exact
  division in machine `divide` state `entry` local `quotient`: the divisor
  must be proven nonzero`. Without names (bodies lowered by `lower_bodies`)
  it names identities only: `cannot prove requires clause 1 of the called
  machine`. A clause not proved again where an open invariant window is
  consumed is one such entry with `consumed` set: `<statement> inside an
  open default-domain invariant window: a prior write left the clause unproved and this use
  consumes the value before it is proved again (<cause>: <detail>)`, placed
  at the consumption, with
  `opening_diagnostic()` placed at the write. It says what was not proved and never that the obligation is
  false: "Failure to find a proof is not proof of falsehood"
  ([compiler_request.md](../../../../../../wiki/spec/build/compiler_request.md#outcomes)),
  and "Unproved source contracts reject in Psi checking"
  ([review.md](../../../../../../wiki/spec/packages/review.md)). A published
  clause that fails only because a return's guarantee failed is reported
  once, at that return.
- `Incomplete`: every unproved obligation's goal has a form the prover has no
  rule for (`NoRuleForGoalForm`: an order or equality over non-integers, a
  proposition form it does not reason about), or its search ran out of steps
  (`GoalStepsExhausted`). The program
  may be valid and the checker cannot judge it, so this is the outcome table's
  "unsupported valid case" (owner ruling on `unproved-goal-form-outcome`). No
  artifact and no fallback either. `Reject` wins when both kinds occur. Each
  such goal's diagnostic carries code 36, `coverage_proof_goal_forms` (limit
  0, requested 1), with the unproved obligation's identity as its coordinate
  ([unproved goal form publication](../../../../../../wiki/spec/build/compiler_request.md#unproved-goal-form-publication)),
  so the request ends `Incomplete`; the message ends
  `[Incomplete code 36 (coverage_proof_goal_forms): obligation N]`. A closed contract
  implication's refusal names its site's owner, so the refusal carries the
  obligation itself (`Refusal::obligation`). A goal that runs out of its step
  budget carries code 37, `proof_search_goal_steps`, with the limit that
  goal was given, and also ends the request `Incomplete`.
- `FallBack`: elaboration or some obligation was refused as `Unsupported`.
  Despite the name, this stops compilation without an artifact; there is no
  second checker to fall back to. This wins over `Unproved`.

There is no variant for a partly proved closure: Omega 00 accepts every
obligation of the module it is given and fills evidence only where a seal
arrives, so a module with an undischarged obligation must never reach it. Any
other refusal is a compiler defect and comes back as `Err`.

The product producer's
[source driver](../../driver/judge_lowered_source.rs)
lowers the compilation, calls `judge_publication` and hands the returned
[checked program](../../representations/checked-program/checked_program.rs) to
stage 07. The driver keeps the advisories and reads them only when the
compilation is reported. Tests can call `judge` on one elaborated closure directly.

## Calls that do not return

A Build helper may return a value computed by a package it prepares. While
that package is unavailable, the checker can admit the helper through the
preparation call with its declared result still owed. Every retained operand
must already be available there. A retained owner keeps its disposition
obligation, and a normal result guarantee remains due when the helper returns.
The interpreter parks at the checked call and enters the admitted successor,
which must preserve the result contract and supply the remaining body. Only a
complete program can produce portable Psi bytes or native code.

A call with no normal completion is a terminator: `InvokeNoNormal` for a
machine no path of which returns, `ExternalCompletion` for the boundary
requirement that ends the process
([calls and outcomes](../../../../../../wiki/spec/terminal-psi/calls_and_outcomes.md#external-process-exit)).
Both are read through one view (`Terminator::declared_invocation`, the
terminator form of `OperationKind::declared_invocation`), so the row checks,
the `CallRequirement` goals, the loans and the ceilings treat the two
declaration classes alike:

- The invocation row restates the callee and actuals, and each `requires`
  clause of the callee is owed at the point before the terminator, over the
  scalar actuals. The exit status is an ordinary operand: its arithmetic
  obligations are owed before the call.
- The call adds no knowledge: guarantees "apply only on normal return;
  external exit neither establishes them nor necessarily violates them".
- A boundary declaration and its call site must state one completion
  (`callee`): a requirement that completes externally is called only as a
  terminator and one that returns only as an operation, or the row is
  `ForeignIdentity`. An external completion whose declaration has a crash
  route is refused: the canonical operation "permits no source crash".
- What the requirement reaches is charged to the caller on the terminator's
  edge, as every call's is ([Published ceilings](#published-ceilings-service-reach-and-synchronous-entries)).
- Nothing is owed after it. Its edge goes to `Destination::NoNormal`, no
  `NormalGuarantee` is minted for it, and no window is consumed
  ([Invariant windows](#invariant-windows)): exit "creates no return value,
  consumption receipt, cleanup successor, or post-exit frontier"
  ([ownership](../../../../../../wiki/spec/terminal-psi/ownership.md)). An arm
  that returns beside one that exits owes its guarantees as before.

The route mints no claim yet, so `frontier_lower_bound` is empty and there is
no abandoned obligation to list on the edge; when owned data arrives, an
exit's edge is where `Fate::Abandoned` belongs.

## Published ceilings: service reach and synchronous entries

A machine's header may bound two things by naming boundary services: the
services its calls may reach (`reaches`) and the callbacks or inbound entries
that may run before it returns (`invokes`). Each is a written upper bound
(ceiling) on what the body's calls contribute, published to callers
([effects](../../../../../../wiki/spec/language/effects.md#declarations),
[direct synchronous invocation](../../../../../../wiki/spec/language/effects.md#direct-synchronous-invocation):
"the callee's published `invokes` joins the caller's, as its reach does"), so
one mechanism serves both, with the axis as its only parameter. Three crates
each do one part, and the fact travels in the module:

- `prepared-typed-trees-to-lowered-program` reads a written clause into `Signature::reaches`
  and `Signature::invokes` (`Ceiling`), through one row reader
  (`read/services.rs`, `named_row`): the named boundary traits as service
  identities of the module's catalog. A reach row is closed under boundary
  parents; an entry row is not. Omission is read per axis: an omitted
  `reaches` is inferred on a checked body and deferred on a requirement, and
  `reaches none` is told from it by the authored row; an omitted `invokes` is
  inferred on a checked body and is the empty written row on a bodyless
  requirement ("No synchronous entry"). `outcome.rs` derives what each machine
  publishes on each axis as one fixed point over the call graph: its written
  ceiling, or else what its calls contribute, where a boundary call
  contributes its requirement's row. Lowering writes the rows:
  `declared_service_reach`, `published_service_ceiling` and
  `published_synchronous_entries` on a machine, the reach rows and
  `published_synchronous_entries` on a boundary declaration, the module's
  `services` and `root_service_reach`. A non-empty published row is part of
  the declaration's contract commitment, with its axis.
- `terminal-psi-to-terminal-psi` judges (`ceilings.rs`): at every call, on each axis,
  what the callee publishes lies inside what the caller publishes. The
  callee's published row is the only thing read of it. A call outside is a
  rejection of the program at that call (`RouteVerdict::Rejected::exceeded`,
  with its axis), with no certificate, like a loan conflict; a row that is
  not canonical, a reach row that omits a boundary parent or disagrees with
  the written ceiling or the root reach, is a table fault. The same
  containment judges the ordinary call in each prepared conformance witness
  on both axes; there is no separate callable-refinement checker.

A call of a boundary requirement is "not itself an entry", and the route
reads no operation that originates one (a call of a callable argument, of a
stored callable, or an arrival at an inbound entry), so an entry row is
non-empty only where a clause was written. `Build::application` writes
`invokes BuildRequestOperations` over a requirement that enters nothing: a
broader ceiling, which its callers are charged.

Not served: a ceiling bound by an installation (`machine service reach bound
by an installation`), a written ceiling over a requirement that defers its
reach, a call through a selector value in a module that declares services,
an `invokes` row that names a parameter path (`machine invokes naming a
parameter path`: charging a caller needs the binding each call supplies,
substituted at the call), and an `invokes` row that names a requirement that
is not a boundary trait (the module's catalog has identities for boundary
services only). The composition checks of the entry ceiling (every entry
reachable through the selected providers fits the calling requirement, and
the realized graph is acyclic) belong to provider selection and are not made
here.

## Running the checker on a source program

The public compiler checks the lowered Product selected by its authored Build.
Use `omega --check path/to/main.omg`. Stage fixtures exercise the checker
directly on its lowered input. The retired `route_probe` supplied a synthetic
entry and provider choices through V1's removed `CompilationSession`. Its
measurement code lives in `_recycling/source-rust/compiler/v1-probes`.

## Reusing a saved proof (experiment)

[reuse.rs](src/proofs/reuse.rs) is an experiment the owner asked to see before
anything is built on it; no production path calls it. `reverify` takes a saved
certificate for one obligation and has the kernel decide it for the goal and
hypotheses the consumer computes from its own lowered program, through the same
`verify` as `discharge`. It runs no search and cannot reach the prover. A stale
proof, a missing assumption, or a proof attached to another obligation or
machine fails because the kernel checks the certificate's conclusion against
the consumer's goal and each citation against the consumer's own hypotheses.
`reverify_at_call` moves a guarantee proved over a callee's formals to one
call by substituting the actuals into the guarantee, the `requires` and the
certificate; the call's own requirements must already be discharged there.
That is substitution of actuals for formals, not type-parameter
instantiation. Certificates have no byte form yet, so a saved proof lives in
memory.

## Generic bodies (experiment)

A generic machine is proved once, against the contracts its parameters
require, and each specialization of it re-checks those proofs instead of
searching again. This section covers three kinds of parameter: an
implementation parameter such as `Selected satisfies Step`, whose body calls
`Selected::apply(..)`, a [value parameter](#value-parameters) such as
`const Units: u64`, and an opaque [type parameter](#type-parameters) `T`,
whose values a body can only pass on. No production path calls any of it. A
type parameter the body branches on (`requires T == u32 || T == u64`) has no
generic form yet: each machine generated from one is read and judged as a
closed machine, under its own type
([prepared-typed-trees-to-lowered-program](../05_prepared-typed-trees-to-lowered-program/README.md#generic-machines)),
and [what is missing](#what-the-lowered-form-does-not-carry) says what a
generic form of it needs.

The example the tests and the fixture use:

```omega
trait Step {
    machine apply(value: u32) -> u32 requires value <= 10 ensures result <= 20;
}
machine advance<Selected satisfies Step>(start: u32) -> u32
requires start <= 10
ensures result <= 21
{ Selected::apply(start) + 1 }
```

In the tests `Double::apply` publishes the requirement's own contract and
returns `value + value`, and `Shift::apply` asks less and promises more
(`requires value <= 12 ensures result <= 17`) and returns `value + 5`. The
fixture (`tests/fixtures/generics/run/implementation_parameter_contract`)
selects `Shift` and `Hold`, which returns `value` and promises
`result <= 10`, so both of its uses satisfy a bound the requirement does not
give.

**The generic check form.** Elaboration lowers `advance` once with
`Selected` rigid ([prepared-typed-trees-to-lowered-program](../05_prepared-typed-trees-to-lowered-program/README.md#generic-machines)):
the requirement `Step::apply` is a machine of the module that has its
formals, result and contract and no block, and the call through the
parameter is an ordinary direct call of it. Nothing else is new. The call's
`CallRequirement` obligations are the requirement's `requires`, and the
knowledge after the call is the requirement's `ensures`, by the same rows as
for any callee. `discharge` skips the requirement itself: its guarantees are
each implementation's to prove, and their verdict is `Unsupported`, so `judge`
never accepts a generic form as a program.

**Refinement.** That an implementation may stand for the requirement is a
proof too. Its statement is a conformance witness: a machine with the
requirement's contract whose body is one call of the implementation. Its call
requirements say the requirement's `requires` imply the implementation's; its
guarantees say the implementation's `ensures` imply the requirement's.
`discharge` proves it like any machine, once per implementation.

**Carrying.** `reverify_specialization` takes the generic body's verdicts,
one witness per selected implementation, and the concrete module in which the
specialization calls the implementation directly. For each obligation of the
specialization it computes the goal and the hypotheses from that module, as
`reverify` does, and offers the kernel a proof assembled from the saved ones:

- a value keeps its identity, because selecting an implementation changes
  only which machine a call names (the function checks that the two
  machines have the same values and calls, and refuses otherwise);
- a hypothesis the generic proof cites becomes the specialization's row with
  the same justification;
- where the generic proof cites the requirement's guarantee after a call, the
  specialization does not assume that: it assumes the implementation's
  published guarantee. The citation is replaced by the witness's proof of the
  requirement's clause, with the witness's formals renamed to the call's
  actuals and its result to the call's result;
- where the witness's proof cites the requirement's `requires`, the generic
  body proved that at the call, and that proof is carried in its place;
- the specialization's own call requirement is the implementation's
  `requires`, and its proof is the witness's, moved the same way.

The kernel decides the assembled proof through `verify`, the only way to
accept one. Nothing in it is trusted and nothing searches: `reuse.rs` cannot
reach the prover, and the tests count prover starts.

**What the tests show** (`tests.rs`, `generic_bodies`):

| Test | Shows |
| --- | --- |
| `the_generic_body_is_checked_against_the_requirement_alone` | three certificates for `advance` from `Step::apply`'s contract; the requirement has no block; `judge` falls back on a generic form |
| `its_certificates_reverify_in_two_different_implementations` | the same three proofs accepted in `advance<Double>` and `advance<Shift>`, no prover started, although the two specializations assume different things after the call |
| `a_body_that_only_holds_for_the_implementation_in_use_is_rejected` | `advance` promising `result <= 18` is proved for `advance<Shift>` alone and refused as a generic body; the specialization then has nothing to carry and is refused without a search |
| `an_implementation_that_does_not_refine_the_requirement_is_refused` | `Wide` (`ensures result <= 25`) has no refinement proof, so the proofs that rest on the guarantee are refused |
| `a_refinement_proof_for_another_implementation_is_refused_by_the_kernel` | `Shift`'s witness offered for a specialization that calls `Wide` is refused by the kernel, three times |

`compiler`'s example `generic_certificate_carry` runs the same three steps
from source, on the typed trees of
`tests/fixtures/generics/run/implementation_parameter_contract` and its fail
twin. The twin differs only in the bound `advance` promises, `result <= 18`:
a search over each of its two specializations alone proves that bound, and
the generic check refuses it.

### Value parameters

A body over `const Units: u64` is proved once for every value of `Units`,
and the machine stage 04 generates for `<7>` takes those proofs with 7 in
`Units`' place. The example:

```omega
machine clamp<const Units: u64>(input: u64) -> u64
requires input <= Units
ensures result <= Units
{ input }
```

**The generic check form.** `Units` is a scalar formal of the machine, after
`input`, that no caller passes
([prepared-typed-trees-to-lowered-program](../05_prepared-typed-trees-to-lowered-program/README.md#generic-machines)).
Nothing else is new: the `requires` row is `input <= Units` over two values,
and the guarantee is proved from it like any relation between formals. What
is known of `Units` is its carrier's range and what the `requires` say, so a
proof found here holds at every admitted value
([generics.md](../../../../../../wiki/spec/language/generics.md#static-type-equality):
"An unspecialized body checks every admitted alternative; one consumer
cannot authorize an otherwise invalid generic body").

**The instantiation is worked out, not read.** The generated machine has one
formal and the literal 7 wherever the declaration named `Units`. The typed
trees record the argument of a generated machine only as its spelling, which
is not read. `Instantiation::of` ([reuse.rs](src/proofs/reuse.rs)) finds the
literals from the two lowered contracts: the ones at which each `ensures`
clause of the generic form is the specialization's clause at that position
and its `requires` clauses are the specialization's in some order (they are
held in canonical order, which a literal in place of a formal can change).
When no literals do that, the machine is not that body's contract at any
value of its parameters and nothing is carried into it. A parameter neither
contract mentions stays unbound, and a proof that names it cannot be moved.

**Carrying.** `reverify_specialization` moves each certificate with the
values renamed: the specialization's formals keep their identities, each
value parameter becomes its literal (in scalar terms, and as the integer a
literal embeds as in mathematical terms), and every later value is the
specialization's value that many identities earlier. A cited entry
requirement is found by what it says once moved, not by its position. The
kernel decides the result against the specialization's own goal and
hypotheses; a literal that did not fit would yield a certificate it refuses.

**A requirement with value parameters.** A call through an implementation
parameter with a static argument, `Implementation::value::<7>(7)`, is held in
the generic form to the requirement's contract at 7, and in the generated
machine it calls the machine generated from the selected member for 7. The
refinement proof is the conformance witness at the same literals. So the
caller's body and the member's body are each proved once, and the member's
refinement of its requirement is proved once per tuple of value arguments in
use, not once for every value.

**What the tests show.**
[tests/value_parameters.rs](src/tests/value_parameters.rs):

| Test | Shows |
| --- | --- |
| `a_body_over_a_value_parameter_is_checked_once_and_carried_to_two_literals` | one proof of `clamp`'s guarantee accepted for `clamp<7>` and `clamp<9>`, each against its own goal, no prover started |
| `a_proof_that_chains_two_requirements_is_carried_too` | a proof citing two entry requirements, one of which names the parameter, is carried with each requirement found by its moved statement |
| `a_body_that_holds_only_at_the_literal_in_use_is_rejected` | `ensures result <= 7` is proved by a search over `clamp<7>` and refused as a generic body; the specialization then has nothing to carry |
| `a_machine_whose_contract_is_no_instance_of_the_generic_contract_is_refused` | a contract that is the generic one at no single literal, and a machine with a formal too many, take no proof |
| `a_body_that_reads_the_parameter_as_an_operand_is_not_carried` | the limit below |

`compiler`'s test `lowered_route_generics` runs the three steps from source
on `source-rust/subsystems/compiler/tests/fixtures/facts/pass/lexical_generic_bound_through_local_exit`, where
`Selected::value<const Units: u64>` is a trait member called as
`Implementation::value::<7>(7)`, and on its fail twin, which differs only in
the requirement's contract (`requires true`): the member then asks `input <=
Units`, which the requirement does not give. A search over the generated
caller proves the call's requirement, because the one call passes 7; the
refinement proof does not exist, and the carried requirement is refused.

Limits:

- **A body that reads the parameter as an operand is not carried**
  (`{ input + Units }`). The generic form reads its formal there and the
  specialization evaluates a constant, so the specialization has an
  operation and a value the generic form does not, and
  `Carry::corresponds` refuses the pair. Carrying it needs the two bodies
  matched operation for operation, with each such constant standing for
  the parameter. Until then such a specialization takes no proof.
- **A member's refinement is proved per value argument in use**, as above,
  not for every value. A conformance declaration whose member has value
  parameters is therefore not yet judged as a declaration; that needs the
  witness stated over the parameters, calling the member's generic form.
- **An equality is matched as written.** Terminal Psi orders the operands
  of `==` and the conjuncts of `&&` canonically. Where a literal in place
  of the parameter changes that order inside one clause, the generic clause
  is not recognized as the specialization's and nothing is carried.
- Only an integer carrier, only `const` binders, and only parameters of
  the machine itself: a value parameter used in a type (`@[u8; N]`) waits
  for arrays.

### Type parameters

A body over an opaque `T` is proved once for every type, and the machine
stage 04 generates for `<u8>` takes those proofs unchanged. The example:

```omega
machine forward<T>(item: T, limit: u32) -> u32
requires limit <= 10 ensures result <= 10
{ limit }
machine relay<T>(item: T, limit: u32) -> u32
requires limit <= 10 ensures result <= 11
{ forward::<T>(item, limit) + 1 }
```

**A value of a rigid type.** In the generic check form `item` is a value:
it has an identity, the call passes it, and a machine may return it. Its row
says which type parameter it is of (`ValueRow::rigid`). The lowered form can
declare a value only with a `ScalarType`, so it is declared there with a
stand-in, `RigidType::DECLARED_AS`, which is not its type and from which
nothing may be concluded.
[lowered type generics](../../../../../../wiki/drafts/designs/lowered_type_generics.md#opaque-type-parameters-a-rigid-sort)
proposes a sort beside `ScalarType`; `ScalarType` is matched at 263 places
across the compiler and its back ends, so the sort lives in the check
tables, which only this route reads.

**Checking an applied helper.** The checker includes `forward<T>`'s
body when it checks `relay<T>`. Each declaration also gets its own check
without choosing a concrete `T`. The caller must prove the helper's
requirements, and the helper must prove its return promise
(`examine_generic_forms`, [verdict.rs](src/verdict.rs)).

Recursive applications use the ordinary call and termination rules. A machine
that promises to terminate needs a valid ranking for a cycle it reaches.
The checker accepts a declared implementation requirement's contract only
with the corresponding checked conformance.

**No proposition names a rigid value.** Under
[generics.md](../../../../../../wiki/spec/language/generics.md#requirements-and-conformance-evidence)
a body can move, pass and return a value of an opaque `T`; no operator or
comparison selects on it, and the reader refuses one applied to it, because
a rigid type is neither a scalar nor a record. So no goal and no knowledge
row of a well-typed body names such a value. That is enforced, not assumed:
`concrete` ([lib.rs](src/lib.rs)) refuses, as outside the route, any goal and
any hypothesis that names one, and the kernel's context does not list rigid
values. The design's rule that a kernel rule over a rigid value needs an
"any sort" premise is met by having no such rule yet: the first rule that
holds at every type (equality transport, say) is where that premise is
added.

**A value of `T` is used at most once.** A type parameter not declared
`[copy]` is affine
([ownership.md](../../../../../../wiki/spec/language/ownership.md#property-declarations):
"every use of `T` receives the property fixed by `T`'s declaration"). The
route has no judgment of moves, so the reader refuses a generic body that
names such a slot twice, on any paths, and refuses a `[linear]` parameter,
whose values must be used exactly once. An unused value is discarded, which
is legal for an affine type; its cleanup is the specialization's.

**A call of another generic machine at the body's own type.**
`forward::<T>(item, limit)` is held, in `relay`'s generic form, to `forward`'s
contract at `T`: a machine with that signature and contract and no body
(`Elaboration::applications`). `forward`'s own body is checked once as the
entry of its own generic form. In `relay<u8>` the call names the machine
generated from `forward` for `u8`, whose contract is the same clauses, so a
proof that cites `forward`'s guarantee finds it there like any callee's, and
no refinement proof stands between. Before judging these forms, the checker
prepares their actual application dependencies and refuses a template that
reaches itself, directly or through another template. A cycle cannot use its
own abstract contracts as proof. The refusal also applies when no `ensures`
clause exists: a `terminates` promise still needs a checked cycle, which this
separate-form judgment cannot provide.

**Carrying.** `reverify_specialization` checks that the specialization has
the generic body's values and calls, where each value of a rigid type is a
value of one concrete type, the same at every value of that parameter
(`Instantiation::types`, read off the two value tables). The certificates
are moved as they are: none names a rigid value, so none changes. The
kernel decides each against the specialization's own goal and hypotheses.

**What the tests show.**
[tests/type_parameters.rs](src/tests/type_parameters.rs):

| Test | Shows |
| --- | --- |
| `a_body_over_a_type_parameter_is_checked_once_and_carried_to_two_types` | three proofs of `relay`, found once, accepted in `relay<i32>` and `relay<bool>`, no prover started |
| `a_bound_the_callee_does_not_give_is_refused_for_every_type` | `ensures result <= 10` refused generically; nothing to carry, no search |
| `nothing_is_proved_from_the_type_a_rigid_value_is_declared_with` | a clause over the item at its stand-in type is no hypothesis and no goal: every obligation it would touch is refused as outside the route |
| `one_type_parameter_is_one_type_in_a_specialization` | a machine whose two `T` values have different types takes no proof |

Limits:

- **Only where `T` is a scalar type.** A specialization at a data type
  holds its `T` values in places, not values, so it does not correspond to
  the generic form and takes no proof. That needs a rigid place type.
- **No operation has a rigid result.** A call that returns a `T`, a local
  of type `T` and a field of type `T` are refused by the reader. A formal
  may be returned.
- **`[copy]` is read; cleanup is not stated.** The generic form says
  nothing of the cleanup an unused affine value owes; the route states no
  cleanup for any value yet.
- **Contracts cannot mention a `T` value.** A requirement such as
  `premise::<T>(kept)` is a proposition over a call term, which this subset
  does not read.

### Finite families

A machine whose one type parameter is listed by a `requires` clause, `requires
T == u8 || T == u16`, is checked once per listed type, with `T` fixed to that
type. generics.md, "Static type equality": "Checking the body once per listed
alternative, each with `T` fixed to that type, satisfies this rule for a
finite family; the list is closed, so those checks cover every admissible
use." Each check is an ordinary closed check of the body
(`prepared_typed_trees_to_lowered_program::elaborate_alternatives`): the family's `requires` is
decided true and states nothing, a branch on `T == u8` is decided and its
other arm is never read, and a contract clause is read left to right with
each type equation guarding the conjuncts after it, so in the `u16` check
the disjunct `T == u8 && result == 7u8` contributes nothing and
`result == 7u8` is never typed. A disjunct whose type equation is false is
treated as false there; a clause that type equations decide false outright
is refused.

A machine stage 04 generated for one alternative is that alternative's
body. Its certificates are carried by `reverify_specialization` from the
alternative it corresponds to; which one is decided by `Carry::corresponds`
on the two lowered bodies, never by the type's spelling, and the kernel
decides each carried certificate against the generated machine's own goals.
An obligation an alternative's check refuses rejects the body whether or not
any caller uses that alternative.

The pair is `run/generics/finite_family_contract`, where `pick<T>` promises
`(T == u8 && result == 7u8) || (T == u16 && result == 9u16)` and both
alternatives are proved and carried, and
`fail/generics/finite_family_contract_wrong_alternative`, which differs only
in what `pick` returns at `u16`: that alternative is refused, and
`pick<u16>` has nothing to carry. The design's witness,
`pass/contracts/generic_effective_result_comparison_exit`, lowers per
alternative and corresponds, and proves nothing: `threshold<T>` owns no
contract clause beyond its family. Compiler test `lowered_route_generics`
pins all three.

Limits: one type parameter, listed by one `requires` clause of type
equations against builtin scalar types; a family that also has an
implementation or value parameter is not checked this way; a generic
machine called from inside an alternative's body at that alternative's type
is not read.

### What the lowered form does not carry

Found by reading the types; not built:

- **A place of a rigid type.** A value of an opaque `T` is served
  ([type parameters](#type-parameters)); a place has a declared
  `StructuralTypeId`, and there is no entry for one whose type is a
  parameter, so generic data and a `T` instantiated at a data type are not.
- **A branch on a type equation in one form.** The lowered form has no
  terminator that selects an arm by a type, and the arms of a finite family
  return values of different types into one result, so such a body has no
  single lowered form. It does not need one: the owner ruled that checking
  the body once per listed type satisfies the rule for a finite family
  (generics.md, "Static type equality"), and that is how it is checked
  ([finite families](#finite-families)).
- **An operation on it.** Terminal operations are selected per type
  (`ExactIntegerAdd` holds its `IntegerType`), and the safety goal of each
  comes from that type's range. One generic body has no single operation.
- **A certificate that holds for every type.** `PropositionContext` takes a
  concrete type per value, terms and rules name `ScalarType`, and the
  integer rules depend on the width. Moving a certificate to another type
  would have to rebuild it, and the correspondence of values and calls this
  experiment relies on would no longer hold.
- **A rigid value parameter** (`const N: u64`) is served as a formal with
  no argument ([value parameters](#value-parameters)); it is the one kind
  of the four that needed no new row.

Limits of what was built:

- Refinement is proved for `requires` and `ensures` only. A requirement that
  leaves `crashes` to its member publishes no route in the generic form, so
  the generic check decides nothing about crashes through that call; each
  specialization's concrete module still reads the member's own routes.
- The witness must be one call that passes its formals on, and its proofs may
  cite only the two contracts.
- Carrying is per machine. The route cannot yet judge the whole closure of
  the fixture (its `main` ends in terminal calls), and `judge` does not use
  carried proofs.
- No timing was taken.

## Facts across a join

A branch fact holds on its edge only: "A fact from one arm does not survive
the join" (`fact_validity.md`). So after `transition total > 9000 ||
total < 3`, the false edge says only that the `||` join's Boolean parameter
`p` is `false`, which alone tells a use nothing about `total <= 9000`. The
specification's answer is a block predicate checked on every arrival before
it justifies anything downstream (`fact_validity.md`, "Premise
availability"; `control_flow.md`, "Equality relations"). [joins.rs](src/control_flow/joins.rs)
builds one per join, so no shape of join is special:

- Lowering mints an `ArrivalInvariant` obligation per edge into a block with
  parameters.
- The predicate is inferred as the disjunction over arrivals of what each
  arrival knows and the join does not, in the predicate's scope (formals and
  the join's parameters): `p == a` with the argument `a` unfolded through the
  Boolean and literal definitions on that edge, and each path fact
  transported into its denotation. For the example: `(p == (9000 < total)
  and 9000 < total) or (p == (total < 3) and total <= 9000)`.
- Each arrival obligation is that predicate with `p` replaced by the edge's
  argument, proved by the same prover and decided by the kernel. Only then is
  the predicate a `BlockInvariant` knowledge row at the join's entry. If any
  arrival is unproved, the predicate becomes `Truth` and the join is listed in
  `MachineDischarge::weakened_joins`; a weaker predicate is still a valid one,
  so this never rejects a program, but nothing downstream relies on the join.
- At the use, the prover splits over the predicate's disjunction. In each
  alternative `p == (...)` is a definition of `p`, so the branch fact `p ==
  false` transports through it (`ValueEqualityTransport`, the existing rule)
  into `total <= 9000`.

The prover's transport takes any proved definition of a Boolean value, not
only an operation's result equation, and a goal whose transported denotation
is `Truth` or a known fact closes by the same rule. A join on a cycle gets
no predicate: cyclic cuts need induction, which this checker does not do.
