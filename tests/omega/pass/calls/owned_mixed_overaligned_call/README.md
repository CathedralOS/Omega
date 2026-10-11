# Mixed over-aligned owned call

After package preparation on macOS Arm64:

```text
omega run tests/omega/pass/calls/owned_mixed_overaligned_call/main.omg
```

An ordinary helper takes independent 32- and 64-aligned owned records and
returns a 64-aligned record. Exit 0 requires members from both arguments and
unchanged caller inputs. The caller-copy reservations must include alignment
padding: a copy's rounded address cannot trespass on the neighboring copy.
