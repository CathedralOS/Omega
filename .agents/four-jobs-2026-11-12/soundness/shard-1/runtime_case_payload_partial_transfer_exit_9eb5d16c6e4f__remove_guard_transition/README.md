Run `omega run --both tests/omega/pass/borrows/runtime_case_payload_partial_transfer_exit/main.omg`.
Both engines must exit 0.

The selected case moves one nested affine child through an enclosing record.
Its sibling and unused active payload retain their own cleanup; its scalar
common field remains observable before transfer. The inactive case contributes
no obligations. Plain scalar contents
do not make the moved Token copyable.
