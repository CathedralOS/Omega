# quotient data decl

Compile-scope probe. No standard-library dependency.

- quotient_data_decl: `data Q = Rep % equiv where equiv satisfies
  Equivalence<Rep,equiv> as Impl` — quotient data in user code,
  `Trait::member = machine` impl roster, `as Q` cast.
- discreteness_calls: `x <= 7` recovers `x < 8` — kernel
  discreteness discharges a strict-order callee requires.
- layout_access_plan: core::layout — `AccessPlan::inaccessible`
  seed + `AccessPlan::with` functional replace +
  `FieldAccess::Stable{read,write,exposure:Exposure::Exported}`
  + `satisfies Access::plan`.
- transitive_effect: callee-declared `reaches Console` propagates
  through an undeclared caller. Binding-field needs a Fused
  provider even at --check (filed gap); effect inference checked
  without the call.

Compile-checked only (no ProcessExit provider):

```sh
mbx run -p omega -- --check samples/cli/probes/quotient_data_decl/main.omg
mbx run -p omega -- update --project samples/cli/probes/quotient_data_decl
```
