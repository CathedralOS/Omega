//! What an expression is allowed to contain, given where it sits.

/*
    The same tokens mean different things after different openings. In

        match point { ... }

    the `{` starts the arms, but in `let p = point { x: 1 };` it starts a
    record literal. Rather than guess, each caller says what the position
    allows, and the answer travels down through the operators of that
    expression. Anything inside parentheses or brackets starts again from
    `FULL`, because the closing bracket ends the doubt.
*/
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct ExpressionContext {
    allows_record_literal: bool, // False where a `{` after a name must be left for the caller.
    allows_membership: bool,     // False where the word `in` must be left for the caller.
    stops_at_greater: bool,      // True inside `<...>`, where `>` closes the list instead of comparing.
}

impl ExpressionContext {
    /// No restrictions: statements, arguments, initializers, anything in brackets.
    pub(crate) const FULL: Self = Self {
        allows_record_literal: true,
        allows_membership: true,
        stops_at_greater: false,
    };

    /// A `{` follows: the subject of a `match`, or a clause's predicate before a body.
    pub(crate) const BEFORE_BRACE: Self = Self {
        allows_record_literal: false,
        allows_membership: true,
        stops_at_greater: false,
    };

    /// The measure of `terminates by`, which may be followed by `in` and then by a body.
    pub(crate) const MEASURE: Self = Self {
        allows_record_literal: false,
        allows_membership: false,
        stops_at_greater: false,
    };

    /// A generic argument, which ends at `,` or `>`.
    pub(crate) const INSIDE_ANGLES: Self = Self {
        allows_record_literal: true,
        allows_membership: true,
        stops_at_greater: true,
    };

    /// The same context, inside `<...>` or not.
    pub(crate) fn with_stops_at_greater(
        self,
        stops_at_greater: bool,
    ) -> Self {
        Self { stops_at_greater, ..self }
    }

    pub(crate) fn get_allows_record_literal(&self) -> bool {
        self.allows_record_literal
    }

    pub(crate) fn get_allows_membership(&self) -> bool {
        self.allows_membership
    }

    pub(crate) fn get_stops_at_greater(&self) -> bool {
        self.stops_at_greater
    }
}
