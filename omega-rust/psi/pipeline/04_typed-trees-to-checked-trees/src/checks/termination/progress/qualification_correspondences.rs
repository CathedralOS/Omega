//! Qualification correspondences and their replayed types.

use crate::fact_plan::{FactPlace, PlaceRoot, PlaceSegment};
use diagnostics::Diagnostic;
use symbols::SymbolHandle;

pub(crate) fn validate_qualification_correspondences(
    program: &symbol_resolved_trees_to_typed_trees::typed_trees::TypedTrees,
    semantic: &crate::fact_plan::FactPlan,
) -> Vec<Diagnostic> {
    let mut diagnostics = Vec::new();
    let mut retained = Vec::new();
    for (_, correspondence) in semantic.qualification_correspondences.iter() {
        if retained.contains(correspondence) {
            diagnostics.push(Diagnostic::error(
                "qualification correspondence is duplicated",
            ));
            continue;
        }
        retained.push(*correspondence);
        if !semantic.facts.is_valid(correspondence.source_fact)
            || !semantic.facts.is_valid(correspondence.destination_fact)
            || correspondence.source_fact == correspondence.destination_fact
            || correspondence.source_fact.arena_index()
                >= correspondence.destination_fact.arena_index()
        {
            diagnostics.push(Diagnostic::error(
                "qualification correspondence fact identity or construction order drifted",
            ));
            continue;
        }
        let crate::fact_plan::ProgramPoint::Statement {
            machine_symbol,
            state_symbol,
            statement_index,
        } = correspondence.formation
        else {
            diagnostics.push(Diagnostic::error(
                "qualification correspondence formation is not an exact statement point",
            ));
            continue;
        };
        if !machine_symbol.is_valid()
            || !state_symbol.is_valid()
            || program.symbols.get(machine_symbol).kind != symbols::SymbolKind::Machine
            || program.symbols.get(state_symbol).kind != symbols::SymbolKind::State
            || program.symbols.get(state_symbol).parent != machine_symbol
        {
            diagnostics.push(Diagnostic::error(
                "qualification correspondence formation owner identity drifted",
            ));
            continue;
        }
        if !exact_correspondence_place(
            program,
            semantic,
            correspondence.source_place,
            machine_symbol,
            state_symbol,
            statement_index,
        ) || !exact_correspondence_place(
            program,
            semantic,
            correspondence.source_occurrence_place,
            machine_symbol,
            state_symbol,
            statement_index,
        ) || !exact_correspondence_place(
            program,
            semantic,
            correspondence.destination_place,
            machine_symbol,
            state_symbol,
            statement_index,
        ) || correspondence.source_place == correspondence.destination_place
        {
            diagnostics.push(Diagnostic::error(
                "qualification correspondence place is not an exact formation-owned structural symbol place",
            ));
            continue;
        }
        let source = semantic.facts.get(correspondence.source_fact);
        let destination = semantic.facts.get(correspondence.destination_fact);
        if source.place != FactPlace::Place(correspondence.source_place)
            || destination.place != FactPlace::Place(correspondence.destination_place)
        {
            diagnostics.push(Diagnostic::error(
                "qualification correspondence place handle drifted from its fact row",
            ));
            continue;
        }
        if !semantic.places_equal(
            correspondence.source_place,
            correspondence.source_occurrence_place,
        ) {
            diagnostics.push(Diagnostic::error(
                "qualification correspondence source occurrence drifted from its fact place",
            ));
            continue;
        }
        if destination.origin != crate::fact_plan::FactOrigin::StatementTransfer
            || destination.point != correspondence.formation
        {
            diagnostics.push(Diagnostic::error(
                "qualification correspondence destination is not its exact statement transfer",
            ));
            continue;
        }
        if source.evidence != correspondence.evidence
            || destination.evidence != correspondence.evidence
            || correspondence.evidence.origin
                != language_semantics::QualificationEvidenceOrigin::CheckedTransformation
            || !exact_correspondence_evidence_source(program, correspondence.evidence)
        {
            diagnostics.push(Diagnostic::error(
                "qualification correspondence evidence identity drifted",
            ));
            continue;
        }
        if crate::fact_plan::QualificationPayloadIdentity::from_fact_payload(source.payload)
            != Some(correspondence.payload)
            || crate::fact_plan::QualificationPayloadIdentity::from_fact_payload(
                destination.payload,
            ) != Some(correspondence.payload)
            || !exact_correspondence_payload(program, correspondence.payload)
        {
            diagnostics.push(Diagnostic::error(
                "qualification correspondence payload or domain identity drifted",
            ));
        }
    }
    diagnostics
}

fn exact_correspondence_payload(
    program: &symbol_resolved_trees_to_typed_trees::typed_trees::TypedTrees,
    payload: crate::fact_plan::QualificationPayloadIdentity,
) -> bool {
    match payload {
        crate::fact_plan::QualificationPayloadIdentity::DomainMembership {
            domain,
            domain_symbol,
            semantic_domain,
        } => {
            (!semantic_domain.is_valid()
                || program.semantic_domains.name(semantic_domain).is_some())
                && domain_symbol.is_valid()
                && program.symbols.get(domain_symbol).kind == symbols::SymbolKind::Domain
                && program.domain_path_members.span(domain).is_some()
        }
        crate::fact_plan::QualificationPayloadIdentity::CarryPermission { .. }
        | crate::fact_plan::QualificationPayloadIdentity::CarryOrigin => true,
    }
}

fn exact_correspondence_evidence_source(
    program: &symbol_resolved_trees_to_typed_trees::typed_trees::TypedTrees,
    evidence: crate::fact_plan::QualificationEvidence,
) -> bool {
    evidence.source_symbol.is_valid()
        && evidence.requirement_symbol == SymbolHandle::invalid()
        && evidence.receipt_identity == 0
        && matches!(
            program.symbols.get(evidence.source_symbol).kind,
            symbols::SymbolKind::Machine | symbols::SymbolKind::Operator
        )
}

fn exact_correspondence_place(
    program: &symbol_resolved_trees_to_typed_trees::typed_trees::TypedTrees,
    semantic: &crate::fact_plan::FactPlan,
    handle: crate::fact_plan::PlaceHandle,
    machine_symbol: SymbolHandle,
    state_symbol: SymbolHandle,
    formation_statement_index: usize,
) -> bool {
    if !semantic.places.is_valid(handle) {
        return false;
    }
    let place = semantic.places.get(handle);
    let PlaceRoot::Symbol(root) = place.root else {
        return false;
    };
    if !root.is_valid() {
        return false;
    }
    let Some(segments) = semantic.place_segments.span(place.segments) else {
        return false;
    };
    let Some(mut current) = replay_root_type_reference(
        program,
        machine_symbol,
        state_symbol,
        formation_statement_index,
        root,
    ) else {
        return false;
    };
    let mut selected_variant = None;
    for segment in segments {
        match segment {
            PlaceSegment::Field { symbol } => {
                if !symbol.is_valid()
                    || program.symbols.get(*symbol).kind != symbols::SymbolKind::Field
                {
                    return false;
                }
                let Some(data) = replay_data_type(program, current, machine_symbol) else {
                    return false;
                };
                let field = if let Some(variant_symbol) = selected_variant.take() {
                    program.data_members(data).iter().find_map(|member| {
                        let symbol_resolved_trees_to_typed_trees::typed_trees::data::DataMember::Variant(variant) = member else {
                            return None;
                        };
                        (variant.symbol == variant_symbol).then(|| {
                            program
                                .data_payload_fields(variant)
                                .iter()
                                .find(|field| field.symbol == *symbol)
                        })?
                    })
                } else {
                    program.data_members(data).iter().find_map(|member| {
                        let symbol_resolved_trees_to_typed_trees::typed_trees::data::DataMember::Field(field) = member else {
                            return None;
                        };
                        (field.symbol == *symbol).then_some(field)
                    })
                };
                let Some(field) = field else {
                    return false;
                };
                current = field.type_reference;
            }
            PlaceSegment::Case { variant } => {
                if selected_variant.is_some()
                    || !variant.is_valid()
                    || program.symbols.get(*variant).kind != symbols::SymbolKind::Variant
                {
                    return false;
                }
                let Some(data) = replay_data_type(program, current, machine_symbol) else {
                    return false;
                };
                if !program.data_members(data).iter().any(|member| {
                    matches!(member, symbol_resolved_trees_to_typed_trees::typed_trees::data::DataMember::Variant(candidate)
                        if candidate.symbol == *variant)
                }) {
                    return false;
                }
                selected_variant = Some(*variant);
            }
            PlaceSegment::FixedIndex { index } => {
                if selected_variant.is_some() {
                    return false;
                }
                loop {
                    match program.type_reference_table.type_reference(current) {
                        symbol_resolved_trees_to_typed_trees::typed_trees::types::TypeReferenceNode::Reference { referee, .. }
                        | symbol_resolved_trees_to_typed_trees::typed_trees::types::TypeReferenceNode::Constrained {
                            base_type: referee,
                            ..
                        } => current = *referee,
                        symbol_resolved_trees_to_typed_trees::typed_trees::types::TypeReferenceNode::FixedArray {
                            element_type,
                            length: symbol_resolved_trees_to_typed_trees::typed_trees::types::FixedArrayLength::Literal(length),
                        } if *index < *length => {
                            current = *element_type;
                            break;
                        }
                        _ => return false,
                    }
                }
            }
            PlaceSegment::FixedRange { .. } | PlaceSegment::Index { .. } => return false,
        }
    }
    true
}

pub(crate) fn replay_root_type_reference(
    program: &symbol_resolved_trees_to_typed_trees::typed_trees::TypedTrees,
    machine_symbol: SymbolHandle,
    state_symbol: SymbolHandle,
    formation_statement_index: usize,
    root: SymbolHandle,
) -> Option<symbol_resolved_trees_to_typed_trees::typed_trees::types::TypeReferenceHandle> {
    let machine = crate::lookup::machine_by_symbol(program, machine_symbol)?;
    let state = program
        .machine_states(machine)
        .iter()
        .find(|state| state.symbol == state_symbol)?;
    match program.symbols.get(root).kind {
        symbols::SymbolKind::Parameter
            if matches!(
                program.symbols.get(root).parent,
                parent if parent == machine_symbol || parent == state_symbol
            ) =>
        {
            program
                .state_parameters(state)
                .iter()
                .find(|parameter| parameter.symbol == root)
                .map(|parameter| parameter.type_reference)
        }
        symbols::SymbolKind::Local
            if program.symbols.get(root).parent == state_symbol
                && formation_statement_index
                    < program
                        .statement_table
                        .statements(state.statement_nodes)
                        .len() =>
        {
            let mut declarations = program
                .statement_table
                .statements(state.statement_nodes)
                .iter()
                .take(formation_statement_index)
                .filter_map(|statement| {
                    let symbol_resolved_trees_to_typed_trees::typed_trees::statement::StatementNode::LocalData(local) = statement else {
                        return None;
                    };
                    (local.symbol == root).then_some(local.type_reference)
                });
            let declared_type = declarations.next()?;
            declarations.next().is_none().then_some(declared_type)
        }
        _ => None,
    }
}

pub(crate) fn replay_data_type(
    program: &symbol_resolved_trees_to_typed_trees::typed_trees::TypedTrees,
    type_reference: symbol_resolved_trees_to_typed_trees::typed_trees::types::TypeReferenceHandle,
    machine_symbol: SymbolHandle,
) -> Option<&symbol_resolved_trees_to_typed_trees::typed_trees::data::DataDefinition> {
    match program.type_reference_table.type_reference(type_reference) {
        symbol_resolved_trees_to_typed_trees::typed_trees::types::TypeReferenceNode::Reference { referee, .. }
        | symbol_resolved_trees_to_typed_trees::typed_trees::types::TypeReferenceNode::Constrained {
            base_type: referee, ..
        } => replay_data_type(program, *referee, machine_symbol),
        symbol_resolved_trees_to_typed_trees::typed_trees::types::TypeReferenceNode::Named { symbol, name }
            if symbol.is_valid()
                && program.symbols.get(*symbol).kind == symbols::SymbolKind::Data =>
        {
            program
                .data_definitions()
                .iter()
                .find(|definition| definition.symbol == *symbol && definition.name == *name)
        }
        symbol_resolved_trees_to_typed_trees::typed_trees::types::TypeReferenceNode::Named { symbol, name }
            if *symbol == machine_symbol && name.as_str() == "Self" =>
        {
            let machine = crate::lookup::machine_by_symbol(program, machine_symbol)?;
            machine.attached_data_symbol.is_valid().then_some(())?;
            program
                .data_definitions()
                .iter()
                .find(|definition| definition.symbol == machine.attached_data_symbol)
        }
        _ => None,
    }
}
