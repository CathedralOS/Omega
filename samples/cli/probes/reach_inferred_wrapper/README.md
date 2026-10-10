# reach_inferred_wrapper

Transitive effect inference: the direct boundary caller `App::helper`
declares `reaches Console`; the wrapper `App::greet` propagates that
reach without repeating the declaration.

Compile-checked only: `core::binding` is depend-free.
