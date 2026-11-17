# Check on the lowered form

Lower typed source to explicit values, places, operations and control-flow edges;
check that form; publish the same admitted module and its evidence. There is one
source-checking verdict, with no alternate checker for unsupported programs.

This note retains the architecture and the lessons that constrain repairs.
[Pipeline ownership](../../../omega-rust/pipeline.md) governs stage boundaries;
`LOWERED-FORM-CHECKER` in [TASKS.md](../../../TASKS.md) owns implementation.

## Why the order matters

A checker that reconstructs execution independently from source trees needs its
own model of evaluation order, storage versions, aliases and control flow.
Lowering then reconstructs those answers again. Each source arrangement becomes
another opportunity to lose a fact or keep it after its premises cease to hold.

Explicit operations and exact value/place identities let one obligation have
one meaning wherever its operand came from. The representation supplies the
relationship, rather than a new recognizer for each source shape.

## The decision

1. **Lower first.** Elaboration consumes resolved and typed decisions and builds
   unsealed Terminal Psi by structure. It does not use checker facts to select
   program meaning or declare a proof successful.
2. **Check there.** The lowered-form checker reconstructs obligations from that
   module. Proof search supplies kernel-checked evidence. Ownership, loans and
   other structural judgments keep their own checks. Missing evidence or
   unsupported behavior refuses compilation.
3. **Publish what was checked.** Publication retains the exact admitted module,
   contracts, subjects and evidence. Selected transformations obey
   [optimization ordering](../../spec/terminal-psi/product.md#optimization-ordering);
   a previous verdict does not automatically admit a changed module.

Psi 00–04 own parsing, selection, typing and preparation. If their output has only
a spelling where elaboration needs a selected declaration, repair the stage that
lost the identity. Do not add name matching downstream, another evaluator in
preparation or a source reader in the backend.

A shared crate does not change semantic ownership. Reusable numerical operations
and representations may be shared; an independent implementation of declaration
selection or proof admission is not made correct by moving it there.

## The model the checker holds

| Information | Meaning |
| --- | --- |
| Values | Each evaluated value has a stable identity. Later writes do not alter facts about that captured value. |
| Places | Storage and its current value are distinct. Writes update that association; dynamic projections retain the index value captured at formation. |
| Possible targets | Alias and callable alternatives form conservative sets. Missing information is unknown, not an empty set. |
| Obligations | Ownership, loans, restoration windows and qualification duties have exact subjects and disposition rules. They are not freely duplicable logical facts. |
| Calls and edges | Entry, substitution, writes, outcomes and handback are explicit. Each contract axis retains its own semantics. |

[Fact validity](../../spec/terminal-psi/fact_validity.md),
[ownership](../../spec/terminal-psi/ownership.md),
[loans](../../spec/terminal-psi/loans.md) and
[control flow](../../spec/terminal-psi/control_flow.md) own these rules.

Facts cannot select a different operator, rename a type or change a specialization
key. Canonical identity and proved compatibility are separate. Additional private
body facts are available only under the fixed verification unit's
[local call-summary rules](../../spec/language/state_contracts.md#local-call-summaries).
Abstract requirements stay abstract even when their implementation is available.
Written preconditions are complete; permitted private entry inference accounts
for every arrival and callable conversion.

## The gate

These are continuing correctness obligations, not a pause on implementation or
a condition for reviving an earlier checker.

**Correct scope.** Check every declaration of the selected validation subjects
and the semantic dependency closure they demand, including executed preparation
and Build entries. Invalid unused subject declarations still reject. An unused
dependency does not become a subject because its package is bundled. Follow
[validation subjects](../../spec/build/validation_subjects.md).

**Complete obligations.** The checker cannot trust a producer's list as proof
that the list is complete. Focused independent controls remove an obligation,
weaken its goal or associate it with another operation/value while leaving the
module unchanged. The responsible checking boundary must reject. Every normal
return owes its applicable postconditions; an impossible path cannot waive the
obligations of a reachable returning path.

**Generic validity.** Check a generic declaration under its complete requirements
for every admitted application, not merely the applications currently selected.
Specialization preserves exact binder substitutions and evidence dependencies.
Transported derivations must establish their receiving obligations without
repeating proof discovery. Separate artifact reuse follows
[producer admission](../../spec/terminal-psi/product.md#publication-and-separate-consumption),
not a blanket replay requirement or a previous build's success flag.

**Real completion.** A new mechanism needs an end-to-end customer and a rejection
control that reaches the intended obligation. Native/Psi agreement covers the
execution tested, not a source-meaning error shared by both routes. Applicable
independent boundary checks remain separate. An isolated helper or deleted old
path does not complete the original program.

### One meaning per obligation

Define the proposition once from its owning operation and exact subjects.
Several proof procedures may establish it, but each supplies the required
checked evidence. Do not invent a different meaning for parameters, fields,
locals, call results, joins or constants.

The [automatic proof floor](../../spec/proofs/contracts.md#required-automatic-discharge)
states what every conforming checker establishes without hints. Failure inside
that floor is a checker defect, not a reason to increase an optional search
budget. Beyond it, distinguish a refuted goal, an unestablished goal and resource
exhaustion.

### The library is a dependency, not a subject

Bundling does not change dependency scope. Consumer compilation checks what its
subjects and selections demand. A library is checked in full when it is a
validation subject, including release validation of bundled libraries. Reuse
follows [compiler storage](../../spec/build/compiler_storage.md): build-local
memory or explicitly admitted published artifacts, not an implicit cross-build
cache of accepted results.

### Lane order

Follow the owner's priority in [TASKS.md](../../../TASKS.md). Repair the shared
relationship blocking the assigned program, carry it through publication and
execution, and return to the original program after each checkpoint.
Independent established mechanisms can proceed in parallel. Do not spread a
speculative representation into several implementations before one independently
checked end-to-end route works.

Unsupported combinations are unfinished work. Accepting them through an unchecked
fallback, weakening their requirements or copying their semantics into another
producer is not a repair.

<a id="census-of-evidence"></a>

## Lessons to preserve

Retain exact selections and subjects instead of reconstructing them from text.
Keep captured-value facts separate from mutable storage contents. Represent
semantic relationships instead of adding a case for each source placement.
Every step returns its result or an explicit refusal; absence is not acceptance.
Test obligation coverage as well as proof validity. Measure progress by delivered
programs and guarantees, not lines of implementation or historical agreement with
another checker.

[Stage trust](stage_trust.md) states what downstream consumers may rely on.
Historical censuses, superseded gates, fallback designs and migration chronology
are in Git; they are not another set of current compiler instructions.
