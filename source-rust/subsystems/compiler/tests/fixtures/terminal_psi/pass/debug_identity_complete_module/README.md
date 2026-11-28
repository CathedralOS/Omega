This fixture binds debug information to the finished semantic module, including
the declaration identities retained after executable-body construction.

`expected_terminal_inspections.txt` selects a constant, a state graph and a
machine-call closure through the public inspection producer. The corpus reloads
each artifact through canonical ingress and rejects a changed debug identity.
The program entry checks that all three results are 42 and returns normally.
Psi interpretation and the separately compiled executable must each exit 0.

Public CLI reproduction from the repository root:

```text
omega psi inspect --machine constant source-rust/subsystems/compiler/tests/fixtures/terminal_psi/pass/debug_identity_complete_module/main.omg
omega psi inspect --machine state_graph source-rust/subsystems/compiler/tests/fixtures/terminal_psi/pass/debug_identity_complete_module/main.omg
omega psi inspect --machine call_closure source-rust/subsystems/compiler/tests/fixtures/terminal_psi/pass/debug_identity_complete_module/main.omg
omega run source-rust/subsystems/compiler/tests/fixtures/terminal_psi/pass/debug_identity_complete_module/main.omg
```

To check the native result, compile the same entry with `omega <entry>` and
launch the executable named in its publication report.
