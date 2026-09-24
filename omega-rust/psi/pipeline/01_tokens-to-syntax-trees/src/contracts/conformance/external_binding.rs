use crate::input::token_cursor::{Input, ParseResult};
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
    if root.as_str() != "ForeignBinding" {
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
    match case.as_str() {
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
        // The string-backed `ForeignBinding::DllImport("module", "symbol")` bootstrap
        // is retired: raw foreign bytes never again become binding authority
        // through an authored magic spelling. Durable source evaluates the
        // compiler-owned `ForeignBinding` data sum through an ordinary producer
        // machine, so `via` remains one exact machine call whose result is a
        // typed locator value.
        "DllImport" => Err(input.error_here(
            "`ForeignBinding::DllImport(\"module\", \"symbol\")` is retired; return the \
             compiler-owned `ForeignBinding::DllImport { import: DllImport::Case { .. } }` \
             value from one `via` producer machine instead",
        )),
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
        // field shorthand because `ForeignBinding::field` would look like an open
        // sum. Keep the normalized binding case explicit.
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
