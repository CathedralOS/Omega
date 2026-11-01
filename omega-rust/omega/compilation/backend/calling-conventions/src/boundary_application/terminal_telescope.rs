//! Recover native argument order and callback binders from the canonical declaration.

use super::{
    BoundaryCallbackBinding, BoundaryCallingApplication, BoundaryNativeParameterGeometry,
    BoundaryNativeParameterShape,
};
use crate::BoundaryNativeParameterOrigin;
use terminal_psi::TerminalModule;

/// Target-closed argument telescope. Value layouts still come from the calling
/// application; declaration identity, native-only arguments and callback
/// correspondence come exclusively from Terminal Psi.
pub struct TerminalCallingTelescope {
    pub parameters: Vec<BoundaryNativeParameterGeometry>,
    pub binders: Vec<BoundaryCallbackBinding>,
}

impl TerminalCallingTelescope {
    /// Recover the declaration and check that the proposed ABI application is
    /// for that exact telescope before native consumers use its placements.
    pub fn for_application(
        module: &TerminalModule,
        boundary: semantic_vocabulary::BoundaryMachineId,
        application: &BoundaryCallingApplication,
    ) -> Result<Self, String> {
        // Several closed applications can share the same template requirement.
        // The actual boundary occurrence, not an inverse template-name search,
        // selects the retained declaration.
        let declaration = module
            .calling_signatures
            .iter()
            .find(|declaration| declaration.boundary == boundary)
            .ok_or("callback boundary has no Terminal calling declaration")?;
        if module.identities.text(declaration.requirement_identity)
            != Some(application.requirement_identity.as_str())
        {
            return Err("calling application names a different Terminal requirement".into());
        }
        let telescope = declaration.telescope.as_ref().map_err(Clone::clone)?;
        let boundary = module
            .boundary_machine(declaration.boundary)
            .ok_or("Terminal calling telescope lost its boundary declaration")?;
        if telescope.parameters.len() != boundary.parameter_order.len()
            || telescope.parameters.len() != application.signature.parameters.len()
        {
            return Err(
                "Terminal calling telescope differs from the semantic parameter roster".into(),
            );
        }
        let mut parameters =
            Vec::with_capacity(telescope.parameters.len() + telescope.direct_callbacks.len());
        for (ordinal, (formal, &shape)) in telescope
            .parameters
            .iter()
            .zip(&application.signature.parameters)
            .enumerate()
        {
            parameters.push(BoundaryNativeParameterGeometry {
                identity: formal.identity,
                native_ordinal: formal.native_ordinal,
                shape: BoundaryNativeParameterShape::Semantic(shape),
                origin: BoundaryNativeParameterOrigin::SemanticFormal {
                    formal_ordinal: u32::try_from(ordinal)
                        .map_err(|_| "Terminal semantic parameter ordinal exceeds u32")?,
                },
            });
        }
        for callback in &telescope.direct_callbacks {
            parameters.push(BoundaryNativeParameterGeometry {
                identity: callback.identity,
                native_ordinal: callback.native_ordinal,
                shape: BoundaryNativeParameterShape::function_pointer(application.target)?,
                origin: BoundaryNativeParameterOrigin::PrivateCallback {
                    binder: callback.binder,
                    requirement: callback.requirement,
                },
            });
        }
        parameters.sort_unstable_by_key(|parameter| parameter.native_ordinal);
        let proposed = application
            .signature
            .native_parameters
            .span(application.signature.native_parameter_rows)
            .ok_or("calling application has stale native parameter storage")?;
        if parameters != proposed {
            return Err(
                "calling application native parameters differ from the Terminal telescope".into(),
            );
        }
        let binders = telescope
            .binders
            .iter()
            .map(|binder| {
                Ok(BoundaryCallbackBinding {
                    binder: binder.binder,
                    requirement: binder.requirement,
                    static_machine_ordinal: binder.static_machine_ordinal,
                    requirement_identity: module
                        .identities
                        .text(binder.requirement_identity)
                        .ok_or("Terminal callback binder lost its exact requirement identity")?
                        .to_owned(),
                    invokes_synchronously: binder.invokes_synchronously,
                })
            })
            .collect::<Result<Vec<_>, String>>()?;
        let proposed = application
            .signature
            .callback_bindings
            .span(application.signature.callback_binding_rows)
            .ok_or("calling application has stale callback binder storage")?;
        if binders != proposed {
            return Err(
                "calling application callback binders differ from the Terminal telescope".into(),
            );
        }
        Ok(Self {
            parameters,
            binders,
        })
    }
}

impl BoundaryNativeParameterShape {
    /// The currently admitted native targets represent a function pointer by
    /// one 64-bit scalar. Refuse new pointer geometry until its ABI is admitted.
    pub fn function_pointer(target: target::NativeTarget) -> Result<Self, String> {
        if target.pointer_size != 8 || target.pointer_alignment != 8 {
            return Err(format!(
                "direct callback parameters currently require an admitted 8/8 target function-pointer shape, got {}/{}",
                target.pointer_size, target.pointer_alignment,
            ));
        }
        Ok(Self::TargetFunctionPointer {
            byte_size: 8,
            alignment: 8,
        })
    }
}
