# A dependency’s declared name supplies its default alias

The directory `dependency/` publishes a package named `custody_kernel`. The first Build registers the directory as a plain `Source::Path` and selects `configure.omg` as the next Build.

The configuration imports `{custody_kernel}/main.omg` and calls its `seed` machine. Once it sees the expected value of 3, it forwards the same incoming package binding to the application. The application imports the same file and checks its value at runtime.

Run either command from this directory. Both report exit 0.

```sh
omega run --engine native build.omg
omega run --engine psi build.omg
```
