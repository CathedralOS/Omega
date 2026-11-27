# Build and Product dependency scopes

The first Build registers the local `dep/` package and returns `configure.omg` as its Product. When the compiler checks that second Build, `{scope_kernel}/main.omg` is already available. The configuration calls its public `v()` and binds the application entry only when the result is 7. The application runs without a package dependency of its own.

The sample inventory excludes the three intentionally invalid projects in `controls/`. From this directory, run the same inputs on either engine.

```sh
omega run --build-input build.omg --build-input configure.omg --build-input main.omg --build-input dep build.omg
```

Each control has its own Build and can run directly from its directory.

- `invalid_product_in_build` imports the package that its first stage would register. The import fails before the stage can execute.
- `invalid_build_in_product` lets the configuration call its supplied package. The application then tries to import that same package without a Product registration.
- `invalid_private_import` registers a Product dependency and imports the right file, then tries to call its private machine.
