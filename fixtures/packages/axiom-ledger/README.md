# axiom-ledger

Accepted-claim fixture. It is intended to test that imported accepted claims or
bodyless proof/boundary assertions are inert until the root explicitly accepts
the package claim set, and that open deferrals are fatal for package admission.

Expected package evidence:

- accepted proof/boundary claim identity is recorded;
- open proof deferrals reject release/admission;
- imported claims cannot self-approve.

