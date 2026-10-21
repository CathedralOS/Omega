# reach_inferred_wrapper

Transitive effect inference: the direct boundary caller `App::helper`
declares `reaches Console`; the wrapper `App::greet` propagates that
reach without repeating the declaration.

Compile-checked only: `build.omg` declares no `Build::depend` row, so the root package
loads with no dependency closure and the program names only compiler-known declarations;
`builder.freestanding = true` selects the freestanding target profile.
