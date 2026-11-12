# build_scope_deps

`Build::build_dependency` registrations declare BUILD-scope edges:
the root build machine may `use kit::main` and call the dependency's
machines while wiring the product. `Build::dependency` registrations
authorize product imports; the two scopes never mix. A registration made
in a helper machine or a conditional branch is ordinary executed Build
code: only the requests that actually run belong to the Build result.

- `invalid_product_in_build`: `use kit::main` inside build.omg over a
  PRODUCT aliased `Build::dependency` edge rejects "names product dependency `kit`
  ... a build import may only select build dependencies".
- `invalid_private_import`: `use kit::main` then calling a non-`pub`
  machine rejects "selects private machine".
- `invalid_build_in_product`: the mirror — `use kit::main` inside
  main.omg over a BUILD aliased `Build::build_dependency` edge rejects "names build
  dependency `kit` ... a product import may only select product
  dependencies". The two scopes reject each other symmetrically.
