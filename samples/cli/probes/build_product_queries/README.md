# build_product_queries

`BuildProduct::{entry, schema, provider}` issue restricted marker values
by lexical lookup under the call occurrence's package — the only route to
compiler-issued product descriptions. A query whose path names no
declared row rejects rather than fabricating a marker:

- `invalid_product_entry`: `entry(product, "custody_kernel", "value")`
  rejects — "names no machine in this build occurrence's package" (the
  package path is not a declaration path, and no ProgramEntry bound rows
  exist on a freestanding check).
- `invalid_product_schema`: `schema(product, "kit")` rejects — "names no
  data declaration" (alias is not a schema path either).
- `invalid_product_provider`: `provider(product, "omega::language::std")`
  rejects — "names no provider declaration" on a freestanding app.
- `invalid_build_in_product`: a product machine calling
  `Build::application()` rejects — the build-declaration namespace
  resolves in product scope, but no product machine can produce a
  `Build` activation; the Unit return cannot supply a value position.

The product-query rejections surface as build-time evaluation failures
naming the requesting package — unrequested rows never produce opaque
markers.
