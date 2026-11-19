//! Where a type sits, for the two decisions a type cannot make from its own tokens.

/// Where a type sits, which decides two things a type cannot know by itself.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum TypePosition {
    /// A parameter, a field, a result: nothing around the type competes for its tokens.
    Plain,

    /// Directly inside `<...>`. A `>` ends the type, even in the middle of a machine type's clause:
    /// in `<const F: machine(v: u64) -> u64 ensures result >= v>` the last `>` closes the list.
    InsideAngles,

    /// After `as`. Here `->` is left alone, because in an arm written `x as T -> ...` the arrow
    /// belongs to the arm. Everywhere else `A -> B` is a function type.
    Cast,
}
