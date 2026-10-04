Run `omega run --both tests/omega/pass/borrows/runtime_case_return_parameter_order_exit/main.omg` from the repository root after `omega update --project tests/omega/pass/borrows/runtime_case_return_parameter_order_exit --offline`. Native and interpreted execution must exit 0.

The fallback disposes three state owners in reverse declaration order, including scalar gaps in their authored positions. The selected case instead transfers owners in a different order and checks their values. Local establishment order must not replace successor parameter order.
