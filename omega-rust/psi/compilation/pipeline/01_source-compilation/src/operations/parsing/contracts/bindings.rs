use crate::token_input::cursor::{Input, ParseResult};
use syntax_trees::item::ExternalBinding;
use tokens::PunctuationKind;

pub(crate) fn parse_external_provider_binding<'tokens, 'source>(
    input: Input<'tokens, 'source>,
) -> ParseResult<'tokens, 'source, ExternalBinding> {
    let start = input;
    let (root, input) = input.take_identifier()?;
    if root.as_str(&input) != "ForeignBinding" {
        return Err(start.error_here(
            "an external realization must construct the compiler-known ForeignBinding sum; \
             write `via ForeignBinding::Syscall(n)` or another qualified `ForeignBinding::Case`",
        ));
    }
    let input = input.take_punctuation(PunctuationKind::ColonColon, "::")?;
    parse_provider_binding_case(input)
}

fn parse_provider_binding_case<'tokens, 'source>(
    input: Input<'tokens, 'source>,
) -> ParseResult<'tokens, 'source, ExternalBinding> {
    let (case, input) = input.take_identifier()?;
    match case.as_str(&input) {
        "Syscall" => {
            let input = input.take_punctuation(PunctuationKind::LeftParen, "(")?;
            let (number, input) = input.take_integer()?;
            let input = input.take_punctuation(PunctuationKind::RightParen, ")")?;
            Ok((ExternalBinding::Syscall { number }, input))
        }
        "VtableSlot" => Err(input.error_here(
            "`ForeignBinding::VtableSlot` is retired; declare the foreign table layout and use \
                 `ForeignBinding::VtableField(field)`",
        )),
        "DllImport" => Err(input.error_here(
            "`ForeignBinding::DllImport(\"module\", \"symbol\")` is retired; return the \
             compiler-owned `ForeignBinding::DllImport { import: DllImport::Case { .. } }` \
             value from one `via` producer machine instead",
        )),
        "CompilerIntrinsic" => Ok((ExternalBinding::CompilerIntrinsic, input)),
        "TableFunction" => {
            let input = input.take_punctuation(PunctuationKind::LeftParen, "(")?;
            let (field, input) = input.take_identifier()?;
            let input = input.take_punctuation(PunctuationKind::RightParen, ")")?;
            Ok((ExternalBinding::TableFunction { field }, input))
        }
        "VtableField" => {
            let input = input.take_punctuation(PunctuationKind::LeftParen, "(")?;
            let (field, input) = input.take_identifier()?;
            let input = input.take_punctuation(PunctuationKind::RightParen, ")")?;
            Ok((ExternalBinding::VtableField { field }, input))
        }
        other => Err(input.error_here(format!(
            "unknown ForeignBinding case `{other}`: external leaves require one of \
             `ForeignBinding::Syscall(n)`, \
             `ForeignBinding::CompilerIntrinsic`, \
             `ForeignBinding::VtableField(field)`, or `ForeignBinding::TableFunction(field)`; \
             imports evaluate a `ForeignBinding::DllImport {{ .. }}` producer through `via`"
        ))),
    }
}
