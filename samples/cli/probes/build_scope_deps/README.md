# build_scope_deps

`Build::build_depend`/`build_depend_as` declare BUILD-scope edges:
the root build machine may `use kit::main` and call the dependency's
machines while wiring the product. Product edges (`depend`/
`depend_as`) authorize product imports; the two scopes never mix.

- `invalid_product_in_build`: `use kit::main` inside build.omg over a
  PRODUCT `depend_as` edge rejects "names product dependency `kit`
  ... a build import may only select build dependencies".
- `invalid_when_dep`: `depend_as_when` is recognized but never
  projects an edge — dependencies are statically projected before
  build execution and must be unconditional direct rows.
- `invalid_helper_dep`: a `depend` call inside a helper machine
  rejects identically — rows must live in the root build entry, not
  in helpers or states.
- `invalid_private_import`: `use kit::main` then calling a non-`pub`
  machine rejects "selects private machine".
- `invalid_build_in_product`: the mirror — `use kit::main` inside
  main.omg over a BUILD `build_depend_as` edge rejects "names build
  dependency `kit` ... a product import may only select product
  dependencies". The two scopes reject each other symmetrically.
