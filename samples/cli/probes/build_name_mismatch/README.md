# build_name_mismatch

The dependency lives in `depdir/` and declares the package name `dep_declared`. The first Build registers it as `depkit`. The second Build imports `{depkit}/main.omg`, checks the returned 9, and forwards the existing binding to the application. The application, named `app_declared`, checks the same value on entry.

Run `omega run build.omg` in this directory. The `build_dual_purpose_alias_controls/invalid_declared_import` sample exercises an attempt to use a package's declared name as an unregistered root.
