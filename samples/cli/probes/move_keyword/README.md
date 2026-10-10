# move keyword

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
mbx run -p omega -- --check samples/cli/probes/move_keyword/main.omg
mbx run -p omega -- update --project samples/cli/probes/move_keyword
```
