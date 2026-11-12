This fixture binds debug information to the finished semantic module, including
the declaration identities retained after executable-body construction.

`expected_terminal_inspections.txt` selects a constant, a state graph and a
machine-call closure through the public inspection producer. The corpus reloads
each artifact through canonical ingress and rejects a changed debug identity.
The program entry checks that all three results are 42 and returns normally;
either engine must exit 0 rather than taking the Trap branch.

Public CLI reproduction from the repository root:

```text
omega inspect-terminal --machine constant tests/omega/pass/terminal_psi/debug_identity_complete_module/main.omg
omega inspect-terminal --machine state_graph tests/omega/pass/terminal_psi/debug_identity_complete_module/main.omg
omega inspect-terminal --machine call_closure tests/omega/pass/terminal_psi/debug_identity_complete_module/main.omg
omega run --both tests/omega/pass/terminal_psi/debug_identity_complete_module/main.omg
```
