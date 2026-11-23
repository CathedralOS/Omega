//! The shapes of source that are not a whole file.

/// What `parse_fragment` should expect to find.
///
/// The compiler only ever parses files. Fragments exist because the spec teaches by example, and
/// its examples include statements and standalone header clauses. Selecting a fragment kind
/// lets the corpus test parse each example in its published form.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FragmentKind {
    /// Declarations and statements in any order, as if a file and a body were one thing.
    Statements,

    /// Header clauses with no header: `requires n > 0 ensures result > n`.
    Clauses,

    /// One generic parameter with no list around it: `const Count: u32`.
    GenericParam,
}
