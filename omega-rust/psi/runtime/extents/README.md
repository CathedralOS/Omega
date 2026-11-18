# Live extent authority

This runtime owner conserves concrete storage authority across issuance,
splitting, borrowing, mapping, external lending and completion. Failures return
the unconsumed carrier. It owns mutable activation-claim ledgers, provider and
program-local root grants, translation receipts and device ordering admission.

Extent coordinates and rights come from `extent-contracts`. Static bound laws
come from Terminal Psi's `resource-bounds` contract. Neither dependency issues live authority. The
compiler uses those smaller owners without importing this runtime ledger.
