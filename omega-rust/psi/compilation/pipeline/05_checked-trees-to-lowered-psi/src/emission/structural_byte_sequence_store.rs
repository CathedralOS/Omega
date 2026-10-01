//! Source correspondence and ordered emission for bounded byte-field replacement.
//!
//! Field operands retain either a formal's storage or an established local's
//! current owned home. Both routes observe live length and snapshot the bytes
//! before replacing the destination; neither reuses an initializer's contents
//! after a call has mutated that home. Destination authority remains separate.
use super::{
    CheckedTrees, CheckedUnitStructuralPathSegment, LoweringError, Operation, OperationKind,
    OperationResult, PrimitiveType, StructuralParameterDeclaration, StructuralPlaceDeclaration,
    StructuralPlaceKind, StructuralTypeDeclaration, StructuralTypeId, StructuralTypeShape,
    ValueDeclaration, allocate_dense, obligation_id, place_id, structural_type_id,
    terminal_scalar_type, unsupported, value_id,
};
use crate::emission::operation_emission::buffer::{OperationBuffer, StructuralBinding};

use checked_trees::{
    CheckedByteSequenceStoreDestination as Destination, CheckedByteSequenceValue as ByteValue,
    CheckedByteSequenceValuePlan,
};
use terminal_psi::control_flow::ByteSequenceOperand;

const LITERAL_VIEW_IDENTITY: &str = "compiler(byte-sequence-literal-view)";

/// The ordered frame supplies a real formal or a previously produced local
/// reference or owned home. Keeping these distinct prevents manufacturing parameter custody
/// for a call result; source replay and Terminal verify the loan independently.
pub(crate) enum StoreRoot<'a> {
    Parameter(&'a StructuralParameterDeclaration),
    Reference {
        place: semantic_vocabulary::PlaceId,
        structural_type: StructuralTypeId,
    },
    Owned {
        place: semantic_vocabulary::PlaceId,
        structural_type: StructuralTypeId,
    },
}

/// Shared callees must use the closure's existing carrier, not allocate one.
pub(crate) fn existing_literal_view_type(
    types: &[StructuralTypeDeclaration],
) -> Result<StructuralTypeId, LoweringError> {
    let declaration = types
        .iter()
        .find(|declaration| declaration.identity == LITERAL_VIEW_IDENTITY)
        .ok_or(LoweringError::Unsupported(
            "composed callable produced a type absent from the shared catalog",
        ))?;
    if !declaration.shape.is_borrowed_byte_view() {
        return unsupported("generated literal-view identity has a different carrier");
    }
    Ok(declaration.id)
}

pub(crate) fn validate_assignment(
    checked: &CheckedTrees,
    machine: symbols::SymbolHandle,
    state_symbol: symbols::SymbolHandle,
    assignment: &checked_trees::statement::TableAssignment,
    store: &checked_trees::CheckedByteSequenceStorePlan,
) -> Result<(), LoweringError> {
    let (owner, state) =
        crate::expression_preparation::source_custody::authored_state(checked, state_symbol)?;
    if owner.symbol != machine {
        return unsupported("byte-field store has a different authored machine");
    }
    // Alias replay needs the use occurrence to rejoin the declaration, active
    // loan and retirement evidence. Discarding this retained coordinate would
    // restrict replacement to directly spelled parameter fields even though
    // checking already normalized a borrowed destination to the same storage.
    let statement_index = store.statement_index as usize;
    let source = crate::emission::call_source_custody::projected_receivers::store_destination(
        checked,
        machine,
        state_symbol,
        Some(statement_index),
        assignment.target,
    )?;
    let (root, mut path) = match store.root {
        checked_trees::CheckedStoreRoot::Parameter { position } => {
            let parameter = checked
                .state_parameters(state)
                .get(position as usize)
                .ok_or(LoweringError::Unsupported(
                    "byte-field store has no authored parameter",
                ))?;
            (parameter.symbol, Vec::new())
        }
        checked_trees::CheckedStoreRoot::Local { symbol } => {
            crate::emission::structural_scalar_store_source::local_store_destination(
                checked,
                machine,
                state,
                state_symbol,
                store.statement_index,
                symbol,
            )?
        }
    };
    match &store.destination {
        Destination::Whole => {}
        Destination::Field {
            carrier_path,
            field_identity,
        }
        | Destination::ViewField {
            carrier_path,
            field_identity,
        } => {
            path.extend_from_slice(carrier_path);
            path.push(CheckedUnitStructuralPathSegment::Field(
                field_identity.clone(),
            ));
        }
    }
    // A descriptor replacement installs a stored loan: only a literal's
    // image storage outlives every destination root.
    if matches!(store.destination, Destination::ViewField { .. })
        && !matches!(
            store.value.nodes.get(store.value.root),
            ByteValue::Literal(_)
        )
    {
        return unsupported("view-field store source is not a literal");
    }
    // A record literal decomposes into one store per member: a shared view
    // member's store writes the authored place's own member field, and its
    // literal is that member's authored initializer.
    if let (
        Destination::ViewField { field_identity, .. },
        checked_trees::expression::ExpressionNode::StructLiteral(literal),
    ) = (
        &store.destination,
        checked.expression_table.expression(assignment.value),
    ) {
        let mut member_path = source.path.clone();
        member_path.push(CheckedUnitStructuralPathSegment::Field(
            field_identity.clone(),
        ));
        let ByteValue::Literal(bytes) = store.value.nodes.get(store.value.root) else {
            return unsupported("record view member store source is not a literal");
        };
        let member = checked
            .expression_table
            .struct_fields(literal.fields)
            .iter()
            .filter(|initializer| {
                crate::emission::structural_scalar_store_source::member_identity(
                    checked,
                    initializer.field_symbol,
                )
                .as_deref()
                    == Some(field_identity.as_str())
            })
            .map(|initializer| checked.expression_table.expression(initializer.value))
            .collect::<Vec<_>>();
        let [checked_trees::expression::ExpressionNode::String(authored)] = member.as_slice()
        else {
            return unsupported("record view member store has no authored literal member");
        };
        if source.root != root || member_path != path || authored.as_ref() != bytes.as_slice() {
            return unsupported("record view member store differs from its authored member");
        }
        return Ok(());
    }
    if source.root != root || source.path != path {
        return unsupported("byte-field store destination differs from its authored place");
    }
    validate_value(
        checked,
        machine,
        state_symbol,
        statement_index,
        assignment.value,
        &store.value,
        store.value.root,
        0,
    )?;
    Ok(())
}

pub(crate) fn validate_value(
    checked: &CheckedTrees,
    machine: symbols::SymbolHandle,
    state_symbol: symbols::SymbolHandle,
    statement_index: usize,
    expression: checked_trees::expression::ExpressionHandle,
    plan: &CheckedByteSequenceValuePlan,
    node: arena::Handle<ByteValue>,
    depth: usize,
) -> Result<(), LoweringError> {
    use checked_trees::expression::{BinaryOperator, ExpressionNode};
    if depth > 128 || !plan.nodes.iter().any(|(identity, _)| identity == node) {
        return unsupported("byte value plan has an invalid child");
    }
    match (
        plan.nodes.get(node),
        checked.expression_table.expression(expression),
    ) {
        (ByteValue::Literal(expected), ExpressionNode::String(actual))
            if actual.as_ref() == expected.as_slice() =>
        {
            Ok(())
        }
        (
            ByteValue::CallResult {
                state,
                statement_index: occurrence,
                capacity,
                ..
            },
            ExpressionNode::Call(_),
        ) => {
            if *state != state_symbol || *occurrence as usize != statement_index {
                return unsupported("byte result names a different authored call occurrence");
            }
            let coordinate = checked_trees::CheckedUnitCallCoordinate {
                statement_index: *occurrence,
                call_ordinal: 0,
            };
            let authored = super::call_source_custody::authored::locate_source(
                checked,
                state_symbol,
                coordinate,
            )?;
            let target = super::call_source_custody::authored::target_signature(
                checked,
                machine,
                authored.source_target,
            )?;
            if checked.type_multiplicity(target.return_type)
                != language_semantics::Multiplicity::Unrestricted
                || validation::bounded_byte_buffer_capacity(&checked.typed, target.return_type)
                    .and_then(|declared| u64::try_from(declared).ok())
                    != Some(*capacity)
                || !validation::has_plain_owned_contents_with_represented_constraints(
                    &checked.typed,
                    target.return_type,
                )
            {
                return unsupported("byte result differs from its owned authored signature");
            }
            super::call_source_custody::occurrences::validate(
                checked,
                machine,
                state_symbol,
                coordinate,
                expression,
            )
        }
        (ByteValue::Concatenation { left, right, .. }, ExpressionNode::Binary(binary))
            if binary.operator == BinaryOperator::Add
                && checked
                    .facts
                    .operators
                    .expression_use(expression)
                    .is_none_or(|selected| {
                        selected.status
                            == checked_trees::CheckedOperatorResolutionStatus::BuiltinFallback
                    }) =>
        {
            validate_value(
                checked,
                machine,
                state_symbol,
                statement_index,
                binary.left,
                plan,
                *left,
                depth + 1,
            )?;
            validate_value(
                checked,
                machine,
                state_symbol,
                statement_index,
                binary.right,
                plan,
                *right,
                depth + 1,
            )
        }
        (ByteValue::Whole { root, capacity }, _) => {
            let (_, state) = crate::expression_preparation::source_custody::authored_state(
                checked,
                state_symbol,
            )?;
            let source =
                crate::emission::call_source_custody::projected_receivers::store_destination(
                    checked,
                    machine,
                    state_symbol,
                    Some(statement_index),
                    expression,
                )?;
            let (root_symbol, type_reference) = match *root {
                checked_trees::CheckedStoreRoot::Parameter { position } => {
                    let parameter = checked
                        .state_parameters(state)
                        .get(position as usize)
                        .ok_or(LoweringError::Unsupported(
                            "whole byte source parameter is absent",
                        ))?;
                    validate_whole_parameter_custody(
                        checked,
                        machine,
                        state_symbol,
                        position,
                        parameter,
                    )?;
                    (parameter.symbol, parameter.type_reference)
                }
                checked_trees::CheckedStoreRoot::Local { symbol } => {
                    let (_, local) = checked
                        .state_local_declaration(state, symbol)
                        .filter(|&(ordinal, _)| ordinal < statement_index)
                        .ok_or(LoweringError::Unsupported(
                            "whole byte source local has no earlier declaration",
                        ))?;
                    if validation::bounded_byte_buffer_capacity(
                        &checked.typed,
                        local.type_reference,
                    )
                    .is_none()
                    {
                        return unsupported("whole byte source local has no owned bounded storage");
                    }
                    (symbol, local.type_reference)
                }
            };
            let carrier =
                crate::unit::attached_unit::structural_carrier_type(checked, type_reference)?;
            let readable = match checked.type_reference_table.type_reference(carrier) {
                checked_trees::types::TypeReferenceNode::FixedArray { .. } => true,
                checked_trees::types::TypeReferenceNode::Reference { access, .. } => matches!(
                    access,
                    language_semantics::ReferenceAccess::Shared
                        | language_semantics::ReferenceAccess::Mutable
                ),
                _ => false,
            };
            let declared_capacity =
                validation::bounded_byte_buffer_capacity(&checked.typed, type_reference)
                    .or_else(|| {
                        validation::loaned_bounded_byte_buffer_capacity(
                            &checked.typed,
                            type_reference,
                        )
                    })
                    .and_then(|capacity| u64::try_from(capacity).ok());
            if source.root != root_symbol
                || !source.path.is_empty()
                || !readable
                || declared_capacity != Some(*capacity)
                || checked.type_multiplicity(type_reference)
                    != language_semantics::Multiplicity::Unrestricted
            {
                return unsupported("whole byte source differs from its readable authored place");
            }
            Ok(())
        }
        (
            ByteValue::Field {
                root,
                carrier_path,
                field_identity,
                ..
            },
            _,
        ) => {
            let (_, state) = crate::expression_preparation::source_custody::authored_state(
                checked,
                state_symbol,
            )?;
            let source =
                crate::emission::call_source_custody::projected_receivers::store_destination(
                    checked,
                    machine,
                    state_symbol,
                    Some(statement_index),
                    expression,
                )?;
            let root_symbol = match *root {
                checked_trees::CheckedStoreRoot::Parameter { position } => {
                    checked
                        .state_parameters(state)
                        .get(position as usize)
                        .ok_or(LoweringError::Unsupported("byte value parameter is absent"))?
                        .symbol
                }
                checked_trees::CheckedStoreRoot::Local { symbol } => {
                    let (_, local) = checked
                        .state_local_declaration(state, symbol)
                        .filter(|&(ordinal, _)| ordinal < statement_index)
                        .ok_or(LoweringError::Unsupported(
                            "byte value local has no earlier declaration",
                        ))?;
                    if !validation::has_plain_owned_contents_with_represented_constraints(
                        &checked.typed,
                        local.type_reference,
                    ) {
                        return unsupported("byte value local has no plain owned storage");
                    }
                    symbol
                }
            };
            let mut path = carrier_path.clone();
            path.push(CheckedUnitStructuralPathSegment::Field(
                field_identity.clone(),
            ));
            if source.root != root_symbol || source.path != path {
                return unsupported("byte value field differs from its authored place");
            }
            Ok(())
        }
        _ => unsupported("byte value differs from its authored expression"),
    }
}

/// Whole byte reads preserve the authored assumptions, not merely a compatible
/// byte layout. Rejoin the selected formal before its qualification roster is
/// lowered into Terminal's parameter assumptions.
fn validate_whole_parameter_custody(
    checked: &CheckedTrees,
    machine: symbols::SymbolHandle,
    state: symbols::SymbolHandle,
    position: u32,
    source: &checked_trees::signature::StateParameter,
) -> Result<(), LoweringError> {
    use crate::unit::attached_unit::bodies::{UnitBody, UnitPlans};
    use checked_trees::types::{TypeConstraintNode, TypeReferenceNode};
    let body = UnitBody::find(
        UnitPlans::published(&checked.facts.flow.terminal_unit_effects),
        machine,
    )?;
    let parameters = match body {
        UnitBody::Ordinary(plan) if plan.state == state => plan.structural_parameters.as_slice(),
        UnitBody::Composed(plan) => {
            let mut states = plan
                .states
                .iter()
                .filter(|candidate| candidate.state == state);
            let row = states.next().ok_or(LoweringError::Unsupported(
                "whole byte source state is absent",
            ))?;
            if states.next().is_some() {
                return unsupported("whole byte source state is duplicated");
            }
            row.structural_parameters.as_slice()
        }
        _ => return unsupported("whole byte source state differs from its body"),
    };
    let mut candidates = parameters
        .iter()
        .filter(|parameter| parameter.position == position);
    let parameter = candidates.next().ok_or(LoweringError::Unsupported(
        "whole byte checked formal is absent",
    ))?;
    if candidates.next().is_some() {
        return unsupported("whole byte checked formal is duplicated");
    }
    let mut reference = source.type_reference;
    let mut access = checked_trees::CheckedStructuralAccess::Owned;
    let mut qualifications = Vec::new();
    let mut visited = Vec::new();
    loop {
        if !reference.is_valid() || visited.contains(&reference) {
            return unsupported("whole byte source type chain is invalid");
        }
        visited.push(reference);
        match checked.type_reference_table.type_reference(reference) {
            TypeReferenceNode::Constrained {
                base_type,
                constraints,
            } => {
                for constraint in checked.type_reference_table.constraints(*constraints) {
                    match constraint {
                        TypeConstraintNode::Domain(domain) if domain.semantic_id.is_valid() => {
                            qualifications.push(domain.semantic_id)
                        }
                        TypeConstraintNode::ArithmeticDomain(_) => {}
                        _ => {
                            return unsupported(
                                "whole byte source has an unsupported qualification",
                            );
                        }
                    }
                }
                reference = *base_type;
            }
            TypeReferenceNode::Reference {
                referee,
                access: reference_access,
                ..
            } if access == checked_trees::CheckedStructuralAccess::Owned => {
                access = match reference_access {
                    language_semantics::ReferenceAccess::Shared => {
                        checked_trees::CheckedStructuralAccess::SharedBorrow
                    }
                    language_semantics::ReferenceAccess::Mutable => {
                        checked_trees::CheckedStructuralAccess::MutableBorrow
                    }
                    _ => return unsupported("whole byte source is not readable"),
                };
                reference = *referee;
            }
            TypeReferenceNode::FixedArray { .. } => break,
            _ => return unsupported("whole byte source has no bounded storage carrier"),
        }
    }
    qualifications.sort_by_key(|domain| domain.0);
    qualifications.dedup();
    if parameter.access != access
        || parameter.is_self != source.is_self
        || parameter.multiplicity != checked.type_multiplicity(source.type_reference)
        || parameter.type_identity != checked.normalized_type_identity(reference).as_str()
        || parameter.qualifications != qualifications
        || !parameter.projected_qualifications.is_empty()
        || parameter.fused_service_erasure.is_some()
    {
        return unsupported("whole byte checked formal changed its authored custody");
    }
    Ok(())
}

/// Generated owned carriers are shared by capacity, independently of live extent.
pub(crate) fn prepare_types(
    value: &CheckedByteSequenceValuePlan,
    types: &mut Vec<StructuralTypeDeclaration>,
) -> Result<(), LoweringError> {
    literal_view_type(types)?;
    for (identity, node) in value.nodes.iter() {
        let capacity = match node {
            ByteValue::Concatenation { capacity, .. } => Some(capacity),
            ByteValue::Field { capacity, .. } | ByteValue::Whole { capacity, .. }
                if identity == value.root =>
            {
                Some(capacity)
            }
            _ => None,
        };
        if let Some(capacity) = capacity {
            let identity = format!("compiler(byte-sequence-owned:{capacity})");
            let shape = StructuralTypeShape::ByteSequence(
                terminal_psi::ByteSequenceCarrier::BoundedOwned {
                    capacity: *capacity,
                },
            );
            if let Some(existing) = types
                .iter()
                .find(|candidate| candidate.identity == identity)
            {
                if existing.shape != shape {
                    return unsupported("generated owned byte carrier has a different shape");
                }
            } else {
                let ordinal = types
                    .iter()
                    .map(|candidate| candidate.id.get())
                    .max()
                    .unwrap_or(0)
                    .checked_add(1)
                    .ok_or(LoweringError::Unsupported(
                        "owned byte type identity overflow",
                    ))?;
                types.push(StructuralTypeDeclaration {
                    id: structural_type_id(ordinal),
                    identity,
                    shape,
                });
            }
        }
    }
    Ok(())
}

/// A generated immutable carrier, not a nominal authored type or parameter.
pub(crate) fn literal_view_type(
    types: &mut Vec<StructuralTypeDeclaration>,
) -> Result<StructuralTypeId, LoweringError> {
    let shape =
        StructuralTypeShape::ByteSequence(terminal_psi::ByteSequenceCarrier::BorrowedView {
            access: Some(terminal_psi::StructuralAccess::SharedBorrow),
        });
    if let Some(declaration) = types
        .iter()
        .find(|declaration| declaration.identity == LITERAL_VIEW_IDENTITY)
    {
        return if declaration.shape == shape {
            Ok(declaration.id)
        } else {
            unsupported("generated literal-view identity has a different carrier")
        };
    }
    let identity = types
        .iter()
        .map(|declaration| declaration.id.get())
        .max()
        .unwrap_or(0)
        .checked_add(1)
        .ok_or(LoweringError::Unsupported(
            "literal-view type identity overflow",
        ))?;
    let id = structural_type_id(identity);
    types.push(StructuralTypeDeclaration {
        id,
        identity: LITERAL_VIEW_IDENTITY.into(),
        shape,
    });
    Ok(id)
}

pub(crate) fn emit(
    store: &checked_trees::CheckedByteSequenceStorePlan,
    root: StoreRoot<'_>,
    parameters: &[(u32, StructuralParameterDeclaration)],
    locals: &[(symbols::SymbolHandle, terminal_psi::StructuralArgument)],
    structural_types: &[StructuralTypeDeclaration],
    literal_places: &mut Vec<StructuralPlaceDeclaration>,
    next_place: &mut u64,
    next_value: &mut u64,
    next_obligation: &mut u64,
    operations: &mut OperationBuffer,
) -> Result<OperationKind, LoweringError> {
    let destination = match &root {
        StoreRoot::Parameter(parameter) => parameter.place,
        StoreRoot::Reference { place, .. } | StoreRoot::Owned { place, .. } => *place,
    };
    if let Destination::ViewField {
        carrier_path,
        field_identity,
    } = &store.destination
    {
        return emit_view_field_store(
            store,
            &root,
            carrier_path,
            field_identity,
            structural_types,
            literal_places,
            next_place,
            operations,
        );
    }
    let field_destination = match &store.destination {
        Destination::Whole => {
            let StoreRoot::Parameter(parameter) = &root else {
                return unsupported("whole byte store requires a borrowed bounded formal");
            };
            if !matches!(
                parameter.access,
                terminal_psi::StructuralAccess::MutableBorrow
                    | terminal_psi::StructuralAccess::WriteOnlyBorrow
            ) || !structural_types
                .iter()
                .find(|declaration| declaration.id == parameter.structural_type)
                .is_some_and(|declaration| {
                    matches!(
                        declaration.shape,
                        StructuralTypeShape::ByteSequence(
                            terminal_psi::ByteSequenceCarrier::BoundedOwned { .. }
                        )
                    )
                })
            {
                return unsupported("whole byte store requires a borrowed bounded owner");
            }
            None
        }
        Destination::Field {
            carrier_path,
            field_identity,
        } => {
            let (path, field) = match &root {
                StoreRoot::Parameter(parameter) => {
                    crate::emission::structural_scalar_store::lower_structural_field_place(
                        store.statement_index,
                        store.statement_index,
                        parameter.position,
                        carrier_path,
                        field_identity,
                        parameter,
                        structural_types,
                        crate::emission::structural_scalar_store::StoreAccessPolicy::Exclusive,
                    )?
                }
                StoreRoot::Reference {
                    structural_type, ..
                } => {
                    if !structural_types.iter().any(|declaration| {
                        declaration.id == *structural_type
                            && matches!(
                                declaration.shape,
                                StructuralTypeShape::Reference {
                                    access: terminal_psi::StructuralAccess::MutableBorrow,
                                    ..
                                }
                            )
                    }) {
                        return unsupported(
                            "byte-field local destination lost mutable reference custody",
                        );
                    }
                    crate::emission::structural_scalar_store::lower_structural_field_path(
                        *structural_type,
                        carrier_path,
                        field_identity,
                        structural_types,
                    )?
                }
                StoreRoot::Owned {
                    place,
                    structural_type,
                } => {
                    // Resolve an existing whole local, never a fabricated formal
                    // or a type-only promise that its fields are initialized.
                    let produced = operations
                        .operations
                        .iter()
                        .filter_map(|operation| operation.result.structural())
                        .find(|result| result.place == *place)
                        .ok_or(LoweringError::Unsupported(
                            "byte-field owned destination has no completed producer",
                        ))?;
                    if produced.structural_type != *structural_type
                        || produced.multiplicity
                            != terminal_psi::StructuralMultiplicity::Unrestricted
                        || !produced.qualifications.is_empty()
                        || !produced.projected_qualifications.is_empty()
                        || !produced.qualification_establishments.is_empty()
                        || !produced.claims.is_empty()
                        || !locals.iter().any(|(_, local)| {
                            local.place == *place
                                && local.access == terminal_psi::StructuralAccess::Owned
                                && local.path.is_empty()
                        })
                        || !structural_types.iter().any(|declaration| {
                            declaration.id == *structural_type
                                && matches!(declaration.shape, StructuralTypeShape::Record { .. })
                        })
                    {
                        return unsupported(
                            "byte-field local destination lost owned record custody",
                        );
                    }
                    crate::emission::structural_scalar_store::lower_structural_field_path(
                        *structural_type,
                        carrier_path,
                        field_identity,
                        structural_types,
                    )?
                }
            };
            if !matches!(
                field.field_type.byte_sequence_carrier(),
                Some(terminal_psi::ByteSequenceCarrier::BoundedOwned { .. })
            ) {
                return unsupported("byte-field store destination is not bounded owned bytes");
            }
            Some((path, field.id))
        }
        Destination::ViewField { .. } => {
            return unsupported("view-field store reached the owned byte emitter");
        }
    };
    let mut emitter = ValueEmitter {
        plan: &store.value,
        parameters,
        locals,
        emission: ByteEmission {
            structural_types,
            places: literal_places,
            next_place,
            next_value,
            next_obligation,
            operations,
        },
    };
    let (operand, capacity) = emitter.emit(store.value.root, 0)?;
    let (source, length) = match operand {
        ByteSequenceOperand::Whole { source, length }
            if !matches!(
                store.value.nodes.get(store.value.root),
                ByteValue::Whole {
                    root: checked_trees::CheckedStoreRoot::Local { .. },
                    ..
                }
            ) =>
        {
            (source, length)
        }
        source => {
            // Local owners and fields must be snapshotted before the
            // destination write, including writes through an alias.
            // Parameter sources retain the store's exact formal-access
            // contract; generated literals and concatenations are already
            // independent sources.
            let (empty, _) = emitter.emission.literal(&[])?;
            emitter.owned(source, empty, capacity)?
        }
    };
    let obligation = obligation_id(allocate_dense(next_obligation)?);
    match field_destination {
        Some((path, field)) => {
            operations.invalidate_field_lengths_under(destination, &path);
            Ok(OperationKind::StructuralByteSequenceFieldStore {
                destination,
                path,
                field,
                source,
                length,
                obligation,
            })
        }
        None => {
            operations.invalidate_field_lengths_under(destination, &[]);
            Ok(OperationKind::ByteSequenceStore {
                destination,
                source,
                length,
                obligation,
            })
        }
    }
}

/// Replace one shared borrowed-view field's descriptor with a literal's view
/// of image storage. The literal is established at the store, without a
/// qualification roster: Terminal replays the field's predicates from the
/// octets. No bytes are copied, so no length observation or capacity
/// obligation accompanies the store.
#[allow(clippy::too_many_arguments)]
fn emit_view_field_store(
    store: &checked_trees::CheckedByteSequenceStorePlan,
    root: &StoreRoot<'_>,
    carrier_path: &[CheckedUnitStructuralPathSegment],
    field_identity: &str,
    structural_types: &[StructuralTypeDeclaration],
    literal_places: &mut Vec<StructuralPlaceDeclaration>,
    next_place: &mut u64,
    operations: &mut OperationBuffer,
) -> Result<OperationKind, LoweringError> {
    let ByteValue::Literal(bytes) = store.value.nodes.get(store.value.root) else {
        return unsupported("view-field store source is not a literal");
    };
    let (destination, (path, field)) = match root {
        StoreRoot::Parameter(parameter) => (
            parameter.place,
            crate::emission::structural_scalar_store::lower_structural_field_place(
                store.statement_index,
                store.statement_index,
                parameter.position,
                carrier_path,
                field_identity,
                parameter,
                structural_types,
                crate::emission::structural_scalar_store::StoreAccessPolicy::Exclusive,
            )?,
        ),
        StoreRoot::Reference {
            place,
            structural_type,
        } => {
            if !structural_types.iter().any(|declaration| {
                declaration.id == *structural_type
                    && matches!(
                        declaration.shape,
                        StructuralTypeShape::Reference {
                            access: terminal_psi::StructuralAccess::MutableBorrow,
                            ..
                        }
                    )
            }) {
                return unsupported("view-field local destination lost mutable reference custody");
            }
            (
                *place,
                crate::emission::structural_scalar_store::lower_structural_field_path(
                    *structural_type,
                    carrier_path,
                    field_identity,
                    structural_types,
                )?,
            )
        }
        // An owned local root has no exclusive loan whose referent the
        // literal's image backing is checked to outlive, so the store stays
        // parameter- and reference-rooted.
        StoreRoot::Owned { .. } => {
            return unsupported("view-field store into an owned local root");
        }
    };
    if field.field_type.byte_sequence_carrier()
        != Some(terminal_psi::ByteSequenceCarrier::BorrowedView {
            access: Some(terminal_psi::StructuralAccess::SharedBorrow),
        })
    {
        return unsupported("view-field store destination is not a shared view field");
    }
    let source = place_id(allocate_dense(next_place)?);
    literal_places.push(StructuralPlaceDeclaration {
        id: source,
        kind: StructuralPlaceKind::ByteSequenceLiteral {
            declaration_ordinal: super::next_byte_sequence_literal_ordinal(literal_places)?,
            structural_type: existing_literal_view_type(structural_types)?,
        },
    });
    let id = operations.allocate();
    operations.push(Operation {
        id,
        static_reach_binding: None,
        suspension_crossing: None,
        result: OperationResult::Unit,
        kind: OperationKind::EstablishByteSequenceLiteral {
            destination: source,
            bytes: bytes.clone(),
            qualifications: Vec::new(),
        },
    });
    operations.invalidate_field_lengths_under(destination, &path);
    Ok(OperationKind::StructuralByteViewFieldStore {
        destination,
        path,
        field: field.id,
        source,
    })
}

/// Establish an owned bounded byte local from its checked initializer.
///
/// `let text: [u8; 8] in Utf8 = "150";` owns capacity 8 and live length 3
/// (wiki/spec/language/domains.md, "Byte containers and encoding domains").
/// The initializer's live bytes are copied into fresh backing of the local's
/// declared carrier. Later mutations of an operand cannot change the local.
/// Concatenation proves capacity and Terminal replays the byte predicates.
///
/// Returns the result place's declaration and the established result. The
/// caller publishes the declaration in its own result roster.
#[allow(clippy::too_many_arguments)]
pub(crate) fn emit_owned_value(
    value: &CheckedByteSequenceValuePlan,
    structural_type: StructuralTypeId,
    qualifications: Vec<semantic_vocabulary::StructuralDomainId>,
    parameters: &[(u32, StructuralParameterDeclaration)],
    locals: &[(symbols::SymbolHandle, terminal_psi::StructuralArgument)],
    structural_types: &[StructuralTypeDeclaration],
    places: &mut Vec<StructuralPlaceDeclaration>,
    next_place: &mut u64,
    next_value: &mut u64,
    next_obligation: &mut u64,
    operations: &mut OperationBuffer,
) -> Result<
    (
        StructuralPlaceDeclaration,
        terminal_psi::StructuralOperationResult,
    ),
    LoweringError,
> {
    let Some(StructuralTypeShape::ByteSequence(terminal_psi::ByteSequenceCarrier::BoundedOwned {
        capacity,
    })) = structural_types
        .iter()
        .find(|row| row.id == structural_type)
        .map(|row| &row.shape)
    else {
        return unsupported("owned byte local has no bounded carrier");
    };
    let mut emitter = ValueEmitter {
        plan: value,
        parameters,
        locals,
        emission: ByteEmission {
            structural_types,
            places,
            next_place,
            next_value,
            next_obligation,
            operations,
        },
    };
    let (operand, maximum_length) = emitter.emit(value.root, 0)?;
    if maximum_length > *capacity {
        return unsupported("owned byte initializer exceeds its declared capacity");
    }
    // Snapshot into the declaration's carrier even when the operand is an
    // existing owner. The local is an independent value, never an alias.
    let (empty, _) = emitter.emission.literal(&[])?;
    emitter
        .emission
        .concatenate(operand, empty, structural_type, qualifications)
}

pub(crate) fn emit_owned_literal(
    bytes: &[u8],
    structural_type: StructuralTypeId,
    qualifications: Vec<semantic_vocabulary::StructuralDomainId>,
    structural_types: &[StructuralTypeDeclaration],
    literal_places: &mut Vec<StructuralPlaceDeclaration>,
    next_place: &mut u64,
    next_value: &mut u64,
    next_obligation: &mut u64,
    operations: &mut OperationBuffer,
) -> Result<
    (
        StructuralPlaceDeclaration,
        terminal_psi::StructuralOperationResult,
    ),
    LoweringError,
> {
    let Some(StructuralTypeShape::ByteSequence(terminal_psi::ByteSequenceCarrier::BoundedOwned {
        capacity,
    })) = structural_types
        .iter()
        .find(|declaration| declaration.id == structural_type)
        .map(|declaration| declaration.shape.clone())
    else {
        return unsupported("owned literal local carrier is not a bounded byte owner");
    };
    if u64::try_from(bytes.len()).map_or(true, |length| length > capacity) {
        return unsupported("owned literal local exceeds its declared capacity");
    }
    let mut emission = ByteEmission {
        structural_types,
        places: literal_places,
        next_place,
        next_value,
        next_obligation,
        operations,
    };
    let (literal, _) = emission.literal(bytes)?;
    let (empty, _) = emission.literal(&[])?;
    emission.concatenate(literal, empty, structural_type, qualifications)
}

/// Materialize one literal at its exact call carrier. Borrowed views lend
/// immutable bytes; owned formals get fresh concatenation backing. Both Unit
/// control forms use this owner so neither can reinterpret image storage as
/// mutable owned storage. The caller retains the returned argument place;
/// `places` retains every supporting declaration, including that place.
#[allow(clippy::too_many_arguments)]
pub(crate) fn emit_call_literal(
    bytes: &[u8],
    structural_type: StructuralTypeId,
    access: checked_trees::CheckedStructuralAccess,
    qualifications: Vec<semantic_vocabulary::StructuralDomainId>,
    structural_types: &[StructuralTypeDeclaration],
    places: &mut Vec<StructuralPlaceDeclaration>,
    next_place: &mut u64,
    next_value: &mut u64,
    next_obligation: &mut u64,
    operations: &mut OperationBuffer,
) -> Result<StructuralPlaceDeclaration, LoweringError> {
    let shape = structural_types
        .iter()
        .find(|row| row.id == structural_type)
        .map(|row| &row.shape);
    let declaration = match (access, shape) {
        (
            checked_trees::CheckedStructuralAccess::Owned,
            Some(StructuralTypeShape::ByteSequence(
                terminal_psi::ByteSequenceCarrier::BoundedOwned { .. },
            )),
        ) => {
            emit_owned_literal(
                bytes,
                structural_type,
                qualifications,
                structural_types,
                places,
                next_place,
                next_value,
                next_obligation,
                operations,
            )?
            .0
        }
        (
            checked_trees::CheckedStructuralAccess::SharedBorrow,
            Some(StructuralTypeShape::ByteSequence(
                terminal_psi::ByteSequenceCarrier::BorrowedView { .. },
            )),
        ) => {
            let declaration = StructuralPlaceDeclaration {
                id: place_id(allocate_dense(next_place)?),
                kind: StructuralPlaceKind::ByteSequenceLiteral {
                    declaration_ordinal: super::next_byte_sequence_literal_ordinal(places)?,
                    structural_type,
                },
            };
            let id = operations.allocate();
            operations.push(Operation {
                static_reach_binding: None,
                suspension_crossing: None,
                id,
                result: OperationResult::Unit,
                kind: OperationKind::EstablishByteSequenceLiteral {
                    destination: declaration.id,
                    bytes: bytes.to_vec(),
                    qualifications,
                },
            });
            declaration
        }
        _ => return unsupported("literal call access differs from its storage carrier"),
    };
    places.push(declaration);
    Ok(declaration)
}

/// The operation, place and value counters one byte materialization emits
/// into. Literal views and owned concatenations are its only producers.
struct ByteEmission<'input> {
    structural_types: &'input [StructuralTypeDeclaration],
    places: &'input mut Vec<StructuralPlaceDeclaration>,
    next_place: &'input mut u64,
    next_value: &'input mut u64,
    next_obligation: &'input mut u64,
    operations: &'input mut OperationBuffer,
}

impl ByteEmission<'_> {
    fn length(
        &mut self,
        kind: OperationKind,
    ) -> Result<semantic_vocabulary::ValueId, LoweringError> {
        let length = value_id(allocate_dense(self.next_value)?);
        let id = self.operations.allocate();
        self.operations.push(Operation {
            id,
            kind,
            static_reach_binding: None,
            suspension_crossing: None,
            result: OperationResult::Scalar(ValueDeclaration {
                id: length,
                scalar_type: terminal_scalar_type(PrimitiveType::U64)?,
                qualifications: Default::default(),
            }),
        });
        Ok(length)
    }

    fn literal(&mut self, bytes: &[u8]) -> Result<(ByteSequenceOperand, u64), LoweringError> {
        let structural_type = existing_literal_view_type(self.structural_types)?;
        let source = place_id(allocate_dense(self.next_place)?);
        let declaration_ordinal = super::next_byte_sequence_literal_ordinal(self.places)?;
        self.places.push(StructuralPlaceDeclaration {
            id: source,
            kind: StructuralPlaceKind::ByteSequenceLiteral {
                declaration_ordinal,
                structural_type,
            },
        });
        let id = self.operations.allocate();
        self.operations.push(Operation {
            id,
            static_reach_binding: None,
            suspension_crossing: None,
            result: OperationResult::Unit,
            kind: OperationKind::EstablishByteSequenceLiteral {
                destination: source,
                bytes: bytes.to_vec(),
                qualifications: Vec::new(),
            },
        });
        let capacity = u64::try_from(bytes.len())
            .map_err(|_| LoweringError::Unsupported("byte literal length overflow"))?;
        let length = self.length(OperationKind::ByteSequenceLength { source })?;
        Ok((ByteSequenceOperand::Whole { source, length }, capacity))
    }

    /// Materialize `left` then `right` into fresh owned backing of
    /// `structural_type`. The result place's declaration is returned rather
    /// than published: a store's temporary and a local binding publish it in
    /// different rosters.
    fn concatenate(
        &mut self,
        left: ByteSequenceOperand,
        right: ByteSequenceOperand,
        structural_type: StructuralTypeId,
        qualifications: Vec<semantic_vocabulary::StructuralDomainId>,
    ) -> Result<
        (
            StructuralPlaceDeclaration,
            terminal_psi::StructuralOperationResult,
        ),
        LoweringError,
    > {
        let source = place_id(allocate_dense(self.next_place)?);
        let id = self.operations.allocate();
        let declaration = StructuralPlaceDeclaration {
            id: source,
            kind: StructuralPlaceKind::OperationResult {
                producer: id,
                structural_type,
            },
        };
        let result = terminal_psi::StructuralOperationResult {
            place: source,
            structural_type,
            multiplicity: terminal_psi::StructuralMultiplicity::Unrestricted,
            qualifications,
            projected_qualifications: Vec::new(),
            qualification_establishments: Vec::new(),
            claims: Vec::new(),
        };
        self.operations.push(Operation {
            id,
            static_reach_binding: None,
            suspension_crossing: None,
            result: OperationResult::Structural(result.clone()),
            kind: OperationKind::EstablishByteSequenceConcatenation {
                left,
                right,
                capacity_obligation: obligation_id(allocate_dense(self.next_obligation)?),
            },
        });
        Ok((declaration, result))
    }
}

struct ValueEmitter<'input> {
    plan: &'input CheckedByteSequenceValuePlan,
    parameters: &'input [(u32, StructuralParameterDeclaration)],
    locals: &'input [(symbols::SymbolHandle, terminal_psi::StructuralArgument)],
    emission: ByteEmission<'input>,
}

impl ValueEmitter<'_> {
    /// Locate the completed owner, without reconstructing its initial contents.
    /// The caller checks the read shape; Terminal checks liveness at the use.
    fn local_source(
        &self,
        symbol: symbols::SymbolHandle,
    ) -> Result<&terminal_psi::StructuralOperationResult, LoweringError> {
        let mut locals = self
            .locals
            .iter()
            .filter(|(candidate, _)| *candidate == symbol);
        let (_, local) = locals.next().ok_or(LoweringError::Unsupported(
            "byte value local has no established home",
        ))?;
        if locals.next().is_some()
            || local.access != terminal_psi::StructuralAccess::Owned
            || !local.path.is_empty()
        {
            return unsupported("byte value local lost its exact whole owned home");
        }
        self.emission
            .operations
            .operations
            .iter()
            .filter_map(|operation| operation.result.structural())
            .find(|result| result.place == local.place)
            .or_else(|| {
                // Control joins register the same completed owner's custody.
                self.emission
                    .operations
                    .structural_values
                    .iter()
                    .filter_map(|(_, binding)| match binding {
                        StructuralBinding::OperationResult(result) => Some(result),
                        _ => None,
                    })
                    .find(|result| result.place == local.place)
            })
            .ok_or(LoweringError::Unsupported(
                "byte value local has no completed producer",
            ))
    }

    /// Reading a field does not consume its root. Owned formals, borrowed
    /// formals and completed local homes retain the same field-read operation;
    /// the receiving validator separately checks live custody and loan conflicts.
    fn field_source(
        &self,
        root: checked_trees::CheckedStoreRoot,
    ) -> Result<(semantic_vocabulary::PlaceId, StructuralTypeId), LoweringError> {
        match root {
            checked_trees::CheckedStoreRoot::Parameter { position } => {
                let (_, parameter) = self
                    .parameters
                    .iter()
                    .find(|(authored, _)| *authored == position)
                    .ok_or(LoweringError::Unsupported("byte value parameter is absent"))?;
                if !matches!(
                    parameter.access,
                    terminal_psi::StructuralAccess::Owned
                        | terminal_psi::StructuralAccess::SharedBorrow
                        | terminal_psi::StructuralAccess::MutableBorrow
                ) || !matches!(
                    parameter.multiplicity,
                    terminal_psi::StructuralMultiplicity::Unrestricted
                        | terminal_psi::StructuralMultiplicity::Affine
                ) || !parameter.qualifications.is_empty()
                    || !parameter.projected_qualifications.is_empty()
                {
                    return unsupported("byte value parameter has no ordinary read authority");
                }
                Ok((parameter.place, parameter.structural_type))
            }
            checked_trees::CheckedStoreRoot::Local { symbol } => {
                let produced = self.local_source(symbol)?;
                if !matches!(
                    produced.multiplicity,
                    terminal_psi::StructuralMultiplicity::Unrestricted
                        | terminal_psi::StructuralMultiplicity::Affine
                ) || !produced.qualifications.is_empty()
                    || !produced.projected_qualifications.is_empty()
                    || !produced.claims.is_empty()
                {
                    return unsupported("byte value local lost its ordinary read custody");
                }
                Ok((produced.place, produced.structural_type))
            }
        }
    }

    fn owned(
        &mut self,
        left: ByteSequenceOperand,
        right: ByteSequenceOperand,
        capacity: u64,
    ) -> Result<(semantic_vocabulary::PlaceId, semantic_vocabulary::ValueId), LoweringError> {
        let identity = format!("compiler(byte-sequence-owned:{capacity})");
        let structural_type = self
            .emission
            .structural_types
            .iter()
            .find(|candidate| {
                candidate.identity == identity
                    && candidate.shape
                        == StructuralTypeShape::ByteSequence(
                            terminal_psi::ByteSequenceCarrier::BoundedOwned { capacity },
                        )
            })
            .ok_or(LoweringError::Unsupported(
                "owned byte carrier absent from shared catalog",
            ))?
            .id;
        let (declaration, result) =
            self.emission
                .concatenate(left, right, structural_type, Vec::new())?;
        self.emission.places.push(declaration);
        let length = self.emission.length(OperationKind::ByteSequenceLength {
            source: result.place,
        })?;
        Ok((result.place, length))
    }

    fn emit(
        &mut self,
        node: arena::Handle<ByteValue>,
        depth: usize,
    ) -> Result<(ByteSequenceOperand, u64), LoweringError> {
        if depth > 128 || !self.plan.nodes.iter().any(|(identity, _)| identity == node) {
            return unsupported("byte value plan has an invalid child");
        }
        match self.plan.nodes.get(node) {
            ByteValue::Literal(bytes) => self.emission.literal(bytes),
            ByteValue::CallResult {
                state,
                statement_index,
                binding_ordinal,
                capacity,
            } => {
                // Join the binding to the already-emitted exact source call.
                // A same-shaped earlier result is not the assigned value.
                let mut bindings = self
                    .emission
                    .operations
                    .structural_values
                    .iter()
                    .filter(|(ordinal, _)| ordinal == binding_ordinal);
                let (_, binding) = bindings.next().ok_or(LoweringError::Unsupported(
                    "byte call result has no completed binding",
                ))?;
                if bindings.next().is_some() {
                    return unsupported("byte call result binding is ambiguous");
                }
                let produced = binding.operation_result()?;
                let mut occurrences =
                    self.emission
                        .operations
                        .source_calls
                        .iter()
                        .filter(|occurrence| {
                            occurrence.source_state == *state
                                && occurrence.statement_index == *statement_index as usize
                                && occurrence.call_ordinal == 0
                        });
                let occurrence = occurrences.next().ok_or(LoweringError::Unsupported(
                    "byte call result has no completed authored occurrence",
                ))?;
                if occurrences.next().is_some()
                    || !self.emission.operations.operations.iter().any(|operation| {
                        operation.id == occurrence.terminal_operation
                            && operation.result.structural() == Some(produced)
                    })
                    || produced.multiplicity != terminal_psi::StructuralMultiplicity::Unrestricted
                    || !produced.projected_qualifications.is_empty()
                    || !produced.claims.is_empty()
                    || !self.emission.structural_types.iter().any(|declaration| {
                        declaration.id == produced.structural_type
                            && declaration.shape
                                == StructuralTypeShape::ByteSequence(
                                    terminal_psi::ByteSequenceCarrier::BoundedOwned {
                                        capacity: *capacity,
                                    },
                                )
                    })
                {
                    return unsupported("byte call result lost exact completed owned custody");
                }
                let source = produced.place;
                let length = self
                    .emission
                    .length(OperationKind::ByteSequenceLength { source })?;
                Ok((ByteSequenceOperand::Whole { source, length }, *capacity))
            }
            ByteValue::Whole { root, capacity } => {
                let (source, structural_type) = match *root {
                    checked_trees::CheckedStoreRoot::Parameter { position } => {
                        let (_, parameter) = self
                            .parameters
                            .iter()
                            .find(|(authored, _)| *authored == position)
                            .ok_or(LoweringError::Unsupported(
                                "whole byte value parameter is absent",
                            ))?;
                        if !matches!(
                            parameter.access,
                            terminal_psi::StructuralAccess::Owned
                                | terminal_psi::StructuralAccess::SharedBorrow
                                | terminal_psi::StructuralAccess::MutableBorrow
                        ) || parameter.multiplicity
                            != terminal_psi::StructuralMultiplicity::Unrestricted
                            || !parameter.projected_qualifications.is_empty()
                        {
                            return unsupported(
                                "whole byte parameter has no ordinary read authority",
                            );
                        }
                        (parameter.place, parameter.structural_type)
                    }
                    checked_trees::CheckedStoreRoot::Local { symbol } => {
                        let produced = self.local_source(symbol)?;
                        if produced.multiplicity
                            != terminal_psi::StructuralMultiplicity::Unrestricted
                            || !produced.projected_qualifications.is_empty()
                            || !produced.claims.is_empty()
                        {
                            return unsupported("whole byte local has no ordinary read custody");
                        }
                        (produced.place, produced.structural_type)
                    }
                };
                if !self.emission.structural_types.iter().any(|row| {
                    row.id == structural_type
                        && row.shape
                            == StructuralTypeShape::ByteSequence(
                                terminal_psi::ByteSequenceCarrier::BoundedOwned {
                                    capacity: *capacity,
                                },
                            )
                }) {
                    return unsupported("whole byte source has a different capacity");
                }
                let length = self
                    .emission
                    .length(OperationKind::ByteSequenceLength { source })?;
                Ok((ByteSequenceOperand::Whole { source, length }, *capacity))
            }
            ByteValue::Field {
                root,
                carrier_path,
                field_identity,
                capacity,
            } => {
                let (source_place, source_type) = self.field_source(*root)?;
                let (path, field) =
                    crate::emission::structural_scalar_store::lower_structural_field_path(
                        source_type,
                        carrier_path,
                        field_identity,
                        self.emission.structural_types,
                    )?;
                if field.field_type.byte_sequence_carrier()
                    != Some(terminal_psi::ByteSequenceCarrier::BoundedOwned {
                        capacity: *capacity,
                    })
                {
                    return unsupported("byte value field has a different carrier");
                }
                let source = terminal_psi::StructuralArgument {
                    place: source_place,
                    path: path.clone(),
                    access: terminal_psi::StructuralAccess::SharedBorrow,
                };
                let field = field.id;
                // Observe at this operand's evaluation point. Later effects cannot
                // retroactively supply this source's extent or content custody.
                let length =
                    self.emission
                        .length(OperationKind::StructuralByteSequenceFieldLength {
                            source: source.place,
                            path,
                            field,
                        })?;
                Ok((
                    ByteSequenceOperand::Field {
                        source,
                        field,
                        length,
                    },
                    *capacity,
                ))
            }
            ByteValue::Concatenation {
                left,
                right,
                capacity,
            } => {
                let (left, left_capacity) = self.emit(*left, depth + 1)?;
                let (right, right_capacity) = self.emit(*right, depth + 1)?;
                if left_capacity.checked_add(right_capacity) != Some(*capacity) {
                    return unsupported(
                        "byte concatenation capacity differs from operand carriers",
                    );
                }
                let (source, length) = self.owned(left, right, *capacity)?;
                Ok((ByteSequenceOperand::Whole { source, length }, *capacity))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{StructuralTypeShape, existing_literal_view_type, literal_view_type};

    #[test]
    fn shared_literal_carrier_requires_exact_prepared_declaration() {
        let mut types = Vec::new();
        assert!(existing_literal_view_type(&types).is_err());
        let identity = literal_view_type(&mut types).expect("prepare literal carrier");
        let original = types.clone();
        assert_eq!(existing_literal_view_type(&types).unwrap(), identity);
        assert_eq!(literal_view_type(&mut types).unwrap(), identity);
        assert_eq!(types, original);

        types[0].shape =
            StructuralTypeShape::ByteSequence(terminal_psi::ByteSequenceCarrier::BoundedOwned {
                capacity: 8,
            });
        assert!(existing_literal_view_type(&types).is_err());
        assert!(literal_view_type(&mut types).is_err());
    }
}
