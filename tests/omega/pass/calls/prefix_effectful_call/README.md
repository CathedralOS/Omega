# prefix_effectful_call

`walk` issues `Main::audit(main, remaining)` BEFORE its recursive
edge: the checked-body callee's complete write frame covers only
`main.audits`, disjoint from every premise carrier (`remaining`
and the pinned `ceiling`), so the entry-relative rank still
describes the transition actuals.

Compile-checked only: no standard-library dependency and
no `Main::exit` — run a check target, not native.
