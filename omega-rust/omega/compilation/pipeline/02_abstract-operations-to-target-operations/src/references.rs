/*
  Reference descriptor metadata transport across control-flow blocks, calls,
  copies, and observations (`ReferenceStorage`, `BorrowedSources`,
  `StoredEffectCatalog`).

  Propagates current carrier types, referent types, and possible backing origins
  across dominating definitions and post-transfer/post-cleanup control-flow
  edges without re-running source borrow checking.
*/
use super::{LiveDefinitions, RowSite};
use crate::LoweringError;
use crate::structural_type_lookup::StructuralTypeLookup;
use abstract_operations::{AbstractFunction, AbstractOperation};
use optimization_unit_semantics::access_references::validate_backing;
use optimization_unit_semantics::descriptor_results::access_results::agree;
use optimization_unit_semantics::descriptor_results::access_results::reconcile;
use optimization_unit_semantics::structural_borrowed_storage_projection;
use semantic_vocabulary::{PlaceId, StructuralTypeId};
use std::collections::{BTreeMap, BTreeSet};
use target_operations::{ReferenceRoot, TargetAccessInterpretation};
use target_operations::{TargetStructuralArgument, TargetStructuralParameter, TargetUnitOperation};
use terminal_psi::{
    StructuralAccess, StructuralMultiplicity, StructuralPathSegment, StructuralTypeShape,
};
use terminal_psi::{StructuralArgument, StructuralOperationResult};
use terminal_semantics::primitive_projection_declaration;
use terminal_semantics::stored_descriptor_paths;

mod borrowed_arrivals {
    use super::*;
    use optimization_unit_semantics::StructuralStorageProjection;

    pub(crate) type BorrowedSources =
        std::sync::Arc<BTreeMap<PlaceId, Option<Vec<BorrowedSource>>>>;

    #[derive(Clone, Debug, PartialEq, Eq)]
    pub(crate) struct BorrowedSource {
        pub(crate) root: ReferenceRoot,
        pub(crate) projection: StructuralStorageProjection,
        pub(crate) access: StructuralAccess,
        pub(crate) placed: Option<target_operations::TargetAccessInterpretation>,
    }

    pub(crate) fn borrowed_source_catalog(
        functions: &BTreeMap<semantic_vocabulary::MachineId, &AbstractFunction>,
        types: &StructuralTypeLookup<'_>,
    ) -> BTreeMap<semantic_vocabulary::MachineId, BorrowedSources> {
        functions
            .values()
            .filter_map(|function| {
                let sources: BTreeMap<_, _> = function
                    .block_structural_parameters()
                    .filter(|parameter| {
                        parameter.access != StructuralAccess::Owned
                            && !terminal_semantics::descriptor_results::is_descriptor(
                                types.values().copied(),
                                parameter.structural_type,
                            )
                    })
                    .map(|parameter| (parameter.place, None))
                    .collect();
                (!sources.is_empty()).then(|| (function.machine, std::sync::Arc::new(sources)))
            })
            .collect()
    }

    pub(crate) fn capture(
        function: &AbstractFunction,
        live: &LiveDefinitions,
        types: &StructuralTypeLookup<'_>,
        bindings: &[abstract_operations::AbstractStructuralBinding],
        parameters: &[terminal_psi::StructuralParameterDeclaration],
    ) -> Result<BorrowedSources, LoweringError> {
        let mut sources = live.borrowed_reference_sources.as_ref().clone();
        for (binding, parameter) in bindings.iter().zip(parameters) {
            if !sources.contains_key(&parameter.place) {
                continue;
            }
            let projection = storage_projection(function, live, types, &binding.argument)
                .ok_or_else(|| LoweringError::unsupported_control_flow(function.machine))?;
            if binding.parameter != parameter.place
                || binding.argument.access != parameter.access
                || !projection.matches(
                    types.values().copied(),
                    StructuralStorageProjection::Endpoint(parameter.structural_type),
                )
            {
                return Err(LoweringError::unsupported_control_flow(function.machine));
            }
            let root = normalized_source(function, live, types, &binding.argument)?;
            for moved in bindings
                .iter()
                .filter(|binding| binding.argument.access == StructuralAccess::Owned)
            {
                for origin in root.origins() {
                    if !matches!(origin.base,
                    target_operations::TargetReferenceOriginBase::Storage { root }
                    if root == moved.argument.place)
                    {
                        continue;
                    }
                    let (_, path) = copies::storage_cursor(
                        function,
                        types,
                        live,
                        origin,
                        parameter.structural_type,
                    )?;
                    if path
                        .strip_prefix(moved.argument.path.as_slice())
                        .is_some_and(|relative| {
                            !relative.contains(&StructuralPathSegment::Referent)
                        })
                    {
                        return Err(LoweringError::StructuralStateBindingMismatch {
                            machine: function.machine,
                            reason: format!(
                                "borrowed arrival {:?} retains owning storage moved by {:?}",
                                parameter.place, moved.argument
                            ),
                        });
                    }
                }
            }
            sources.insert(
                parameter.place,
                Some(vec![BorrowedSource {
                    root,
                    projection: StructuralStorageProjection::Endpoint(parameter.structural_type),
                    access: parameter.access,
                    placed: access_source(function, live, types, &binding.argument),
                }]),
            );
        }
        Ok(std::sync::Arc::new(sources))
    }

    pub(crate) fn merge(left: &mut BorrowedSources, right: &BorrowedSources) {
        for (place, sources) in std::sync::Arc::make_mut(left) {
            let Some(incoming) = right.get(place).and_then(Option::as_ref) else {
                *sources = None;
                continue;
            };
            let Some(existing) = sources.as_mut() else {
                continue;
            };
            for source in incoming {
                if !existing.contains(source) {
                    existing.push(source.clone());
                }
            }
            existing
                .sort_by(|left, right| (&left.root, left.access).cmp(&(&right.root, right.access)));
        }
    }

    pub(crate) fn storage_sources(
        function: &AbstractFunction,
        live: &LiveDefinitions,
        types: &StructuralTypeLookup<'_>,
        source: &BorrowedSource,
    ) -> Result<Vec<StructuralArgument>, LoweringError> {
        let StructuralStorageProjection::Endpoint(referent) = source.projection else {
            return Err(LoweringError::unsupported_control_flow(function.machine));
        };
        source
            .root
            .origins()
            .into_iter()
            .map(|origin| {
                let (place, path) =
                    copies::storage_cursor(function, types, live, origin, referent)?;
                Ok(StructuralArgument {
                    place,
                    path,
                    access: source.access,
                })
            })
            .collect()
    }
}
pub(crate) use borrowed_arrivals::{BorrowedSources, borrowed_source_catalog};
pub(crate) use borrowed_arrivals::{
    capture as capture_borrowed_arrivals, merge as merge_borrowed_arrivals,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ReferenceStorage {
    pub(crate) root: ReferenceRoot,
    pub(crate) carrier_type: StructuralTypeId,
    pub(crate) referent: StructuralTypeId,
    pub(crate) placed: Option<target_operations::TargetAccessInterpretation>,
}

pub(crate) struct ReferentProjection {
    pub(crate) carrier_type: StructuralTypeId,
    pub(crate) referent: StructuralTypeId,
    pub(crate) carrier_path: Vec<StructuralPathSegment>,
    pub(crate) path: Vec<StructuralPathSegment>,
}

pub(crate) fn referent_projection(
    function: &AbstractFunction,
    live: &LiveDefinitions,
    types: &StructuralTypeLookup<'_>,
    carrier: PlaceId,
    path: &[StructuralPathSegment],
    required_access: StructuralAccess,
) -> Option<ReferentProjection> {
    let (root_type, multiplicity, access) =
        if let Some(parameter) = function.arrival_parameter(carrier) {
            if function
                .entry_claims
                .iter()
                .any(|claim| claim.input == carrier)
            {
                return None;
            }
            (
                parameter.structural_type,
                parameter.multiplicity,
                parameter.access,
            )
        } else {
            let home = live.structural_homes.get(&carrier)?;
            if home.operation_result().is_some_and(|(_, result)| {
                !result.qualification_establishments.is_empty() || !result.claims.is_empty()
            }) {
                return None;
            }
            (home.structural_type(), home.multiplicity(), home.access())
        };
    if multiplicity == StructuralMultiplicity::Linear
        || !terminal_semantics::reference_source_projection_accessible(
            types.values().copied(),
            root_type,
            multiplicity,
            access,
            path,
            required_access,
        )
    {
        return None;
    }
    window_projection(function, live, types, carrier, path, required_access)
}

pub(crate) fn window_projection(
    function: &AbstractFunction,
    live: &LiveDefinitions,
    types: &StructuralTypeLookup<'_>,
    carrier: PlaceId,
    path: &[StructuralPathSegment],
    required_access: StructuralAccess,
) -> Option<ReferentProjection> {
    let crossing = path
        .iter()
        .rposition(|segment| *segment == StructuralPathSegment::Referent)?;
    let carrier_path = &path[..crossing];
    let root_type = source_contract(function, live, carrier)?;
    let descriptor = leaf_type(types, root_type, carrier_path)?;
    let StructuralTypeShape::Reference { referent, access } = types.get(descriptor)?.shape else {
        return None;
    };
    if !required_access.admits_referent(access) {
        return None;
    }
    crate::structural_layout::reference_referent_projection(
        root_type,
        &path[..=crossing],
        required_access,
        None,
        types,
    )
    .ok()?;
    let descriptors = window_descriptors(function, types, live, carrier, path).ok()?;
    if descriptors
        .iter()
        .any(|leaf| leaf.carrier_type != descriptor || leaf.referent != referent)
    {
        return None;
    }
    Some(ReferentProjection {
        carrier_type: root_type,
        carrier_path: carrier_path.to_vec(),
        referent,
        path: path[crossing + 1..].to_vec(),
    })
}

pub(crate) fn reference_call_argument(
    argument: &StructuralArgument,
    declaration: &terminal_psi::StructuralParameterDeclaration,
    destination: &TargetStructuralParameter,
    function: &AbstractFunction,
    prepared: &crate::function_signature::PreparedFunctionSignature,
    live: &LiveDefinitions,
    types: &StructuralTypeLookup<'_>,
) -> Result<TargetStructuralArgument, LoweringError> {
    if argument.access == StructuralAccess::Owned
        || argument.access != declaration.access
        || declaration.multiplicity != StructuralMultiplicity::Unrestricted
    {
        return Err(LoweringError::unsupported_control_flow(function.machine));
    }
    let (root_type, source) = if let Some(home) = live.structural_homes.get(&argument.place) {
        if home.has_claims()
            || home.multiplicity() == StructuralMultiplicity::Linear
            || home
                .operation_result()
                .is_some_and(|(_, result)| !result.qualification_establishments.is_empty())
        {
            return Err(LoweringError::unsupported_control_flow(function.machine));
        }
        let source = home.argument_source();
        (home.structural_type(), source)
    } else {
        let parameter = prepared
            .parameter(argument.place)
            .ok_or(LoweringError::unsupported_control_flow(function.machine))?;
        if parameter.access == StructuralAccess::WriteOnlyBorrow {
            return Err(LoweringError::unsupported_control_flow(function.machine));
        }
        (
            parameter.structural_type,
            parameter.placement.clone().into(),
        )
    };
    let (selected, shape, offset, runs) = crate::structural_layout::reference_referent_projection(
        root_type,
        &argument.path,
        argument.access,
        Some(declaration.structural_type),
        types,
    )?;
    let indices = super::scalar_sources::runtime_indices(
        runs,
        function,
        &super::scalar_sources::ScalarSources::from(live),
    )?;
    if selected != declaration.structural_type
        || crate::structural_signature::StructuralCallSignature::parameter_shape(
            crate::structural_signature::ParameterLane::Internal,
            selected,
            shape,
            declaration.access,
            types,
        ) != destination.shape
    {
        return Err(LoweringError::unsupported_control_flow(function.machine));
    }
    Ok(TargetStructuralArgument {
        place: argument.place,
        access: argument.access,
        path: argument.path.clone(),
        root_structural_type: root_type,
        structural_type: selected,
        shape: destination.shape,
        source_byte_offset: offset,
        fixed_array_length: None,
        element_stride: None,
        indices,
        source,
        destination: destination.placement.clone(),
    })
}

pub(crate) fn contains_reference(types: &StructuralTypeLookup<'_>, root: StructuralTypeId) -> bool {
    terminal_psi::structural_type_contains_reference_by(|id| types.get(id).copied(), root)
}

pub(crate) fn leaf_paths(
    types: &StructuralTypeLookup<'_>,
    root: StructuralTypeId,
    maximum_leaves: usize,
) -> Option<Vec<Vec<StructuralPathSegment>>> {
    let mut output = Vec::new();
    let mut pending = vec![(root, Vec::new())];
    while let Some((current, path)) = pending.pop() {
        if !contains_reference(types, current) {
            continue;
        }
        let Some(declaration) = types.get(current) else {
            continue;
        };
        match &declaration.shape {
            StructuralTypeShape::Reference { .. } => {
                if output.len() == maximum_leaves {
                    return None;
                }
                output.push(path);
            }
            StructuralTypeShape::Record { .. }
            | StructuralTypeShape::Sum { .. }
            | StructuralTypeShape::Mixed { .. } => {
                let fields = declaration
                    .shape
                    .direct_fields()
                    .into_iter()
                    .flatten()
                    .map(|field| (None, field))
                    .chain(
                        declaration
                            .shape
                            .sum_cases()
                            .into_iter()
                            .flatten()
                            .flat_map(|case| {
                                case.fields.iter().map(move |field| (Some(case.id), field))
                            }),
                    );
                for (case, field) in fields.rev() {
                    if field.relevance.is_erased() {
                        continue;
                    }
                    if let Some(child) = field.field_type.as_structural() {
                        if !contains_reference(types, child) {
                            continue;
                        }
                        if pending.len().checked_add(output.len())? >= maximum_leaves {
                            return None;
                        }
                        let mut child_path = path.clone();
                        if let Some(case) = case {
                            child_path.push(StructuralPathSegment::Case(case));
                        }
                        child_path.push(StructuralPathSegment::Field(field.identity));
                        pending.push((child, child_path));
                    }
                }
            }
            StructuralTypeShape::FixedArray { element, length } => {
                if !contains_reference(types, *element) {
                    continue;
                }
                let length = length.literal()?;
                let count = usize::try_from(length).ok()?;
                if pending
                    .len()
                    .checked_add(output.len())?
                    .checked_add(count)?
                    > maximum_leaves
                {
                    return None;
                }
                for index in (0..length).rev() {
                    let mut child_path = path.clone();
                    child_path.push(StructuralPathSegment::FixedIndex(index));
                    pending.push((*element, child_path));
                }
            }
            _ => {}
        }
    }
    Some(output)
}

pub(crate) fn referent(
    types: &StructuralTypeLookup<'_>,
    structural_type: StructuralTypeId,
) -> Option<StructuralTypeId> {
    match types.get(structural_type)?.shape {
        StructuralTypeShape::Reference {
            referent,
            access:
                StructuralAccess::SharedBorrow
                | StructuralAccess::MutableBorrow
                | StructuralAccess::WriteOnlyBorrow,
        } => Some(referent),
        _ => None,
    }
}

pub(crate) fn leaf_referent(
    types: &StructuralTypeLookup<'_>,
    current: StructuralTypeId,
    fields: &[StructuralPathSegment],
) -> Option<StructuralTypeId> {
    let leaf = leaf_type(types, current, fields)?;
    match &types.get(leaf)?.shape {
        StructuralTypeShape::Reference { .. } => referent(types, leaf),
        _ => Some(leaf),
    }
}

pub(crate) fn leaf_type(
    types: &StructuralTypeLookup<'_>,
    current: StructuralTypeId,
    fields: &[StructuralPathSegment],
) -> Option<StructuralTypeId> {
    terminal_semantics::runtime_structural_path_tip(types.values().copied(), current, fields)
        .map(|declaration| declaration.id)
        .or_else(|| primitive_projection_declaration(types.values().copied(), current, fields))
}

fn source_contract(
    function: &AbstractFunction,
    live: &LiveDefinitions,
    place: PlaceId,
) -> Option<StructuralTypeId> {
    if let Some(parameter) = function.arrival_parameter(place) {
        return Some(parameter.structural_type);
    }
    if let Some(home) = live.structural_homes.get(&place) {
        return Some(home.structural_type());
    }
    if let Some((_, view)) = live.views.get(&place) {
        return Some(*view);
    }
    live.references
        .get(&(place, Vec::new()))
        .map(|leaf| leaf.carrier_type)
}

fn direct_storage_type(
    function: &AbstractFunction,
    live: &LiveDefinitions,
    types: &StructuralTypeLookup<'_>,
    source: &StructuralArgument,
) -> Option<StructuralTypeId> {
    let (structural_type, access) =
        if let Some(parameter) = function.arrival_parameter(source.place) {
            (parameter.structural_type, parameter.access)
        } else if let Some(home) = live.structural_homes.get(&source.place) {
            (home.structural_type(), home.access())
        } else {
            let result = live.view_result(function, source.place)?;
            (result.structural_type, StructuralAccess::Owned)
        };
    if !source.access.admits(access) {
        return None;
    }
    if source.access == StructuralAccess::SharedBorrow
        && let Some(observed) = terminal_semantics::borrowed_primitive_projection_declaration(
            types.values().copied(),
            structural_type,
            &source.path,
            None,
        )
    {
        return Some(observed);
    }
    leaf_type(types, structural_type, &source.path)
}

pub(crate) fn reference_source_type(
    function: &AbstractFunction,
    live: &LiveDefinitions,
    types: &StructuralTypeLookup<'_>,
    source: &StructuralArgument,
) -> Option<StructuralTypeId> {
    if !matches!(
        source.access,
        StructuralAccess::SharedBorrow
            | StructuralAccess::MutableBorrow
            | StructuralAccess::WriteOnlyBorrow
    ) {
        return None;
    }
    if source.path.contains(&StructuralPathSegment::Referent) {
        let mut current = source_contract(function, live, source.place)?;
        let mut start = 0;
        for (position, segment) in source.path.iter().enumerate() {
            if *segment != StructuralPathSegment::Referent {
                continue;
            }
            current = leaf_type(types, current, &source.path[start..position])?;
            let StructuralTypeShape::Reference { referent, access } = &types.get(current)?.shape
            else {
                return (position + 1 == source.path.len()).then_some(current);
            };
            let compatible = source.access.admits_referent(*access);
            if !compatible {
                return None;
            }
            current = *referent;
            start = position + 1;
        }
        return leaf_type(types, current, &source.path[start..]);
    }
    direct_storage_type(function, live, types, source)
}

fn storage_projection(
    function: &AbstractFunction,
    live: &LiveDefinitions,
    types: &StructuralTypeLookup<'_>,
    source: &StructuralArgument,
) -> Option<optimization_unit_semantics::StructuralStorageProjection> {
    let (carrier, path) = if let Some(crossing) = source
        .path
        .iter()
        .rposition(|segment| *segment == StructuralPathSegment::Referent)
    {
        let leaf = reference_leaf(
            function,
            live,
            types,
            source.place,
            &source.path[..crossing],
        )?;
        let StructuralTypeShape::Reference { referent, access } =
            types.get(leaf.carrier_type)?.shape
        else {
            return None;
        };
        if referent != leaf.referent || !access_covers(access, source.access) {
            return None;
        }
        (referent, &source.path[crossing + 1..])
    } else {
        (
            source_contract(function, live, source.place)?,
            source.path.as_slice(),
        )
    };
    structural_borrowed_storage_projection(types.values().copied(), carrier, path, source.access)
}

fn reference_leaf<'live>(
    function: &AbstractFunction,
    live: &'live LiveDefinitions,
    types: &StructuralTypeLookup<'_>,
    place: PlaceId,
    path: &[StructuralPathSegment],
) -> Option<std::borrow::Cow<'live, ReferenceStorage>> {
    reference_leaf_following(function, live, types, place, path, &mut Vec::new())
}

fn reference_leaf_following<'live>(
    function: &AbstractFunction,
    live: &'live LiveDefinitions,
    types: &StructuralTypeLookup<'_>,
    place: PlaceId,
    path: &[StructuralPathSegment],
    active: &mut Vec<(PlaceId, Vec<StructuralPathSegment>)>,
) -> Option<std::borrow::Cow<'live, ReferenceStorage>> {
    let coordinate = (place, path.to_vec());
    if let Some(resident) = live.references.get(&coordinate) {
        return Some(std::borrow::Cow::Borrowed(resident));
    }
    if active.contains(&coordinate) {
        return None;
    }
    active.push(coordinate);
    let observed = observations::observe(function, live, types, place, path)
        .or_else(|| observations::through_resident(function, live, types, place, path, active));
    active.pop();
    observed.map(std::borrow::Cow::Owned)
}

pub(crate) fn access_source(
    function: &AbstractFunction,
    live: &LiveDefinitions,
    types: &StructuralTypeLookup<'_>,
    source: &StructuralArgument,
) -> Option<target_operations::TargetAccessInterpretation> {
    if source.path.is_empty() {
        if let Some(origins) = live.borrowed_reference_sources.get(&source.place) {
            return agree(origins.as_ref()?.iter().map(|origin| origin.placed.clone()));
        }
        return live
            .access_formals
            .get(&source.place)
            .cloned()
            .map(target_operations::TargetAccessInterpretation::Formal);
    }
    let (StructuralPathSegment::Referent, path) = source.path.split_last()? else {
        return None;
    };
    reference_leaf(function, live, types, source.place, path)?
        .placed
        .clone()
}

pub(crate) fn access_formals(
    function: &AbstractFunction,
    types: &StructuralTypeLookup<'_>,
) -> Result<BTreeMap<PlaceId, terminal_psi::TerminalAccessViewInput>, LoweringError> {
    let catalog = types.catalog();
    let mut formals = BTreeMap::new();
    for input in catalog
        .access_view_inputs()
        .iter()
        .filter(|input| input.machine == function.machine)
    {
        let parameter = function
            .structural_parameters
            .get(input.runtime_position as usize)
            .ok_or_else(|| LoweringError::unsupported_control_flow(function.machine))?;
        let (matched, _) = catalog
            .access_parameter_demand(function.machine, input.runtime_position as usize, parameter)
            .ok_or_else(|| LoweringError::unsupported_control_flow(function.machine))?;
        if matched != input || formals.insert(parameter.place, input.clone()).is_some() {
            return Err(LoweringError::unsupported_control_flow(function.machine));
        }
    }
    Ok(formals)
}

pub(crate) fn access_call_custody(
    caller: &AbstractFunction,
    callee: &AbstractFunction,
    invocation: semantic_vocabulary::InvocationSite,
    arguments: &[StructuralArgument],
    types: &StructuralTypeLookup<'_>,
    live: &LiveDefinitions,
) -> Result<Vec<target_operations::TargetAccessCallBinding>, LoweringError> {
    use crate::model::AccessCallCustodyError;
    let catalog = types.catalog();
    let mut bindings = Vec::new();
    for (runtime_position, parameter) in callee.structural_parameters.iter().enumerate() {
        if !catalog.access_view_inputs().iter().any(|input| {
            input.machine == callee.machine
                && usize::try_from(input.runtime_position).ok() == Some(runtime_position)
        }) {
            continue;
        }
        let invalid = |reason| LoweringError::AccessCallCustody {
            caller: caller.machine,
            callee: callee.machine,
            invocation,
            runtime_position,
            reason,
        };
        let (input, plan) = catalog
            .access_parameter_demand(callee.machine, runtime_position, parameter)
            .ok_or_else(|| invalid(AccessCallCustodyError::FormalDemandMismatch))?;
        let argument = arguments
            .get(runtime_position)
            .filter(|argument| argument.access == parameter.access)
            .ok_or_else(|| invalid(AccessCallCustodyError::FormalDemandMismatch))?;
        let association = match access_source(caller, live, types, argument) {
            Some(target_operations::TargetAccessInterpretation::Captured(captured)) => {
                optimization_unit_semantics::access_references::validate_backing(
                    catalog.as_slice(),
                    catalog.access_views(),
                    plan,
                    captured.source_type(),
                    &captured,
                )
                .map_err(|_| invalid(AccessCallCustodyError::CapturedInterpretationMismatch))?;
                target_operations::TargetAccessCallAssociation::Captured {
                    schema: captured.schema(),
                    carrier: captured.carrier(),
                }
            }
            Some(target_operations::TargetAccessInterpretation::Formal(formal)) => {
                if !parameter.access.admits(formal.access)
                    || !live.access_formals.values().any(|input| input == &formal)
                {
                    return Err(invalid(AccessCallCustodyError::FormalDemandMismatch));
                }
                let source_parameter = caller
                    .structural_parameters
                    .get(formal.runtime_position as usize)
                    .ok_or_else(|| invalid(AccessCallCustodyError::FormalDemandMismatch))?;
                let (_, source_plan) = catalog
                    .access_parameter_demand(
                        caller.machine,
                        formal.runtime_position as usize,
                        source_parameter,
                    )
                    .ok_or_else(|| invalid(AccessCallCustodyError::FormalDemandMismatch))?;
                if source_plan != plan {
                    return Err(invalid(
                        AccessCallCustodyError::CapturedInterpretationMismatch,
                    ));
                }
                target_operations::TargetAccessCallAssociation::CallerFormal(formal)
            }
            None => {
                return Err(invalid(
                    AccessCallCustodyError::CapturedInterpretationMissing,
                ));
            }
        };
        bindings.push(target_operations::TargetAccessCallBinding {
            input: input.clone(),
            argument_index: input.runtime_position,
            association,
        });
    }
    Ok(bindings)
}

pub(crate) fn normalized_source(
    function: &AbstractFunction,
    live: &LiveDefinitions,
    types: &StructuralTypeLookup<'_>,
    source: &StructuralArgument,
) -> Result<ReferenceRoot, LoweringError> {
    let Some(origins) = live.borrowed_reference_sources.get(&source.place) else {
        return normalized_direct_source(function, live, types, source);
    };
    let origins = origins
        .as_ref()
        .filter(|origins| !origins.is_empty())
        .ok_or(LoweringError::unsupported_control_flow(function.machine))?;
    let expected = storage_projection(function, live, types, source)
        .ok_or(LoweringError::unsupported_control_flow(function.machine))?;
    let mut candidates = Vec::with_capacity(origins.len());
    for origin in origins {
        if !source.access.admits(origin.access) {
            return Err(LoweringError::unsupported_control_flow(function.machine));
        }
        if source.path.contains(&StructuralPathSegment::Referent) {
            for resolved in borrowed_arrivals::storage_sources(function, live, types, origin)? {
                let mut resolved = resolved;
                resolved.path.extend_from_slice(&source.path);
                resolved.access = source.access;
                let actual = storage_projection(function, live, types, &resolved);
                if !actual.is_some_and(|actual| actual.matches(types.values().copied(), expected)) {
                    return Err(LoweringError::BorrowedSourceProjectionMismatch {
                        machine: function.machine,
                        source: source.clone(),
                        origin: resolved.clone(),
                        expected,
                        actual,
                        origin_carrier: source_contract(function, live, resolved.place),
                    });
                }
                candidates.push(normalized_direct_source(function, live, types, &resolved)?);
            }
        } else {
            candidates.push(origin.root.projected(&source.path));
        }
    }
    combine_candidate_origins(function, candidates)
}

fn normalized_direct_source(
    function: &AbstractFunction,
    live: &LiveDefinitions,
    types: &StructuralTypeLookup<'_>,
    source: &StructuralArgument,
) -> Result<ReferenceRoot, LoweringError> {
    if let Some(crossing) = source
        .path
        .iter()
        .rposition(|segment| *segment == StructuralPathSegment::Referent)
    {
        let carrier_path = &source.path[..crossing];
        let suffix = &source.path[crossing + 1..];
        let leaf = reference_leaf(function, live, types, source.place, carrier_path);
        let Some(leaf) = leaf else {
            let carrier = source_contract(function, live, source.place)
                .ok_or_else(|| LoweringError::unsupported_control_flow(function.machine))?;
            return match leaf_type(types, carrier, carrier_path).and_then(|leaf| {
                types
                    .get(leaf)
                    .and_then(|declaration| match declaration.shape {
                        StructuralTypeShape::Reference { .. } => None,
                        _ if suffix.is_empty() => Some(()),
                        _ => None,
                    })
            }) {
                Some(()) => Ok(ReferenceRoot::Ingress(source.place, carrier_path.to_vec())),
                None => Err(LoweringError::ReferenceCarrierUnavailable {
                    machine: function.machine,
                    source: source.clone(),
                    carrier,
                    carrier_path: carrier_path.to_vec(),
                }),
            };
        };
        if optimization_unit_semantics::structural_borrowed_storage_projection(
            types.values().copied(),
            leaf.referent,
            suffix,
            source.access,
        )
        .is_none()
        {
            return Err(LoweringError::unsupported_control_flow(function.machine));
        }
        Ok(leaf.root.projected(suffix))
    } else if source.path.is_empty() {
        Ok(ReferenceRoot::Place(source.place))
    } else if storage_projection(function, live, types, source).is_some() {
        Ok(ReferenceRoot::Ingress(source.place, source.path.clone()))
    } else {
        Err(LoweringError::unsupported_control_flow(function.machine))
    }
}

use terminal_psi::structural_access_covers as access_covers;

mod copies {
    use super::*;

    pub(crate) fn storage_regions(
        function: &AbstractFunction,
        types: &StructuralTypeLookup<'_>,
        live: &LiveDefinitions,
        source: PlaceId,
        path: &[StructuralPathSegment],
    ) -> Result<Vec<(PlaceId, Vec<StructuralPathSegment>)>, LoweringError> {
        let mut cursors = if let Some(sources) = live.borrowed_reference_sources.get(&source) {
            let sources = sources
                .as_ref()
                .filter(|sources| !sources.is_empty())
                .ok_or(LoweringError::unsupported_control_flow(function.machine))?;
            let mut cursors = Vec::with_capacity(sources.len());
            for candidate in sources {
                for storage in borrowed_arrivals::storage_sources(function, live, types, candidate)?
                {
                    let cursor = (storage.place, storage.path);
                    if !cursors.contains(&cursor) {
                        cursors.push(cursor);
                    }
                }
            }
            cursors
        } else {
            vec![(source, Vec::new())]
        };
        let mut segment_start = 0;
        let budget = live
            .references
            .len()
            .max(1)
            .checked_mul(path.len().max(1))
            .ok_or(LoweringError::unsupported_control_flow(function.machine))?;
        for (position, segment) in path.iter().enumerate() {
            if *segment != StructuralPathSegment::Referent {
                continue;
            }
            let mut next = Vec::new();
            for (carrier, mut prefix) in cursors {
                prefix.extend_from_slice(&path[segment_start..position]);
                let root = source_contract(function, live, carrier)
                    .ok_or(LoweringError::unsupported_control_flow(function.machine))?;
                let selected = terminal_semantics::stored_descriptor_paths(
                    types.values().copied(),
                    root,
                    &prefix,
                    budget,
                )
                .ok_or(LoweringError::unsupported_control_flow(function.machine))?;
                for selected in selected {
                    let leaf = live
                        .references
                        .get(&(carrier, selected.clone()))
                        .cloned()
                        .or_else(|| {
                            observations::observe(function, live, types, carrier, &selected)
                        })
                        .ok_or(LoweringError::unsupported_control_flow(function.machine))?;
                    if leaf_type(types, root, &selected) != Some(leaf.carrier_type) {
                        return Err(LoweringError::unsupported_control_flow(function.machine));
                    }
                    for origin in leaf.root.origins() {
                        let cursor = storage_cursor(function, types, live, origin, leaf.referent)?;
                        if !next.contains(&cursor) {
                            if next.len() >= budget {
                                return Err(LoweringError::unsupported_control_flow(
                                    function.machine,
                                ));
                            }
                            next.push(cursor);
                        }
                    }
                }
            }
            cursors = next;
            segment_start = position + 1;
        }
        for (_, prefix) in &mut cursors {
            prefix.extend_from_slice(&path[segment_start..]);
        }
        Ok(cursors)
    }

    pub(crate) fn storage_cursor(
        function: &AbstractFunction,
        types: &StructuralTypeLookup<'_>,
        live: &LiveDefinitions,
        origin: target_operations::TargetReferenceOrigin,
        expected: StructuralTypeId,
    ) -> Result<(PlaceId, Vec<StructuralPathSegment>), LoweringError> {
        let invalid = || LoweringError::unsupported_control_flow(function.machine);
        let mut path = origin.exact_path().ok_or_else(invalid)?;
        let place = origin.base.root_place().ok_or_else(invalid)?;
        let root = source_contract(function, live, place).ok_or_else(invalid)?;
        let selected = leaf_type(types, root, &path).ok_or_else(invalid)?;
        if selected != expected {
            let parameter = function
                .structural_parameter(place)
                .or_else(|| function.block_structural_parameter(place))
                .ok_or_else(invalid)?;
            if parameter.structural_type != root
                || path.iter().any(|segment| {
                    !matches!(
                        segment,
                        StructuralPathSegment::Field(_)
                            | StructuralPathSegment::Case(_)
                            | StructuralPathSegment::FixedIndex(_)
                    )
                })
                || !matches!(types.get(selected).map(|declaration| &declaration.shape),
                Some(StructuralTypeShape::Reference { referent, .. }) if *referent == expected)
            {
                return Err(invalid());
            }
            path.push(StructuralPathSegment::Referent);
        }
        Ok((place, path))
    }

    pub(crate) fn copied_leaves(
        function: &AbstractFunction,
        types: &StructuralTypeLookup<'_>,
        live: &LiveDefinitions,
        result: &StructuralOperationResult,
        source: PlaceId,
        path: &abstract_operations::StructuralCopyPath,
    ) -> Result<BTreeMap<(PlaceId, Vec<StructuralPathSegment>), ReferenceStorage>, LoweringError>
    {
        let mut copied = BTreeMap::new();
        if !contains_reference(types, result.structural_type) {
            return Ok(copied);
        }
        let source_type = source_contract(function, live, source)
            .ok_or(LoweringError::unsupported_control_flow(function.machine))?;
        let path: std::borrow::Cow<'_, [StructuralPathSegment]> = match path {
            abstract_operations::StructuralCopyPath::Ordinary(path) => {
                std::borrow::Cow::Borrowed(path)
            }
            abstract_operations::StructuralCopyPath::Case(path) => std::borrow::Cow::Owned(
                terminal_semantics::canonical_structural_storage_path(
                    types.values().copied(),
                    source_type,
                    path,
                )
                .ok_or(LoweringError::unsupported_control_flow(function.machine))?,
            ),
        };
        if leaf_type(types, source_type, &path) != Some(result.structural_type)
            || live
                .references
                .keys()
                .any(|(carrier, _)| *carrier == result.place)
            || live.structural_homes.contains_key(&result.place)
        {
            return Err(LoweringError::unsupported_control_flow(function.machine));
        }
        let cursors = storage_regions(function, types, live, source, &path)?;
        let mut candidates: BTreeMap<Vec<StructuralPathSegment>, Vec<ReferenceStorage>> =
            BTreeMap::new();
        for (carrier, prefix) in cursors {
            let root = source_contract(function, live, carrier)
                .ok_or(LoweringError::unsupported_control_flow(function.machine))?;
            let selected = terminal_semantics::StructuralStorageSelection::new(
                types.values().copied(),
                root,
                &prefix,
            )
            .ok_or(LoweringError::unsupported_control_flow(function.machine))?;
            if leaf_type(types, root, &prefix) != Some(result.structural_type) {
                return Err(LoweringError::unsupported_control_flow(function.machine));
            }
            for ((resident, path), original) in &live.references {
                if *resident != carrier {
                    continue;
                }
                let Some(relative) = selected.relative_path(path) else {
                    continue;
                };
                if relative.contains(&StructuralPathSegment::Referent) {
                    continue;
                }
                let carrier_type = leaf_type(types, result.structural_type, relative)
                    .ok_or(LoweringError::unsupported_control_flow(function.machine))?;
                if carrier_type != original.carrier_type
                    || leaf_referent(types, result.structural_type, relative)
                        != Some(original.referent)
                {
                    return Err(LoweringError::unsupported_control_flow(function.machine));
                }
                candidates
                    .entry(relative.to_vec())
                    .or_default()
                    .push(original.clone());
            }
        }
        for (relative, alternatives) in candidates {
            copied.insert(
                (result.place, relative),
                combine_alternatives(function, alternatives)?,
            );
        }
        Ok(copied)
    }
}
mod observations {
    use super::*;
    use terminal_semantics::descriptor_lifetimes::{
        descriptor_location, descriptor_path, fixed_runtime_index_alternatives,
    };
    use terminal_semantics::descriptor_results::{DescriptorCallOrigin, DescriptorSourceOrigin};

    pub(crate) fn through_resident(
        function: &AbstractFunction,
        live: &LiveDefinitions,
        types: &StructuralTypeLookup<'_>,
        place: PlaceId,
        path: &[StructuralPathSegment],
        active: &mut Vec<(PlaceId, Vec<StructuralPathSegment>)>,
    ) -> Option<ReferenceStorage> {
        let crossing = path
            .iter()
            .position(|segment| *segment == StructuralPathSegment::Referent)?;
        let carrier = super::reference_leaf_following(
            function,
            live,
            types,
            place,
            &path[..crossing],
            active,
        )?;
        let expected = leaf_type(types, source_contract(function, live, place)?, path)?;
        let mut candidates = Vec::new();
        for origin in carrier.root.origins() {
            let target_operations::TargetReferenceOriginBase::Storage { root } = origin.base else {
                return None;
            };
            let mut selected = origin.exact_path()?;
            selected.extend_from_slice(&path[crossing + 1..]);
            let resident =
                super::reference_leaf_following(function, live, types, root, &selected, active)?;
            if resident.carrier_type != expected {
                return None;
            }
            candidates.push(resident.into_owned());
        }
        combine_alternatives(function, candidates).ok()
    }

    pub(crate) fn observe(
        function: &AbstractFunction,
        live: &LiveDefinitions,
        types: &StructuralTypeLookup<'_>,
        place: PlaceId,
        path: &[StructuralPathSegment],
    ) -> Option<ReferenceStorage> {
        let root = source_contract(function, live, place)?;
        let carrier_type = leaf_type(types, root, path)?;
        let StructuralTypeShape::Reference { referent, .. } = types.get(carrier_type)?.shape else {
            return None;
        };
        let locations =
            if let Some(location) = descriptor_location(types.catalog(), place, root, path, None) {
                vec![location]
            } else {
                fixed_runtime_index_alternatives(types.catalog(), place, root, path)?
            };
        let mut candidates = Vec::with_capacity(locations.len());
        for location in locations {
            let selected_path = descriptor_path(types.catalog(), root, &location.path)?;
            if let Some(leaf) = live.references.get(&(place, selected_path)) {
                if leaf.carrier_type != carrier_type || leaf.referent != referent {
                    return None;
                }
                candidates.push(leaf.clone());
                continue;
            }
            let mut origins = Vec::new();
            let snapshot = live
                .stored_reference_effects
                .observations
                .get(&function.machine)?
                .get(&live.reference_position)?;
            let current = sources_at_storage(snapshot, &location);
            let current = current?;
            for source in current {
                let origin = if let Some(slot) = source.captured_entry_slot() {
                    let contract = function.structural_parameter(slot.root)?.structural_type;
                    let slot_path = descriptor_path(types.catalog(), contract, &slot.path)?;
                    if leaf_type(types, contract, &slot_path) != Some(carrier_type) {
                        return None;
                    }
                    target_operations::TargetReferenceOrigin {
                        base: target_operations::TargetReferenceOriginBase::IncomingContents {
                            root: slot.root,
                            slot: slot_path,
                        },
                        path: Vec::new(),
                        extent: semantic_vocabulary::ReferenceOriginExtent::Projected,
                    }
                } else {
                    match source {
                        DescriptorSourceOrigin::Provider(provider) => {
                            let origin = live.stored_reference_effects.provider_origin(provider)?;
                            let target_operations::TargetReferenceOriginBase::Provider {
                                referent: promised,
                                access,
                                ..
                            } = &origin.base
                            else {
                                return None;
                            };
                            let StructuralTypeShape::Reference {
                                access: required, ..
                            } = types.get(carrier_type)?.shape
                            else {
                                return None;
                            };
                            if leaf_type(types, *promised, &provider.projection.path)
                                != Some(referent)
                                || !access_covers(*access, required)
                            {
                                return None;
                            }
                            origin
                        }
                        DescriptorSourceOrigin::Backing(DescriptorCallOrigin::Argument {
                            source,
                            projection,
                        }) if projection.exact
                            && !projection.indirect
                            && projection.window.is_none()
                            && projection.captured_at.is_none() =>
                        {
                            target_operations::TargetReferenceOrigin {
                                base: target_operations::TargetReferenceOriginBase::Storage {
                                    root: *source,
                                },
                                path: projection
                                    .path
                                    .iter()
                                    .cloned()
                                    .map(terminal_psi::ReferenceOriginProjection::Exact)
                                    .collect(),
                                extent: semantic_vocabulary::ReferenceOriginExtent::Projected,
                            }
                        }
                        _ => return None,
                    }
                };
                origins.push(origin);
            }
            if origins.is_empty() {
                return None;
            }
            origins.sort();
            origins.dedup();
            candidates.push(ReferenceStorage {
                root: ReferenceRoot::Alternatives(origins),
                carrier_type,
                referent,
                placed: None,
            });
        }
        combine_alternatives(function, candidates).ok()
    }

    fn sources_at_storage<'snapshot>(
        snapshot: &'snapshot BTreeMap<
            terminal_semantics::descriptor_lifetimes::DescriptorLocation,
            terminal_semantics::descriptor_results::DescriptorSourceObservation,
        >,
        location: &terminal_semantics::descriptor_lifetimes::DescriptorLocation,
    ) -> Option<Vec<&'snapshot DescriptorSourceOrigin>> {
        if let Some(observed) = snapshot.get(location) {
            return Some(
                observed
                    .origins
                    .as_ref()
                    .filter(|origins| !origins.is_empty())?
                    .iter()
                    .collect(),
            );
        }
        let mut sources = Vec::new();
        for observed in snapshot
            .values()
            .filter(|observed| observed.storage.as_ref() == Some(location))
        {
            sources.extend(
                observed
                    .origins
                    .as_ref()
                    .filter(|origins| !origins.is_empty())?
                    .iter(),
            );
        }
        (!sources.is_empty()).then_some(sources)
    }
}
pub(crate) use copies::copied_leaves;
mod arrivals {
    use super::*;

    pub(crate) fn merge_arrivals(
        function: &AbstractFunction,
        left: &mut BTreeMap<(PlaceId, Vec<StructuralPathSegment>), ReferenceStorage>,
        right: &BTreeMap<(PlaceId, Vec<StructuralPathSegment>), ReferenceStorage>,
    ) -> Result<(), LoweringError> {
        for (key, incoming) in right {
            let Some(existing) = left.get_mut(key) else {
                left.insert(key.clone(), incoming.clone());
                continue;
            };
            if existing.carrier_type != incoming.carrier_type
                || existing.referent != incoming.referent
            {
                return Err(LoweringError::unsupported_control_flow(function.machine));
            }
            if existing.root != incoming.root {
                existing.root = combine_candidate_origins(
                    function,
                    vec![existing.root.clone(), incoming.root.clone()],
                )?;
            }
        }
        Ok(())
    }
}
pub(crate) use arrivals::merge_arrivals;
mod stored_calls {
    use super::*;
    use abstract_operations::call_source;
    use semantic_vocabulary::{BoundaryMachineId, MachineId};
    use std::borrow::Cow;
    use terminal_semantics::descriptor_lifetimes::descriptor_path;
    use terminal_semantics::descriptor_results::{
        DescriptorAnalysis, DescriptorSlotKind, DescriptorStoredOrigin,
    };

    type Location = (PlaceId, Vec<StructuralPathSegment>);
    struct Effect {
        destination: StructuralArgument,
        sources: Vec<Source>,
    }
    #[derive(PartialEq, Eq, PartialOrd, Ord)]
    enum Source {
        Incoming(StructuralArgument),
        Backing(StructuralArgument),
        Provider(target_operations::TargetReferenceOrigin),
    }

    pub(crate) struct StoredEffects {
        machines: BTreeMap<MachineId, Result<Vec<Effect>, &'static str>>,
        provider_outputs: BTreeMap<(BoundaryMachineId, u32), (StructuralTypeId, StructuralAccess)>,
        pub(crate) observations: BTreeMap<
            MachineId,
            terminal_semantics::descriptor_results::DescriptorSourceObservations,
        >,
        pub(crate) access_results: BTreeMap<
            MachineId,
            Option<
                Vec<optimization_unit_semantics::descriptor_results::access_results::ResultSlot>,
            >,
        >,
    }
    pub(crate) type StoredEffectCatalog = std::sync::Arc<StoredEffects>;

    impl StoredEffects {
        pub(crate) fn provider_origin<Dependency>(
            &self,
            provider: &terminal_semantics::descriptor_results::DescriptorProviderOrigin<Dependency>,
        ) -> Option<target_operations::TargetReferenceOrigin> {
            let (referent, access) = *self
                .provider_outputs
                .get(&(provider.boundary, provider.output))?;
            if provider.invocations.is_empty()
                || !provider.projection.exact
                || provider.projection.indirect
                || provider.projection.window.is_some()
                || provider.projection.captured_at.is_some()
            {
                return None;
            }
            Some(target_operations::TargetReferenceOrigin {
                base: target_operations::TargetReferenceOriginBase::Provider {
                    boundary: provider.boundary,
                    output: provider.output,
                    invocations: provider
                        .invocations
                        .iter()
                        .map(|site| (site.machine, site.operation))
                        .collect(),
                    referent,
                    access,
                },
                path: provider
                    .projection
                    .path
                    .iter()
                    .cloned()
                    .map(terminal_psi::ReferenceOriginProjection::Exact)
                    .collect(),
                extent: semantic_vocabulary::ReferenceOriginExtent::Projected,
            })
        }
    }

    fn writable_slots(function: &AbstractFunction, types: &StructuralTypeLookup<'_>) -> bool {
        function.structural_parameters.iter().any(|parameter| {
            DescriptorSlotKind::References
                .writable_slots(
                    types.catalog(),
                    parameter.place,
                    parameter.structural_type,
                    parameter.access,
                )
                .is_none_or(|slots| !slots.is_empty())
        })
    }

    fn trace_origin(machine: MachineId, origin: &DescriptorStoredOrigin) {
        if std::env::var_os("OMEGA_COMPILER_CHECK_SITES").is_none() {
            return;
        }
        match origin {
            DescriptorStoredOrigin::IncomingSlot { position, path } => eprintln!(
                "CHECKSITE stored-reference-origin machine={machine:?} kind=incoming-slot parameter={position} path={path:?}"
            ),
            DescriptorStoredOrigin::Parameter {
                position,
                projection,
            } => eprintln!(
                "CHECKSITE stored-reference-origin machine={machine:?} kind=parameter parameter={position} projection={projection:?}"
            ),
            DescriptorStoredOrigin::IncomingContents {
                position,
                steps,
                projection,
            } => eprintln!(
                "CHECKSITE stored-reference-origin machine={machine:?} kind=incoming-contents parameter={position} steps={steps:?} projection={projection:?}"
            ),
            DescriptorStoredOrigin::Invocation {
                carrier_type,
                projection,
            } => eprintln!(
                "CHECKSITE stored-reference-origin machine={machine:?} kind=invocation carrier={carrier_type:?} projection={projection:?}"
            ),
            DescriptorStoredOrigin::AccessFormation {
                machine: owner,
                operation,
                place,
                carrier,
                projection,
            } => eprintln!(
                "CHECKSITE stored-reference-origin machine={machine:?} kind=access-formation owner={owner:?} operation={operation:?} place={place:?} carrier={carrier:?} projection={projection:?}"
            ),
            DescriptorStoredOrigin::Provider(provider) => eprintln!(
                "CHECKSITE stored-reference-origin machine={machine:?} kind=provider boundary={:?} output={} invocations={:?} projection={:?} lifetime_dependencies={}",
                provider.boundary,
                provider.output,
                provider.invocations,
                provider.projection,
                provider.lifetime_dependencies.len()
            ),
            DescriptorStoredOrigin::Literal {
                machine: owner,
                place,
            } => eprintln!(
                "CHECKSITE stored-reference-origin machine={machine:?} kind=literal owner={owner:?} place={place:?}"
            ),
            DescriptorStoredOrigin::Empty => {
                eprintln!("CHECKSITE stored-reference-origin machine={machine:?} kind=empty")
            }
        }
    }

    pub(crate) fn catalog(
        functions: &BTreeMap<MachineId, &AbstractFunction>,
        boundaries: &[terminal_psi::BoundaryMachineDeclaration],
        types: &StructuralTypeLookup<'_>,
        dynamic_dispatch: &terminal_psi::TerminalDynamicDispatchCatalog,
        selected_calls: &crate::selected_boundary_calls::SelectedCallsByCall,
        settlements: &BTreeMap<BoundaryMachineId, target_operations::BoundarySettlementBinding>,
    ) -> StoredEffectCatalog {
        let candidates = functions
            .values()
            .copied()
            .filter(|function| writable_slots(function, types))
            .collect::<Vec<_>>();
        let mut effects = StoredEffects {
            provider_outputs: BTreeMap::new(),
            observations: BTreeMap::new(),
            access_results: BTreeMap::new(),
            machines: candidates
                .iter()
                .map(|function| {
                    (
                        function.machine,
                        Err("stored-reference catalog graph could not be reconstructed"),
                    )
                })
                .collect(),
        };
        for boundary in boundaries {
            if !settlements.get(&boundary.id).is_some_and(|settlement| {
                matches!(
                    settlement.realization,
                    target_operations::BoundarySettlementRealization::NormalizedForeignCall(_)
                )
            }) {
                continue;
            }
            for (position, output) in boundary.reference_outputs.iter().enumerate() {
                let root = match output.destination {
                    terminal_psi::BoundaryReferenceDestination::Parameter(position) => boundary
                        .structural_parameters
                        .get(position as usize)
                        .map(|parameter| parameter.structural_type),
                };
                let Some(StructuralTypeShape::Reference { referent, access }) = root
                    .and_then(|root| leaf_type(types, root, &output.path))
                    .and_then(|carrier| types.get(carrier).map(|declaration| &declaration.shape))
                else {
                    continue;
                };
                if *access == output.access
                    && let Ok(position) = u32::try_from(position)
                {
                    effects
                        .provider_outputs
                        .insert((boundary.id, position), (*referent, *access));
                }
            }
        }
        let Some(graph) =
        optimization_unit_semantics::descriptor_results::abstract_reference_catalog_graph_with_boundary_preservation(
            functions.values().copied(),
            boundaries,
            types.catalog(),
        dynamic_dispatch,
            |function, node| {
                if let AbstractOperation::BoundaryCall { psi_operation, boundary, .. } = &node.operation
                    && let Some(selected) = selected_calls.get(&(function.machine, (*psi_operation).into(), *boundary))
                {
                    return super::super::selected_boundary_calls::resolve(&node.operation, selected, functions)
                        .ok().map(|(operation, _)| Cow::Owned(operation));
                }
                Some(Cow::Borrowed(&node.operation))
            },
            |function, node| {
                let AbstractOperation::BoundaryCall {
                    psi_operation, boundary, arguments, structural_arguments,
                    completion_claim_sources, completion_receipts, ..
                } = &node.operation else { return false; };
                if selected_calls.contains_key(&(function.machine, (*psi_operation).into(), *boundary))
                    || !completion_claim_sources.is_empty() || !completion_receipts.is_empty() {
                    return false;
                }
                let Some(settlement) = settlements.get(boundary) else { return false; };
                let target_operations::BoundarySettlementRealization::NormalizedForeignCall(binding) =
                    &settlement.realization else { return false; };
                let mut declarations = boundaries.iter().filter(|declaration| declaration.id == *boundary);
                let Some(declaration) = declarations.next() else { return false; };
                declarations.next().is_none()
                    && arguments.len() == declaration.scalar_parameters.len()
                    && structural_arguments.len() == declaration.structural_parameters.len()
                    && structural_arguments.iter().zip(&declaration.structural_parameters)
                        .all(|(actual, formal)| actual.access == formal.access)
                    && target_operations::descriptor_input_guard(
                        declaration,
                        binding,
                        types.catalog(),
                        |buffer| {
                            crate::borrowed_storage::capture_type(buffer, types)
                        },
                    )
                    .is_ok_and(|guard| guard.is_some())
            },
        )
    else {
        if std::env::var_os("OMEGA_COMPILER_CHECK_SITES").is_some() {
            eprintln!("CHECKSITE stored-reference-catalog phase=graph result=unresolved");
        }
        return std::sync::Arc::new(effects);
    };
        let mut analysis = DescriptorAnalysis::with_reference_slots(types.catalog(), &graph);
        let has_access_views = !types.catalog().access_views().is_empty();
        for machine in &graph {
            effects
                .observations
                .insert(machine.id, analysis.source_observations(machine.id));
            let result = if has_access_views {
                functions.get(&machine.id).and_then(|function| {
                    let result = function.result.structural()?;
                    optimization_unit_semantics::descriptor_results::access_results::summarize(
                        types.catalog(),
                        boundaries,
                        &function.structural_parameters,
                        result.structural_type,
                        &analysis.result_slots(machine.id)?,
                        |machine, operation| {
                            functions
                                .get(&machine)?
                                .operations
                                .iter()
                                .find(|candidate| candidate.psi_operation() == Some(operation))
                        },
                    )
                })
            } else {
                None
            };
            effects.access_results.insert(machine.id, result);
        }
        for function in candidates {
            let summary = analysis
            .stored_effects(function.machine)
            .ok_or("callee stored-reference body replay is unresolved or invalidates contents")
            .and_then(|rows| {
            if std::env::var_os("OMEGA_COMPILER_CHECK_SITES").is_some() {
                eprintln!("CHECKSITE stored-reference-catalog machine={:?} phase=replay rows={}", function.machine, rows.len());
            }
            rows.into_iter()
                .map(|row| {
                    if std::env::var_os("OMEGA_COMPILER_CHECK_SITES").is_some() {
                        eprintln!("CHECKSITE stored-reference-effect machine={:?} parameter={} steps={:?} origins={}", function.machine, row.parameter, row.steps, row.origins.len());
                    }
                    let parameter = function.structural_parameters.get(row.parameter)
                        .ok_or("stored-reference destination parameter is unavailable")?;
                    let destination = StructuralArgument {
                        place: parameter.place,
                        path: descriptor_path(
                            types.catalog(),
                            parameter.structural_type,
                            &row.exact_path().ok_or("stored-reference destination is not an exact path")?,
                        ).ok_or("stored-reference destination path has no typed storage")?,
                        access: parameter.access,
                    };
                    let mut sources = Vec::new();
                    for origin in row.origins {
                        trace_origin(function.machine, &origin);
                        if let Some((position, path)) = origin.captured_entry_slot() {
                            let parameter = function.structural_parameters.get(position)
                                .ok_or("stored-reference incoming parameter is unavailable")?;
                            sources.push(Source::Incoming(StructuralArgument {
                                place: parameter.place,
                                path: descriptor_path(
                                    types.catalog(),
                                    parameter.structural_type,
                                    &path,
                                ).ok_or("stored-reference incoming path has no typed storage")?,
                                access: parameter.access,
                            }));
                            continue;
                        }
                        match origin {
                            DescriptorStoredOrigin::Parameter {
                                position,
                                projection,
                            } => {
                                if !projection.exact
                                    || projection.indirect
                                    || projection.window.is_some()
                                    || projection.captured_at.is_some()
                                {
                                    return Err("stored-reference parameter projection is not direct current storage");
                                }
                                let source = function.structural_parameters.get(position)
                                    .ok_or("stored-reference backing parameter is unavailable")?;
                                let carrier =
                                    leaf_type(types, parameter.structural_type, &destination.path)
                                        .ok_or("stored-reference destination carrier type is unavailable")?;
                                let StructuralTypeShape::Reference { access, .. } =
                                    types.get(carrier).ok_or("stored-reference destination carrier declaration is unavailable")?.shape
                                else {
                                    return Err("stored-reference destination carrier is not a reference");
                                };
                                if !access_covers(source.access, access) {
                                    return Err("stored-reference backing parameter access is insufficient");
                                }
                                sources.push(Source::Backing(StructuralArgument {
                                    place: source.place,
                                    path: projection.path,
                                    access,
                                }));
                            }
                            DescriptorStoredOrigin::Empty => {}
                            DescriptorStoredOrigin::Provider(provider) => {
                                sources.push(Source::Provider(effects.provider_origin(&provider)
                                    .ok_or("stored-reference provider origin is unresolved")?));
                            }
                            DescriptorStoredOrigin::Invocation { .. } => return Err("stored-reference invocation origin has no physical metadata conversion"),
                            DescriptorStoredOrigin::IncomingSlot { .. }
                            | DescriptorStoredOrigin::IncomingContents { .. } => return Err("stored-reference captured incoming contents have no physical metadata conversion"),
                            DescriptorStoredOrigin::Literal { .. } => return Err("stored-reference literal origin has no physical metadata conversion"),
                            DescriptorStoredOrigin::AccessFormation { .. } => return Err("stored-reference access formation has no physical metadata conversion"),
                        }
                    }
                    sources.sort();
                    sources.dedup();
                    Ok(Effect {
                        destination,
                        sources,
                    })
                })
                .collect()
        });
            if let Err(reason) = &summary
                && std::env::var_os("OMEGA_COMPILER_CHECK_SITES").is_some()
            {
                eprintln!(
                    "CHECKSITE stored-reference-catalog machine={:?} result=unresolved reason={reason}",
                    function.machine
                );
            }
            effects.machines.insert(function.machine, summary);
        }
        std::sync::Arc::new(effects)
    }

    pub(crate) struct Staged {
        replacements: Vec<(Location, Option<ReferenceStorage>)>,
    }

    pub(crate) fn stage(
        function: &AbstractFunction,
        operation: semantic_vocabulary::OperationId,
        callee: Option<&AbstractFunction>,
        types: &StructuralTypeLookup<'_>,
        live: &LiveDefinitions,
        arguments: &[StructuralArgument],
    ) -> Result<Staged, LoweringError> {
        let invalid = |reason| LoweringError::StoredReferenceCall {
            caller: function.machine,
            callee: callee.map(|callee| callee.machine),
            operation,
            reason,
        };
        let mut staged = Staged {
            replacements: Vec::new(),
        };
        let Some(callee) = callee else {
            if arguments.iter().any(|argument| {
                source_contract(function, live, argument.place)
                    .and_then(|ty| leaf_type(types, ty, &argument.path))
                    .is_some_and(|ty| {
                        DescriptorSlotKind::References
                            .writable_slots(types.catalog(), argument.place, ty, argument.access)
                            .is_none_or(|slots| !slots.is_empty())
                    })
            }) {
                return Err(invalid(
                    "dynamic call has no stored-reference effect summary",
                ));
            }
            return Ok(staged);
        };
        let Some(effects) = live.stored_reference_effects.machines.get(&callee.machine) else {
            return Ok(staged);
        };
        let effects = effects.as_ref().map_err(|reason| invalid(*reason))?;
        let actual = |formal: &StructuralArgument| {
            let argument = call_source(&callee.structural_parameters, arguments, formal)
                .ok_or_else(|| invalid("effect formal has no corresponding actual argument"))?;
            window_storage(function, types, live, argument.place, &argument.path)
        };
        let mut destinations = BTreeSet::new();
        for effect in effects {
            let destination = actual(&effect.destination)?;
            if !destinations.insert(destination.clone()) {
                return Err(invalid(
                    "stored-reference effects overlap at one destination",
                ));
            }
            let carrier_type = source_contract(function, live, destination.0)
                .and_then(|ty| leaf_type(types, ty, &destination.1))
                .ok_or_else(|| invalid("destination has no structural carrier type"))?;
            let referent = referent(types, carrier_type)
                .ok_or_else(|| invalid("destination carrier is not a reference"))?;
            let StructuralTypeShape::Reference {
                access: requested, ..
            } = types
                .get(carrier_type)
                .ok_or_else(|| invalid("destination reference type is unavailable"))?
                .shape
            else {
                return Err(invalid("destination carrier is not a reference"));
            };
            let mut candidates = Vec::new();
            for source in &effect.sources {
                let mut leaf = match source {
                    Source::Incoming(formal) => {
                        let position = terminal_psi::structural_parameter_position(
                            &callee.structural_parameters,
                            formal.place,
                        )
                        .ok_or_else(|| invalid("incoming source has no declared parameter"))?;
                        let parameter = &callee.structural_parameters[position];
                        let supplied = arguments
                            .get(position)
                            .ok_or_else(|| invalid("incoming source has no actual argument"))?;
                        let mut path = formal.path.clone();
                        path.push(StructuralPathSegment::Referent);
                        if !access_covers(supplied.access, formal.access)
                        || !abstract_operations::control_flow::address_joins::loan_access_permitted(
                            types.values().copied(),
                            parameter.structural_type,
                            &path,
                            formal.access,
                            requested,
                        )
                    {
                        return Err(invalid(
                            "incoming reference exceeds its original access ceiling",
                        ));
                    }
                        let source = actual(formal)?;
                        let Some(leaf) = live.references.get(&source) else {
                            if source
                                .1
                                .iter()
                                .any(|segment| matches!(segment, StructuralPathSegment::Case(_)))
                            {
                                continue;
                            }
                            return Err(invalid("incoming reference slot has no live provenance"));
                        };
                        leaf.clone()
                    }
                    Source::Backing(formal) => {
                        let source = call_source(&callee.structural_parameters, arguments, formal)
                            .ok_or_else(|| {
                                invalid("backing formal has no corresponding actual argument")
                            })?;
                        if reference_source_type(function, live, types, &source) != Some(referent) {
                            return Err(invalid(
                                "backing argument does not have the destination referent type",
                            ));
                        }
                        ReferenceStorage {
                            root: normalized_source(function, live, types, &source)?,
                            carrier_type,
                            referent,
                            placed: super::access_source(function, live, types, &source),
                        }
                    }
                    Source::Provider(origin) => {
                        let mut origin = origin.clone();
                        let target_operations::TargetReferenceOriginBase::Provider {
                            referent: promised,
                            access,
                            invocations,
                            ..
                        } = &mut origin.base
                        else {
                            return Err(invalid("provider output has no admitted origin"));
                        };
                        let projected = origin
                            .path
                            .iter()
                            .map(|segment| match segment {
                                terminal_psi::ReferenceOriginProjection::Exact(segment) => {
                                    Some(segment.clone())
                                }
                                _ => None,
                            })
                            .collect::<Option<Vec<_>>>()
                            .ok_or_else(|| {
                                invalid("provider output has unresolved referent projection")
                            })?;
                        let StructuralTypeShape::Reference {
                            access: required, ..
                        } = types
                            .get(carrier_type)
                            .ok_or_else(|| invalid("reference carrier is undeclared"))?
                            .shape
                        else {
                            return Err(invalid("provider output destination is not a reference"));
                        };
                        if leaf_type(types, *promised, &projected) != Some(referent)
                            || !access_covers(*access, required)
                        {
                            return Err(invalid(
                                "provider promise does not cover the destination reference",
                            ));
                        }
                        invocations.insert(0, (function.machine, operation));
                        ReferenceStorage {
                            root: ReferenceRoot::Alternatives(vec![origin]),
                            carrier_type,
                            referent,
                            placed: None,
                        }
                    }
                };
                let StructuralTypeShape::Reference {
                    referent: source_referent,
                    access: source_access,
                } = types
                    .get(leaf.carrier_type)
                    .ok_or_else(|| invalid("source reference type is unavailable"))?
                    .shape
                else {
                    return Err(invalid("source carrier is not a reference"));
                };
                if leaf.referent != referent
                    || source_referent != referent
                    || !access_covers(source_access, requested)
                {
                    return Err(invalid("source and destination reference types disagree"));
                }
                leaf.carrier_type = carrier_type;
                candidates.push(leaf);
            }
            let replacement = if candidates.is_empty() {
                None
            } else {
                Some(combine_alternatives(function, candidates)?)
            };
            staged.replacements.push((destination, replacement));
        }
        Ok(staged)
    }

    pub(crate) fn commit(live: &mut LiveDefinitions, staged: Staged) {
        for (destination, replacement) in staged.replacements {
            if let Some(leaf) = replacement {
                live.references.insert(destination, leaf);
            } else {
                live.references.remove(&destination);
            }
        }
    }
}
pub(crate) use stored_calls::{StoredEffectCatalog, catalog as stored_effect_catalog};
pub(crate) use stored_calls::{commit as commit_stored_effects, stage as stage_stored_effects};

pub(crate) fn moved_argument_leaves(
    live: &LiveDefinitions,
    arguments: &[StructuralArgument],
) -> BTreeMap<(PlaceId, Vec<StructuralPathSegment>), ReferenceStorage> {
    let mut moved = BTreeMap::new();
    for argument in arguments {
        if argument.access != StructuralAccess::Owned
            || argument.path.contains(&StructuralPathSegment::Referent)
        {
            continue;
        }
        for ((carrier, path), leaf) in &live.references {
            if *carrier == argument.place
                && path
                    .strip_prefix(argument.path.as_slice())
                    .is_some_and(|relative| !relative.contains(&StructuralPathSegment::Referent))
            {
                moved.insert((*carrier, path.clone()), leaf.clone());
            }
        }
    }
    moved
}

pub(crate) fn commit_argument_moves(
    live: &mut LiveDefinitions,
    moved: &BTreeMap<(PlaceId, Vec<StructuralPathSegment>), ReferenceStorage>,
) {
    for key in moved.keys() {
        live.references.remove(key);
    }
}

pub(crate) fn establish(
    operation: &AbstractOperation,
    site: &mut RowSite<'_>,
) -> Result<(), LoweringError> {
    let (function, types, live, operations, provenance) = site.parts();
    let AbstractOperation::EstablishReference {
        psi_operation,
        result,
        source,
        storage,
    } = operation
    else {
        return Err(LoweringError::unsupported_control_flow(function.machine));
    };
    let referent = referent(types, result.structural_type)
        .ok_or_else(|| LoweringError::unsupported_control_flow(function.machine))?;
    if result.multiplicity != StructuralMultiplicity::Affine
        || live.structural_homes.contains_key(&result.place)
        || live
            .references
            .keys()
            .any(|(carrier, _)| *carrier == result.place)
    {
        return Err(LoweringError::unsupported_control_flow(function.machine));
    }
    let compatible = match storage {
        Some(storage)
            if matches!(
                source.path.last(),
                Some(StructuralPathSegment::FixedRange { .. })
            ) =>
        {
            source_contract(function, live, source.place).is_some_and(|root| {
                terminal_semantics::reference_storage_region_source_compatible(
                    types.values().copied(),
                    root,
                    &source.path,
                    result.structural_type,
                    storage.stored_scalar,
                )
            })
        }
        Some(storage) => {
            reference_source_type(function, live, types, source).is_some_and(|actual| {
                terminal_semantics::reference_storage_source_compatible(
                    types.values().copied(),
                    actual,
                    result.structural_type,
                    storage.stored_scalar,
                )
            })
        }
        None => source_contract(function, live, source.place).is_some_and(|root| {
            terminal_semantics::reference_source_projection_compatible(
                types.values().copied(),
                root,
                &source.path,
                result.structural_type,
            )
        }),
    };
    if !compatible {
        return Err(LoweringError::unsupported_control_flow(function.machine));
    }
    let mut root = normalized_source(function, live, types, source)?;
    if let Some(storage) = storage {
        root = root.projected(&[StructuralPathSegment::RepresentationView {
            structural_type: referent,
            stored_scalar: storage.stored_scalar,
        }]);
    }
    live.references.insert(
        (result.place, Vec::new()),
        ReferenceStorage {
            root,
            carrier_type: result.structural_type,
            referent,
            placed: access_source(function, live, types, source),
        },
    );
    let result_home = super::aggregate_results::home(*psi_operation, result, types)?;
    live.structural_homes
        .insert(result.place, result_home.clone());
    operations.push(TargetUnitOperation::EstablishReference {
        psi_operation: *psi_operation,
        result_home,
        source: source.clone(),
        storage: *storage,
    });
    provenance.operations.push(*psi_operation);
    Ok(())
}

pub(crate) fn establish_access(
    operation: &AbstractOperation,
    site: &mut RowSite<'_>,
) -> Result<(), LoweringError> {
    let (function, types, live, operations, provenance) = site.parts();
    let invalid = || LoweringError::unsupported_control_flow(function.machine);
    let AbstractOperation::EstablishAccessReference {
        psi_operation,
        result,
        source,
        correspondence: expected,
    } = operation
    else {
        return Err(invalid());
    };
    let StructuralTypeShape::Reference {
        referent: carrier,
        access,
    } = types.get(result.structural_type).ok_or_else(invalid)?.shape
    else {
        return Err(invalid());
    };
    if !result.is_affine_plain()
        || !matches!(
            access,
            StructuralAccess::SharedBorrow | StructuralAccess::MutableBorrow
        )
        || !matches!(
            source.access,
            StructuralAccess::SharedBorrow | StructuralAccess::MutableBorrow
        )
        || !access.admits(source.access)
        || live.structural_homes.contains_key(&result.place)
        || live
            .references
            .keys()
            .any(|(place, _)| *place == result.place)
    {
        return Err(invalid());
    }
    let schema = reference_source_type(function, live, types, source).ok_or_else(invalid)?;
    let catalog = types.catalog();
    let mut selected = catalog
        .access_views()
        .iter()
        .filter(|plan| plan.carrier == carrier);
    let (Some(plan), None) = (selected.next(), selected.next()) else {
        return Err(invalid());
    };
    validate_backing(
        catalog.as_slice(),
        catalog.access_views(),
        plan,
        schema,
        expected,
    )
    .map_err(|_| invalid())?;
    let correspondence = expected.clone();
    let root = normalized_source(function, live, types, source)?;
    if root.origins().iter().any(|origin| {
        origin.path.iter().any(|segment| {
            matches!(
                segment,
                terminal_psi::ReferenceOriginProjection::Exact(
                    StructuralPathSegment::RepresentationView { .. }
                )
            )
        })
    }) {
        return Err(invalid());
    }
    let result_home = super::aggregate_results::home(*psi_operation, result, types)?;
    live.references.insert(
        (result.place, Vec::new()),
        ReferenceStorage {
            root,
            carrier_type: result.structural_type,
            referent: carrier,
            placed: Some(TargetAccessInterpretation::Captured(correspondence.clone())),
        },
    );
    live.structural_homes
        .insert(result.place, result_home.clone());
    operations.push(TargetUnitOperation::EstablishAccessReference {
        psi_operation: *psi_operation,
        result_home,
        source: source.clone(),
        correspondence,
    });
    provenance.operations.push(*psi_operation);
    Ok(())
}

pub(crate) fn retain_access_result(
    function: &AbstractFunction,
    types: &StructuralTypeLookup<'_>,
    live: &mut LiveDefinitions,
    result: &StructuralOperationResult,
    source: &StructuralArgument,
    path: Vec<StructuralPathSegment>,
    backing: abstract_operations::AccessReferenceBacking,
) -> Result<(), LoweringError> {
    let root = normalized_source(function, live, types, source)?;
    let carrier_type = leaf_type(types, result.structural_type, &path)
        .ok_or_else(|| LoweringError::unsupported_control_flow(function.machine))?;
    if live
        .references
        .insert(
            (result.place, path),
            ReferenceStorage {
                root,
                carrier_type,
                referent: backing.carrier(),
                placed: Some(TargetAccessInterpretation::Captured(backing)),
            },
        )
        .is_some()
    {
        return Err(LoweringError::unsupported_control_flow(function.machine));
    }
    Ok(())
}

pub(crate) fn release(
    operation: &AbstractOperation,
    site: &mut RowSite<'_>,
) -> Result<(), LoweringError> {
    let (function, _, live, operations, provenance) = site.parts();
    let AbstractOperation::ReleaseReference {
        psi_operation,
        source,
        path,
    } = operation
    else {
        return Err(LoweringError::unsupported_control_flow(function.machine));
    };
    live.references
        .remove(&(*source, path.clone()))
        .ok_or_else(|| LoweringError::unsupported_control_flow(function.machine))?;
    operations.push(TargetUnitOperation::ReleaseReference {
        psi_operation: *psi_operation,
        source: *source,
        path: path.clone(),
    });
    provenance.operations.push(*psi_operation);
    Ok(())
}

pub(crate) fn discard_owned(
    function: &AbstractFunction,
    types: &StructuralTypeLookup<'_>,
    live: &mut LiveDefinitions,
    source: PlaceId,
) -> Result<bool, LoweringError> {
    if live.retired_reference_roots.contains(&source) {
        return Ok(true);
    }
    let discarded = discard_subtree(function, types, live, source, &[])?;
    if discarded {
        live.retired_reference_roots.insert(source);
    }
    Ok(discarded)
}

pub(crate) fn discard_subtree(
    function: &AbstractFunction,
    types: &StructuralTypeLookup<'_>,
    live: &mut LiveDefinitions,
    source: PlaceId,
    prefix: &[StructuralPathSegment],
) -> Result<bool, LoweringError> {
    let Some(contract) = source_contract(function, live, source) else {
        return Ok(false);
    };
    let selected = leaf_type(types, contract, prefix)
        .ok_or(LoweringError::unsupported_control_flow(function.machine))?;
    if !contains_reference(types, selected) {
        return Ok(false);
    }
    live.references.retain(|(carrier, path), _| {
        *carrier != source
            || !path
                .strip_prefix(prefix)
                .is_some_and(|relative| !relative.contains(&StructuralPathSegment::Referent))
    });
    Ok(true)
}

pub(crate) fn window_descriptors(
    function: &AbstractFunction,
    types: &StructuralTypeLookup<'_>,
    live: &LiveDefinitions,
    place: PlaceId,
    path: &[StructuralPathSegment],
) -> Result<Vec<ReferenceStorage>, LoweringError> {
    let invalid = || LoweringError::unsupported_control_flow(function.machine);
    let crossing = path
        .iter()
        .rposition(|segment| *segment == StructuralPathSegment::Referent)
        .ok_or_else(invalid)?;
    let regions = copies::storage_regions(function, types, live, place, &path[..crossing])?;
    let budget = live
        .references
        .len()
        .max(1)
        .checked_mul(path.len().max(1))
        .ok_or_else(invalid)?;
    let mut descriptors = Vec::new();
    for (carrier, prefix) in regions {
        let root = source_contract(function, live, carrier).ok_or_else(invalid)?;
        let candidates = stored_descriptor_paths(types.values().copied(), root, &prefix, budget)
            .ok_or_else(invalid)?;
        for candidate in candidates {
            let leaf = live
                .references
                .get(&(carrier, candidate.clone()))
                .cloned()
                .or_else(|| observations::observe(function, live, types, carrier, &candidate))
                .ok_or_else(invalid)?;
            if leaf_type(types, root, &candidate) != Some(leaf.carrier_type) {
                return Err(invalid());
            }
            descriptors.push(leaf);
        }
    }
    if descriptors.is_empty() {
        return Err(invalid());
    }
    Ok(descriptors)
}

pub(crate) fn window_storage(
    function: &AbstractFunction,
    types: &StructuralTypeLookup<'_>,
    live: &LiveDefinitions,
    place: PlaceId,
    path: &[StructuralPathSegment],
) -> Result<(PlaceId, Vec<StructuralPathSegment>), LoweringError> {
    let mut regions = copies::storage_regions(function, types, live, place, path)?;
    if regions.len() != 1 {
        return Err(LoweringError::unsupported_control_flow(function.machine));
    }
    regions
        .pop()
        .ok_or(LoweringError::unsupported_control_flow(function.machine))
}

pub(crate) fn subtree_leaves(
    function: &AbstractFunction,
    types: &StructuralTypeLookup<'_>,
    live: &LiveDefinitions,
    destination: &[StructuralPathSegment],
    nested: StructuralTypeId,
    argument: &StructuralArgument,
    transfers_storage: bool,
    relocations: &mut Vec<(ReferenceStorage, Vec<StructuralPathSegment>)>,
    moved: &mut BTreeSet<(PlaceId, Vec<StructuralPathSegment>)>,
) -> Result<(), LoweringError> {
    if !contains_reference(types, nested) {
        return Ok(());
    }
    let contract = source_contract(function, live, argument.place)
        .ok_or_else(|| LoweringError::unsupported_control_flow(function.machine))?;
    if leaf_type(types, contract, &argument.path) != Some(nested) {
        return Err(LoweringError::unsupported_control_flow(function.machine));
    }
    for ((carrier, source_path), descriptor) in &live.references {
        if *carrier != argument.place {
            continue;
        }
        let Some(path) = source_path.strip_prefix(argument.path.as_slice()) else {
            continue;
        };
        if path.contains(&StructuralPathSegment::Referent) {
            continue;
        }
        if transfers_storage {
            moved.insert((*carrier, source_path.clone()));
        }
        let mut destination = destination.to_vec();
        destination.extend(path.iter().cloned());
        relocations.push((descriptor.clone(), destination));
    }
    Ok(())
}

pub(crate) fn relocate_leaves(
    live: &mut LiveDefinitions,
    result_place: PlaceId,
    relocations: Vec<(ReferenceStorage, Vec<StructuralPathSegment>)>,
    moved: BTreeSet<(PlaceId, Vec<StructuralPathSegment>)>,
) {
    for source in moved {
        live.references.remove(&source);
    }
    for (descriptor, destination) in relocations {
        live.references
            .insert((result_place, destination), descriptor);
    }
}

pub(crate) fn boundary_result_references(
    function: &AbstractFunction,
    types: &StructuralTypeLookup<'_>,
    result: &StructuralOperationResult,
    live: &mut LiveDefinitions,
) -> Result<(), LoweringError> {
    if !contains_reference(types, result.structural_type) {
        return Ok(());
    }
    let paths = leaf_paths(types, result.structural_type, 4096)
        .ok_or_else(|| LoweringError::unsupported_control_flow(function.machine))?;
    for path in paths {
        let carrier_type = leaf_type(types, result.structural_type, &path)
            .ok_or_else(|| LoweringError::unsupported_control_flow(function.machine))?;
        let referent = referent(types, carrier_type)
            .ok_or_else(|| LoweringError::unsupported_control_flow(function.machine))?;
        let leaf = ReferenceStorage {
            root: ReferenceRoot::Alternatives(vec![target_operations::TargetReferenceOrigin {
                base: target_operations::TargetReferenceOriginBase::Invocation {
                    root: result.place,
                    carrier_type: referent,
                },
                path: Vec::new(),
                extent: semantic_vocabulary::ReferenceOriginExtent::Projected,
            }]),
            carrier_type,
            referent,
            placed: None,
        };
        if live.references.insert((result.place, path), leaf).is_some() {
            return Err(LoweringError::unsupported_control_flow(function.machine));
        }
    }
    Ok(())
}

pub(crate) fn call_results(
    function: &AbstractFunction,
    declaration: abstract_operations::AbstractCallSignature<'_>,
    types: &StructuralTypeLookup<'_>,
    result: &StructuralOperationResult,
    structural_arguments: &[StructuralArgument],
    live: &mut LiveDefinitions,
    moved: &BTreeMap<(PlaceId, Vec<StructuralPathSegment>), ReferenceStorage>,
) -> Result<Vec<target_operations::TargetReferenceResult>, LoweringError> {
    if !contains_reference(types, result.structural_type) {
        return Ok(Vec::new());
    }
    let callee = match &declaration {
        abstract_operations::AbstractCallSignature::Machine(callee) => Some(*callee),
        abstract_operations::AbstractCallSignature::Requirement(_)
        | abstract_operations::AbstractCallSignature::Callable(_) => None,
    };
    let contract = match declaration {
        abstract_operations::AbstractCallSignature::Machine(callee) => {
            let result = callee
                .result
                .structural()
                .ok_or_else(|| LoweringError::unsupported_control_flow(function.machine))?;
            terminal_semantics::ReferenceCallContract::Machine {
                parameters: &callee.structural_parameters,
                sources: &result.reference_sources,
            }
        }
        abstract_operations::AbstractCallSignature::Requirement(slot) => {
            terminal_semantics::ReferenceCallContract::Requirement(slot)
        }
        abstract_operations::AbstractCallSignature::Callable(contract) => {
            terminal_semantics::ReferenceCallContract::Callable(contract)
        }
    };
    let mappings = terminal_semantics::bind_reference_call_results(
        types.values().copied(),
        contract,
        structural_arguments,
    )
    .map_err(|reason| LoweringError::InvalidReferenceResult {
        machine: function.machine,
        reason,
    })?;
    if live
        .references
        .keys()
        .any(|(carrier, _)| *carrier == result.place)
    {
        return Err(LoweringError::unsupported_control_flow(function.machine));
    }
    let mut established = Vec::new();
    let mut reference_results = Vec::new();
    for mapping in mappings {
        if mapping.sources.is_empty() {
            return Err(LoweringError::unsupported_control_flow(function.machine));
        }
        let expected = leaf_referent(types, result.structural_type, &mapping.path)
            .ok_or_else(|| LoweringError::unsupported_control_flow(function.machine))?;
        let result_access = leaf_type(types, result.structural_type, &mapping.path)
            .and_then(|leaf| types.get(leaf))
            .and_then(|declaration| match declaration.shape {
                StructuralTypeShape::Reference { access, .. } => Some(access),
                _ => None,
            })
            .ok_or_else(|| LoweringError::unsupported_control_flow(function.machine))?;
        let mut candidates = Vec::with_capacity(mapping.sources.len());
        let mut public_interpretations = Vec::with_capacity(mapping.sources.len());
        for binding in &mapping.sources {
            let (source, transfers_existing) = match &binding.origin {
                terminal_semantics::ReferenceCallOrigin::Parameter {
                    source,
                    transfers_existing,
                } => (source, *transfers_existing),
                terminal_semantics::ReferenceCallOrigin::Invocation {
                    carrier_type,
                    path: origin_path,
                    access,
                } => {
                    if !access_covers(*access, result_access)
                        || binding.endpoint != expected
                        || binding.extent != semantic_vocabulary::ReferenceOriginExtent::Projected
                    {
                        return Err(LoweringError::unsupported_control_flow(function.machine));
                    }
                    let root = ReferenceRoot::Alternatives(vec![
                        target_operations::TargetReferenceOrigin {
                            base: target_operations::TargetReferenceOriginBase::Invocation {
                                root: result.place,
                                carrier_type: *carrier_type,
                            },
                            path: origin_path.clone(),
                            extent: binding.extent,
                        },
                    ]);
                    candidates.push(ReferenceStorage {
                        root,
                        carrier_type: leaf_type(types, result.structural_type, &mapping.path)
                            .ok_or_else(|| {
                                LoweringError::unsupported_control_flow(function.machine)
                            })?,
                        referent: expected,
                        placed: None,
                    });
                    public_interpretations.push(None);
                    continue;
                }
            };
            if !access_covers(source.access, result_access) {
                return Err(LoweringError::unsupported_control_flow(function.machine));
            }
            let Some(suffix) =
                reference_source_type(function, live, types, source).and_then(|root| {
                    binding.projected_origin_suffix(
                        types.values().copied(),
                        root,
                        expected,
                        result_access,
                    )
                })
            else {
                return Err(LoweringError::InvalidReferenceResult {
                    machine: function.machine,
                    reason: "returned reference origin does not resolve to its declared referent storage type",
                });
            };
            let mut leaf = if transfers_existing {
                let Some((StructuralPathSegment::Referent, carrier_path)) =
                    source.path.split_last()
                else {
                    return Err(LoweringError::unsupported_control_flow(function.machine));
                };
                let coordinate = (source.place, carrier_path.to_vec());
                let Some(leaf) = moved
                    .get(&coordinate)
                    .map(std::borrow::Cow::Borrowed)
                    .or_else(|| reference_leaf(function, live, types, source.place, carrier_path))
                else {
                    if !carrier_path
                        .iter()
                        .any(|segment| matches!(segment, StructuralPathSegment::Case(_)))
                    {
                        return Err(LoweringError::unsupported_control_flow(function.machine));
                    }
                    continue;
                };
                leaf.into_owned()
            } else {
                ReferenceStorage {
                    root: normalized_source(function, live, types, source)?
                        .projected_family(&suffix, binding.extent),
                    carrier_type: leaf_type(types, result.structural_type, &mapping.path)
                        .ok_or_else(|| LoweringError::unsupported_control_flow(function.machine))?,
                    referent: expected,
                    placed: access_source(function, live, types, source),
                }
            };
            public_interpretations.push(leaf.placed.take().filter(|_| {
                binding.suffix.is_empty()
                    && binding.extent == semantic_vocabulary::ReferenceOriginExtent::Projected
                    && leaf.referent == expected
            }));
            candidates.push(leaf);
        }
        if candidates.is_empty() {
            continue;
        }
        let mut leaf = combine_alternatives(function, candidates)?;
        leaf.carrier_type = leaf_type(types, result.structural_type, &mapping.path)
            .ok_or_else(|| LoweringError::unsupported_control_flow(function.machine))?;
        if types
            .catalog()
            .access_views()
            .iter()
            .any(|plan| plan.carrier == expected)
        {
            let private = callee.and_then(|callee| {
                let slots = live
                    .stored_reference_effects
                    .access_results
                    .get(&callee.machine)?
                    .as_ref()?;
                optimization_unit_semantics::descriptor_results::access_results::resolve(
                    slots,
                    &mapping.path,
                    |formal| {
                        let actual = abstract_operations::call_source(
                            &callee.structural_parameters,
                            structural_arguments,
                            formal,
                        )?;
                        if let Some((StructuralPathSegment::Referent, path)) =
                            actual.path.split_last()
                            && let Some(transferred) = moved.get(&(actual.place, path.to_vec()))
                        {
                            return transferred.placed.clone();
                        }
                        access_source(function, live, types, &actual)
                    },
                    |correspondence| TargetAccessInterpretation::Captured(correspondence.clone()),
                )
            });
            let association = reconcile(agree(public_interpretations), private).ok_or(
                LoweringError::AccessReturnRequiresWholeView {
                    machine: function.machine,
                },
            )?;
            leaf.placed = Some(association);
        }
        reference_results.push(target_operations::TargetReferenceResult {
            path: mapping.path.clone(),
            origins: leaf.root.origins(),
        });
        established.push(((result.place, mapping.path.clone()), leaf));
    }
    for (key, leaf) in established {
        live.references.insert(key, leaf);
    }
    Ok(reference_results)
}

fn combine_alternatives(
    function: &AbstractFunction,
    candidates: Vec<ReferenceStorage>,
) -> Result<ReferenceStorage, LoweringError> {
    let mut candidates = candidates.into_iter();
    let mut leaf = candidates
        .next()
        .ok_or(LoweringError::unsupported_control_flow(function.machine))?;
    let mut origins = vec![leaf.root.clone()];
    for candidate in candidates {
        if leaf.referent != candidate.referent || leaf.placed != candidate.placed {
            return Err(LoweringError::unsupported_control_flow(function.machine));
        }
        origins.push(candidate.root);
    }
    leaf.root = combine_candidate_origins(function, origins)?;
    Ok(leaf)
}

fn combine_candidate_origins(
    function: &AbstractFunction,
    candidates: Vec<ReferenceRoot>,
) -> Result<ReferenceRoot, LoweringError> {
    let mut candidates = candidates.into_iter();
    let root = candidates
        .next()
        .ok_or(LoweringError::unsupported_control_flow(function.machine))?;
    if candidates.len() == 0 {
        return Ok(root);
    }
    let mut origins = root.origins();
    for candidate in candidates {
        origins.extend(candidate.origins());
    }
    origins.sort();
    origins.dedup();
    Ok(ReferenceRoot::Alternatives(origins))
}

pub(crate) fn entry(
    function: &AbstractFunction,
    types: &StructuralTypeLookup<'_>,
) -> Result<BTreeMap<(PlaceId, Vec<StructuralPathSegment>), ReferenceStorage>, LoweringError> {
    let mut references = BTreeMap::new();
    for parameter in &function.structural_parameters {
        if !contains_reference(types, parameter.structural_type) {
            continue;
        }
        let paths = leaf_paths(
            types,
            parameter.structural_type,
            4096usize.saturating_sub(references.len()),
        )
        .ok_or_else(|| LoweringError::unsupported_control_flow(function.machine))?;
        for path in paths {
            references.insert(
                (parameter.place, path.clone()),
                ReferenceStorage {
                    root: ReferenceRoot::incoming_contents(parameter.place, path.clone()),
                    carrier_type: leaf_type(types, parameter.structural_type, &path)
                        .ok_or_else(|| LoweringError::unsupported_control_flow(function.machine))?,
                    referent: leaf_referent(types, parameter.structural_type, &path)
                        .ok_or_else(|| LoweringError::unsupported_control_flow(function.machine))?,
                    placed: None,
                },
            );
        }
    }
    Ok(references)
}
