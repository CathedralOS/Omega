# Nested opaque fields

Ordinary Product data can nest boundary-opaque fields. `TokenPair` carries two
`Token` fields whose carrier shape resolves through the Build's selected
`OpaqueRepresentation` application; moving the record or projecting a field
moves the opaque values inside it. The nested declaration stays in Product
scope: the Build context only describes `Token`'s representation and does not
host the values.

Run `omega update --project tests/omega/pass/data/opaque_nested_field_selected --offline`,
then `omega run tests/omega/pass/data/opaque_nested_field_selected/main.omg --both`.
Both engines exit 0.

The negative control `fail/build/opaque_nested_field_receipt_unadmitted` pins
the same nested declaration rejecting in the Build context, where no Copy
receipt can exist before the selection it gates.
