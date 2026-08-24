# axiom-ledger

Accepted-claim fixture. Its bodyless boundary machine is a compiler-visible,
trust-bearing claim rather than a proof. It is intended to test that imported
accepted claims remain inert until the root explicitly accepts the package
claim set, and that open deferrals are fatal for package admission.

Expected package evidence:

- accepted proof/boundary claim identity is recorded;
- open proof deferrals reject release/admission;
- imported claims cannot self-approve.
