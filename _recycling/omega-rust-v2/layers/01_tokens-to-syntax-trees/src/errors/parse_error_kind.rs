//! The ways a token list can fail to match the grammar.

/// Fixtures match on the kind's name and on what was expected, so message wording can change freely.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum ParseErrorKind {
    /// A token is here that the grammar does not allow at this point.
    UnexpectedToken,

    /// The source ended in the middle of something.
    UnexpectedEnd,

    /// `if` used as a statement. Omega has none; see decision 1 in GRAMMAR.md.
    IfStatement,

    /// Two arms of a `match` or `transition` with no comma between them.
    MissingArmComma,

    /// A `;` or `,` between a declaration's header and the body that follows it.
    SeparatorBeforeBody,

    /// A statement after a `transition`, which must be the last thing in a body.
    TransitionNotLast,

    /// A statement after a nested machine declaration.
    StatementAfterMachine,

    /// A comparison used as the operand of another comparison without parentheses.
    ChainedComparison,

    /// Brackets, types, patterns or declarations nested more deeply than the parser follows.
    NestingTooDeep,

    /// More tokens than a tree can number.
    SourceTooLarge,
}

impl ParseErrorKind {
    /// The sentence shown to a person. `expected` completes the phrase "expected ...".
    pub fn get_message(
        &self,
        expected: &str,
    ) -> String {
        match self {
            Self::UnexpectedToken => format!("expected {expected}"),
            Self::UnexpectedEnd => format!("the source ended where {expected} was expected"),
            Self::IfStatement => "Omega has no `if` statement; branch with `transition condition { true -> ..., false -> ... }`".to_string(),
            Self::MissingArmComma => "arms are separated by commas; put `,` after the previous arm".to_string(),
            Self::SeparatorBeforeBody => "nothing goes between a header and its body; remove this `;` or `,`".to_string(),
            Self::TransitionNotLast => "a `transition` must be the last statement of a body".to_string(),
            Self::StatementAfterMachine => "statements cannot follow a nested machine; move this above the nested declarations".to_string(),
            Self::ChainedComparison => {
                "a comparison cannot be compared again without parentheses; write `(a < b) == c`, or `a < b && b < c` for a range".to_string()
            }
            Self::NestingTooDeep => "this is nested more deeply than the parser follows".to_string(),
            Self::SourceTooLarge => "this source has more tokens than one file may hold".to_string(),
        }
    }
}
