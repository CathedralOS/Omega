# Checked provider result bounds

Run the [direct example](direct/main.omg) with `omega run direct/build.omg` from this directory.

The selected provider implements the caller's `Required::get` contract and proves its result is at most 1. The caller can use that result to index the two-element array holding 7 and 11. Reading its second element returns 11.

The negative programs let the caller receive a result as large as 2. The provider still returns 1, but the declared contract permits an index outside the array. The checker refuses the array access. The examples exercise the bound through locals, fields, parameters, call results, joins, constants, nested machines, other machines and target-selected providers.

[Provider contract migration](../../../../wiki/spec/resources/native_borrows.md#provider-contracts-and-migration)
