# Project declarations

Start at [build_declaration.rs](src/build_declaration.rs) for validated project
roles and names. [syntax_projection](src/syntax_projection/mod.rs) projects those
roles and dependency declarations from ordinary Omega syntax.

This independent package supplies the same declaration grammar to compilation
and package commands. It parses but never executes build code, loads imports,
selects providers or authorizes host access. Execution remains with build
evaluation; source-language parsing remains with Psi.
