# Predicate type formation

Case membership `Boolean` plus the formed default domain recovers `T == bool`,
so the Boolean payload is a `bool`.

Parent: `DOMAIN-DEFAULT-PREDICATE-UNIFICATION` in
[TASKS.md](../../../../TASKS.md). This witness is formation of that
predicate. `Value::Boolean { value: true }` and `Value::Boolean { value: false }`
are `Value<bool>`. The match reads only `Boolean` because `T == bool` refutes
`Integer`.

| Place | Positive | Reject twin |
| --- | --- | --- |
| root | [main.omg](main.omg) | [invalid](invalid/main.omg) |
| local | [local/main.omg](local/main.omg) | [invalid](local/invalid/main.omg) |
| field | [field/main.omg](field/main.omg) | [invalid](field/invalid/main.omg) |
| parameter | [parameter/main.omg](parameter/main.omg) | [invalid](parameter/invalid/main.omg) |
| call_result | [call_result/main.omg](call_result/main.omg) | [invalid](call_result/invalid/main.omg) |
| match_join | [match_join/main.omg](match_join/main.omg) | [invalid](match_join/invalid/main.omg) |
| const_name | [const_name/main.omg](const_name/main.omg) | [invalid](const_name/invalid/main.omg) |
| other_state | [other_state/main.omg](other_state/main.omg) | [invalid](other_state/invalid/main.omg) |
| other_machine | [other_machine/main.omg](other_machine/main.omg) | [invalid](other_machine/invalid/main.omg) |
| target_body | [target_body/main.omg](target_body/main.omg) | [invalid](target_body/invalid/main.omg) |

Every positive binds the four hosted targets. `omega run --both` requires
native and Terminal Psi agreement on exit 0. Each twin changes only the `true`
formation to `Value::Integer { value: 1 }`; `Value<bool>` cannot hold
`Integer`, so formation must reject at the construction gate.

The formed value reaches `payload` through each place. `field` surrounds it with
non-zero `head` and `tail` canaries, `parameter` forms it in the argument,
`call_result` returns it from helpers, `match_join` selects between two formed
values by a flag, `const_name` forms it in named constants, `other_machine`
writes it through `&mut`, and `target_body` forms it in four ordinary named
implementations selected through Build. `true` and `false` both run, so a wrong
payload read changes the result.

Requirement evidence:

- **Establishment:** construction proves the complete default domain before it
  produces the value. `Integer` fails `T == i32` on `Value<bool>`, and no caller
  guard supplies it.
- **Composition:** the same formation is judged at a local, a field, an
  argument, a return, a join, a constant, a nested machine, another machine and
  a selected implementation.
- **Psi 05-07 and Omega 00-09:** the formed `Value<bool>` executes through the
  existing layouts, calls and entry bindings; no new ABI or provider mechanism
  is claimed.

The Windows x86-64 host ran all ten positives with native exit 0 and Terminal
exit 0 on an optimized producer built from `128bd350aea`. Every twin rejected
with `literal violates the default domain`. Other hosts' native execution is
unverified.

Matching the same predicate through a generic extractor is
[case type equation recovery](../case_type_equation_recovery/README.md) and its
writes are [default domain field write](../default_domain_field_write/README.md).
Coverage and erasure of the same predicate remain on the parent bullet.
