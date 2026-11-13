A Console read makes array contents unknown. The first read of the conjunctive
guard is normalized into a scalar temporary; its stable captured observation
must still establish the source array's bound at the named-state arrival.

Canonical rules: `wiki/spec/language/state_contracts.md` (bindings and arrivals,
mutation and subject identity) and `wiki/spec/language/expressions.md`
(evaluation schedule). Scalar copies retain their observations. An unchanged
source and selector relate to that copy; a write, restoration, selector change
or unknown effect cannot be ignored. This pins a proof completeness defect,
not a new program-entry or dispatch-coverage rule.
