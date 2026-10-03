# Selected payload loans across state edges

The bundled foreign `Optional<T>` lends an owned payload, or a field inside
that payload, to a shared state parameter. The successor reads the original
storage. `None` takes no payload loan. Success exits 70; a wrong read exits 71.

From the repository root:

```sh
omega run --both tests/omega/pass/borrows/case_payload_shared_transition_exit/main.omg
python3 tools/corpus_gate.py --both --filter borrows/case_payload_shared_transition_exit
```

The ordinary CLI requires reviewed package admission for the bundled `std`
dependency. Pending review is not compiler acceptance. The corpus route tests
native execution and checked interpretation without asserting package approval.

Terminal verifier tests in `straight_line/case_access.rs` independently reject
wrong-case successor operands, reversed conditional arms, and case observations
invalidated by an intervening mutable call. Static path resolution alone never
proves a case active or grants a loan.
