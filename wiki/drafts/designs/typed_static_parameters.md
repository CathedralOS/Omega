# Typed static parameters and explicit requirement binding

Ratified 2026-11-10. Passing code at compile time had four spellings: a
`<machine F>` binder completed by a separate `requires machine F(...)` clause,
the same binder completed by `requires machine F satisfies Trait::member`, a
`<Order satisfies Ranked<T>>` binder, and a bare `machine Requirement` binder
that could not be called. A machine also became bound to a requirement in
places that did not say so, because a field or parameter typed with the
requirement accepted any machine of the right shape. A static code parameter
is now one typed value, and a machine is bound to a requirement only by a form
that names both:

```omega
// before
machine apply<machine F>(x: u32) -> u32
requires machine F(value: u32) -> u32;
{ F(x) }

machine sort<T, Order satisfies Ranked<T>>(items: &mut [T]) { ... }

machine install<machine Handler>(slot: &mut HookSlot)
requires machine Handler satisfies HookProcedure::call;
{ ... }

// after
machine apply<const F: machine(value: u32) -> u32>(x: u32) -> u32 { F(x) }

machine sort<T, const Order: Ranked<T>>(items: &mut [T]) { ... }

machine install<const Handler: HookProcedure::call>(slot: &mut HookSlot) { ... }
```

```omega
machine clear(item: &mut u32) { item = 0; }

const clear_a = (clear satisfies A::clear);

machine run<const F: A::clear>(item: &mut u32) { F(item); }

run<clear_a>(&mut x);    // accepted
run<clear>(&mut x);      // rejected: `clear` is not bound to A::clear
```

The normative owners are
[typed static parameters](../../spec/language/generics.md#typed-static-parameters)
and [binding](../../spec/language/conformances.md#binding-a-machine-to-a-requirement).
Track implementation under `TYPED-STATIC-PARAMETERS` in [TASKS.md](../../../TASKS.md).
Delete this record when the implementation and acceptance are complete.

## Decisions

1. `<>` holds static parameters and `()` holds runtime ones. A static
   parameter is `name: Type`, optionally `const`. There is one binder form.
2. The type says what the parameter is:

   | Type | Parameter is |
   | --- | --- |
   | `machine(x: u32) -> u32` | A structural callable. |
   | `Trait<T>::member` | A nominal callable, bound to that requirement. |
   | `Ranked<T>` | An implementation; members are called `Order::member(...)`. |
   | `reflect::CallableInfo` | A declaration identity; never invoked. |
   | `machine` | A callable whose exact type is inferred from where it is forwarded. |

3. An open callable contract may be the type of any static value. A runtime
   value needs the closed form.
4. A callable type may take static parameters of its own,
   `machine<const Inner: ...>(...)`. Such a family is usable only statically.
5. Three forms bind a machine to a requirement: a declared edge or inline
   member, whose body is checked as a conformer; a selection row; and the
   expression `(m satisfies Trait::member)`, which yields the bound selection.
   A field, a generic argument, a result and an assignment accept a binding
   and never create one.
6. A bound selection keeps its route when copied, projected, returned or
   widened to a structural type. Its identity is the machine and the route.
7. The operand of a row, a binding expression or `reflect::callable` is a
   plain declaration the author can select. A parameter, a field, a handed
   value and a declared conformer reject. There is no rebinding; an adapter
   machine re-presents a handed selection.
8. A machine declares at most one requirement. A declared conformer's name
   denotes its routed entry and it has no separate direct entry.
9. A row or expression grants a body nothing and establishes no domain. A
   conformer issues with a written `+D` in a declared conformer's body. A
   declared conformer of a boundary requirement is reached only through its
   provider slot, and no declared conformer is named as an exact-machine
   authority route.
10. A parameter carries one nominal route.
11. A declared conformer is at most as visible as its requirement.
12. `const F: machine` replaces forwarded requirement inference. A forwarded
    position supplies the type, structural uses combine, two different nominal
    uses reject, a bare call supplies none, an authored type is the bound, and
    the inferred type is published.

## What changes for source

| Before | After |
| --- | --- |
| `<machine F>` with `requires machine F(...);` | `<const F: machine(...)>` |
| `<machine F>` with `requires machine F satisfies T::m;` | `<const F: T::m>` |
| `<Order satisfies Ranked<T>>` | `<const Order: Ranked<T>>` |
| `trait Slot<Layout, machine Requirement>` | `trait Slot<Layout, const Requirement: reflect::CallableInfo>` |
| `<machine F>` with the clause inferred | `<const F: machine>` |
| A record literal field given a plain machine for a requirement-typed field | The field is given `(m satisfies T::m)` |
| A machine declaring two requirements | A plain machine bound to each |

A text search for the retired spellings, by tree:

| Spelling | `source` | `samples` | `tests` |
| --- | --- | --- | --- |
| `<machine F>` binders | 35 | 456 | 1,332 |
| `requires machine` clauses | 54 | 481 | 1,504 |
| `<X satisfies T>` binders | 11 | 609 | 931 |
| Machines declaring two or more requirements | 0 | 58 | 98 |

The first three rows are respellings. The counts include mathematical
declarations, which keep their form.

## Behavior changes

These reject programs that were accepted:

- A record literal, argument or assignment given an unbound machine where a
  requirement type is expected.
- A parameter required to satisfy two nominal requirements at once.
- A machine declaring two requirements.
- A declared conformer more visible than its requirement.
- A declared conformer of a boundary requirement called by name.
- A declared conformer named as an exact-machine authority route.
- Binding a parameter, a field or a declared conformer.

A call by name to a declared conformer of an ordinary requirement stays
accepted and now always runs through the requirement.

## Acceptance

- Each row of the source table compiles in its new spelling under `--both`,
  and each retired spelling rejects with a diagnostic that prints the
  replacement.
- `run<clear>` rejects and `run<(clear satisfies A::clear)>` is accepted; the
  same machine bound to `A::clear` and `B::clear` gives two selections that do
  not substitute for each other.
- A bound selection passed through a `machine(...)` parameter and a record
  field still invokes through its requirement.
- A row or expression applied to a machine whose body needs the requirement's
  authority rejects at the machine.
- `visit_one<T, const F: machine>` publishes `ElementStep<T>::apply` as the
  type of `F`; forwarding uses that need two nominal requirements reject and
  name both.
- Each item under behavior changes has a rejecting fixture.
- `source`, `samples` and `tests` are migrated and no retired spelling remains
  in executable declarations.

## Left open

- Mathematical declarations keep `requires machine` hypotheses and machine
  binders used as proof-family indices. Whether those take the typed form is
  a separate decision.
- Callables as runtime-capable generic indices are out of scope.
