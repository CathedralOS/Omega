# Owned input storage does not escape

`omega --check tests/omega/fail/borrows/owned_input_reference_escape/main.omg`
must reject. The unrelated borrowed input supplies a matching lifetime, not
authority to return the callee's private copy of `owned`.

The valid counterpart returns an owned holder's stored external reference in
`pass/generics/runtime_case_qualified_reference_storage_exit`.
