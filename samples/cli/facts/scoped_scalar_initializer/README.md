# Scoped scalar initializer

Owning requirement: the domain-family instance a scalar initializer must
establish. A **scoped** domain is a const-generic family
(`domain<const Bound: u64> u64::BelowIndex<Bound>(value) requires value < Bound;`):
one declaration whose predicates mention an index binder, so membership is a
property of an exact *application* (`BelowIndex<4>`), not of the family symbol.

**The fact:** a const-generic scoped domain family proves a literal initializer
against the exact instance it was declared with, and the consumed value is 3.

Every place copies that sentence and establishes `u64 in BelowIndex<4>` from the
literal `3`, then consumes the established value as `3`. Only the transport
changes. The original [root](main.omg) is frozen unchanged. All positives bind
the four hosted `ProgramEntry` targets and declare `Expected exit: 0`; each runs
`omega run --both` at native exit 0 with the admitted Terminal program agreeing
at exit 0.

| Positive place | Transport | One-obligation twin |
|---|---|---|
| [local](local/main.omg) | the scoped initializer itself establishes the instance | [reject](local/invalid/main.omg) |
| [field](field/main.omg) | the declared record field establishes it | [reject](field/invalid/main.omg) |
| [parameter](parameter/main.omg) | it arrives as an explicit parameter | [reject](parameter/invalid/main.omg) |
| [call_result](call_result/main.omg) | the callee completes it and returns | [reject](call_result/invalid/main.omg) |
| [match_join](match_join/main.omg) | both arms establish the same instance | [reject](match_join/invalid/main.omg) |
| [const_name](const_name/main.omg) | a named constant carries it | [reject](const_name/invalid/main.omg) |
| [other_state](other_state/main.omg) | a successor state receives it | [reject](other_state/invalid/main.omg) |
| [other_machine](other_machine/main.omg) | a forwarding machine supplies it | [reject](other_machine/invalid/main.omg) |
| [target_body](target_body/main.omg) | each of four target bodies establishes it | [reject](target_body/invalid/main.omg) |

Every twin changes exactly one thing: the construct's own initializer, `3` to
`4`, so `BelowIndex<4>` no longer holds. `git diff <place>/main.omg
<place>/invalid/main.omg` is that one literal. The field twin changes only the
field's literal, and the target-body twin changes only the `windows_x86_64`
body while the other three keep `3`. No twin adds a caller equality guard,
narrows a parameter type, strengthens a state contract or drops an import; the
consuming operation is the same in both directions.

Eight twins reject at the scoped initializer obligation:

```text
cannot prove initializer of `scoped` in exercise is in domain `BelowIndex`;
an annotation cannot establish routed qualification
```

The const-name twin reports the same obligation at the constant's result
domain, and the field twin reports it through the field's declared interval
(`value 4 is outside its declared range ...=3`). Both are the same obligation
on the same instance; no twin is rejected for an unrelated reason.

## Establishment and identity

`BelowIndex<Bound>` is predicate-only over its index binder: membership means
`value < Bound` for the *selected* `Bound`. The declaration alone establishes
nothing, and the annotation on a local is an obligation on its initializer
rather than an establishment route. The initializer proves its own predicates
against the roster the declared type already carries, so no part of this row
relies on assuming the qualification it is proving.

Identity is the exact instance. `BelowIndex<4>` and `BelowIndex<9>` share a
declaration symbol and are unrelated classifications: the compiler conserves
the interned instance identity of each application and refuses to reduce one to
the family symbol, which is why an initializer must be checked against the
application its receiving type names rather than against the family.

The value's identity and lifetime follow ordinary rules. A local's snapshot
retains its meaning; the match join requires both incoming arms to establish
the same instance, and the successor-state and forwarding-machine places
transport the already-established value rather than re-minting it. The
consuming read only observes `3`; it never supplies the static fact.

## Composition

Calls substitute exact actuals: the parameter place passes the established
scoped local into a parameter declared with the same instance, and the
call-result place returns it from a callee whose result type names it. Joins
retain only justified facts on every incoming path, and a transition proves its
arrival obligations. The consuming operation is proved directly from the fact
carried through the place. `<place>/main.omg` and `<place>/invalid/main.omg`
differ only in the initializer literal.

## Soundness boundaries

Establishing membership requires carrier, predicate and provenance obligations
together. `initializer_satisfies_indexed_domain` refuses a routed theory
outright, so a numerical predicate proof never mints authorized provenance; it
also refuses a non-integer carrier, an unmatched instance, and an initializer
whose value is not a literal or a live place read. The declaration supplies
goals, never premises, and a mismatched instance is never satisfied by a
same-symbol application.

## Checked-to-Terminal Psi lowering (Psi 05–07)

`u64 in BelowIndex<4>` is the ordinary `u64` carrier: no runtime tag, wrapper,
hidden storage or instance-specific ABI is introduced. Source construction
lowers without loss into canonical target-neutral Terminal Psi and the same
target-neutral scalar operations, erasing the proof-only qualification binding
while preserving the executable value, and every place realizes that same
carrier. `omega run --both` observes the identical result from the interpreter
core and native execution, which is the agreement recorded below.

## Provider settlement, layout geometry, and ABI calling plans (Omega 00–02)

The scoped qualification adds no provider demand, no layout and no call-plan
placement: `BelowIndex<4>` has the `u64` carrier's ordinary size and alignment,
and the places that call and return use the same scalar register lane they would
without the qualification. What changes is only which predicates the
initializer must prove before that carrier becomes readable.

## Instruction selection, register homes, and activation frame realization (Omega 03–06)

The scoped instance selects no instruction, register home, spill slot or frame
layout. The `parameter`, `call_result` and `other_machine` places exercise the
same scalar call and return path, and the `other_state` place the same state
transition, with the established value live across them unchanged.

## Form encoding, branch relaxation, relocations, and native image emission (Omega 07–09 + emission)

Each place emits an ordinary image for its bound targets; the qualification
introduces no relocation, byte-pool or dispatch-table entry. The `match_join`
place exercises the ordinary branch layout for its two arms, and every place
executes at the authored exit.

## Native realization and image emission

Each place is realized and executed, not merely checked. All nine positives and
the root bind `windows_x86_64`, `linux_x86_64`, `macos_arm64` and `linux_arm64`
`ProgramEntry`, and each records native exit 0 with the admitted Terminal
program agreeing at exit 0 on the available host. The scoped instance changes
no register home, frame slot, return home, branch layout or relocation; it
selects which predicates the initializer must prove.

## Before and after

Measured on base `f98259e86dd2`, and re-established after rebasing onto
`232ee85de96`. Before: the root and all nine positives **refused at source
check** with

```text
cannot prove initializer of `scoped` in exercise is in domain `BelowIndex`;
an annotation cannot establish routed qualification
```

The initializer path read only the declaration symbol and the interned instance
identity from `domain_constraint_identities`, dropping the closed index roster
the typed constraint already carried, and then reached the deliberate
`has_symbol_only_proof` fence, which refuses every indexed theory. That fence is
correct and was not relaxed: rendering a family predicate without its retained
index arguments would judge the declaration's binder as a free name.

Controls that isolated the defect, all measured on the same base: a
non-generic `domain u64::BelowFour(value)` accepted the same unqualified literal
initializer; `const SCALAR: u64 in BelowIndex<4> = 3` accepted it; an explicit
`3 as u64 in BelowIndex<4>` cast accepted it. Only the *bare* initializer of an
indexed family refused, and it refused for a family whose predicate does not
mention its binder at all, which is what identified the roster and not the
predicate as the missing piece.

The repair keeps the exact declared constraint on the initializer,
construct-field, array-element and field-read paths and discharges an indexed
integer family through the existing `validation::integer_domain_membership`
judgment, which joins `closed_integer_index_bindings` and substitutes every
index binder. A scoped field read matches the carried fact's own interned
instance identity instead of its family symbol. The unindexed path is
unchanged.

After: all nine places and the root run at native exit 0 / Terminal exit 0, and
every twin rejects on the construct's own initializer. The oracle discriminates:
changing only `3` to `4` flips an accepting program to the membership rejection
above.

Canonical authority: [domains](../../../../wiki/spec/language/domains.md)
(declaration and membership, refinement and executable membership) and
[default domains and zero initialization](../../../../wiki/spec/language/dependent_values.md#default-domains-and-zero-initialization).

Remaining open scope: a scoped-domain field passed *directly* as a qualified
call argument still refuses, because the field-projection route has not yet been
given the declared instance; the field place therefore binds the declared field
into its own exact scoped local before transport. That is a field-projection
gap of the same family, not a property of this row's fact, and it is recorded
here rather than worked around with a caller guard.