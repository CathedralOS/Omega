# Constant predicate materialization

Ordinary immutable constants establish their selected data and active-case
predicates before publication or copying. This does not make a refined record
eligible for a canonical generic index. The fixture exercises a public nested
record, a fixed array of records, a selected case payload, an inactive impossible
case, and exact `u64::MAX` bits through two fresh copies. Both engines must exit 0.

From the repository root, with an optimized compiler:

```sh
omega update --project tests/omega/pass/constants/runtime_predicate_constant_materialization_exit --target macos_arm64 --offline --dangerously-accept-all
omega run --both tests/omega/pass/constants/runtime_predicate_constant_materialization_exit/main.omg < /dev/null
```

The acceptance flag is for this disposable test package's local review only;
no lock or package acceptance is part of the fixture. Compiler or source changes
require fresh review. The Build declares all four hosted entries; native runs
still need the matching host.

Refusal controls under `fail/constants/` retain unused private declaration
obligations for data, nested arrays, closed generic data and active cases.
Field-shadowing controls ensure outer constants cannot stand in for a field's
actual value. Static-index controls keep the stronger eligibility boundary.

## Related library declaration inventory

At library inventory revision `c1303f89d4`, and before this repair,
`source/library/std/time.omg` declares
`Duration.subsecond_nanoseconds: u32 [0..=999_999_999]`. It is a common field:
the replacement is `subsecond_nanoseconds: u32` under
`pub data Duration [copy] requires subsecond_nanoseconds <= 999_999_999`.
The unsigned carrier establishes the original lower bound 0. The upper bound
remains the containing duration's normalization obligation; no nominal interval
domain replaces it. `Duration::ZERO` and `Duration::MAX` retain their authored
values and ordinary declaration identity.

The existing `pass/time/runtime_duration_core_exit` source and exit 70 are
unchanged. It exercises carry, borrow, underflow, saturation and full-width MAX
materialization on both engines. This inventory covers this field only;
remaining local suffixes, library parameter predicates and other historical
declarations remain on CANONICALIZE-SCALAR-RANGE-CONTRACTS.
