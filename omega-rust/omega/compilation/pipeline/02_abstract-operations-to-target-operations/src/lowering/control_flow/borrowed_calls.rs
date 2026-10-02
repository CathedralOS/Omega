//! Borrowed call arguments resolved against live graph storage: primitive
//! referents, projected loans, address joins, and byte or element views.
use super::LiveDefinitions;
use crate::LoweringError;
use crate::lowering::function_signature::PreparedFunctionSignature;
use crate::lowering::structural_type_lookup::StructuralTypeLookup;
use abstract_operations::{AbstractFunction, AbstractOperation};
use calling_conventions::ValueShape;
use semantic_vocabulary::MachineId;
use std::collections::{BTreeMap, BTreeSet};
use target_operations::{
    TargetStructuralArgument, TargetStructuralArgumentSource, TargetStructuralParameter,
};
use terminal_psi::{StructuralAccess, StructuralMultiplicity, StructuralPathSegment};

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
        .ok_or_else(|| LoweringError::unsupported_control_flow(function.machine));
    }
    if live.address_joins.contains(&argument.place) {
        return address_join_argument(argument, declaration, destination, function, live, types);
    }
    // A live aggregate home lends one exact subtree through a borrow: the
    // producer keeps whole storage, and the argument transports the leaf's
    // address inside it. The verifier admits the same projected loan only
    // under its exact-access and claim-free bounds, which this shape check
    // replays against the home's own result metadata. An empty path borrows
    // the home whole when the formal is no primitive reference — primitive
    // storage keeps its own custody channel below.
    if let Some(home) = live.structural_homes.get(&argument.place)
        && (!argument.path.is_empty()
            || !super::primitive_storage::is_primitive_reference(declaration, types))
    {
        let (producer, result) = home
            .operation_result()
            .ok_or_else(|| LoweringError::unsupported_control_flow(function.machine))?;
        let mut shape_cache = BTreeMap::new();
        let mut active = BTreeSet::new();
        let (projected_type, projected_shape, source_byte_offset) = if argument.path.is_empty() {
            (
                result.structural_type,
                crate::lowering::structural_layout::structural_shape(
                    result.structural_type,
                    types,
                    &mut shape_cache,
                    &mut active,
                )?,
                0,
            )
        } else {
            crate::lowering::structural_layout::resolve_structural_projection_path(
                result.structural_type,
                &argument.path,
                types,
                &mut shape_cache,
                &mut active,
            )?
        };
        // A whole owned bounded byte local lends its home's address to a
        // shared bounded-owner formal. Unit validation replayed the formal's
        // byte predicates from the local's own operands; the qualifications
        // add no carrier, so the home layout is the loan's whole ABI.
        let owner_loan = argument.path.is_empty()
            && abstract_operations::control_flow::owner_loans::is_shared_bounded_owner_loan(
                declaration,
                |identity| types.get(&identity).copied(),
            )
            && function.operations.iter().any(|operation| {
                matches!(operation,
                    AbstractOperation::EstablishByteSequenceConcatenation { psi_operation, result, .. }
                        if *psi_operation == producer && result.place == argument.place)
            });
        if argument.access != declaration.access
            || !matches!(
                argument.access,
                StructuralAccess::SharedBorrow
                    | StructuralAccess::MutableBorrow
                    | StructuralAccess::WriteOnlyBorrow
            )
            || declaration.multiplicity != StructuralMultiplicity::Unrestricted
            || (!declaration.qualifications.is_empty() && !owner_loan)
            || !declaration.projected_qualifications.is_empty()
            || projected_type != declaration.structural_type
            || result.multiplicity == StructuralMultiplicity::Linear
            || (!result.qualifications.is_empty() && !owner_loan)
            || !result.projected_qualifications.is_empty()
            || !result.qualification_establishments.is_empty()
            || !result.claims.is_empty()
            || crate::lowering::structural_layout::structural_parameter_shape(
                projected_shape,
                declaration.access,
            ) != destination.shape
            || projected_shape
                .byte_size
                .checked_add(source_byte_offset)
                .is_none_or(|end| end > home.layout.shape().byte_size)
        {
            return Err(LoweringError::unsupported_control_flow(function.machine));
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
            indices: Vec::new(),
            source: TargetStructuralArgumentSource::StructuralHome {
                psi_operation: producer,
            },
            destination: destination.placement.clone(),
        });
    }
    // A projected borrow through an incoming machine parameter carries the
    // parameter's placement whole and transports the leaf's address inside
    // it, exactly as the structural-home loan above does for a produced
    // result. The parameter's own access authorizes the argument.
    if !argument.path.is_empty()
        && let Some(source) = prepared
            .parameters
            .iter()
            .find(|source| source.place == argument.place)
    {
        let allowed = match source.access {
            StructuralAccess::MutableBorrow => argument.access != StructuralAccess::Owned,
            StructuralAccess::SharedBorrow => argument.access == StructuralAccess::SharedBorrow,
            StructuralAccess::WriteOnlyBorrow => {
                argument.access == StructuralAccess::WriteOnlyBorrow
            }
            StructuralAccess::Owned => false,
        };
        let mut shape_cache = BTreeMap::new();
        let mut active = BTreeSet::new();
        let Some((projected_type, projected_shape, source_byte_offset)) =
            crate::lowering::structural_layout::resolve_structural_projection_path(
                source.structural_type,
                &argument.path,
                types,
                &mut shape_cache,
                &mut active,
            )
            .ok()
        else {
            return Err(LoweringError::unsupported_control_flow(function.machine));
        };
        if !allowed
            || argument.access != declaration.access
            || declaration.multiplicity != StructuralMultiplicity::Unrestricted
            || !declaration.qualifications.is_empty()
            || !declaration.projected_qualifications.is_empty()
            || projected_type != declaration.structural_type
            || crate::lowering::structural_layout::structural_parameter_shape(
                projected_shape,
                declaration.access,
            ) != destination.shape
            || projected_shape
                .byte_size
                .checked_add(source_byte_offset)
                .is_none_or(|end| end > source.shape.byte_size)
        {
            return Err(LoweringError::unsupported_control_flow(function.machine));
        }
        return Ok(TargetStructuralArgument {
            place: argument.place,
            access: argument.access,
            path: argument.path.clone(),
            root_structural_type: source.structural_type,
            structural_type: projected_type,
            shape: destination.shape,
            source_byte_offset,
            fixed_array_length: None,
            element_stride: None,
            indices: Vec::new(),
            source: source.placement.clone().into(),
            destination: destination.placement.clone(),
        });
    }
    if !argument.path.is_empty()
        || argument.access != declaration.access
        || !super::primitive_storage::is_primitive_reference(declaration, types)
    {
        return Err(LoweringError::unsupported_control_flow(function.machine));
    }
    let (identity, source) = if let Some(home) = live.structural_homes.get(&argument.place) {
        let (defining_operation, home_result) = home
            .operation_result()
            .ok_or_else(|| LoweringError::unsupported_control_flow(function.machine))?;
        if !function.operations.iter().any(|operation| {
            matches!(operation,
            AbstractOperation::EstablishPrimitiveLocal { psi_operation, result, .. }
            if *psi_operation == defining_operation && result == home_result)
        }) {
            return Err(LoweringError::unsupported_control_flow(function.machine));
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
            .ok_or_else(|| LoweringError::unsupported_control_flow(function.machine))?;
        let allowed = match source.access {
            StructuralAccess::MutableBorrow => argument.access != StructuralAccess::Owned,
            StructuralAccess::SharedBorrow => argument.access == StructuralAccess::SharedBorrow,
            StructuralAccess::WriteOnlyBorrow => {
                argument.access == StructuralAccess::WriteOnlyBorrow
            }
            StructuralAccess::Owned => false,
        };
        if !allowed {
            return Err(LoweringError::unsupported_control_flow(function.machine));
        }
        (source.structural_type, source.placement.clone().into())
    };
    if identity != declaration.structural_type {
        return Err(LoweringError::unsupported_control_flow(function.machine));
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
        indices: Vec::new(),
        source,
        destination: destination.placement.clone(),
    })
}

/// An available address join lends from its carried address, not copied storage.
/// Projections use the same typed layout and dominating scalar operands as an
/// incoming parameter; the block source retains the joined root's identity.
/// Access cannot widen, and the projected referent must match the callee.
pub(super) fn address_join_argument(
    argument: &terminal_psi::StructuralArgument,
    declaration: &terminal_psi::StructuralParameterDeclaration,
    destination: &TargetStructuralParameter,
    function: &AbstractFunction,
    live: &LiveDefinitions,
    types: &StructuralTypeLookup<'_>,
) -> Result<TargetStructuralArgument, LoweringError> {
    if !live.address_joins.contains(&argument.place)
        || !matches!(
            argument.access,
            StructuralAccess::SharedBorrow | StructuralAccess::MutableBorrow
        )
        || declaration.access != argument.access
        || declaration.multiplicity != StructuralMultiplicity::Unrestricted
        || !declaration.qualifications.is_empty()
        || !declaration.projected_qualifications.is_empty()
    {
        return Err(LoweringError::unsupported_control_flow(function.machine));
    }
    let (block, parameter) = address_join_parameter(function, argument.place)
        .ok_or_else(|| LoweringError::unsupported_control_flow(function.machine))?;
    let (selected, referent, offset, runs) =
        crate::lowering::structural_layout::runtime_projection(
            parameter.structural_type,
            &argument.path,
            types,
            &mut BTreeMap::new(),
            &mut BTreeSet::new(),
        )?;
    if !super::transfers::is_address_join(parameter, types)
        || argument.access == StructuralAccess::MutableBorrow
            && parameter.access != StructuralAccess::MutableBorrow
        || selected != declaration.structural_type
        || destination.shape
            != crate::lowering::structural_layout::structural_parameter_shape(
                referent,
                declaration.access,
            )
    {
        return Err(LoweringError::unsupported_control_flow(function.machine));
    }
    let indices = super::scalar_sources::runtime_indices(
        runs,
        function,
        &super::scalar_sources::ScalarSources::from(live),
    )?;
    Ok(TargetStructuralArgument {
        place: argument.place,
        access: argument.access,
        path: argument.path.clone(),
        root_structural_type: parameter.structural_type,
        structural_type: selected,
        shape: destination.shape,
        source_byte_offset: offset,
        fixed_array_length: None,
        element_stride: None,
        indices,
        source: TargetStructuralArgumentSource::BlockParameter {
            block,
            place: parameter.place,
        },
        destination: destination.placement.clone(),
    })
}

/// An available address join lends a fixed-array window of its joined
/// referent exactly as an incoming parameter's field window does: the backing
/// keeps its real type and field path, the carried address roots it, and the
/// presented length grants neither storage nor resize.
pub(super) fn address_join_window_argument(
    argument: &terminal_psi::StructuralArgument,
    declaration: &terminal_psi::StructuralParameterDeclaration,
    destination: &TargetStructuralParameter,
    callee: MachineId,
    function: &AbstractFunction,
    live: &LiveDefinitions,
    types: &StructuralTypeLookup<'_>,
) -> Result<TargetStructuralArgument, LoweringError> {
    let unsupported = || LoweringError::unsupported_control_flow(function.machine);
    let (block, parameter) = address_join_parameter(function, argument.place)
        .filter(|_| live.address_joins.contains(&argument.place))
        .ok_or_else(unsupported)?;
    if !super::transfers::is_address_join(parameter, types) {
        return Err(unsupported());
    }
    let mut shape_cache = BTreeMap::new();
    let mut active = BTreeSet::new();
    let referent = crate::lowering::structural_layout::structural_shape(
        parameter.structural_type,
        types,
        &mut shape_cache,
        &mut active,
    )?;
    crate::lowering::unit::roster_arguments::fixed_array_view_argument(
        argument,
        declaration,
        destination.shape,
        &destination.placement,
        callee,
        parameter,
        referent.byte_size,
        TargetStructuralArgumentSource::BlockParameter {
            block,
            place: parameter.place,
        },
        types,
        &mut shape_cache,
        &mut active,
    )?
    .ok_or_else(unsupported)
}

/// The block that binds `place` and its structural declaration.
fn address_join_parameter(
    function: &AbstractFunction,
    place: semantic_vocabulary::PlaceId,
) -> Option<(
    semantic_vocabulary::BlockId,
    &terminal_psi::StructuralParameterDeclaration,
)> {
    function.block_entries.iter().find_map(|entry| {
        entry
            .structural_parameters
            .iter()
            .find(|parameter| parameter.place == place)
            .map(|parameter| (entry.block, parameter))
    })
}

/// Borrowed byte views keep their exact descriptor source through ordinary
/// calls: an established literal or subslice producer, a borrowed block
/// parameter, or the caller's own machine parameter. Shared views may arrive
/// from all three. An exclusive view arrives on its incoming machine parameter
/// or on an exclusive block parameter, which control flow establishes with the
/// same custody; an established producer stays shared. The root's own access
/// authorizes the argument, so a shared root never supplies an exclusive one.
/// Fixed arrays instead present their backing: a parameter's field window or
/// a dominating local array window, shared or exclusive.
#[allow(clippy::too_many_arguments)]
pub(super) fn byte_argument(
    argument: &terminal_psi::StructuralArgument,
    declaration: &terminal_psi::StructuralParameterDeclaration,
    destination: &TargetStructuralParameter,
    callee: MachineId,
    function: &AbstractFunction,
    prepared: &PreparedFunctionSignature,
    live: &LiveDefinitions,
    types: &StructuralTypeLookup<'_>,
) -> Result<TargetStructuralArgument, LoweringError> {
    let shared = argument.access == StructuralAccess::SharedBorrow;
    // A fixed array lends its original backing through the same call-scoped
    // presentation the structural-result lane emits: an incoming parameter's
    // field window keeps its real path, and a dominating local array lends its
    // own home. A scalar or unit result changes no custody. Resolve local
    // backing before a projected parameter lookup: geometry does not determine
    // whether the source storage is an incoming parameter or a local result.
    let unsupported = || LoweringError::unsupported_control_flow(function.machine);
    let mut shape_cache = BTreeMap::new();
    let mut active = BTreeSet::new();
    // A home that is no whole plain array (a mutable view's affine custody
    // home, for one) keeps the descriptor routes below.
    if let Some(home) = live.structural_homes.get(&argument.place)
        && let Some(loan) = crate::lowering::unit::roster_arguments::local_array_loan(
            argument,
            declaration,
            destination.shape,
            &destination.placement,
            callee,
            home,
            types,
            &mut shape_cache,
            &mut active,
        )?
    {
        return Ok(loan);
    }
    if !argument.path.is_empty() {
        let (Some(source), Some(root)) = (
            prepared
                .parameters
                .iter()
                .find(|source| source.place == argument.place),
            function
                .structural_parameters
                .iter()
                .find(|root| root.place == argument.place),
        ) else {
            return Err(unsupported());
        };
        return crate::lowering::unit::roster_arguments::fixed_array_view_argument(
            argument,
            declaration,
            destination.shape,
            &destination.placement,
            callee,
            root,
            source.shape.byte_size,
            source.placement.clone().into(),
            types,
            &mut shape_cache,
            &mut active,
        )?
        .ok_or_else(unsupported);
    }
    if argument.access != declaration.access
        || (!crate::lowering::scalar::byte_views::is_byte_parameter(declaration, types)
            && !crate::lowering::scalar::element_views::is_element_view_parameter(
                declaration,
                types,
            ))
        || destination.shape != ValueShape::borrowed_reference(16, 8)
    {
        return Err(LoweringError::unsupported_control_flow(function.machine));
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
            .ok_or_else(|| LoweringError::unsupported_control_flow(function.machine))?;
        // The root's own access authorizes the argument, exactly as an
        // incoming machine parameter does: an exclusive block parameter
        // lends shared, exclusive or write-only, a shared one only shared.
        let allowed = match parameter.access {
            StructuralAccess::MutableBorrow => argument.access != StructuralAccess::Owned,
            StructuralAccess::SharedBorrow => argument.access == StructuralAccess::SharedBorrow,
            StructuralAccess::WriteOnlyBorrow | StructuralAccess::Owned => false,
        };
        if !allowed {
            return Err(LoweringError::unsupported_control_flow(function.machine));
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
            .ok_or_else(|| LoweringError::unsupported_control_flow(function.machine))?;
        if source.access != argument.access
            || source.multiplicity != declaration.multiplicity
            || !source.projected_qualifications.is_empty()
            || source.shape != destination.shape
        {
            return Err(LoweringError::unsupported_control_flow(function.machine));
        }
        (source.structural_type, source.placement.clone().into())
    };
    if identity != declaration.structural_type {
        return Err(LoweringError::unsupported_control_flow(function.machine));
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
        indices: Vec::new(),
        source,
        destination: destination.placement.clone(),
    })
}
