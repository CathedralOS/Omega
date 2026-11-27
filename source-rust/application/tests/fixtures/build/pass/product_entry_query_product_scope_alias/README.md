# Product query dependency scope

The Product query can name a public entry supplied through a Product dependency.
The paired [Build-only control](../../../../../../../tests/fixtures/build/fail/product_entry_query_build_scope_alias/build.omg)
rejects that same path: a Build library supplied at invocation gives no
Product lookup scope.

The positive binds all four hosted targets and executes natively and in Terminal.
