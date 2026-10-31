# Public case predicates survive package review

The two cases have distinct `length` fields with different constraints. Package
review must retain the resolved whole-data subject and each selected case field;
same-spelled fields are not interchangeable or ambiguous.

`omega update --project . --target windows_x86_64` exercises public-contract
projection. `omega run --both main.omg` checks both cases at runtime. The reject
control is `../../../fail/packages/public_case_default_domain_wrong_case`:
`Absolute { length: 2 }` cannot borrow `Module`'s bound.
