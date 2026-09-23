//! Borrow primitive referents from incoming pointers or established local storage.
use super::LiveDefinitions;
use crate::LoweringError;
use crate::lowering::function_signature::{PreparedFunctionSignature, prepare_function_signature};
use crate::lowering::structural_type_lookup::StructuralTypeLookup;
use abstract_operations::{AbstractFunction, AbstractFunctionResult, AbstractOperation};
use calling_conventions::ValueShape;
use semantic_vocabulary::MachineId;
use std::collections::{BTreeMap, BTreeSet};
use target::NativeTarget;
use target_operations::TargetStructuralArgumentSource;
use target_operations::{
    TargetStructuralArgument, TargetStructuralParameter, TargetUnitOperation,
    TargetUnitScalarCallArgument, TerminalPsiProvenance,
};
use terminal_psi::{StructuralAccess, StructuralMultiplicity, StructuralPathSegment};

#[allow(clippy::too_many_arguments)]
pub(super) fn lower(
    operation: &AbstractOperation,
    function: &AbstractFunction,
    target: NativeTarget,
    functions: &BTreeMap<MachineId, &AbstractFunction>,
    types: &StructuralTypeLookup<'_>,
    prepared: &PreparedFunctionSignature,
    live: &mut LiveDefinitions,
    operations: &mut Vec<TargetUnitOperation>,
    provenance: &mut TerminalPsiProvenance,
) -> Result<(), LoweringError> {
    let invalid = || LoweringError::UnsupportedControlFlow(function.machine);
    let (psi_operation, result, callee, values, arguments, claims, requirements, crashes) =
        match operation {
            AbstractOperation::CallStructuralScalar {
                psi_operation,
                result,
                callee,
                arguments,
                structural_arguments,
                claim_transfers,
                requirement_obligations,
                crash_continuations,
            } => (
                *psi_operation,
                Some(*result),
                *callee,
                arguments,
                structural_arguments,
                claim_transfers,
                requirement_obligations,
                crash_continuations,
            ),
            AbstractOperation::CallUnit {
                psi_operation,
                callee,
                arguments,
                structural_arguments,
                claim_transfers,
                requirement_obligations,
                crash_continuations,
            } => (
                *psi_operation,
                None,
                *callee,
                arguments,
                structural_arguments,
                claim_transfers,
                requirement_obligations,
                crash_continuations,
            ),
            _ => return Err(invalid()),
        };
    let callee_function = functions
        .get(&callee)
        .copied()
        .ok_or(LoweringError::UnknownCallTarget(callee))?;
    // Requirement obligations are discharged proof metadata, not ownership
    // claims or ABI arguments. Retain their exact ordered IDs on the target
    // call for independent translation replay, as for ordinary Unit calls.
    // Crash continuations ride the same correspondence: they record the
    // call's verified surviving routes without changing custody, so the row
    // carries them unchanged. Ownership transfers still need their own
    // realization.
    //
    // A callee's published service ceiling is verified reach, not a custody
    // or ABI change: the caller's own ceiling already covers it, and the
    // retained call row keeps the callee identity that authority review walks
    // to the callee's ceiling. A Unit call therefore lowers exactly as the
    // parameter-rooted Unit-call route does. A scalar result still needs the
    // service-free fixed-native ABI the callee publishes, which is the same
    // bound the call-row replay enforces.
    if !claims.is_empty()
        || !callee_function.entry_claims.is_empty()
        || (result.is_some() && !callee_function.published_service_ceiling.is_empty())
        || values.len() != callee_function.parameters.len()
        || arguments.len() != callee_function.structural_parameters.len()
        || result.map(|result| result.scalar_type)
            != callee_function
                .result
                .scalar()
                .map(|result| result.scalar_type)
        || (result.is_none() && callee_function.result != AbstractFunctionResult::Unit)
    {
        return Err(invalid());
    }
    let signature = prepare_function_signature(callee_function, target, types)?;
    // Validate the structural argument roster under reference custody before
    // any per-argument lowering runs; moved carrier leaves commit only after
    // every argument row is emitted.
    let moved = super::references::call_arguments(function, types, live, arguments)?;
    let scalar_arguments = values
        .iter()
        .zip(&signature.scalar_parameters)
        .enumerate()
        .map(|(position, (value, parameter))| {
            let source = super::scalar_sources::source(*value, function, live)?;
            if source.scalar_type() != parameter.scalar_type {
                return Err(LoweringError::ValueTypeMismatch(*value));
            }
            Ok(TargetUnitScalarCallArgument {
                parameter_index: u32::try_from(position).map_err(|_| invalid())?,
                source,
                placement: parameter.placement.clone(),
            })
        })
        .collect::<Result<Vec<_>, _>>()?;
    let target_arguments = arguments
        .iter()
        .zip(&callee_function.structural_parameters)
        .zip(&signature.parameters)
        .map(|((argument, declaration), destination)| {
            if super::aggregate_borrows::is_reference(declaration, types) {
                return super::aggregate_borrows::argument(
                    argument,
                    declaration,
                    destination,
                    function,
                    prepared,
                    live,
                    types,
                );
            }
            if super::owned_arguments::is_owned_parameter(declaration, types) {
                return super::owned_arguments::argument(
                    argument,
                    declaration,
                    destination,
                    function,
                    prepared,
                    live,
                    types,
                );
            }
            if crate::lowering::scalar::byte_views::is_byte_parameter(declaration, types)
                || crate::lowering::scalar::element_views::is_element_view_parameter(
                    declaration,
                    types,
                )
            {
                return self::byte_argument(
                    argument,
                    declaration,
                    destination,
                    function,
                    prepared,
                    live,
                    types,
                );
            }
            self::argument(
                argument,
                declaration,
                destination,
                function,
                prepared,
                live,
                types,
            )
        })
        .collect::<Result<Vec<_>, _>>()?;
    super::references::commit_argument_moves(live, &moved);
    let result_home = result
        .map(|result| super::primitive_storage::retain_result(psi_operation, result, live))
        .transpose()?;
    operations.push(TargetUnitOperation::Call {
        origin: target_operations::NativeCallOrigin::Authored,
        psi_operation,
        callee,
        call_plan: signature.call_plan,
        result: result_home.map_or(
            target_operations::TargetCallResult::Unit,
            target_operations::TargetCallResult::Scalar,
        ),
        scalar_arguments,
        arguments: target_arguments,
        claim_transfers: claims.clone(),
        requirement_obligations: requirements.clone(),
        crash_continuations: crashes.clone(),
    });
    provenance.operations.push(psi_operation);
    Ok(())
}

/// Retain the same primitive referent custody independently of the call result.
/// A `.., Referent` argument resolves through live reference custody instead
/// of an owned or borrowed place.
pub(super) fn argument(
    argument: &terminal_psi::StructuralArgument,
    declaration: &terminal_psi::StructuralParameterDeclaration,
    destination: &TargetStructuralParameter,
    function: &AbstractFunction,
    prepared: &PreparedFunctionSignature,
    live: &LiveDefinitions,
    types: &StructuralTypeLookup<'_>,
) -> Result<TargetStructuralArgument, LoweringError> {
    let invalid = || LoweringError::UnsupportedControlFlow(function.machine);
    if matches!(argument.path.last(), Some(StructuralPathSegment::Referent)) {
        return super::references::referent_argument(
            argument,
            declaration,
            destination,
            function,
            prepared,
            live,
            types,
        )?
        .ok_or_else(invalid);
    }
    if live.address_joins.contains(&argument.place) {
        return address_join_argument(argument, declaration, destination, function, live, types);
    }
    // A live aggregate home lends one exact subtree through a borrow: the
    // producer keeps whole storage, and the argument transports the leaf's
    // address inside it. The verifier admits the same projected loan only
    // under its exact-access and claim-free bounds, which this shape check
    // replays against the home's own result metadata.
    if !argument.path.is_empty()
        && let Some(home) = live.structural_homes.get(&argument.place)
    {
        let (producer, result) = home.operation_result().ok_or_else(invalid)?;
        let mut shape_cache = BTreeMap::new();
        let mut active = BTreeSet::new();
        let (projected_type, projected_shape, source_byte_offset) =
            crate::lowering::structural_layout::resolve_structural_projection_path(
                result.structural_type,
                &argument.path,
                types,
                &mut shape_cache,
                &mut active,
            )?;
        if argument.access != declaration.access
            || !matches!(
                argument.access,
                StructuralAccess::SharedBorrow
                    | StructuralAccess::MutableBorrow
                    | StructuralAccess::WriteOnlyBorrow
            )
            || declaration.multiplicity != StructuralMultiplicity::Unrestricted
            || !declaration.qualifications.is_empty()
            || !declaration.projected_qualifications.is_empty()
            || projected_type != declaration.structural_type
            || result.multiplicity == StructuralMultiplicity::Linear
            || !result.qualifications.is_empty()
            || !result.projected_qualifications.is_empty()
            || !result.claims.is_empty()
            || crate::lowering::structural_layout::structural_parameter_shape(
                projected_shape,
                declaration.access,
            ) != destination.shape
            || u32::from(projected_shape.byte_size)
                .checked_add(source_byte_offset)
                .is_none_or(|end| end > u32::from(home.layout.shape().byte_size))
        {
            return Err(invalid());
        }
        return Ok(TargetStructuralArgument {
            place: argument.place,
            access: argument.access,
            path: argument.path.clone(),
            root_structural_type: result.structural_type,
            structural_type: projected_type,
            shape: destination.shape,
            source_byte_offset,
            fixed_array_length: None,
            element_stride: None,
            source: TargetStructuralArgumentSource::StructuralHome {
                psi_operation: producer,
            },
            destination: destination.placement.clone(),
        });
    }
    if !argument.path.is_empty()
        || argument.access != declaration.access
        || !super::primitive_storage::is_primitive_reference(declaration, types)
    {
        return Err(invalid());
    }
    let (identity, source) = if let Some(home) = live.structural_homes.get(&argument.place) {
        let (defining_operation, home_result) = home.operation_result().ok_or_else(invalid)?;
        if !function.operations.iter().any(|operation| {
            matches!(operation,
            AbstractOperation::EstablishPrimitiveLocal { psi_operation, result, .. }
            if *psi_operation == defining_operation && result == home_result)
        }) {
            return Err(invalid());
        }
        (
            home.structural_type(),
            TargetStructuralArgumentSource::EstablishedPrimitiveLocal {
                psi_operation: defining_operation,
            },
        )
    } else {
        let source = prepared
            .parameters
            .iter()
            .find(|source| source.place == argument.place)
            .ok_or_else(invalid)?;
        let allowed = match source.access {
            StructuralAccess::MutableBorrow => argument.access != StructuralAccess::Owned,
            StructuralAccess::SharedBorrow => argument.access == StructuralAccess::SharedBorrow,
            StructuralAccess::WriteOnlyBorrow => {
                argument.access == StructuralAccess::WriteOnlyBorrow
            }
            StructuralAccess::Owned => false,
        };
        if !allowed {
            return Err(invalid());
        }
        (source.structural_type, source.placement.clone().into())
    };
    if identity != declaration.structural_type {
        return Err(invalid());
    }
    Ok(TargetStructuralArgument {
        place: argument.place,
        access: argument.access,
        path: Vec::new(),
        root_structural_type: identity,
        structural_type: identity,
        shape: destination.shape,
        source_byte_offset: 0,
        fixed_array_length: None,
        element_stride: None,
        source,
        destination: destination.placement.clone(),
    })
}

/// An available address join lends its own joined referent, whole and shared:
/// the callee receives the carried address, exactly as it would receive a
/// caller's borrowed parameter. A projection through the join, a wider access
/// or a substituted referent type needs its own place evidence and rejects.
pub(super) fn address_join_argument(
    argument: &terminal_psi::StructuralArgument,
    declaration: &terminal_psi::StructuralParameterDeclaration,
    destination: &TargetStructuralParameter,
    function: &AbstractFunction,
    live: &LiveDefinitions,
    types: &StructuralTypeLookup<'_>,
) -> Result<TargetStructuralArgument, LoweringError> {
    let invalid = || LoweringError::UnsupportedControlFlow(function.machine);
    if !live.address_joins.contains(&argument.place)
        || !argument.path.is_empty()
        || argument.access != StructuralAccess::SharedBorrow
        || declaration.access != StructuralAccess::SharedBorrow
        || declaration.multiplicity != StructuralMultiplicity::Unrestricted
        || !declaration.qualifications.is_empty()
        || !declaration.projected_qualifications.is_empty()
    {
        return Err(invalid());
    }
    let (entry, parameter) = function
        .block_entries
        .iter()
        .find_map(|entry| {
            entry
                .structural_parameters
                .iter()
                .find(|parameter| parameter.place == argument.place)
                .map(|parameter| (entry, parameter))
        })
        .ok_or_else(invalid)?;
    let referent = crate::lowering::structural_layout::structural_shape(
        parameter.structural_type,
        types,
        &mut BTreeMap::new(),
        &mut BTreeSet::new(),
    )?;
    if !super::transfers::is_address_join(parameter, types)
        || parameter.structural_type != declaration.structural_type
        || destination.shape
            != crate::lowering::structural_layout::structural_parameter_shape(
                referent,
                StructuralAccess::SharedBorrow,
            )
    {
        return Err(invalid());
    }
    Ok(TargetStructuralArgument {
        place: argument.place,
        access: argument.access,
        path: Vec::new(),
        root_structural_type: parameter.structural_type,
        structural_type: parameter.structural_type,
        shape: destination.shape,
        source_byte_offset: 0,
        fixed_array_length: None,
        element_stride: None,
        source: TargetStructuralArgumentSource::BlockParameter {
            block: entry.block,
            place: parameter.place,
        },
        destination: destination.placement.clone(),
    })
}

/// Borrowed byte views keep their exact descriptor source through ordinary
/// calls: an established literal or subslice producer, a borrowed block
/// parameter, or the caller's own machine parameter. Shared views may arrive
/// from all three. An exclusive view arrives on its incoming machine parameter
/// or on an exclusive block parameter, which control flow establishes with the
/// same custody; an established producer stays shared. The root's own access
/// authorizes the argument, so a shared root never supplies an exclusive one.
#[allow(clippy::too_many_arguments)]
fn byte_argument(
    argument: &terminal_psi::StructuralArgument,
    declaration: &terminal_psi::StructuralParameterDeclaration,
    destination: &TargetStructuralParameter,
    function: &AbstractFunction,
    prepared: &PreparedFunctionSignature,
    live: &LiveDefinitions,
    types: &StructuralTypeLookup<'_>,
) -> Result<TargetStructuralArgument, LoweringError> {
    let invalid = || LoweringError::UnsupportedControlFlow(function.machine);
    let shared = argument.access == StructuralAccess::SharedBorrow;
    // A fixed-array byte window is the same call-scoped presentation of the
    // caller's own backing the structural-call lane emits: the argument keeps
    // its real path, and a scalar or unit result changes no custody.
    if !argument.path.is_empty() {
        let Some(source) = prepared
            .parameters
            .iter()
            .find(|source| source.place == argument.place)
        else {
            return Err(invalid());
        };
        let Some(window) = function.structural_parameters.iter().find_map(|actual| {
            terminal_semantics::fixed_byte_array_window(
                types.values().copied(),
                actual,
                argument,
                declaration,
            )
        }) else {
            return Err(invalid());
        };
        let mut shape_cache = BTreeMap::new();
        let mut active = BTreeSet::new();
        let backing_offset = if window.backing_path.is_empty() {
            0
        } else {
            crate::lowering::structural_layout::resolve_structural_field_path(
                source.structural_type,
                window.backing_path,
                types,
                &mut shape_cache,
                &mut active,
            )?
            .2
        };
        let source_byte_offset = u64::from(backing_offset)
            .checked_add(window.offset)
            .and_then(|offset| u32::try_from(offset).ok())
            .ok_or(LoweringError::StructuralTypeTooLarge(
                source.structural_type,
            ))?;
        if destination.shape != ValueShape::borrowed_reference(16, 8)
            || u64::from(backing_offset)
                .checked_add(window.backing_length)
                .is_none_or(|end| end > u64::from(source.shape.byte_size))
        {
            return Err(invalid());
        }
        return Ok(TargetStructuralArgument {
            place: argument.place,
            access: argument.access,
            path: argument.path.clone(),
            root_structural_type: source.structural_type,
            structural_type: declaration.structural_type,
            shape: destination.shape,
            source_byte_offset,
            fixed_array_length: Some(window.length),
            element_stride: Some(1),
            source: source.placement.clone().into(),
            destination: destination.placement.clone(),
        });
    }
    if argument.access != declaration.access
        || (!crate::lowering::scalar::byte_views::is_byte_parameter(declaration, types)
            && !crate::lowering::scalar::element_views::is_element_view_parameter(
                declaration,
                types,
            ))
        || destination.shape != ValueShape::borrowed_reference(16, 8)
    {
        return Err(invalid());
    }
    let (identity, source) = if shared
        && let Some((producer, structural_type)) = live.views.get(&argument.place)
    {
        (
            *structural_type,
            if crate::lowering::scalar::element_views::is_element_view_parameter(declaration, types)
            {
                target_operations::TargetStructuralArgumentSource::EstablishedElementView {
                    psi_operation: *producer,
                }
            } else {
                target_operations::TargetStructuralArgumentSource::EstablishedByteView {
                    psi_operation: *producer,
                }
            },
        )
    } else if live.block_views.contains(&argument.place) {
        let (entry, parameter) = function
            .block_entries
            .iter()
            .find_map(|entry| {
                entry
                    .structural_parameters
                    .iter()
                    .find(|parameter| parameter.place == argument.place)
                    .map(|parameter| (entry, parameter))
            })
            .ok_or_else(invalid)?;
        // The root's own access authorizes the argument, exactly as an
        // incoming machine parameter does: an exclusive block parameter
        // lends shared, exclusive or write-only, a shared one only shared.
        let allowed = match parameter.access {
            StructuralAccess::MutableBorrow => argument.access != StructuralAccess::Owned,
            StructuralAccess::SharedBorrow => argument.access == StructuralAccess::SharedBorrow,
            StructuralAccess::WriteOnlyBorrow | StructuralAccess::Owned => false,
        };
        if !allowed {
            return Err(invalid());
        }
        (
            parameter.structural_type,
            target_operations::TargetStructuralArgumentSource::BlockParameter {
                block: entry.block,
                place: parameter.place,
            },
        )
    } else {
        let source = prepared
            .parameters
            .iter()
            .find(|source| source.place == argument.place)
            .ok_or_else(invalid)?;
        if source.access != argument.access
            || source.multiplicity != declaration.multiplicity
            || !source.projected_qualifications.is_empty()
            || source.shape != destination.shape
        {
            return Err(invalid());
        }
        (source.structural_type, source.placement.clone().into())
    };
    if identity != declaration.structural_type {
        return Err(invalid());
    }
    Ok(TargetStructuralArgument {
        place: argument.place,
        access: argument.access,
        path: Vec::new(),
        root_structural_type: identity,
        structural_type: identity,
        shape: destination.shape,
        source_byte_offset: 0,
        fixed_array_length: None,
        element_stride: None,
        source,
        destination: destination.placement.clone(),
    })
}
