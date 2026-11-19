//! The shapes of source that are not a whole file.

/// What `parse_fragment` should expect to find.
///
/// The compiler only ever parses files. Fragments exist because the spec teaches by example, and
/// most of its examples are not files: a few statements, a header's clauses, one generic
/// parameter. Parsing them as what they are lets all 132 examples serve as tests.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FragmentKind {
    /// Declarations and statements in any order, as if a file and a body were one thing.
    Statements,

    /// Header clauses with no header: `requires n > 0 ensures result > n`.
    Clauses,

    /// One generic parameter with no list around it: `const Count: u32`.
    GenericParam,
}
