use crate::program::Lowerer;
use crate::state::lower_state_signature;
use omega_core::diagnostics::Diagnostic;
use omega_resolved_trees as resolved;
use omega_typed_trees as typed;

pub(crate) fn lower_platform(
    lowerer: &mut Lowerer,
    platform: &resolved::platform::Platform,
) -> Result<typed::platform::Platform, Diagnostic> {
    let mut typed_platform = typed::platform::Platform {
        symbol: platform.symbol,
        name: crate::name::lower_name(&platform.name),
        states: Vec::new(),
    };

    for signature in lowerer.source_program.platform_state_signatures(platform.states) {
        typed_platform
            .states
            .push(lower_state_signature(lowerer, signature)?);
    }

    Ok(typed_platform)
}
