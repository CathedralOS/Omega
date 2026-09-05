//! Translate the exact authored native telescope without serializing its IDs.

use super::rejected;
use crate::record::{
    PackagePolicyCallbacks, PackagePolicyCallingParameter, PackagePolicyNativeParameter,
    PackagePolicyNativeParameterOrigin,
};
use omega_calling_conventions::nominal_callback_native_parameter_id;
use omega_provider_planning::calling_policy_plans::{
    BoundaryNativeParameterOrigin, BoundaryNativeParameterShape, MaterializedBoundarySignature,
};
use psi_diagnostics::Diagnostic;

pub(super) fn project(
    signature: &MaterializedBoundarySignature,
    semantic: &[PackagePolicyCallingParameter],
    callbacks: &PackagePolicyCallbacks,
) -> Result<Vec<PackagePolicyNativeParameter>, Vec<Diagnostic>> {
    let mut rows = Vec::new();
    let mut formals = Vec::new();
    let mut private_count = 0;
    for (ordinal, parameter) in signature.native_parameters().iter().enumerate() {
        if parameter.native_ordinal() as usize != ordinal
            || signature.native_parameters()[..ordinal]
                .iter()
                .any(|prior| prior.identity() == parameter.identity())
        {
            return Err(rejected(
                "repeated native identity or changed native ordinal",
            ));
        }
        let (name, origin) = match (parameter.origin(), parameter.shape()) {
            (
                BoundaryNativeParameterOrigin::SemanticFormal { formal_ordinal },
                BoundaryNativeParameterShape::Semantic(shape_root),
            ) => {
                let formal = semantic
                    .get(formal_ordinal as usize)
                    .ok_or_else(|| rejected("missing semantic native formal"))?;
                if formal.shape_root != shape_root || formals.contains(&formal_ordinal) {
                    return Err(rejected("repeated or changed semantic native formal"));
                }
                formals.push(formal_ordinal);
                (
                    formal.name.clone(),
                    PackagePolicyNativeParameterOrigin::SemanticFormal {
                        formal_ordinal,
                        shape_root,
                    },
                )
            }
            (
                BoundaryNativeParameterOrigin::PrivateCallback {
                    binder,
                    requirement,
                },
                BoundaryNativeParameterShape::TargetFunctionPointer {
                    byte_size,
                    alignment,
                },
            ) => {
                let direct = signature
                    .direct_callback_parameters()
                    .iter()
                    .filter(|direct| {
                        direct.identity() == parameter.identity()
                            && direct.native_ordinal() == parameter.native_ordinal()
                            && direct.binder() == binder
                            && direct.requirement() == requirement
                    })
                    .collect::<Vec<_>>();
                let [direct] = direct.as_slice() else {
                    return Err(rejected("missing exact direct callback declaration"));
                };
                let binders = signature
                    .callback_binders()
                    .iter()
                    .filter(|candidate| {
                        candidate.binder == binder && candidate.requirement == requirement
                    })
                    .collect::<Vec<_>>();
                let [binder] = binders.as_slice() else {
                    return Err(rejected("ambiguous direct callback binder"));
                };
                let projected = callbacks
                    .binders
                    .iter()
                    .enumerate()
                    .filter(|(_, candidate)| {
                        candidate.static_machine_ordinal == binder.static_machine_ordinal
                    })
                    .collect::<Vec<_>>();
                let [(index, _)] = projected.as_slice() else {
                    return Err(rejected("missing canonical direct callback binder"));
                };
                private_count += 1;
                (
                    direct.name().to_owned(),
                    PackagePolicyNativeParameterOrigin::PrivateCallback {
                        binder_index: u32::try_from(*index)
                            .map_err(|_| rejected("callback binder index exceeds u32"))?,
                        byte_size,
                        alignment,
                    },
                )
            }
            _ => return Err(rejected("native shape and origin disagree")),
        };
        if nominal_callback_native_parameter_id(signature.owner_requirement_identity(), &name)
            != parameter.identity()
            || rows
                .iter()
                .any(|prior: &PackagePolicyNativeParameter| prior.name == name)
        {
            return Err(rejected("native parameter lost its exact authored name"));
        }
        rows.push(PackagePolicyNativeParameter { name, origin });
    }
    if formals.len() != semantic.len()
        || private_count != signature.direct_callback_parameters().len()
    {
        return Err(rejected(
            "native telescope omits a semantic or private parameter",
        ));
    }
    Ok(rows)
}
