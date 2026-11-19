# build_product_queries

Build queries select entries, schemas, and providers in the Product scope
of the requesting occurrence. Each result carries the identity of its
selected declaration. The positive probe checks an ordinary Product call
through the `kit` dependency alias.

- `invalid_product_entry` queries `custody_kernel` as a machine.
- `invalid_product_schema` queries the alias `kit` as a data declaration.
- `invalid_product_provider` queries `omega::language::std` as a provider.
- `invalid_build_in_product` calls `Build::application()` from a Product
  machine. The Product call site has no visible Build declaration.
