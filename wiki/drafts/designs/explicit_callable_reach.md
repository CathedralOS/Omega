# Explicit callable reach

Ratified 2026-11-09. Leaving `reaches` off a machine usually asks the compiler
to work the answer out: a checked body derives it from its calls, and a static
`requires machine` requirement defers to the machine selected for it. On a
runtime callable type, such as a `machine(...)` field, the same omission
silently promised that the stored machine reaches no service. A runtime
callable type must now state its ceiling, and `reaches none` is how it says
empty:

```omega
data CounterOps {
    clear: machine(counter: &mut Counter) reaches none terminates;
    value: machine(counter: &Counter) -> u64 reaches none terminates;
}
```

The normative owner is [deferred requirement reach](../../spec/language/effects.md#deferred-requirement-reach),
with the callable-type statement in [machine values](../../spec/language/machine_values.md#callable-types-and-construction).
Track implementation under `CALLABLE-REACH` in [TASKS.md](../../../TASKS.md).
Delete this record when the implementation and acceptance are complete.

## Decisions

- An authored runtime callable type with no `reaches` clause rejects, as a
  boundary requirement does. The diagnostic offers `reaches none`.
- Nothing else changes. Checked bodies still infer. Static requirements still
  defer to the selected implementation, so a generic can accept both
  service-free and service-reaching callbacks.
- Suspension, blocking, crash and termination clauses keep their omission
  rules on callable types.

The reason is the copy hazard. A callable type uses the same signature and
clauses as a `requires machine` requirement, so the same text meant open reach
in one position and empty reach in the other.

## Migration

Every existing callable type that omits the clause means empty today, so the
migration is mechanical: add `reaches none`. A text search on the ratification
revision found about 100 callable types across `source`, `samples` and
`tests/omega`, of which about 90 omit the clause. The count is a text search,
not a compiler query.

## Acceptance

A callable type without `reaches` rejects with the suggested clause; the same
type with `reaches none` admits a service-free machine and rejects a
service-reaching one. No callable type in `source`, `samples` or `tests/omega`
omits the clause.
