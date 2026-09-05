# Omega language cases

Compiler cases are organized by outcome first, then by feature area.

- `pass/`: should compile, and runnable ones should behave as asserted
- `fail/`: should be rejected with the expected diagnostic fragment
- `run/`: ad hoc runnable probes and exploratory cases that are not part of the
  main pass/fail contract suite

Inside each bucket, prefer feature folders when a group becomes noisy:

- `arithmetic/`
- `borrows/`
- `calls/`
- `collections/`
- `constraints/`
- `control_flow/`
- `ownership/`
- `parameters/`
- `rewards/`
- `slices/`
- `storage/`
- `text/`
- `domains/`
- `dungeon/`
- `traits/`

These are not hard semantic silos. A case may mix features. The goal is
simply to keep the tree navigable and let the dominant pressure of the test
decide the folder.

Conventions:

- Keep each case self-contained in its own directory with `main.omg`.
- Add `platform/` shims only when the case needs host boundaries.
- Name cases by the behavior under test, not by the fix that motivated them.
- If a case graduates into a clearer feature family, move the directory and
  update the suite path rather than duplicating it.

The `canary_suite::roster` tests validate registered source paths and file-based
failure expectations independently of host support and compile filters. A
dedicated integration test can expose its execution table through the compiler
tests' `fixture_rosters/` directory; both its test loop and the inventory consume
that same table. Inventory membership does not schedule another compilation or
change a checked-only case into a native case. Inline diagnostic owners need not
add an unused `expected.txt` merely to appear in the inventory.

Reverse closure remains `CANARY-ROSTER-DERIVATION` in `TASKS.md`: not all
dedicated test owners are represented yet, so the inventory does not yet reject
every unregistered directory. New registrations must name an actual executing
owner rather than an inventory-only exemption list.
