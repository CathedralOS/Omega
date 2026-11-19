# Project declarations

The ratified [staged Build contract](../../../../wiki/spec/build/scoped_execution.md)
uses atomic dependency registration, inert deferred imports, checked
continuations and ordinary delegated filesystem services. The current prelude
still exposes old dependency names and the BuildInputs/BuildSource/BuildOutput
filesystem facets. These are migration debt, not extra accepted public APIs.
`STAGED-BUILD-CAPABILITIES` in [TASKS.md](../../../../TASKS.md) requires
deprecating them, migrating their users and deleting their superseded compiler
paths under the existing execution owners.

Start at [build_declaration.rs](src/build_declaration.rs) for validated project
roles and names. [syntax_projection](src/syntax_projection.rs) reads those
roles and dependency declarations from ordinary Omega syntax;
[direct_build_call.rs](src/direct_build_call.rs) owns the single direct
`Build::<operation>(builder, ...)` call shape that both projections use.

[prelude.rs](src/prelude.rs) supplies the toolchain-written `Build` API as
ordinary Omega source. Its optimization vocabulary comes from the shared
catalog in [prelude/optimizations.rs](src/prelude/optimizations.rs). The package
source owner decides when this source joins a compilation; the compiler
schedules loading, lexing and parsing it. Static declarations grant no runtime
authority.

This independent package gives compilation and package commands the same
declaration grammar. It parses build code but never runs it, loads imports,
selects providers or authorizes host access. Build evaluation runs it; Psi
parses the source language.
