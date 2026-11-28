//! Settle only reached, source-authenticated activation boundaries. Case and
//! field positions come from the same target layout that realizes the caller.
use crate::{
    AdmissionSource, AdmittedBoundaryExecution, AdmittedBoundarySettlement,
    AdmittedTerminalMechanism, SettledCompilerBuiltin, TerminalAuthorityPolicy,
};
use calling_conventions::{CallSignature, CallingPolicy, ValueShape};
use diagnostics::Diagnostic;
use language_core::sealed_intrinsics::ActivationOperation;
use legalized_operations::native_geometry::NativeGeometry;
use semantic_vocabulary::{StructuralCaseId, StructuralFieldId, StructuralTypeId};
use target_operations::{
    BoundarySettlementRealization, FixedActivationAction, FixedActivationCall, FixedCase,
    FixedField, NormalizedForeignCallBinding, NormalizedForeignCallMechanism,
};
use terminal_psi::{
    BoundaryMachineDeclaration, BoundaryMachineResult, BoundaryParameterKind,
    CheckedBoundaryExecution, TerminalModule,
};
use terminal_semantics::calls::owned_activation;

pub(super) fn settle<'a>(
    input: &AdmissionSource,
    profile: target::TargetProfile,
    policy: &TerminalAuthorityPolicy,
) -> Result<
    (
        Vec<AdmittedBoundarySettlement<'a>>,
        Vec<AdmittedTerminalMechanism>,
    ),
    Vec<Diagnostic>,
> {
    let catalog = &input.plan().structural_types;
    let mut geometry = None;
    let mut settled = Vec::new();
    let mut mechanisms = Vec::new();
    let mut reached = std::collections::BTreeSet::new();
    for op in input.plan().operations() {
        let abstract_operations::AbstractOperation::BoundaryCall { boundary, .. } = op else {
            continue;
        };
        if !reached.insert(*boundary) {
            continue;
        }
        let Some(decl) = input.module().boundary_machine(*boundary) else {
            continue;
        };
        let Some(CheckedBoundaryExecution::OwnedActivation(operation)) =
            decl.declaration.as_ref().map(|row| row.execution)
        else {
            continue;
        };
        let fail = |reason: &str| {
            vec![Diagnostic::error(format!(
                "native fixed activation {operation:?} at {boundary:?}: {reason}"
            ))]
        };
        if profile != target::TargetProfile::LinuxX64 {
            return Err(fail(
                "caller-funded activation transfer requires Linux x86-64 System V",
            ));
        }
        let geometry = match geometry.as_ref() {
            Some(layout) => layout,
            None => geometry.insert(
                NativeGeometry::with_value_storage(
                    catalog.as_slice(),
                    catalog.value_storage(),
                    profile.native_target(),
                )
                .map_err(|_| {
                    fail("activation storage catalog has duplicate or missing identities")
                })?,
            ),
        };
        let signature = signature(decl, geometry).map_err(fail)?;
        let plan = calling_conventions::evaluate_ordinary_boundary_entry_plan(
            CallingPolicy::SystemVAMD64,
            &signature,
        )
        .map_err(|_| fail("activation call has no ordinary System V transport"))?;
        let action = action(input.module(), decl, operation, &signature, geometry).map_err(fail)?;
        let mechanism =
            native_intrinsics::CompilerIntrinsicExecutionIdentity::FixedActivation(operation);
        policy
            .classify(mechanism)
            .map_err(|_| fail("compiler mechanism has no receiving classification"))?;
        let builtin = SettledCompilerBuiltin::from_identity(mechanism, *boundary, None)
            .ok_or_else(|| fail("compiler execution catalog does not describe this operation"))?;
        settled.push(AdmittedBoundarySettlement {
            boundary: *boundary,
            execution: AdmittedBoundaryExecution::CompilerBuiltin(builtin),
            realization: BoundarySettlementRealization::NormalizedForeignCall(
                NormalizedForeignCallBinding {
                    boundary_entry_plan: plan.plan().clone(),
                    mechanism: NormalizedForeignCallMechanism::FixedActivation(
                        FixedActivationCall {
                            boundary: *boundary,
                            action,
                        },
                    ),
                },
            ),
        });
        mechanisms.push(AdmittedTerminalMechanism {
            boundary: *boundary,
            mechanism: mechanism.into(),
        });
    }
    Ok((settled, mechanisms))
}

fn signature(
    boundary: &BoundaryMachineDeclaration,
    geometry: &NativeGeometry<'_>,
) -> Result<CallSignature, &'static str> {
    let mut scalars = boundary.scalar_parameters.iter();
    let mut structures = boundary.structural_parameters.iter();
    let mut parameters = Vec::with_capacity(boundary.parameter_order.len());
    for kind in &boundary.parameter_order {
        parameters.push(
            match kind {
                BoundaryParameterKind::Scalar => {
                    scalars.next().and_then(|s| ValueShape::for_scalar(*s))
                }
                BoundaryParameterKind::Structural => structures.next().and_then(|s| {
                    legalized_operations::structural_inputs::parameter_shape(s, geometry)
                }),
            }
            .ok_or("activation parameter has no physical carrier")?,
        );
    }
    if scalars.next().is_some() || structures.next().is_some() {
        return Err("activation parameter order is incomplete");
    }
    let result = match &boundary.result {
        BoundaryMachineResult::Unit => None,
        BoundaryMachineResult::Scalar(s) => {
            Some(ValueShape::for_scalar(s.scalar_type).ok_or("unclosed activation scalar result")?)
        }
        BoundaryMachineResult::Structural(s) => Some(
            geometry
                .shape(s.structural_type)
                .map_err(|_| "unclosed activation structural result")?,
        ),
    };
    Ok(CallSignature { parameters, result })
}

fn action(
    module: &TerminalModule,
    boundary: &BoundaryMachineDeclaration,
    operation: ActivationOperation,
    sig: &CallSignature,
    geometry: &NativeGeometry<'_>,
) -> Result<FixedActivationAction, &'static str> {
    let tag = |root| {
        let layout = geometry
            .sum_layout(root)
            .map_err(|_| "activation result has no native sum layout")?;
        Ok::<_, &'static str>(FixedField {
            offset: layout.tag_byte_offset,
            bytes: layout.tag_shape.byte_size,
        })
    };
    let anchor = |index: usize, id| {
        let root = boundary
            .structural_parameters
            .get(index)
            .ok_or("activation anchor parameter is missing")?
            .structural_type;
        let field = geometry
            .declaration(root)
            .map_err(|_| "missing publication or role")?
            .shape
            .fields()
            .find(|f| f.id == id)
            .ok_or("activation role has no checked anchor field")?;
        field_at(geometry, root, field, 0)
    };
    use FixedActivationAction as A;
    Ok(match operation {
        ActivationOperation::Prepare => {
            let s = owned_activation::prepare_shape(module, boundary)?;
            A::Prepare {
                worker: s.selected.entry_machine,
                tag: tag(s.result)?,
                ready: case(geometry, s.result, s.ready, [s.owner])?,
                rejected: case(geometry, s.result, s.rejected, [s.arguments, s.retained])?,
            }
        }
        ActivationOperation::StepFixed => {
            let s = owned_activation::step_shape(
                module,
                boundary,
                owned_activation::preparation_for(module, boundary)?,
            )?;
            A::Step {
                worker: s.selected.entry_machine,
                tag: tag(s.result)?,
                returned: case(geometry, s.result, s.returned, [s.value, s.retained])?,
                parked: case(geometry, s.result, s.parked, [s.owner])?,
            }
        }
        ActivationOperation::Park => {
            let s = owned_activation::park_shape(module, boundary)?;
            A::Park {
                anchor: anchor(0, s.anchor)?,
                reply_type: s.reply,
                reply_bytes: sig.result.map_or(0, |shape| shape.byte_size),
            }
        }
        ActivationOperation::Resume => {
            let s = owned_activation::resume_shape(
                module,
                boundary,
                owned_activation::preparation_for(module, boundary)?,
            )?;
            A::Resume {
                worker: s.selected.entry_machine,
                anchor: anchor(1, s.permit.anchor)?,
                reply_type: s.permit.reply,
                reply_bytes: sig
                    .parameters
                    .get(2)
                    .ok_or("missing activation reply")?
                    .byte_size,
                tag: tag(s.result)?,
                ready: case(geometry, s.result, s.ready, [s.ready_field])?,
                mismatch: case(
                    geometry,
                    s.result,
                    s.mismatch,
                    [s.mismatch_owner, s.mismatch_permit, s.mismatch_value],
                )?,
            }
        }
    })
}

fn field_at(
    geometry: &NativeGeometry<'_>,
    root: StructuralTypeId,
    field: &terminal_psi::StructuralFieldDeclaration,
    conventional: u32,
) -> Result<FixedField, &'static str> {
    let location = geometry
        .field_placement(field, root, conventional)
        .map_err(|_| "activation field has no target storage")?;
    Ok(FixedField {
        offset: location.byte_offset,
        bytes: location.shape.byte_size,
    })
}

fn case<const N: usize>(
    geometry: &NativeGeometry<'_>,
    root: StructuralTypeId,
    id: StructuralCaseId,
    roles: [StructuralFieldId; N],
) -> Result<FixedCase<N>, &'static str> {
    let native = geometry
        .sum_layout(root)
        .map_err(|_| "activation result has no native layout")?;
    let (ordinal, case) = geometry
        .declaration(root)
        .map_err(|_| "activation result is absent")?
        .shape
        .cases()
        .iter()
        .enumerate()
        .find(|(_, case)| case.id == id)
        .ok_or("result has no selected case")?;
    let fields = &native
        .cases
        .get(ordinal)
        .ok_or("native layout has no selected case")?
        .fields;
    let mut places = [FixedField {
        offset: 0,
        bytes: 0,
    }; N];
    for (slot, role) in places.iter_mut().zip(roles) {
        let (field, packed) = case
            .fields
            .iter()
            .zip(fields)
            .find(|(field, _)| field.id == role)
            .ok_or("case has no selected field")?;
        *slot = field_at(geometry, root, field, packed.byte_offset)?;
    }
    Ok(FixedCase {
        tag: u32::try_from(ordinal).map_err(|_| "case tag exceeds native width")?,
        fields: places,
    })
}
