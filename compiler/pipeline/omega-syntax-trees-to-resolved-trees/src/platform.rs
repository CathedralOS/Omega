use crate::program::Lowerer;
use crate::state::lower_state_signature;
use omega_core::diagnostics::Diagnostic;
use omega_core::symbols::SymbolHandle;
use omega_resolved_trees::platform::Platform;
use omega_syntax_trees::{self as syntax, SyntaxTrees};

pub(crate) fn lower_platform(
    lowerer: &mut Lowerer,
    syntax_trees: &SyntaxTrees,
    platform: &syntax::item::Platform,
) -> Result<Platform, Diagnostic> {
    let states = platform
        .states
        .iter()
        .map(|signature| lower_state_signature(lowerer, syntax_trees, signature))
        .collect::<Result<Vec<_>, _>>()?;

    Ok(Platform {
        symbol: SymbolHandle::invalid(),
        name: crate::name::lower_name(&platform.name),
        states,
    })
}
