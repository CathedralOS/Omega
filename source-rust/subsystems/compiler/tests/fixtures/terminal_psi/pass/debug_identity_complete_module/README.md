This fixture binds debug information to the finished semantic module, including
the declaration identities retained after executable-body construction.

`expected_terminal_inspections.txt` selects a constant, a state graph and a
machine-call closure through the public inspection producer. The corpus reloads
each artifact through canonical ingress and rejects a changed debug identity.
The program entry checks that all three results are 42 and returns normally;
either engine must exit 0 rather than taking the Trap branch.

Public CLI reproduction from the repository root:

```text
omega psi inspect --machine constant source-rust/subsystems/compiler/tests/fixtures/terminal_psi/pass/debug_identity_complete_module/main.omg
omega psi inspect --machine state_graph source-rust/subsystems/compiler/tests/fixtures/terminal_psi/pass/debug_identity_complete_module/main.omg
omega psi inspect --machine call_closure source-rust/subsystems/compiler/tests/fixtures/terminal_psi/pass/debug_identity_complete_module/main.omg
omega run --engine native source-rust/subsystems/compiler/tests/fixtures/terminal_psi/pass/debug_identity_complete_module/main.omg
omega run --engine psi source-rust/subsystems/compiler/tests/fixtures/terminal_psi/pass/debug_identity_complete_module/main.omg
```
