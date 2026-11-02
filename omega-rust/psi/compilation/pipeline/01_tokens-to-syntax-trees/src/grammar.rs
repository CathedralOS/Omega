//! The grammar: one recursive-descent module per source construct.
//!
//! These modules are not stages and run in no fixed order: each one calls the
//! ones nested inside it, for as long as the source nests. Outermost first,
//! the containment is `declarations`, `bodies`, `contracts`, `parameters`,
//! `expressions`, `type_syntax`, and that is the real reach direction —
//! `declarations` calls all five that follow it and `bodies` calls the four
//! after it. Two pairs close back on each other where the grammar itself does:
//! `contracts` reads a return type through `parameters` while `parameters`
//! reads signature clauses and conformance arguments through `contracts`, and
//! `expressions` and `type_syntax` each bottom out in the other. `patterns`
//! reads the structural field patterns that `match` expressions and
//! `transition` guards share.

pub(crate) mod bodies;
pub(crate) mod contracts;
pub(crate) mod declarations;
pub(crate) mod expressions;
pub(crate) mod parameters;
pub(crate) mod patterns;
pub(crate) mod type_syntax;
