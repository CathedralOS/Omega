# `dyn` retirement inventory

`dyn` cannot be migrated yet, by hand or by a rewrite rule, because the
compiler accepts none of its replacement syntax. The ratified replacement
([machine values](../../spec/language/machine_values.md) and
[some packages](../../spec/language/some_packages.md)) has no parser,
resolver, checker, lowering or codec support, and `omega source rewrite` has
no implementation. This inventory explains why a checked rewrite rule is the right carrier once the target
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
at the `(` of `clear: machine(counter: &mut Counter) terminates;`. The unmigrated `dyn` original
of a one-field operations record passed `omega run --both` with the same binary.

## Why the migration needs a checked rewrite rule

The edit is not token-local. Each of the following needs resolved names and
types, which only a compiler-side rule that rechecks its output can supply:

1. **Synthesized operations records.** Each dynamically used trait needs one
   `data TraitOps<...> { member: machine(...) ...; }` declaration whose field
   types copy every member's complete signature and clauses, with the trait's
   generic arguments substituted. A callable field type omitting a clause is a
   weaker contract, not the same object.
2. **Invocation sites.** A qualified `Trait::member(...)` call on such a trait whose first argument is a selector value becomes
   `(selector.member)(rest...)`; a static call on the same trait must stay. Only
   the resolved type of the first argument tells them apart.
3. **Selections.** `Impl as dyn Trait<Args>` becomes a record literal naming
   each of `Impl`'s members for that conformance. That needs the conformance's
   resolved member map, including default members the implementation omits.
4. **Finite value families.** Many of the files also declare machines with
   const value binders, the finite-family shape
   (`code<Width: u32>(...) requires Width == 16 || ...`).
   The callable field keeps the binder and its roster `requires`; the
   [finite-family contract](../../spec/terminal-psi/dynamic_dispatch.md#finite-generic-method-families)
   keeps the roster canonical. The rule must copy the roster exactly.

The [rewrite contract](../../spec/build/command_line.md#source-rewriting)
already requires rechecking the whole edited source set before writing, which
is what makes such a rule trustworthy across every customer file. A sed-style
or hand migration of this size would not be rechecked as one object.

Proposed rule name: `dyn-operations-records`. Its applicability and
preservation argument: every rewritten selector carries the same selected
implementation per member, the same member contracts and the same argument
order, so each call reaches the same body under the same obligations.
Program behavior is then re-proved by running each migrated program under
`omega run --both`; rechecking alone does not prove behavioral equivalence.

## Compiler work, in order

Both the rule and the hand-written controls depend on step 1. Each step is a
whole-pipeline slice (`Psi 05 -> 06 -> Native`), not a stage-only task.

1. **Callable types end to end.** Parse `machine(...) -> R <clauses>` as a type
   (`01_tokens-to-syntax-trees/src/type_syntax_and_patterns.rs`, beside the
   current `dyn` branch), resolve and type it, check construction refinement
   from a named machine or named conformance member, check
   `(record.field)(args)` against the current callable contract, and lower it.
   Re-point the existing dynamic-descriptor carrier rather than adding a second
   one: today `DynamicTrait` references carry a whole-trait selector. A one-callable descriptor is the general carrier; a
   whole-trait table becomes a record of them.
2. **`omega source rewrite` with `dyn-operations-records`.** Implement the
   command's staging, recheck and transactional commit once, with this rule as
   its first customer, then migrate every customer file in one change, re-run every
   affected program with `omega run --both` on the four hosts, and attribute
   each pinned diagnostic that names `dyn`.
3. **Retire `dyn`.** Delete the `dyn` type branch and `as dyn` coercion,
   `TypeReferenceNode::DynamicTrait`, the whole-trait table ownership, and the
   diagnostics that spell `` `dyn ``; change the Terminal codec marker
   coherently.
4. **`some` packages** for heterogeneous containers and state-bearing
   operations, then **OWNING-SOME-PACKAGES**.

## Open spelling to settle during step 1

[Machine values](../../spec/language/machine_values.md#callable-types-and-construction)
admits "a qualified named implementation member" as a callable value but shows
no example. The natural spelling for `Direct satisfies Window { machine view ... }`
is `Direct::view`. Confirm it against
[conformances](../../spec/language/conformances.md) member identity before the
rule emits such references.
