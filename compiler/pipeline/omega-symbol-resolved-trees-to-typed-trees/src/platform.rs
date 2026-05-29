use crate::lowerer::Lowerer;
use crate::state::lower_state_signature;
use omega_core::diagnostics::Diagnostic;
use omega_symbol_resolved_trees as resolved;
use omega_typed_trees as typed;

pub(crate) fn lower_platform(
    lowerer: &mut Lowerer,
    platform: &resolved::platform::Platform,
) -> Result<typed::platform::Platform, Diagnostic> {
    let mut typed_platform = typed::platform::Platform {
        symbol: platform.symbol,
        name: crate::name::lower_name(&platform.name),
        states: omega_core::arena::HandleSpan::empty(),
    };

    for signature in lowerer
        .source_trees
        .platform_state_signatures(platform.states)
    {
        let signature = lower_state_signature(lowerer, signature)?;
        lowerer
            .typed_trees
            .push_platform_state_signature(&mut typed_platform, signature);
    }

    Ok(typed_platform)
}
