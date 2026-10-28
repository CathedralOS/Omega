# Contract Match composition

Parent outcome: `CORE-INTEGER-FLOAT-APIS`. This focused compiler witness exercises
ordinary contract terms needed by the original four payload-preserving NaN
constructors; it does not claim completion of the parent fact row.

The original `source/library/core/float.omg` contracts and their R21 failures
remain the repair baseline. This variation removes floats entirely: nominal
case and Boolean matches compose under a bitwise operation, an unknown match
joins equal values, and an integer match consumes an independently established
entry premise. `omega run --both main.omg` must exit 0 in both engines.

Invalid controls in `tests/omega/fail/contracts/`:

- `match_contract_wrong_case_rejected`: one body arm returns the wrong value.
- `match_contract_unknown_arm_rejected`: different feasible arm values cannot be
  replaced by the first arm's value.
- `match_contract_partial_operand_rejected`: a pattern cannot establish the
  nonzero-divisor obligation needed to form itself.
- `match_contract_replaced_subject_rejected`: a captured tag cannot be reused
  for the replaced post-state subject.

Selection follows authored order and keeps exact observed-value custody. Case
facts come from independently admitted owned captures, and query-local branch
facts never escape the common-value join. Existing call totality, effects,
formation, storage-version, and payload-admission owners remain authoritative.

Validation is pending the parent's shared combined generation. Fresh outcomes
and producer identities belong in `build/core-float-20261028/`; no historical
engine result establishes this candidate's acceptance.
