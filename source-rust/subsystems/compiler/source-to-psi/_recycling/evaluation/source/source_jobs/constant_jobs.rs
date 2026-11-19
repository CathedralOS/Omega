use crate::source_state::TypedTrees;
use crate::evaluation::{SourceCompletionAccounting, evaluate_constant_initializers_with_source_consent as evaluate_with_source_consent};
use diagnostics::Diagnostic;
use symbols::SymbolHandle;

pub fn evaluate(
    program: &mut TypedTrees,
    request: crate::production::TerminalSourceCompilation<'_>,
    authority: Option<std::sync::Arc<dyn crate::evaluation::BuildTimeSelectionAuthority>>,
    accounting: &mut SourceCompletionAccounting,
) -> Result<Vec<SymbolHandle>, Vec<Diagnostic>> {
    evaluate_with_source_consent(program, request, &[], authority, None, accounting)
}
