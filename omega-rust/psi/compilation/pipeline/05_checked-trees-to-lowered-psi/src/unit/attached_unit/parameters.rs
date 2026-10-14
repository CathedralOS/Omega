//! Parameter, transfer, and structural-argument lowering for attached Unit
//! closures.

use super::super::{
    BTreeSet, CheckedUnitStructuralPathSegment, ServiceId, ServiceReachId, ServiceReachInterface,
    ServiceReachPlan, StructuralAccess, StructuralArgument, StructuralParameterDeclaration,
    StructuralTypeDeclaration,
};
use super::{
    CheckedBoundaryMachinePlan, CheckedTrees, LoweringError, Multiplicity, PlaceId, ScalarType,
    ServiceReachSummary, StructuralPlaceDeclaration, StructuralPlaceKind, StructuralTypeId,
    ValueDeclaration, allocate_dense, lookup_service_id, lookup_type_id, place_id,
    require_valid_service_row, terminal_scalar_type, unsupported,
};
use crate::emission::operation_emission::buffer::OperationBuffer;
use crate::emission::operation_emission::calls::CallEmissionContext;
use crate::emission::operation_emission::expressions::emit_direct_expression;
use crate::expression_preparation::bindings::structural_paths::{
    lower_typed_argument_path, structural_access_can_supply, structural_argument, terminal_access,
};
use crate::expression_preparation::bindings::{
    StructuralLocalBinding, StructuralScalarFieldBinding,
};
use crate::terminal_identities::{StructuralDomainBinding, lookup_domain_id_for_carrier};
use semantic_vocabulary::ValueId;
mod reference_sources;
pub(crate) use reference_sources::ReferenceSourceEvaluation;
pub(crate) use reference_sources::{lower_reference_result_sources, lower_reference_source};

/// The carrier beneath a type's qualification shells, for contents
/// classification and shape inspection. Qualifications have their own
/// retained row, and reference presentation is preserved rather than silently
/// turning a view into ownership. Rejoining a structural plan compares
/// identities through `validation::stored_structural_type_identity` instead,
/// which keeps the qualification a bounded byte owner's plan is named by.
pub(crate) fn structural_carrier_type(
    checked: &CheckedTrees,
    mut reference: checked_trees::types::TypeReferenceHandle,
) -> Result<checked_trees::types::TypeReferenceHandle, LoweringError> {
    let mut visited = Vec::new();
    loop {
        if !reference.is_valid() || visited.contains(&reference) {
            return unsupported("structural carrier has an invalid type chain");
        }
        visited.push(reference);
        match checked.type_reference_table.type_reference(reference) {
            checked_trees::types::TypeReferenceNode::Constrained { base_type, .. } => {
                reference = *base_type
            }
            _ => return Ok(reference),
        }
    }
}

/// A borrowed Binding is an ordinary reference parameter, not an owned affine
/// binding. Provider storage never changes source access or multiplicity.
fn service_parameter_multiplicity(
    access: checked_trees::CheckedStructuralAccess,
) -> Result<Multiplicity, LoweringError> {
    match access {
        checked_trees::CheckedStructuralAccess::Owned => Ok(Multiplicity::Affine),
        checked_trees::CheckedStructuralAccess::SharedBorrow
        | checked_trees::CheckedStructuralAccess::MutableBorrow => Ok(Multiplicity::Unrestricted),
        checked_trees::CheckedStructuralAccess::WriteOnlyBorrow => {
            unsupported("Binding parameter cannot use write-only access")
        }
    }
}

/// Merge retained formals without turning lane-local proof/custody coordinates
/// into ABI positions. Gaps may denote omitted receivers or erased formals.
pub(crate) fn lower_boundary_parameter_order(
    scalars: &[checked_trees::CheckedStructuralScalarParameterPlan],
    structural: &[checked_trees::CheckedUnitStructuralParameterPlan],
) -> Result<Vec<terminal_psi::BoundaryParameterKind>, LoweringError> {
    use terminal_psi::BoundaryParameterKind::{Scalar, Structural};
    if scalars
        .windows(2)
        .any(|pair| pair[0].source_position >= pair[1].source_position)
        || structural
            .windows(2)
            .any(|pair| pair[0].position >= pair[1].position)
    {
        return unsupported("boundary parameter lanes are not in strict source order");
    }
    let mut scalar = scalars.iter().peekable();
    let mut structure = structural.iter().peekable();
    let mut order = Vec::with_capacity(scalars.len() + structural.len());
    loop {
        let kind = match (scalar.peek(), structure.peek()) {
            (Some(left), Some(right)) => match left.source_position.cmp(&right.position) {
                std::cmp::Ordering::Less => Scalar,
                std::cmp::Ordering::Greater => Structural,
                std::cmp::Ordering::Equal => {
                    return unsupported("boundary parameter lanes share a source position");
                }
            },
            (Some(_), None) => Scalar,
            (None, Some(_)) => Structural,
            (None, None) => break,
        };
        match kind {
            Scalar => {
                scalar.next();
            }
            Structural => {
                structure.next();
            }
        }
        order.push(kind);
    }
    Ok(order)
}

/// Declare one Unit state's scalar formals: each takes its authored type,
/// tags included, from the closure's qualification namespace and a fresh
/// value identity.
pub(crate) fn lower_unit_scalar_parameter_declarations(
    checked: &CheckedTrees,
    state: symbols::SymbolHandle,
    parameters: &[checked_trees::CheckedStructuralScalarParameterPlan],
    qualifications: &crate::expression_preparation::qualifications::PreparedScalarQualifications,
    next_value: &mut u64,
) -> Result<Vec<ValueDeclaration>, LoweringError> {
    let carriers = lower_unit_scalar_parameter_types(parameters)?;
    let (_, declaration) = crate::declarations::state_declaration(checked, state)?;
    let authored = checked.state_parameters(declaration);
    parameters
        .iter()
        .zip(carriers)
        .map(|(parameter, carrier)| {
            let authored = usize::try_from(parameter.source_position)
                .ok()
                .and_then(|position| authored.get(position))
                .ok_or(LoweringError::Unsupported(
                    "Unit scalar formal has no authored parameter",
                ))?;
            let declared = qualifications.signature_value_type(checked, authored.type_reference)?;
            if declared.scalar_type != carrier {
                return unsupported("Unit scalar formal has a different authored carrier");
            }
            Ok(ValueDeclaration {
                qualifications: declared.qualifications,
                id: crate::terminal_identities::value_id(allocate_dense(next_value)?),
                scalar_type: carrier,
            })
        })
        .collect()
}

/// The declared type of a Unit state's scalar result: its authored result
/// type, tags included, from the closure's qualification namespace.
pub(crate) fn unit_scalar_result_type(
    checked: &CheckedTrees,
    state: symbols::SymbolHandle,
    carrier: ScalarType,
    qualifications: &crate::expression_preparation::qualifications::PreparedScalarQualifications,
) -> Result<semantic_vocabulary::QualifiedScalarType, LoweringError> {
    let (_, declaration) = crate::declarations::state_declaration(checked, state)?;
    let declared = qualifications.signature_value_type(checked, declaration.return_type)?;
    if declared.scalar_type != carrier {
        return unsupported("Unit scalar result has a different authored carrier");
    }
    Ok(declared)
}

pub(crate) fn lower_unit_scalar_parameter_types(
    parameters: &[checked_trees::CheckedStructuralScalarParameterPlan],
) -> Result<Vec<ScalarType>, LoweringError> {
    if parameters
        .windows(2)
        .any(|pair| pair[0].source_position >= pair[1].source_position)
    {
        return unsupported("Unit scalar parameters are not in strict source order");
    }
    parameters
        .iter()
        .map(|parameter| terminal_scalar_type(parameter.primitive_type))
        .collect()
}
pub(crate) fn lower_unit_parameters(
    identities: &mut terminal_psi::TerminalIdentityTableBuilder,
    parameters: &[checked_trees::CheckedUnitStructuralParameterPlan],
    type_ids: &[(typed_trees::type_identity::TypeIdentityId, StructuralTypeId)],
    structural_types: &[StructuralTypeDeclaration],
    domain_ids: &[StructuralDomainBinding],
    next_place: &mut u64,
) -> Result<Vec<StructuralParameterDeclaration>, LoweringError> {
    let mut positions = BTreeSet::new();
    parameters
        .iter()
        .enumerate()
        .map(|(dense_position, parameter)| {
            if !positions.insert(parameter.position) {
                return Err(LoweringError::Unsupported(
                    "Unit structural parameters contain duplicate source positions",
                ));
            }
            let mut qualifications = parameter
                .qualifications
                .iter()
                .map(|domain| {
                    lookup_domain_id_for_carrier(
                        domain_ids,
                        *domain,
                        lookup_type_id(type_ids, &parameter.type_identity)?,
                    )
                })
                .collect::<Result<Vec<_>, LoweringError>>()?;
            qualifications.sort();
            qualifications.dedup();
            if qualifications.len() != parameter.qualifications.len() {
                return Err(LoweringError::Unsupported(
                    "Unit structural parameter repeats a qualification",
                ));
            }
            if parameter.binding_carrier.is_some()
                && (parameter.multiplicity != service_parameter_multiplicity(parameter.access)?
                    || !parameter.qualifications.is_empty()
                    || !parameter.projected_qualifications.is_empty())
            {
                return Err(LoweringError::Unsupported(
                    "fused Service parameter has an invalid checked structural shape",
                ));
            }
            Ok(StructuralParameterDeclaration {
                place: place_id(allocate_dense(next_place)?),
                position: u32::try_from(dense_position).map_err(|_| {
                    LoweringError::Unsupported("Unit structural parameter count exceeds u32")
                })?,
                structural_type: lookup_type_id(type_ids, &parameter.type_identity)?,
                multiplicity: crate::terminal_identities::structural_multiplicity(
                    parameter.multiplicity,
                ),
                access: terminal_access(parameter.access),
                qualifications,
                projected_qualifications: lower_projected_qualifications(
                    identities,
                    &parameter.projected_qualifications,
                    domain_ids,
                    lookup_type_id(type_ids, &parameter.type_identity)?,
                    structural_types,
                )?,
            })
        })
        .collect()
}

pub(crate) fn lower_contract_service_ceiling(
    rows: &language_semantics::ServiceReachRowTable,
    contract: ServiceReachPlan,
    summary: ServiceReachSummary,
    service_ids: &[(ServiceReachId, ServiceId)],
) -> Result<Vec<ServiceId>, LoweringError> {
    if contract.checked_inferred != summary.transitive {
        return unsupported("Unit contract reach does not match checked transitive reach");
    }
    let source = match contract.interface {
        ServiceReachInterface::PublishedCeiling(row) => {
            require_valid_service_row(row)?;
            let ceiling = rows.services(row);
            if rows
                .services(summary.transitive)
                .iter()
                .any(|service| !ceiling.contains(service))
            {
                return unsupported("checked Unit service reach exceeds its published ceiling");
            }
            ceiling
        }
        ServiceReachInterface::InternalInferred => rows.services(summary.transitive),
    };
    let mut lowered = source
        .iter()
        .map(|service| lookup_service_id(service_ids, *service))
        .collect::<Result<Vec<_>, LoweringError>>()?;
    lowered.sort();
    lowered.dedup();
    if lowered.len() != source.len() {
        return unsupported("Unit published service ceiling contains duplicates");
    }
    Ok(lowered)
}

/// Keep the authored concrete basis, not the flattened inferred ceiling. A
/// Console declaration and a Console installation upper bound may overlap;
/// subtracting the bound would erase the declaration. Terminal independently
/// unions this basis with retained calls to reconstruct the concrete root row.
pub(crate) fn lower_declared_service_reach(
    checked: &CheckedTrees,
    machine: symbols::SymbolHandle,
    service_ids: &[(ServiceReachId, ServiceId)],
) -> Result<Vec<ServiceId>, LoweringError> {
    let mut sources = checked
        .typed
        .machines()
        .iter()
        .filter(|source| source.symbol == machine);
    let source = sources.next().ok_or(LoweringError::Unsupported(
        "service declaration lost its source machine",
    ))?;
    if sources.next().is_some() {
        return unsupported("service declaration has ambiguous source machines");
    }
    if source.service_reach_is_installation_bound {
        return Ok(Vec::new());
    }
    let declared = checked
        .typed
        .service_reach_rows
        .services(source.service_reach_row);
    crate::terminal_identities::lower_service_reach_sorted(declared, service_ids)
}

pub(crate) fn lower_fixed_boundary_service_reach(
    checked: &CheckedTrees,
    boundary: &CheckedBoundaryMachinePlan,
    service_ids: &[(ServiceReachId, ServiceId)],
) -> Result<Vec<ServiceId>, LoweringError> {
    let source =
        validation::fixed_installation_boundary_service_reach(&checked.typed, boundary.state)
            .ok_or(LoweringError::Unsupported(
                "fixed boundary reach has no exact typed requirement",
            ))?;
    crate::terminal_identities::lower_service_reach_sorted(&source, service_ids)
}

pub(crate) fn lower_published_service_ceiling(
    rows: &language_semantics::ServiceReachRowTable,
    contract: ServiceReachPlan,
    summary: ServiceReachSummary,
    service_ids: &[(ServiceReachId, ServiceId)],
) -> Result<Vec<ServiceId>, LoweringError> {
    if matches!(contract.interface, ServiceReachInterface::InternalInferred) {
        return unsupported("public Unit contract has no published service ceiling");
    }
    lower_contract_service_ceiling(rows, contract, summary, service_ids)
}

pub(crate) fn lower_installation_machine_service_ceiling(
    checked: &CheckedTrees,
    machine: symbols::SymbolHandle,
    contract: ServiceReachPlan,
    summary: ServiceReachSummary,
    service_ids: &[(ServiceReachId, ServiceId)],
) -> Result<Vec<ServiceId>, LoweringError> {
    if let Some(reach) = checked.facts.service_reaches.for_machine(machine)
        && matches!(contract.interface, ServiceReachInterface::InternalInferred)
        && !reach.unresolved_installation_reaches.is_empty()
    {
        let source = checked
            .facts
            .service_reaches
            .rows
            .services(summary.transitive);
        let mut lowered = source
            .iter()
            .map(|service| lookup_service_id(service_ids, *service))
            .collect::<Result<Vec<_>, LoweringError>>()?;
        lowered.sort();
        lowered.dedup();
        if lowered.len() != source.len() {
            return unsupported("installation-bound service ceiling contains duplicates");
        }
        return Ok(lowered);
    }
    lower_contract_service_ceiling(
        &checked.facts.service_reaches.rows,
        contract,
        summary,
        service_ids,
    )
}

pub(crate) fn literal_argument_places(
    arguments: &[checked_trees::CheckedUnitStructuralArgumentPlan],
    literals: &[StructuralPlaceDeclaration],
    next_literal: &mut usize,
) -> Result<Vec<PlaceId>, LoweringError> {
    let count = arguments
        .iter()
        .filter(|argument| argument.byte_sequence_literal().is_some())
        .count();
    let end = next_literal
        .checked_add(count)
        .ok_or(LoweringError::Unsupported(
            "byte-sequence literal argument count overflows usize",
        ))?;
    let places = literals
        .get(*next_literal..end)
        .ok_or(LoweringError::Unsupported(
            "byte-sequence literal argument place is absent",
        ))?
        .iter()
        .map(|place| place.id)
        .collect();
    *next_literal = end;
    Ok(places)
}

/// Consume address operands retained by the authored call schedule. Every
/// call selector must arrive here already evaluated; reading its source at
/// the final call would observe effects from later argument positions.
pub(crate) fn call_index_selectors(
    arguments: &[checked_trees::CheckedUnitStructuralArgumentPlan],
    evaluated: Option<&[ValueDeclaration]>,
) -> Result<Vec<ValueId>, LoweringError> {
    let count = arguments
        .iter()
        .flat_map(|argument| argument.call_selector_segments())
        .count();
    let evaluated = evaluated.unwrap_or_default();
    if evaluated.len() != count || evaluated.iter().any(|value|
        !matches!(value.scalar_type, ScalarType::Integer(integer) if !integer.is_address())) {
        return unsupported("captured call selectors disagree with their argument paths");
    }
    Ok(evaluated.iter().map(|value| value.id).collect())
}

/// Emit the declaration-resolved scalar read for every `StructuralField`
/// runtime-index selector in argument order, before the call they select
/// into. The call's `RuntimeIndex` segment then carries the read's value as
/// its operand — the same operand position a `Parameter` selector carries —
/// so the reads land on the operation buffer ahead of the call, which is why
/// they mint here rather than inside `lower_structural_arguments` (its
/// earlier-results slice holds the buffer by shared borrow).
pub(crate) fn mint_runtime_index_selectors(
    identities: &mut terminal_psi::TerminalIdentityTableBuilder,
    arguments: &[checked_trees::CheckedUnitStructuralArgumentPlan],
    fields: &[StructuralScalarFieldBinding],
    bindings: &crate::expression_preparation::bindings::ScalarBindings,
    caller_scalar_values: &[ValueDeclaration],
    next_value: &mut u64,
    operations: &mut OperationBuffer,
) -> Result<Vec<ValueId>, LoweringError> {
    let mut values = Vec::new();
    for argument in arguments {
        for segment in &argument.path {
            let observation = match segment {
                CheckedUnitStructuralPathSegment::RuntimeIndex(
                    checked_trees::CheckedRuntimeIndex::StorageRead { symbol },
                ) => bindings.runtime_index_storage(*symbol)?,
                CheckedUnitStructuralPathSegment::RuntimeIndex(
                    checked_trees::CheckedRuntimeIndex::StructuralField(field),
                ) => crate::expression_preparation::bindings::structural_fields::resolve_typed(
                    identities,
                    fields,
                    field.parameter_position,
                    &field.path,
                )?,
                _ => continue,
            };
            if !matches!(observation.scalar_type(), ScalarType::Integer(integer) if !integer.is_address())
            {
                return unsupported("runtime index field selector is not an integer read");
            }
            values.push(emit_direct_expression(
                &observation,
                caller_scalar_values,
                next_value,
                operations,
            ));
        }
    }
    Ok(values)
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn lower_structural_arguments(
    identities: &mut terminal_psi::TerminalIdentityTableBuilder,
    checked: &checked_trees::CheckedTrees,
    arguments: &[checked_trees::CheckedUnitStructuralArgumentPlan],
    structural_types: &[StructuralTypeDeclaration],
    parameters: &[StructuralParameterDeclaration],
    trivial_affine_locals: &[StructuralPlaceDeclaration],
    structural_results: &[(Option<StructuralPlaceDeclaration>, bool)],
    completed_results: &[(
        u32,
        crate::emission::operation_emission::buffer::StructuralBinding,
    )],
    byte_argument_places: &[PlaceId],
    primitive_locals: &[super::primitive_locals::PrimitiveLocal],
    structural_locals: &[StructuralLocalBinding],
    scalar_bindings: Option<&crate::expression_preparation::bindings::ScalarBindings>,
    caller_scalar_values: Option<&[ValueDeclaration]>,
    selector_values: &mut dyn Iterator<Item = ValueId>,
    mut calls: Option<&mut CallEmissionContext<'_>>,
    mut emission: Option<(&mut u64, &mut OperationBuffer)>,
) -> Result<Vec<StructuralArgument>, LoweringError> {
    let mut next_byte_argument = 0usize;
    arguments
        .iter()
        .map(|argument| {
            if argument.access != checked_trees::CheckedStructuralAccess::Owned
                && argument.structural_storage_root().is_some()
                && let Some(bindings) = scalar_bindings
            {
                let bindings = bindings.clone().with_structural_locals(structural_locals)
                    .with_structural_observations(structural_types);
                let prepared = bindings.prepared_borrowed_argument(
                    identities, checked, argument, &[], structural_types,
                )?;
                let path = crate::expression_preparation::bindings::structural_paths::emit_captured_argument_path(
                    &prepared.path, prepared.place, caller_scalar_values, &mut *selector_values,
                    calls.as_deref_mut(), emission.as_mut().map(|(next, operations)| (&mut **next, &mut **operations)),
                )?;
                return Ok(structural_argument(prepared.place, path, prepared.access));
            }
            let mut lower_path = |identities: &mut terminal_psi::TerminalIdentityTableBuilder,
                                  argument: &checked_trees::CheckedUnitStructuralArgumentPlan,
                                  root,
                                  place,
                                  prefix: &[terminal_psi::StructuralPathSegment]| {
                let suffix =
                    crate::expression_preparation::bindings::structural_paths::prepare_call_argument_path(
                        identities,
                        checked,
                        argument,
                        root,
                        structural_types,
                    )?;
                let path = prefix.iter().cloned().map(crate::expression_preparation::bindings::structural_paths::PreparedStructuralPathSegment::Static).chain(suffix).collect::<Vec<_>>();
                crate::expression_preparation::bindings::structural_paths::emit_captured_argument_path(
                    &path,
                    place,
                    caller_scalar_values,
                    &mut *selector_values,
                    calls.as_deref_mut(),
                    emission
                        .as_mut()
                        .map(|(next, operations)| (&mut **next, &mut **operations)),
                )
            };
            if let Some(root) = argument.structural_storage_root() {
                let mut bindings = structural_locals.iter().filter(|binding| binding.root == root);
                let binding = bindings.next().ok_or(LoweringError::Unsupported(
                    "structural argument has no established storage",
                ))?;
                let access = terminal_access(argument.access);
                if bindings.next().is_some()
                    || !structural_access_can_supply(binding.argument.access, access)
                {
                    return unsupported("structural argument changes its established storage custody");
                }
                // A published local may own a projected subtree without a
                // separate place. Continue from its retained path and type.
                let path = lower_path(identities, argument, binding.structural_type, binding.argument.place, &binding.argument.path)?;
                return Ok(structural_argument(
                    binding.argument.place,
                    path,
                    access,
                ));
            }
            if let checked_trees::CheckedUnitStructuralArgumentSourcePlan::PrimitiveLocal { symbol } = argument.source {
                let local = super::primitive_locals::find(primitive_locals, symbol)?;
                if !argument.path.is_empty() { return unsupported("primitive local borrow has a projection"); }
                // Not `terminal_access`: a primitive local lends no owned custody.
                let access = match argument.access {
                    checked_trees::CheckedStructuralAccess::SharedBorrow => StructuralAccess::SharedBorrow,
                    checked_trees::CheckedStructuralAccess::MutableBorrow => StructuralAccess::MutableBorrow,
                    checked_trees::CheckedStructuralAccess::WriteOnlyBorrow => StructuralAccess::WriteOnlyBorrow,
                    checked_trees::CheckedStructuralAccess::Owned => return unsupported("primitive local cannot transfer owned custody"),
                };
                return Ok(structural_argument(
                    local.declaration.id,
                    Vec::new(),
                    access,
                ));
            }
            // Literals and view ranges of either family were staged into
            // places of their own, in argument order.
            if argument.byte_sequence_literal().is_some()
                || matches!(argument.source, checked_trees::CheckedUnitStructuralArgumentSourcePlan::ByteSequenceSubslice { .. } | checked_trees::CheckedUnitStructuralArgumentSourcePlan::ElementViewSubslice { .. })
            {
                let place = *byte_argument_places
                    .get(next_byte_argument)
                    .ok_or(LoweringError::Unsupported(
                        "byte-sequence argument place is absent",
                    ))?;
                next_byte_argument += 1;
                return Ok(structural_argument(
                    place,
                    Vec::new(),
                    terminal_access(argument.access),
                ));
            }
            if let Some(declaration_ordinal) = argument.source_local_declaration_ordinal() {
                let source = trivial_affine_locals
                    .get(usize::try_from(declaration_ordinal).map_err(|_| {
                        LoweringError::Unsupported("Unit local argument ordinal exceeds usize")
                    })?)
                    .ok_or(LoweringError::Unsupported(
                        "Unit local argument has an invalid declaration ordinal",
                    ))?;
                return Ok(structural_argument(
                    source.id,
                    Vec::new(),
                    StructuralAccess::Owned,
                ));
            }
            if let Some(binding_ordinal) = argument.source_structural_result_binding_ordinal() {
                // A projected owned alias declares no place: it names an
                // owned subtree of a live root, so the argument is that
                // root's place walked to the leaf, and the use's own
                // projection continues from there. This is the operation
                // lane Terminal Psi admits for a partial move.
                if let Some(projected_alias) = projected_alias(completed_results, binding_ordinal) {
                    let path = lower_path(identities, argument, projected_alias.structural_type, projected_alias.place, projected_alias.path)?;
                    return Ok(structural_argument(
                        projected_alias.place,
                        path,
                        terminal_access(argument.access),
                    ));
                }
                let source = structural_result_source(structural_results, binding_ordinal, argument)?;
                let structural_type = match source.kind {
                    StructuralPlaceKind::OperationResult { structural_type, .. }
                    | StructuralPlaceKind::ByteSequenceLiteral { structural_type, .. } => structural_type,
                    StructuralPlaceKind::BlockParameter { .. } => {
                        // A completed arrival retains its typed binding, just
                        // like an operation result. Borrowed descriptor uses
                        // still walk through Referent, preserving the selected
                        // backing and codec instead of erasing their projection.
                        let mut bindings = completed_results.iter()
                            .filter(|(ordinal, _)| *ordinal == binding_ordinal);
                        let (_, binding) = bindings.next().ok_or(LoweringError::Unsupported(
                            "structural block arrival has no typed binding",
                        ))?;
                        if bindings.next().is_some()
                            || binding.place() != source.id
                            || !binding.path().is_empty()
                        {
                            return unsupported("structural block arrival disagrees with its typed binding");
                        }
                        binding.structural_type()
                    }
                    _ => return unsupported("structural result argument has no declared type"),
                };
                return Ok(structural_argument(
                    source.id,
                    lower_path(identities, argument, structural_type, source.id, &[])?,
                    terminal_access(argument.access),
                ));
            }
            if let checked_trees::CheckedUnitStructuralArgumentSourcePlan::ScalarParameter { .. } =
                argument.source
            {
                return scalar_bindings
                    .ok_or(LoweringError::Unsupported(
                        "borrowed scalar formal has no scalar namespace",
                    ))?
                    .scalar_parameter_borrow(identities, checked, argument, structural_types);
            }
            let source_parameter_index =
                argument
                    .source_parameter_index()
                    .ok_or(LoweringError::Unsupported(
                        "Unit structural argument is neither a parameter nor a lowered literal",
                    ))?;
            let parameter = parameters
                .get(usize::try_from(source_parameter_index).map_err(|_| {
                    LoweringError::Unsupported("Unit structural argument index exceeds usize")
                })?)
                .ok_or(LoweringError::Unsupported(
                    "Unit structural argument has an invalid caller parameter index",
                ))?;
            Ok(structural_argument(
                parameter.place,
                lower_path(identities, argument, parameter.structural_type, parameter.place, &[])?,
                terminal_access(argument.access),
            ))
        })
        .collect::<Result<Vec<_>, _>>()
        .and_then(|lowered| {
            if next_byte_argument == byte_argument_places.len() {
                Ok(lowered)
            } else {
                unsupported("byte-sequence place count disagrees with arguments")
            }
        })
}

/// One completed binding that is a projected owned alias: the place that
/// holds the storage, the walk to the aliased leaf, and the leaf's declared
/// carrier. Every other binding occupies its whole place and answers here
/// with `None`.
struct ProjectedAliasBinding<'results> {
    place: PlaceId,
    path: &'results [terminal_psi::StructuralPathSegment],
    structural_type: StructuralTypeId,
}

fn projected_alias<'results>(
    completed_results: &'results [(
        u32,
        crate::emission::operation_emission::buffer::StructuralBinding,
    )],
    binding_ordinal: u32,
) -> Option<ProjectedAliasBinding<'results>> {
    let (_, binding) = completed_results
        .iter()
        .find(|(ordinal, _)| *ordinal == binding_ordinal)?;
    let crate::emission::operation_emission::buffer::StructuralBinding::ProjectedAlias {
        place,
        path,
        structural_type,
        ..
    } = binding
    else {
        return None;
    };
    Some(ProjectedAliasBinding {
        place: *place,
        path,
        structural_type: *structural_type,
    })
}

fn structural_result_source<'results>(
    results: &'results [(Option<StructuralPlaceDeclaration>, bool)],
    binding_ordinal: u32,
    argument: &checked_trees::CheckedUnitStructuralArgumentPlan,
) -> Result<&'results StructuralPlaceDeclaration, LoweringError> {
    let (source, discard) = results
        .get(usize::try_from(binding_ordinal).map_err(|_| {
            LoweringError::Unsupported("Unit structural result binding ordinal exceeds usize")
        })?)
        .ok_or(LoweringError::Unsupported(
            "Unit structural result binding is not produced",
        ))?;
    // A projected owned alias declares no place of its own, so there is no
    // producer custody for this join to read. Its uses resolve through the
    // local namespace, which composes the alias path with the use's own.
    let source = source.as_ref().ok_or(LoweringError::Unsupported(
        "Unit structural result binding is a projected alias",
    ))?;
    // A whole owned move must not come from a result still marked for its
    // return drop. A projected owned move is different: the subtree's custody
    // was proven upstream (the bare reference result's leaf loan), and the
    // carrier's residual legitimately keeps that return drop.
    if (*discard
        && argument.access == checked_trees::CheckedStructuralAccess::Owned
        && argument.path.is_empty())
        || !matches!(
            source.kind,
            StructuralPlaceKind::OperationResult { .. }
                | StructuralPlaceKind::ByteSequenceLiteral { .. }
                | StructuralPlaceKind::BlockParameter { .. }
        )
    {
        return unsupported("Unit structural result argument disagrees with its producer cleanup");
    }
    Ok(source)
}

/// Authored `ensures result in Domain` clauses publish the result's semantic
/// qualifications beside the declared type's constrained rows: the checked
/// contract keeps them as source-checked guarantees (no scalar consequence),
/// and a boundary's declared qualification roster replays against them here.
/// A membership over another value names a different carrier and is skipped;
/// a machine without a canonical contract plan authors no result clauses.
/// Domains stay in the checked namespace; each callsite remaps them to its
/// carrier's Terminal domain identity.
pub(crate) fn authored_result_qualification_domains(
    checked: &CheckedTrees,
    machine: symbols::SymbolHandle,
) -> Vec<language_semantics::SemanticDomainId> {
    let mut qualifications = Vec::new();
    let Some(contract) = checked.facts.contract_plans.for_machine(machine) else {
        return qualifications;
    };
    for clause in contract.closed_scalar_values.ensures() {
        let Some(checked_trees::ClosedScalarContractValue::SourceCheckedGuarantee {
            result_memberships,
            ..
        }) = clause
        else {
            continue;
        };
        qualifications.extend(result_memberships.iter().copied());
    }
    qualifications.sort_by_key(|domain| domain.0);
    qualifications.dedup();
    qualifications
}

pub(crate) fn lower_projected_qualifications(
    identities: &mut terminal_psi::TerminalIdentityTableBuilder,
    rows: &[checked_trees::CheckedStructuralPathQualification],
    domain_ids: &[StructuralDomainBinding],
    root: StructuralTypeId,
    structural_types: &[StructuralTypeDeclaration],
) -> Result<Vec<terminal_psi::StructuralPathQualification>, LoweringError> {
    let mut qualifications = rows
        .iter()
        .map(|row| {
            if row.path.is_empty() {
                return unsupported("projected qualification has an empty path");
            }
            // Case identities are local to the exact sum declaration. Resolve the
            // static qualification path against its carrier through the shared
            // typed projection route; no runtime selectors are available here.
            let path = lower_typed_argument_path(
                identities,
                &row.path,
                root,
                structural_types,
                None,
                &mut std::iter::empty(),
                None,
            )?;
            let carrier = terminal_semantics::runtime_structural_path_tip(
                structural_types.iter(),
                root,
                &path,
            )
            .ok_or(LoweringError::Unsupported(
                "projected qualification has no exact structural carrier",
            ))?
            .id;
            Ok(terminal_psi::StructuralPathQualification {
                path,
                domain: lookup_domain_id_for_carrier(domain_ids, row.domain, carrier)?,
            })
        })
        .collect::<Result<Vec<_>, LoweringError>>()?;
    // Terminal domain IDs use catalog order, not the source semantic-domain
    // order. Canonicalize after remapping without collapsing duplicate evidence.
    qualifications.sort();
    if qualifications.windows(2).any(|pair| pair[0] == pair[1]) {
        return unsupported("projected qualifications contain duplicate domain paths");
    }
    Ok(qualifications)
}

/// The primitive storage each borrowed primitive formal lends (`&u8`,
/// `&mut i64`), as (formal, place, carrier) rows: a bare referent read
/// resolves the loaned scalar through it, never a snapshot. `plans` and
/// `parameters` are one formal roster in the same order.
pub(crate) fn primitive_reference_storage(
    checked: &CheckedTrees,
    state: symbols::SymbolHandle,
    plans: &[checked_trees::CheckedUnitStructuralParameterPlan],
    parameters: &[(u32, StructuralParameterDeclaration)],
) -> Result<
    Vec<(
        symbols::SymbolHandle,
        semantic_vocabulary::PlaceId,
        ScalarType,
    )>,
    LoweringError,
> {
    let (_, authored) = crate::declarations::state_declaration(checked, state)?;
    let formals = checked.state_parameters(authored);
    let mut storage = Vec::new();
    for (plan, (_, parameter)) in plans.iter().zip(parameters) {
        let Some(primitive) = plan.primitive_referent else {
            continue;
        };
        if primitive == checked_trees::types::PrimitiveType::Addr
            || !matches!(
                plan.access,
                checked_trees::CheckedStructuralAccess::SharedBorrow
                    | checked_trees::CheckedStructuralAccess::MutableBorrow
            )
        {
            continue;
        }
        let formal = formals
            .get(plan.position as usize)
            .ok_or(LoweringError::Unsupported(
                "primitive reference formal lost its authored position",
            ))?;
        storage.push((
            formal.symbol,
            parameter.place,
            terminal_scalar_type(primitive)?,
        ));
    }
    Ok(storage)
}
