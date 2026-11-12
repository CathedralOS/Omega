# Open callable contracts

Ratified 2026-11-09. The same callable signature meant different things in
different places. With `reaches` or `crashes` left off, a `requires machine`
requirement meant "whatever the selected machine does", while a `machine(...)`
field type meant "nothing". Copying a signature from one to the other changed
the promise with no text changing. An omitted `reaches` or `crashes` now means
open in every callable contract, and the places that cannot hold an open
contract say so by rejecting it:

```omega
data CounterOps {
    clear: machine(counter: &mut Counter) reaches none crashes none terminates;
    value: machine(counter: &Counter) -> u64 reaches none crashes none terminates;
}
```

This record replaces the explicit-callable-reach record, whose rule is now a
consequence. The normative owner is [open callable contracts](../../spec/language/effects.md#open-callable-contracts),
with the callable-type statement in [machine values](../../spec/language/machine_values.md#callable-types-and-construction).
Track implementation under `OPEN-CALLABLE-CONTRACTS` in [TASKS.md](../../../TASKS.md).
Delete this record when the implementation and acceptance are complete.

## Decisions

- A callable contract is the signature and clauses of a callable type, a trait
  member or a machine-shaped logical hypothesis. Its meaning does not depend
  on where it is written.
- Omitted `reaches` and omitted `crashes` are open: no bound is authored and
  the selected operation's published reach or crash routes are retained.
- Omitted `suspends` and `blocks` still forbid, and omitted `terminates` still
  promises nothing. Call-site markers need a known bound, and termination is a
  positive promise.
- An open contract admits only a compile-time selection. A runtime callable
  type and a boundary requirement must write both clauses, with `none` for
  empty. An open contract there rejects and the diagnostic names the clause.
- Checked machine definitions are unchanged: a body infers reach, a private
  body may infer the other axes, and a public body with no `crashes` promises
  none.
- No `reaches _` spelling is added.

## What changes for source

Every site below meant "none" by omission, so the migration appends the clause
and changes no behavior. The counts are text searches on the ratification
revision, not compiler queries, and the sample and test trees repeat fixtures.

| Site | Clause to add | `source` | `samples` | `tests` |
| --- | --- | --- | --- | --- |
| Runtime callable type without `reaches` | `reaches none` | 39 | 23 | 27 |
| Runtime callable type without `crashes` | `crashes none` | 38 | 28 | 32 |
| Boundary requirement without `crashes` | `crashes none` | 397 | 1,799 | 3,392 |

The boundary rows are most of the work. They can migrate after the runtime
callable types without changing the rule.

## Implementation order

1. Normalize one callable-contract representation that records, per axis,
   whether reach and crashes are open, and use it for requirements, trait
   members and callable types.
2. Reject an open contract in a runtime callable position, offering the
   missing clause.
3. Reject an open contract on a boundary requirement, offering the missing
   clause.
4. Sweep `source/library`, then `samples` and `tests/omega`, re-recording the
   touched golden rows.

## Acceptance

A callable type or boundary requirement missing either clause rejects with the
suggested clause. The same type with `reaches none crashes none` admits a
service-free, crash-free machine and rejects one that reaches a service or can
crash. A static requirement with both clauses omitted still accepts both kinds
and publishes the selected machine's reach and crash routes. No callable type
or boundary requirement in `source`, `samples` or `tests/omega` omits either
clause.

## Follow-on

The rest of the mechanism this decision began, a static code parameter as a
typed value with `const`, is ratified in
[typed static parameters](typed_static_parameters.md).
