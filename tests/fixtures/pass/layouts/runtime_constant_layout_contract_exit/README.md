# Target-closed constant contracts

Run `omega run --both tests/omega/pass/layouts/runtime_constant_layout_contract_exit/main.omg`.
Native and Terminal execution must both exit 0. Calls require a layout query
directly, through a named constant, through a cast and arithmetic, and through
Boolean composition in a later state. These obligations must close before
Terminal production, using the selected target's validated layout.

The adjacent `runtime_target_layout_queries_exit` fixture exercises different
sizes in independently authored target bodies. The fail/layouts controls reject
false predicates (including an unselected body), runtime-dependent predicates,
and a selected equality implementation that does not grant builtin equality.
