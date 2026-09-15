use crate::parser::input::{Input, ParseResult};
use syntax_trees::item::ExternalBinding;
use tokens::PunctuationKind;

/// Parse the external-realization spelling used after `via`.
///
/// The leaf names the closed compiler-known sum explicitly as
/// `Binding::Case(...)`, so a package-local declaration cannot masquerade as
/// compiler binding data.
pub(crate) fn parse_external_provider_binding<'tokens, 'source>(
    input: Input<'tokens, 'source>,
) -> ParseResult<'tokens, 'source, ExternalBinding> {
    let start = input;
    let (root, input) = input.take_identifier()?;
    if root.as_str() != "Binding" {
        return Err(start.error_here(
            "an external realization must construct the compiler-known Binding sum; \
             write `via Binding::DllImport(...)` or another qualified `Binding::Case`",
        ));
    }
    let input = input.take_punctuation(PunctuationKind::ColonColon, "::")?;
    parse_provider_binding_case(input)
}

fn parse_provider_binding_case<'tokens, 'source>(
    input: Input<'tokens, 'source>,
) -> ParseResult<'tokens, 'source, ExternalBinding> {
    let (case, input) = input.take_identifier()?;
    match case.as_str() {
        "Syscall" => {
            let input = input.take_punctuation(PunctuationKind::LeftParen, "(")?;
            let (number, input) = input.take_integer()?;
            let input = input.take_punctuation(PunctuationKind::RightParen, ")")?;
            Ok((ExternalBinding::Syscall { number }, input))
        }
        "VtableSlot" => Err(input.error_here(
            "`Binding::VtableSlot` is retired; declare the foreign table layout and use \
                 `Binding::VtableField(field)`",
        )),
        "DllImport" => {
            let input = input.take_punctuation(PunctuationKind::LeftParen, "(")?;
            let (module, input) = input.take_string()?;
            let input = input.take_punctuation(PunctuationKind::Comma, ",")?;
            let (symbol, input) = input.take_string()?;
            let input = input.take_punctuation(PunctuationKind::RightParen, ")")?;
            Ok((ExternalBinding::DllImport { module, symbol }, input))
        }
        "CompilerIntrinsic" => Ok((ExternalBinding::CompilerIntrinsic, input)),
        // A service-table function: dispatch through the `over` struct's
        // fn-ptr FIELD like a bare-field arm, but the table pointer is
        // dispatch-only -- never a wire argument (EFI table services take
        // no This; protocol/COM methods do).
        "TableFunction" => {
            let input = input.take_punctuation(PunctuationKind::LeftParen, "(")?;
            let (field, input) = input.take_identifier()?;
            let input = input.take_punctuation(PunctuationKind::RightParen, ")")?;
            Ok((ExternalBinding::TableFunction { field }, input))
        }
        // The qualified external-leaf spelling cannot use the legacy bare
        // field shorthand because `Binding::field` would look like an open
        // sum. Keep the normalized binding case explicit.
        "VtableField" => {
            let input = input.take_punctuation(PunctuationKind::LeftParen, "(")?;
            let (field, input) = input.take_identifier()?;
            let input = input.take_punctuation(PunctuationKind::RightParen, ")")?;
            Ok((ExternalBinding::VtableField { field }, input))
        }
        other => Err(input.error_here(format!(
            "unknown Binding case `{other}`: external leaves require one of \
             `Binding::Syscall(n)`, `Binding::DllImport(\"module\", \"symbol\")`, \
             `Binding::CompilerIntrinsic`, \
             `Binding::VtableField(field)`, or `Binding::TableFunction(field)`"
        ))),
    }
}
