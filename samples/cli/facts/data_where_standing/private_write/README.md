# Private receiving writes

Counter.value <= 60 makes exact i32 70 + value representable without a per-call bound.

Owning task: **DOMAIN-DEFAULT-PREDICATE-UNIFICATION**, including its product
native-harness prerequisite. The existing [data default-domain fact](../README.md)
records the historical CANONICALIZE-SCALAR-RANGE-CONTRACTS context. This production establishes the same
standing predicate through an ordinary exclusive field write. A private `fill`
helper has no authored bound; every actual arrival must establish the receiving
store's `Counter` default domain. The destination goal is never a premise.

The nine frozen positive projects retain their exact bounds, arithmetic and
130 / -2147483578 exit checks. Their twins relax only the producer's input
requirement from `replacement <= 60` to `replacement <= 61`; they retain the
receiving default and consuming operation.

| Place | Positive | Reject twin |
| --- | --- | --- |
| Local | [program](local/main.omg) | [reject](local/invalid/main.omg) |
| Field | [program](field/main.omg) | [reject](field/invalid/main.omg) |
| Parameter | [program](parameter/main.omg) | [reject](parameter/invalid/main.omg) |
| Call result | [program](call_result/main.omg) | [reject](call_result/invalid/main.omg) |
| Match join | [program](match_join/main.omg) | [reject](match_join/invalid/main.omg) |
| Const name | [program](const_name/main.omg) | [reject](const_name/invalid/main.omg) |
| Nested machine | [program](other_state/main.omg) | [reject](other_state/invalid/main.omg) |
| Other machine | [program](other_machine/main.omg) | [reject](other_machine/invalid/main.omg) |
| Build-selected implementation | [program](target_body/main.omg) | [reject](target_body/invalid/main.omg) |

The row is pinned as `data_where_private_write_through_<place>_{exit,rejected}`
under `tests/omega/{pass,fail}/facts`. The [focused controls](controls/README.md)
add two independent store-admission checks without duplicating the existing
private arithmetic controls.

- **Establishment:** all closed private arrivals justify the completed value's
  bound before the assignment is admitted; the ordinary receiving default-domain
  validator still checks the store. A single bad arrival must reject.
- **Identity and lifetime:** an earlier primitive actual is an immutable snapshot;
  a later mutation cannot change it. A read performed after that mutation has the
  new value and cannot borrow the old bound.
- **Composition and soundness:** substitution is simultaneous and occurrence
  based. Lexical nesting, escaping selections, recursive calls and an unproved
  partial operation supply no independent premises. Existing controls listed
  below retain those boundaries; exclusive borrow authority is unchanged.
- **Psi 05–07:** normal helper calls, field writes and branch results preserve
  ordinary executable meaning. The predicate contributes erased proof evidence,
  with no new runtime subject, field or authority.
- **Omega 00–02:** existing `Counter` layout, reference arguments and four hosted
  entry bindings remain unchanged; no new ABI or provider mechanism is claimed.
- **Omega 03–06:** the same exact addition and exclusive write use ordinary
  register/frame realization. The focused programs check both selected arrivals
  or the saved scalar together with the changed source and untouched canary.
- **Omega 07–09:** authored exit-zero oracles require actual Terminal/native
  agreement and canonical reload in the owning qualification; a source check
  alone does not establish execution or cross-host runtime support.

## Baseline and remaining acceptance

The retained optimized producer is fixed source
`177a9d65e1092030f28bcf6f3babfe4eda638606`. Both new positive controls wrongly
refuse at `fill`'s default-domain assignment, and both invalid controls refuse at
that same intended store gate. Raw diagnostics remain in the owning qualification
evidence; the initial obsolete-library attempt is retained separately and is not
semantic baseline evidence. Candidate source/runtime and invalid-control results
must establish acceptance; these baseline refusals do not close the row or parent.
Product/bootstrap migration, arbitrary higher-order or cyclic inference, and the
parent's remaining default-domain requirements remain open.
