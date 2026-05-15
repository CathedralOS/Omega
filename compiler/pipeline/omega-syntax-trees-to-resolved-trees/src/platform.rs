use crate::program::Lowerer;
use crate::state::lower_state_signature_node;
use omega_core::arena::HandleSpan;
use omega_core::diagnostics::Diagnostic;
use omega_core::symbols::SymbolHandle;
use omega_resolved_trees::platform::{Platform, PlatformStorage};
use omega_resolved_trees::signature::StateSignature;
use omega_syntax_trees::{self as syntax, SyntaxTrees};

pub(crate) fn lower_platform(
    lowerer: &mut Lowerer,
    syntax_trees: &SyntaxTrees,
    platform: &syntax::item::Platform,
) -> Result<Platform, Diagnostic> {
    let states = lower_platform_state_signatures(lowerer, syntax_trees, platform.states)?;

    Ok(Platform {
        symbol: SymbolHandle::invalid(),
        name: crate::name::lower_name(&platform.name),
        storage: PlatformStorage { states },
    })
}

fn lower_platform_state_signatures(
    lowerer: &mut Lowerer,
    syntax_trees: &SyntaxTrees,
    states: HandleSpan<syntax::item::StateSignatureHandle>,
) -> Result<HandleSpan<StateSignature>, Diagnostic> {
    let mut span = HandleSpan::empty();

    for signature in syntax_trees.items.state_signatures(states) {
        let signature = lower_state_signature_node(
            lowerer,
            syntax_trees,
            syntax_trees.items.state_signature(*signature),
        )?;
        lowerer
            .program
            .tables
            .declarations
            .platform_state_signatures
            .append_to_span(&mut span, signature);
    }

    Ok(span)
}
