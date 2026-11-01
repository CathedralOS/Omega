//! Replay exact declaration ABI applications without reopening source.

use crate::codec_error::{CodecError, malformed};
use terminal_psi::{
    BoundaryMachineResult, BoundaryParameterKind, CallingSignatureTelescope, StructuralAccess,
    StructuralTypeShape, TerminalModule,
};

pub(crate) fn validate(module: &TerminalModule) -> Result<(), CodecError> {
    super::opaque_representations::validate(module)?;
    validate_census(module)?;
    super::super::super::wire::require_increasing(
        module.calling_signatures.iter().map(|row| row.boundary),
        "calling signatures by boundary declaration",
    )?;
    for binding in &module.calling_signatures {
        let boundary = module.boundary_machine(binding.boundary).ok_or(
            CodecError::MalformedStructuralFoundation("calling signature lost its exact boundary"),
        )?;
        if boundary.declaration.is_none() {
            return malformed("calling signature lacks its checked declaration");
        }
        let application = match &binding.application {
            Ok(application) => *application,
            Err(reason) if !reason.is_empty() => continue,
            Err(_) => return malformed("calling signature has an empty deferred refusal"),
        };
        let declaration = module.calling_abi_signature(application).ok_or(
            CodecError::MalformedStructuralFoundation(
                "calling signature lost its declaration ABI application",
            ),
        )?;
        let Ok(storage) = &declaration.storage else {
            continue;
        };
        if storage.parameters.len() != boundary.parameter_order.len() {
            return malformed("calling telescope differs from its exact boundary parameter roster");
        }
        let mut scalars = boundary.scalar_parameters.iter();
        let mut structural = boundary.structural_parameters.iter();
        for (kind, formal) in boundary.parameter_order.iter().zip(&storage.parameters) {
            match kind {
                BoundaryParameterKind::Scalar => {
                    let scalar =
                        scalars
                            .next()
                            .ok_or(CodecError::MalformedStructuralFoundation(
                                "calling signature lost a scalar formal",
                            ))?;
                    if formal.access != StructuralAccess::Owned
                        || !matches!(module.structural_type(formal.structural_type).map(|row| &row.shape), Some(StructuralTypeShape::PrimitiveScalar(actual)) if actual == scalar)
                    {
                        return malformed("calling signature substitutes a scalar carrier");
                    }
                }
                BoundaryParameterKind::Structural => {
                    let parameter =
                        structural
                            .next()
                            .ok_or(CodecError::MalformedStructuralFoundation(
                                "calling signature lost a structural formal",
                            ))?;
                    if formal.structural_type != parameter.structural_type
                        || formal.access != parameter.access
                    {
                        return malformed(
                            "calling signature substitutes a structural carrier or access",
                        );
                    }
                }
            }
        }
        match (&boundary.result, storage.result) {
            (BoundaryMachineResult::Unit, None) => {}
            (BoundaryMachineResult::Scalar(result), Some(identity)) if matches!(module.structural_type(identity).map(|row| &row.shape), Some(StructuralTypeShape::PrimitiveScalar(actual)) if *actual == result.scalar_type) =>
                {}
            (BoundaryMachineResult::Structural(result), Some(identity))
                if identity == result.structural_type => {}
            _ => return malformed("calling signature substitutes its exact result carrier"),
        }
    }
    Ok(())
}

fn validate_census(module: &TerminalModule) -> Result<(), CodecError> {
    super::super::super::wire::require_increasing(
        module.calling_abi_signatures.iter().map(|row| row.id),
        "declaration ABI applications",
    )?;
    for row in &module.calling_abi_signatures {
        let requirement = module
            .identities
            .text(row.requirement_identity)
            .filter(|text| !text.is_empty())
            .ok_or(CodecError::MalformedStructuralFoundation(
                "declaration ABI lost its exact requirement identity",
            ))?;
        let application = module.identities.text(row.declaring_application).ok_or(
            CodecError::MalformedStructuralFoundation(
                "declaration ABI lost its declaring application",
            ),
        )?;
        let mut operands = Vec::with_capacity(row.operands.len());
        for operand in &row.operands {
            let identity = module
                .identities
                .text(operand.identity)
                .filter(|text| !text.is_empty())
                .ok_or(CodecError::MalformedStructuralFoundation(
                    "declaration ABI lost a static operand",
                ))?;
            operands.push((operand.ordinal, operand.kind, identity));
        }
        if row
            .operands
            .windows(2)
            .any(|pair| pair[0].ordinal >= pair[1].ordinal)
            || semantic_vocabulary::CallingSignatureId::for_application(
                requirement,
                application,
                operands,
            ) != row.id
        {
            return malformed(
                "declaration ABI application key differs from its complete static operands",
            );
        }
        for (ordinal, formal) in row.formals.iter().enumerate() {
            if usize::try_from(formal.ordinal).ok() != Some(ordinal)
                || formal.callback.is_some()
                    && formal.kind != semantic_vocabulary::CallingOperandKind::Machine
            {
                return malformed(
                    "declaration ABI generic formal roster is not declaration ordered",
                );
            }
        }
        let storage = match &row.storage {
            Ok(storage) => storage,
            Err(reason) if !reason.is_empty() => continue,
            Err(_) => return malformed("declaration ABI has an empty deferred refusal"),
        };
        if application.is_empty()
            || storage.parameters.len() != storage.telescope.parameters.len()
            || storage.parameters.iter().any(|parameter| {
                module.structural_type(parameter.structural_type).is_none()
                    && !(parameter.access != StructuralAccess::Owned
                        && parameter.callback_backing.is_none()
                        && module
                            .foreign_reference_descriptor(parameter.structural_type)
                            .is_some())
            })
            || storage
                .result
                .is_some_and(|identity| module.structural_type(identity).is_none())
        {
            return malformed("declaration ABI storage lost an exact type or formal");
        }
        for parameter in &storage.parameters {
            let backing = parameter.structural_type;
            match parameter.callback_backing {
                Some(actual) if actual == backing && module.structural_type(actual).is_some() => {}
                None if parameter.access != StructuralAccess::Owned
                    && module.structural_type(backing).is_none()
                    && module
                        .structural_referents
                        .iter()
                        .any(|row| row.id == backing && row.abstract_opaque) => {}
                _ => {
                    return malformed(
                        "declaration ABI substitutes or omits its complete callback backing",
                    );
                }
            }
        }
        let required = row
            .formals
            .iter()
            .filter(|formal| formal.callback.is_none())
            .collect::<Vec<_>>();
        if required.len() != row.operands.len()
            || required.iter().zip(&row.operands).any(|(formal, actual)| {
                formal.ordinal != actual.ordinal || formal.kind != actual.kind
            })
        {
            return malformed("declaration ABI omits or substitutes a non-callback static operand");
        }
        for (machine_ordinal, formal) in row
            .formals
            .iter()
            .filter(|formal| formal.kind == semantic_vocabulary::CallingOperandKind::Machine)
            .enumerate()
        {
            let Some(callback) = formal.callback else {
                continue;
            };
            if storage
                .telescope
                .binders
                .iter()
                .filter(|binder| binder.binder == callback)
                .count()
                != 1
            {
                return malformed(
                    "declaration ABI generic callback formal lost its universal binder",
                );
            }
            if storage.telescope.binders.iter().any(|binder| {
                binder.binder == callback
                    && usize::try_from(binder.static_machine_ordinal).ok() != Some(machine_ordinal)
            }) {
                return malformed(
                    "declaration ABI callback binder has the wrong declared machine ordinal",
                );
            }
        }
        for binder in &storage.telescope.binders {
            if row
                .formals
                .iter()
                .filter(|formal| formal.callback == Some(binder.binder))
                .count()
                != 1
            {
                return malformed(
                    "declaration ABI callback binder lost its declared generic formal",
                );
            }
        }
        validate_telescope(module, &storage.telescope)?;
    }
    for request in &module.calling_policy_requests {
        match &request.declaration {
            Ok(identity)
                if module
                    .identities
                    .text(*identity)
                    .is_some_and(|text| !text.is_empty()) => {}
            Err(reason) if !reason.is_empty() => {}
            _ => {
                return malformed(
                    "calling policy request lost its exact declaring owner or refusal",
                );
            }
        }
        let instances = match &request.instances {
            Ok(instances) => instances,
            Err(reasons)
                if !reasons.is_empty() && reasons.iter().all(|reason| !reason.is_empty()) =>
            {
                continue;
            }
            Err(_) => return malformed("calling policy request has an empty deferred refusal"),
        };
        for instance in instances {
            if module
                .identities
                .text(instance.declaring_application)
                .is_none_or(str::is_empty)
            {
                return malformed("calling policy instance lost its exact application");
            }
            match instance.policy {
                Ok(terminal_psi::TargetMachineCallable::Machine(identity))
                    if module.machine(identity).is_some() => {}
                Ok(terminal_psi::TargetMachineCallable::Boundary(identity))
                    if module.boundary_machine(identity).is_some() => {}
                Err(ref reason) if !reason.is_empty() => {}
                _ => {
                    return malformed("calling policy instance lost its exact callable or refusal");
                }
            }
            match &instance.signatures {
                Ok(signatures)
                    if signatures
                        .iter()
                        .all(|identity| module.calling_abi_signature(*identity).is_some()) => {}
                Err(reason) if !reason.is_empty() => {}
                _ => {
                    return malformed(
                        "calling policy instance lost its ordered declaration ABI applications",
                    );
                }
            }
        }
    }
    Ok(())
}

fn validate_telescope(
    module: &TerminalModule,
    telescope: &CallingSignatureTelescope,
) -> Result<(), CodecError> {
    let mut positions =
        Vec::with_capacity(telescope.parameters.len() + telescope.direct_callbacks.len());
    for formal in &telescope.parameters {
        positions.push((formal.native_ordinal, formal.identity));
    }
    if telescope
        .parameters
        .windows(2)
        .any(|pair| pair[0].native_ordinal >= pair[1].native_ordinal)
        || telescope
            .binders
            .windows(2)
            .any(|pair| pair[0].static_machine_ordinal >= pair[1].static_machine_ordinal)
    {
        return malformed("calling telescope lost its authored formal or binder order");
    }
    for (ordinal, binder) in telescope.binders.iter().enumerate() {
        if telescope.binders[..ordinal]
            .iter()
            .any(|prior| prior.binder == binder.binder)
        {
            return malformed("calling telescope repeats a nominal callback binder");
        }
        let identity = module
            .identities
            .text(binder.requirement_identity)
            .filter(|text| !text.is_empty())
            .ok_or(CodecError::MalformedStructuralFoundation(
                "callback binder lost its requirement identity",
            ))?;
        if semantic_vocabulary::callback_requirement_id(identity) != binder.requirement {
            return malformed("callback binder substitutes another requirement identity");
        }
    }
    for callback in &telescope.direct_callbacks {
        if !telescope.binders.iter().any(|binder| {
            binder.binder == callback.binder && binder.requirement == callback.requirement
        }) {
            return malformed("direct callback lost its exact nominal binder requirement");
        }
        positions.push((callback.native_ordinal, callback.identity));
    }
    positions.sort_by_key(|(position, _)| *position);
    for (ordinal, (position, identity)) in positions.iter().enumerate() {
        if usize::try_from(*position).ok() != Some(ordinal)
            || positions[..ordinal]
                .iter()
                .any(|(_, prior)| prior == identity)
        {
            return malformed(
                "calling telescope repeats or omits a native formal identity or position",
            );
        }
    }
    Ok(())
}
