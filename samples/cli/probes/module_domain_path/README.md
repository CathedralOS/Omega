# module domain path

Compile-scope probe. No standard-library dependency.

- `units.omg` sibling file: `module units;` + `pub domain
  u64::Distance(value) requires value > 0` + `pub operator +
  u64::Distance::add` operator home.
- `u64 in units::u64::Distance` as a parameter constraint;
  literals satisfy the `> 0` requires at `measure(3, 4)`.

## Gaps found while authoring (filed)

- `x as (u64 in units::u64::Distance)` — module-qualified domain
  paths don't parse in `as` cast targets (params only).

Compile-checked only (no ProcessExit provider):

```sh
mbx run -p omega -- --check samples/cli/probes/module_domain_path/main.omg
mbx run -p omega -- update --project samples/cli/probes/module_domain_path
```
