# linear defer assign

Compile-scope probe. No standard-library dependency.

- linear_defer_assign: `let mut receipt: Receipt;` deferred
  establishment — bare decl owns zeroed storage, assignment
  creates the obligation; `boundary requirement T::ack` is the
  only authorized disposition.
- move_keyword: explicit `main.f = move replacement;` store-move
  spelling.
- linear_sum_obligation: affine sum carries the linear obligation
  only in its Live case; `boundary requirement State::settle`
  consumes the whole sum.
- linear_domain_payloads: `[linear]` + `domain R::Issued` +
  predicate `R::Extra` + conjunction `R::ExtraIssued = I & E`
  + `Receipt in R::Issued` as field/array/sum payload + drop-plan
  discharge inside a satisfies impl.

Compile-checked only (no ProcessExit provider):

```sh
mbx run -p omega -- --check samples/cli/probes/linear_defer_assign/main.omg
mbx run -p omega -- update --project samples/cli/probes/linear_defer_assign
```

## Native realization

Measured on macOS arm64 on 2026-10-13. The probe binds `ProgramEntry`, but its
only disposition of `Receipt` is a `boundary requirement` with no provider, and
native realization correctly refuses a reachable requirement with zero
selected provider rows (pinned by
`tests/omega/fail/providers/consuming_trait_requirement_without_provider`).
A native build needs either a selected provider for `Receipt::ack` or a probe
that binds no entry and moves into `tests/omega/`.
Before that refusal, `let mut receipt: Receipt;` itself fails Terminal
validation ("zeroed structural establishment has an invalid result shape"):
the local is established as zeroed storage with linear multiplicity, which
spec/language/ownership.md rules out ("Implicit zero-filling establishes no
linear obligation"). Routing it to a deferred home instead reaches stage 05's
"linear local lost its exact call result custody": a linear home has no
custody rule for a value established by a later whole assignment.
