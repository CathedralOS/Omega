# Project declarations

Start at [build_declaration.rs](src/build_declaration.rs) for validated project
roles and names. [syntax_projection](src/syntax_projection/mod.rs) projects those
roles and dependency declarations from ordinary Omega syntax;
[direct_build_call.rs](src/direct_build_call.rs) owns the one direct
`Build::<operation>(builder, ...)` call shape both projections consume.

[prelude.rs](src/prelude.rs) supplies the toolchain-authored `Build` API as
ordinary Omega source. Its optimization vocabulary is projected from the shared
catalog in [prelude/optimizations.rs](src/prelude/optimizations.rs). The package
source owner decides when this source joins a compilation; the compiler schedules
its loading, lexing and parsing. Static declarations grant no runtime authority.

This independent package supplies the same declaration grammar to compilation
and package commands. It parses but never executes build code, loads imports,
selects providers or authorizes host access. Execution remains with build
evaluation; source-language parsing remains with Psi.
