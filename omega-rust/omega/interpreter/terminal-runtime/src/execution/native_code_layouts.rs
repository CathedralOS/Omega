//! A reference execution may inspect its explicitly retained completed image.
//! The native owner checks final instruction attribution and relocations. This
//! adapter preserves that image's identity and linked coordinate space without
//! guessing a process load slide or requesting another compilation.

use super::{TerminalRuntimeError, TerminalRuntimeRequest};
use terminal_interpreter::{
    TerminalNativeCodeLayout, TerminalNativeCodeLayoutEnvironment, TerminalNativeCodeRegion,
};

pub(super) fn inspect(
    request: &TerminalRuntimeRequest<'_>,
    semantic: terminal_psi::TerminalPsiIdentity,
    execution_target: Option<&str>,
) -> Result<Option<TerminalNativeCodeLayoutEnvironment>, TerminalRuntimeError> {
    let Some(image) = request.native_image else {
        return Ok(None);
    };
    let Some(target) = execution_target else {
        return Err(TerminalRuntimeError::InvalidProvision(
            "native layout inspection requires the completed image's target",
        ));
    };
    let selected_target =
        target::NativeTarget::from_omega_target_name(Some(target)).map_err(|diagnostic| {
            TerminalRuntimeError::missing("native-code-layout-target", diagnostic.message)
        })?;
    if image.psi_artifact().manifest().identity() != request.artifact.manifest().identity()
        || image.target() != selected_target
    {
        return Err(TerminalRuntimeError::InvalidProvision(
            "native layout inspection names another artifact or target",
        ));
    }
    let descriptions = image.inspect_native_code_layouts().map_err(|diagnostic| {
        TerminalRuntimeError::missing("native-code-layout", diagnostic.message)
    })?;
    // Private object machine numbers are local to their own semantic artifacts.
    // They can supply a component only when that component has its own exact
    // completed-image input; they are never aliases of this root's rows.
    let layouts = descriptions
        .into_iter()
        .filter(|layout| layout.private_function.is_none() && layout.terminal_psi == semantic)
        .map(|layout| TerminalNativeCodeLayout {
            machine: layout.machine,
            entry: layout.entry,
            regions: layout
                .regions
                .into_iter()
                .map(|region| TerminalNativeCodeRegion {
                    base: region.base,
                    length: region.length,
                })
                .collect(),
        })
        .collect();
    TerminalNativeCodeLayoutEnvironment::linked_image(
        semantic,
        target,
        *image.identity().as_bytes(),
        layouts,
    )
    .map(Some)
    .map_err(TerminalRuntimeError::Requirements)
}
