# Opaque representation descriptions

Build selects a valid copyable representation of `Token`, imported separately
in the Build and Product contexts. Its selection describes declarations; it
does not use `Token` as a host value or grant Build the Product's Copy receipt.
The selected carrier still undergoes independent structural admission.

Run `omega update --project tests/omega/pass/data/opaque_copy_selected --offline`,
then `omega run tests/omega/pass/data/opaque_copy_selected/main.omg --both`.
Both engines exit 0. The ordinary same-named generic machine remains an ordinary
call and creates no representation selection.

The negative controls `fail/build/opaque_descriptor_host_type_use` and
`fail/build/opaque_descriptor_invalid_carrier` require host receipt and carrier
validation respectively; withholding descriptor roots must bypass neither.
