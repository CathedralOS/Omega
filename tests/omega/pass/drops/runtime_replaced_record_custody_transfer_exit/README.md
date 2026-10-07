An owned record replaces nominal and scalar-case fields directly and then through
a borrowed helper before transferring the complete record to an owned callee. Both engines
must observe the replacements and finish without duplicate disposal or lost
parent custody.

Run from the repository root:

```sh
omega run --both tests/omega/pass/drops/runtime_replaced_record_custody_transfer_exit/main.omg
```
