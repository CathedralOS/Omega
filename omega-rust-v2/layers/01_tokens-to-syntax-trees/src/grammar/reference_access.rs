//! Disambiguate the optional permission after `@` on meaningful tokens, before emitting it.

use super::contexts::expression_context::{ExpressionContext, ExpressionSuffix};
use super::contexts::type_position::TypePosition;
use crate::parsing::syntax_parser::SyntaxParser;
use omega_tokens::Punctuation;

impl SyntaxParser<'_> {
    fn at_access_word(&self) -> bool {
        matches!(self.nth_word(0), Some(b"mut" | b"write"))
    }

    pub(super) fn at_type_access_marker(
        &self,
        position: TypePosition,
    ) -> bool {
        if !self.at_access_word() || !self.nth_can_start_type(1) {
            return false;
        }
        match position {
            TypePosition::Plain => true,
            TypePosition::Cast(context) => self.marker_before_expression_suffix(context, true),
            TypePosition::Qualified | TypePosition::Result => {
                // `in` qualifies the reference only if a domain path follows. `in in D`
                // instead names the referenced type `in` and qualifies it with `D`.
                !self.nth_at_keyword(1, "in")
                    || !self.nth_at_word(2)
                    || (self.nth_at_keyword(2, "in") && self.nth_at_word(3))
                    || (position == TypePosition::Result && self.nth_can_start_clause(2))
            }
        }
    }

    pub(super) fn at_borrow_access_marker(
        &self,
        context: ExpressionContext,
    ) -> bool {
        self.at_access_word() && self.nth_can_start_expression(1) && self.marker_before_expression_suffix(context, false)
    }

    /// If the operand starts with `as` or `in`, that word could instead operate on
    /// the shared reference. In a run of those words, names and operators alternate:
    /// `@mut as as T` marks access, `@mut as as;` casts to a type named `as`.
    /// Inspect just that run and its right edge; never reparse an operand.
    fn marker_before_expression_suffix(
        &self,
        context: ExpressionContext,
        cast_carrier: bool,
    ) -> bool {
        let mut end = 1;
        while matches!(self.nth_word(end), Some(b"as" | b"in")) {
            end += 1;
        }
        if end == 1 {
            return true;
        }
        let last = end - 1;
        let rhs_follows = if self.nth_at_keyword(last, "as") {
            self.nth_can_start_type(end)
                || (matches!(self.nth_punctuation(end), Some(Punctuation::Plus | Punctuation::Minus | Punctuation::Equal)) && self.nth_at_word(end + 1))
        } else {
            // Also leave `in` for the enclosing bound in a termination measure.
            self.nth_can_start_expression(end)
        };
        if last % 2 == 0 {
            // With a marker, the last word would be an operator needing its RHS.
            return rhs_follows;
        }
        // With a marker, the last word would be a name. Keep that reading if it can
        // end here or continue (notably `@mut in(x)` and `@mut as + D`). Otherwise
        // the next operand would be stranded: `@mut as T` / `@write in S`.
        let type_name = if last == 1 { cast_carrier } else { self.nth_at_keyword(last - 1, "as") };
        !rhs_follows
            || (context.suffix() == ExpressionSuffix::Clause && self.nth_can_start_clause(end))
            || match self.nth_punctuation(end) {
                Some(Punctuation::LeftParenthesis | Punctuation::LeftBracket) => !type_name,
                Some(Punctuation::Plus | Punctuation::Minus) => true,
                Some(Punctuation::Equal) => type_name || context.suffix() == ExpressionSuffix::Assignment,
                _ => false,
            }
    }
}
