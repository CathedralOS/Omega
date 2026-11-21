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
pub(crate) enum ExpressionSuffix {
    Delimiter,
    Clause,
    Assignment,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct ExpressionContext {
    allows_record_literal: bool, // False where a `{` after a name must be left for the caller.
    allows_membership: bool,     // False where the word `in` must be left for the caller.
    suffix: ExpressionSuffix,    // What the caller may consume after the expression.
    stops_at_greater: bool,      // True inside `<...>`, where `>` closes the list instead of comparing.
}

impl ExpressionContext {
    /// No restrictions: statements, arguments, initializers, anything in brackets.
    pub(crate) const FULL: Self = Self {
        allows_record_literal: true,
        allows_membership: true,
        suffix: ExpressionSuffix::Delimiter,
        stops_at_greater: false,
    };

    pub(crate) const STATEMENT_HEAD: Self = Self {
        suffix: ExpressionSuffix::Assignment,
        ..Self::FULL
    };

    /// A `{` follows, for instance after the subject of a `match`.
    pub(crate) const BEFORE_BRACE: Self = Self {
        allows_record_literal: false,
        ..Self::FULL
    };

    pub(crate) const HEADER: Self = Self {
        suffix: ExpressionSuffix::Clause,
        ..Self::BEFORE_BRACE
    };

    pub(crate) const PATTERN: Self = Self {
        suffix: ExpressionSuffix::Assignment,
        ..Self::BEFORE_BRACE
    };

    /// The measure of `terminates by`, which may be followed by `in` and then by a body.
    pub(crate) const MEASURE: Self = Self {
        allows_membership: false,
        ..Self::HEADER
    };

    /// A generic argument, which ends at `,` or `>`.
    pub(crate) const INSIDE_ANGLES: Self = Self {
        stops_at_greater: true,
        ..Self::FULL
    };

    pub(crate) fn get_allows_record_literal(&self) -> bool {
        self.allows_record_literal
    }

    pub(crate) fn get_allows_membership(&self) -> bool {
        self.allows_membership
    }

    pub(crate) fn suffix(&self) -> ExpressionSuffix {
        self.suffix
    }

    pub(crate) fn get_stops_at_greater(&self) -> bool {
        self.stops_at_greater
    }
}
