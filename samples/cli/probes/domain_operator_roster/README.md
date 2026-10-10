# Domain Operator Roster

Compile-scope probe: domain-scoped operator declarations. No
standard-library dependency.

- `operator Quantity::Additive::combine(left, right) -> T;` —
  registers an operator spelling on the domain's members,
  overloadable by operand type (i32 and u64 here).
- The domain's `requires quantity.value >= 0` decides which values
  the operator may see; declarations carry no body.

Compile-checked only (no ProcessExit provider):

```sh
mbx run -p omega -- --check samples/cli/probes/domain_operator_roster/main.omg
mbx run -p omega -- update --project samples/cli/probes/domain_operator_roster
```

Both green on Linux x86-64; `omega.lock` publishes locally, ignored.
