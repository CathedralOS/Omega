# `dyn` retirement inventory

`dyn` cannot be migrated yet, by hand or by a rewrite rule, because the
compiler accepts none of its replacement syntax. The ratified replacement
([machine values](../../spec/language/machine_values.md) and
[some packages](../../spec/language/some_packages.md)) has no parser,
resolver, checker, lowering or codec support, and `omega source rewrite` has
no implementation. This inventory measures what the migration has to move,
explains why a checked rewrite rule is the right carrier once the target
syntax exists, and orders the compiler work so the existing dynamic-descriptor
coverage migrates instead of disappearing.

Owner: **RESTORE-DYNAMIC-DESCRIPTOR-AND-TABLE-CUSTODY** in
[TASKS.md](../../../TASKS.md). Measured at `82e12564e` (`work/repo-gate-movement`)
on linux x86-64. Delete this inventory once source `dyn` parsing and
`TypeReferenceNode::DynamicTrait` are gone.

## Witness

The compiler built from `82e12564e` (`mbx build -p omega`, dev profile at
opt-level 1) rejects the spec's own first example, the `CounterOps` record from
[machine values](../../spec/language/machine_values.md#callable-types-and-construction),
at the parser: `main.omg:3:19: expected identifier, found punctuation `(``,
at the `(` of `clear: machine(counter: &mut Counter) terminates;`. A hand
migration of `pass/traits/runtime_dyn_slot_metadata_exit` (an operations record
with one `machine<'input>(...)` field, selected by `Direct::view`) stops at the
same point. The unmigrated `dyn` original passes `omega run --both` (native
exit 0, interpreter exit 0) with the same binary.

## Source census: 2,279 lines in 642 files

Counted with `git grep -w dyn -- '*.omg'`; 2,059 of those lines are code, the
rest comments. No `source/` library or `bootstrap/` file uses `dyn`, so every
customer is a fixture or sample.

| Area | Files | Lines |
|---|---:|---:|
| `samples/cli/facts/` (frozen) | 220 | 872 |
| `tests/omega/pass/facts/` (frozen) | 108 | 439 |
| `tests/omega/fail/facts/` (frozen) | 99 | 401 |
| `tests/omega/pass/traits/` | 69 | 217 |
| `tests/omega/fail/traits/` | 66 | 173 |
| `samples/cli/probes/` | 9 | 45 |
| `tests/omega/run/` | 4 | 11 |
| other `tests/omega/{pass,fail}/` directories | 67 | 121 |

The code lines contain four forms:

| Form | Occurrences | Replacement |
|---|---:|---|
| `x: dyn Trait<Args>` (local, field, parameter) | 1,340 | operations-record type |
| `Impl as dyn Trait<Args>` (selection) | 1,176 | operations-record literal of the implementation's members |
| `-> dyn Trait<Args>` (result) | 52 | operations-record type |
| `[dyn Trait<Args>; N]` (array element) | 8 | operations-record type |

No source uses the captured `dyn Trait<_>` form; nothing in the corpus needs
`some` for its first migration step. 64 distinct traits are used dynamically.
451 occurrences pass generic arguments (`dyn Shape<Item>`).

### Corpus state of the customers

413 corpus directories contain `dyn`. In `tests/omega/corpus_outcomes.txt`,
193 `pass/` and `run/` rows are `checked`, 184 `fail/` rows are
`rejected ... expected`, one `pass/` row is `rejected` and one `fail/` row is
`unexpected`. Thirty rows (the `dynamic_suspension_through_*` facts and the
`runtime_dyn_suspending_*` / `dyn_suspending_*` traits fixtures) have no
recorded row. The migration therefore preserves roughly 377 green rows; it
turns almost nothing green by itself. Thirty-three `fail/` rows pin a
diagnostic whose `expected.txt` text names `dyn`; those photographs change
wording when the construct changes, and each must be re-attributed rather than
re-recorded blindly.

## Why the migration needs a checked rewrite rule

The edit is not token-local. Each of the following needs resolved names and
types, which only a compiler-side rule that rechecks its output can supply:

1. **Synthesized operations records.** Each of the 64 traits needs one
   `data TraitOps<...> { member: machine(...) ...; }` declaration whose field
   types copy every member's complete signature and clauses, with the trait's
   generic arguments substituted. A callable field type omitting a clause is a
   weaker contract, not the same object.
2. **Invocation sites.** There are 2,884 qualified `Trait::member(...)` calls on
   these 64 traits, and the same traits appear in 1,831 `satisfies` edges.
   A call whose first argument is a selector value becomes
   `(selector.member)(rest...)`; a static call on the same trait must stay. Only
   the resolved type of the first argument tells them apart.
3. **Selections.** `Impl as dyn Trait<Args>` becomes a record literal naming
   each of `Impl`'s members for that conformance. That needs the conformance's
   resolved member map, including default members the implementation omits.
4. **Finite value families.** 216 of the 642 files also declare machines with
   const value binders, the finite-family shape
   (`code<Width: u32>(...) requires Width == 16 || ...`).
   The callable field keeps the binder and its roster `requires`; the
   [finite-family contract](../../spec/terminal-psi/dynamic_dispatch.md#finite-generic-method-families)
   keeps the roster canonical. The rule must copy the roster exactly.

The [rewrite contract](../../spec/build/command_line.md#source-rewriting)
already requires rechecking the whole edited source set before writing, which
is what makes such a rule trustworthy across 642 files. A sed-style or
hand migration of this size would not be rechecked as one object.

Proposed rule name: `dyn-operations-records`. Its applicability and
preservation argument: every rewritten selector carries the same selected
implementation per member, the same member contracts and the same argument
order, so each call reaches the same body under the same obligations.
Fixture behavior is then re-proved by running each migrated row under
`omega run --both`; rechecking alone does not prove behavioral equivalence.

## Compiler work, in order

Both the rule and the hand-written controls depend on step 1. Each step is a
whole-pipeline slice (`Psi 05 -> 06 -> 08 -> Native`), not a stage-only task.

1. **Callable types end to end.** Parse `machine(...) -> R <clauses>` as a type
   (`01_tokens-to-syntax-trees/src/type_syntax_and_patterns.rs`, beside the
   current `dyn` branch), resolve and type it, check construction refinement
   from a named machine or named conformance member, check
   `(record.field)(args)` against the current callable contract, and lower it.
   Re-point the existing dynamic-descriptor carrier rather than adding a second
   one: today 216 `DynamicTrait` references in 77 files carry a whole-trait
   selector: 209 in 71 `psi/` files (16 files in `validation/`, 11 in stage 02,
   10 each in typed trees and stage 05) and 7 in 6 `omega/` files
   (`realization/provider-planning`, `packages/review-evidence`). A one-callable descriptor is the general carrier; a
   whole-trait table becomes a record of them.
2. **`omega source rewrite` with `dyn-operations-records`.** Implement the
   command's staging, recheck and transactional commit once, with this rule as
   its first customer, then migrate all 642 files in one change, re-run every
   affected row with `omega run --both` on the four hosts, and attribute each of
   the 33 diagnostic photographs.
3. **Retire `dyn`.** Delete the `dyn` type branch and `as dyn` coercion,
   `TypeReferenceNode::DynamicTrait`, the whole-trait table ownership, and the
   17 diagnostics that spell `` `dyn ``; change the Terminal codec marker
   coherently.
4. **`some` packages** for heterogeneous containers and state-bearing
   operations, then **OWNING-SOME-PACKAGES**.

## Open spelling to settle during step 1

[Machine values](../../spec/language/machine_values.md#callable-types-and-construction)
admits "a qualified named implementation member" as a callable value but shows
no example. The natural spelling for `Direct satisfies Window { machine view ... }`
is `Direct::view`. Confirm it against
[conformances](../../spec/language/conformances.md) member identity before the
rule emits 1,176 such references.
