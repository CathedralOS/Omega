# Borrowed carrier round trip

With the compiler on PATH:

```sh
omega update --project tests/omega/pass/ownership/runtime_borrowed_carried_loan_roundtrip_exit --target macos_arm64 --dangerously-accept-all
omega run --both tests/omega/pass/ownership/runtime_borrowed_carried_loan_roundtrip_exit/main.omg
```

The authored exit is 42. The helper extracts and restores a reference-bearing
field under an exclusive storage window, then observes the primitive through
the restored descriptor. Repeated calls retain backing and marker identity.
Consuming transforms are a separate unfinished requirement; this fixture does
not claim their returned-loan custody works.
