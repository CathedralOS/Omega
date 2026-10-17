# Reviewed address-model contracts

Expected native and Terminal exit: 0. The consumer calls a reviewed package
whose exact address conversion requires an explicit 64-bit address model.
That platform premise is a caller obligation, not a universal address-width
rule or authority to access memory.

The same symbolic builtin occurs alongside a recast bound in a named state's
contract. An authored parameter and nominal constant named `Bound` retain
their separate identities. The program checks the converted address, both
authored values, and a bounded byte-region read.

Prepare with `omega update --project samples/cli/memory/address_model_package`,
then run `omega run --both samples/cli/memory/address_model_package/main.omg`.
The invalid state-bound control is in
`tests/omega/fail/recast/address_model_does_not_bound_offset`.
