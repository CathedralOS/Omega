# Exact count/address conversion

Run `omega run samples/cli/memory/exact_address_conversion/main.omg --both`.
Expected native and Terminal exit: 0.

This isolates Cathedral's physical-address conversion prerequisite. Counts and
addresses have independent carriers: equal native widths are not a fit proof.
The program uses an address witness to establish fit, converts nested fields,
call results and state parameters, and converts an address back to a count.
A copied field retains its old value after the original storage changes.
No conversion creates memory authority.

The entry also explicitly requires the 64-bit address model and uses that
premise to convert an address field to a count without a separate numeric
witness. The `types/address_model_conversion*` fixtures exercise the carrier
endpoints through parameters, fields, locals, call results and state transfers,
and reject a larger address model or a narrower destination.

The matching `types/exact_address_conversion*` fixtures reject an unbounded
source in either direction, a fixed count bound without an address-model
premise, the exclusive address bound itself, a negative value, a stale field,
and a conversion that would need a later operand's effects.
