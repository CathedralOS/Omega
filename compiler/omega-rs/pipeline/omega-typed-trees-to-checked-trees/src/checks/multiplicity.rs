use omega_checked_trees::{CheckFacts, FlowOwnershipEventSource};
use omega_core::diagnostics::Diagnostic;
use omega_core::semantics::Multiplicity;
use omega_core::symbols::SymbolHandle;
use omega_typed_trees::statement::StatementNode;
use omega_typed_trees::types::{TypeReferenceHandle, TypeReferenceNode};

#[derive(Debug)]
struct LinearPlace {
    symbol: SymbolHandle,
    name: String,
    live: bool,
    /// Parameters are established on entry. A local is established only by an
    /// explicit initializer/assignment; implicit zero-fill creates no debt.
    ever_established: bool,
}

pub(crate) fn check_linear_obligations(
    program: &omega_typed_trees::TypedTrees,
    facts: &CheckFacts,
) -> Result<(), Vec<Diagnostic>> {
    let mut diagnostics = Vec::new();

    for (_, state_flow) in facts.flow.control.states.iter() {
        let Some(state) = crate::find_state(program, state_flow.state_symbol) else {
            continue;
        };
        let statements = program.statement_table.statements(state.statement_nodes);
        let mut places = Vec::<LinearPlace>::new();

        for parameter in program.state_parameters(state) {
            // A by-value `self` parameter is the language's terminal-consumer
            // form. The caller transfers the obligation into the call; an
            // outcome carrying it would instead establish a new linear result.
            if parameter.is_self {
                continue;
            }
            if type_multiplicity(program, parameter.type_reference) == Multiplicity::Linear {
                places.push(LinearPlace {
                    symbol: parameter.symbol,
                    name: parameter.name.as_str().to_owned(),
                    live: true,
                    ever_established: true,
                });
            }
        }

        for statement in statements {
            if let StatementNode::LocalData(local) = statement
                && type_multiplicity(program, local.type_reference) == Multiplicity::Linear
            {
                places.push(LinearPlace {
                    symbol: local.symbol,
                    name: local.name.as_str().to_owned(),
                    live: false,
                    ever_established: false,
                });
            }
        }

        if places.is_empty() {
            continue;
        }

        let moves = facts.flow.ownership.moves.span_or_empty(state_flow.moves);
        for (statement_index, statement) in statements.iter().enumerate() {
            let written_target = written_whole_linear_target(
                program,
                state.symbol,
                statement_index,
                statement,
                &places,
            );

            // Moves out of initializer/assignment sources happen before the
            // destination becomes established. The old move-only summary also
            // contains a production event *at* the destination; exclude that
            // compatibility event here rather than mistaking creation for use.
            for event in moves.iter().filter(|event| {
                event_statement_index(event.source) == Some(statement_index)
                    && facts
                        .flow
                        .ownership
                        .segments
                        .span_or_empty(event.segments)
                        .is_empty()
                    && written_target != Some(event.root)
            }) {
                let omega_facts::PlaceRoot::Symbol(symbol) = event.root else {
                    continue;
                };
                let Some(place) = places.iter_mut().find(|place| place.symbol == symbol) else {
                    continue;
                };
                if !place.live {
                    let reason = if place.ever_established {
                        "was already transferred or consumed"
                    } else {
                        "has not been established (implicit zero-fill creates no linear obligation)"
                    };
                    diagnostics.push(Diagnostic::error(format!(
                        "linear value `{}` {reason}; it cannot be moved here",
                        place.name
                    )));
                } else {
                    place.live = false;
                }
            }

            if let Some(omega_facts::PlaceRoot::Symbol(symbol)) = written_target {
                let place = places
                    .iter_mut()
                    .find(|place| place.symbol == symbol)
                    .expect("written linear target came from the tracked place set");
                if place.live {
                    diagnostics.push(Diagnostic::error(format!(
                        "assignment would overwrite live linear value `{}`; consume or transfer the existing obligation first",
                        place.name
                    )));
                }
                place.live = true;
                place.ever_established = true;
            }
        }

        for place in places.iter().filter(|place| place.live) {
            diagnostics.push(Diagnostic::error(format!(
                "linear value `{}` reaches scope exit without being consumed or transferred",
                place.name
            )));
        }
    }

    if diagnostics.is_empty() {
        Ok(())
    } else {
        Err(diagnostics)
    }
}

fn event_statement_index(source: FlowOwnershipEventSource) -> Option<usize> {
    match source {
        FlowOwnershipEventSource::Statement { statement_index }
        | FlowOwnershipEventSource::Call {
            statement_index, ..
        } => Some(statement_index),
        FlowOwnershipEventSource::StateExit => None,
    }
}

fn written_whole_linear_target(
    program: &omega_typed_trees::TypedTrees,
    state_symbol: SymbolHandle,
    statement_index: usize,
    statement: &StatementNode,
    places: &[LinearPlace],
) -> Option<omega_facts::PlaceRoot> {
    match statement {
        StatementNode::LocalData(local) => {
            (local.initial_value.is_valid()
                && places.iter().any(|place| place.symbol == local.symbol))
            .then_some(omega_facts::PlaceRoot::Symbol(local.symbol))
        }
        StatementNode::Assignment(assignment) => {
            let place = crate::flow::canonical_place_from_expression_in_state(
                program,
                state_symbol,
                statement_index,
                assignment.target,
            )?;
            (place.segments.is_empty()
                && matches!(place.root, omega_facts::PlaceRoot::Symbol(symbol) if places.iter().any(|tracked| tracked.symbol == symbol)))
            .then_some(place.root)
        }
        _ => None,
    }
}

fn type_multiplicity(
    program: &omega_typed_trees::TypedTrees,
    type_reference: TypeReferenceHandle,
) -> Multiplicity {
    if !type_reference.is_valid() {
        return Multiplicity::Affine;
    }
    match program.type_reference_table.type_reference(type_reference) {
        TypeReferenceNode::Reference { .. } | TypeReferenceNode::Unit => {
            Multiplicity::Unrestricted
        }
        TypeReferenceNode::Constrained { base_type, .. } => {
            type_multiplicity(program, *base_type)
        }
        TypeReferenceNode::FixedArray { element_type, .. } => {
            type_multiplicity(program, *element_type)
        }
        TypeReferenceNode::Named { name, .. } => {
            if omega_typed_trees::types::PrimitiveType::from_name(name.as_str()).is_some() {
                return Multiplicity::Unrestricted;
            }
            program
                .data_definitions()
                .iter()
                .find(|definition| definition.name.as_str() == name.as_str())
                .map(|definition| definition.properties.multiplicity)
                .unwrap_or(Multiplicity::Affine)
        }
        TypeReferenceNode::Generic { base_name, .. } => program
            .data_definitions()
            .iter()
            .find(|definition| definition.name.as_str() == base_name.as_str())
            .map(|definition| definition.properties.multiplicity)
            .unwrap_or(Multiplicity::Affine),
        TypeReferenceNode::DynamicTrait { .. } | TypeReferenceNode::Slice { .. } => {
            Multiplicity::Affine
        }
    }
}
