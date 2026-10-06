# Complete owned argument forwarding

The program checks all three words after a resultless owned call. Equal caller
and callee signatures and an additional scalar input use the same argument
transport. An incorrect value traps; the authored result is exit 0.

```sh
cargo run -p omega -- run tests/omega/pass/calls/owned_indirect_argument_complete_forwarding_exit/main.omg
```
