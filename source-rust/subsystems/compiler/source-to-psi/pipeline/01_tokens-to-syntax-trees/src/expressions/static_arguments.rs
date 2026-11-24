use crate::errors::parse_error::ParseError;
use crate::expressions::parse::parse_const_integer_expression_handle;
use crate::token_input::cursor::Input;
use syntax_trees::SyntaxTrees;
use syntax_trees::expression::ExpressionHandle;
use tokens::PunctuationKind;

/// A static argument written as a value: one constant integer expression
/// that ends at the `,` or `>` after it. Between `<` and `>` a `>` always
/// closes the list, so the expression is read once and never re-read with
/// its end moved; a right shift inside an argument is written in
/// parentheses.
pub(crate) fn parse_static_argument_expression<'tokens, 'source>(
    scratch: &mut SyntaxTrees,
    input: Input<'tokens, 'source>,
) -> Result<Option<(ExpressionHandle, Input<'tokens, 'source>)>, ParseError> {
    let attempt = scratch.append_checkpoint();
    match parse_const_integer_expression_handle(scratch, input) {
        Ok((expression, rest))
            if rest.at_punctuation(PunctuationKind::Comma)
                || rest.at_punctuation(PunctuationKind::Greater) =>
        {
            Ok(Some((expression, rest)))
        }
        _ => {
            scratch.rewind_appended(attempt);
            Ok(None)
        }
    }
}
