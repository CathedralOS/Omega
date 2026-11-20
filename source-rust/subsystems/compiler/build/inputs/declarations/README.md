# Build declarations

[BuildDeclaration](build_declaration.rs) holds the project role, validated name and workspace member paths produced by a Build. [BuildRequest](build_request.rs) records requests the Build made, including dependency registration. [BuildPrelude](prelude.rs) supplies the Omega declarations available while executing it. The [Build value](prelude/declarations/builder.omg) names its facets. The [declarations](prelude/declarations) and [module sources](prelude/modules) define the API. Target and optimization cases come from their compiler catalogs.

The package editor reads literal source rows when preparing a source rewrite. Its grammar and diagnostics live with the [package editor](../../../../package-manager/dependencies/declarations/editable_build/mod.rs). The compiler receives package identities and dependency edges from Build evaluation.
