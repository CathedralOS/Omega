Run from the repository root after preparing the project dependencies:

```sh
omega run --both tests/omega/pass/borrows/runtime_state_boolean_reference_exit/main.omg
```

Both engines must exit 70. A shared Boolean reference returned through a named
state is observed from its established carrier. The adjacent false field catches
a read from the wrong backing, and negation exercises ordinary Boolean composition.
