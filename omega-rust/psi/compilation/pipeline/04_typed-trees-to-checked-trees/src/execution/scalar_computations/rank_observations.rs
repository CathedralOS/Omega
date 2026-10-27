//! Arrival ranks are ordinary checked scalar computations. The source ranking
//! owner selects the view, proves formation and every cyclic edge, and retains
//! its exact state telescope. This producer only binds that view's inputs to
//! captured formals; it does not infer descent from the executable expression.
//! A mutable formal here denotes its arrival value, never a later StorageRead.

use super::{Builder, CheckedScalarComputationHandle, CheckedScalarComputationKind};
use checked_trees::{
    CheckedNaturalRankMeasure, CheckedScalarExpression, CheckedScalarExpressionRole,
    CheckedStateNaturalRank,
};
use numerics::arithmetic::ArithmeticDomain;
use typed_trees::expression::{BinaryOperator, ExpressionHandle};
use typed_trees::types::PrimitiveType;

#[allow(clippy::too_many_arguments)]
pub(super) fn record(
    program: &dyn validation::CheckingInput,
    operators: &checked_trees::CheckedOperatorFacts,
    semantic: &facts::FactPlan,
    flow: &checked_trees::FlowFacts,
    borrow: &checked_trees::BorrowFacts,
    proof: &checked_trees::ProofFacts,
    pure: &checked_trees::CheckedScalarExpressionPlans,
    exact_integer_casts: &[validation::ExactIntegerCastFact],
    admitted: &checked_trees::CheckedRankArrivalPlan,
    plans: &mut checked_trees::CheckedScalarComputationPlans,
    structural_values: &mut checked_trees::CheckedStructuralValuePlans,
) {
    // Fusion may expose the same authored state through several dispatch
    // machines. A state may share its root only when the complete selected
    // descriptor agrees; incompatible rosters leave no executable observation.
    if admitted.observations.is_empty() {
        return;
    }
    let mut roster = Vec::<CheckedStateNaturalRank>::new();
    for machine in program.machines() {
        if let Some((ranks, _)) = crate::execution::terminal_unit::state_graph::natural_rank_plans(
            program, machine, admitted,
        ) {
            roster.extend(ranks);
        }
    }
    roster.sort_by_key(|rank| (rank.state.arena_index(), rank.state.generation()));
    let mut remaining = roster.as_slice();
    while let Some(rank) = remaining.first() {
        let count = remaining
            .iter()
            .take_while(|candidate| candidate.state == rank.state)
            .count();
        let candidates = &remaining[..count];
        remaining = &remaining[count..];
        if candidates.iter().any(|candidate| candidate != rank) {
            continue;
        }
        let Some((owner, state)) = validation::find_state(program, rank.state) else {
            continue;
        };
        let mut inputs = program.state_parameters(state).to_vec();
        // These bindings describe entry snapshots. Body planning still
        // uses the original mutable declarations and their storage homes.
        for parameter in &mut inputs {
            parameter.is_mutable = false;
        }
        let Some(body) = admitted.for_body(owner.symbol, owner.states) else {
            continue;
        };
        let mut rows = admitted
            .states
            .span_or_empty(body.states)
            .iter()
            .filter(|row| row.state == state.symbol);
        let Some(row) = rows.next() else {
            continue;
        };
        if rows.next().is_some() {
            continue;
        }
        let Some(bindings) = validation::RankStateBindings::from_checked(admitted, row) else {
            continue;
        };
        let place = |coordinate: &validation::RankSubjectCoordinate| validation::CanonicalPlace {
            root: facts::PlaceRoot::Symbol(coordinate.parameter),
            segments: coordinate.path.clone(),
        };
        let mut declaration_bindings = bindings
            .coordinates
            .iter()
            .map(|binding| (place(&binding.entry), place(&binding.destination)))
            .collect::<Vec<_>>();
        let subject = typed_trees::ranking::resolve_machine_witness_subjects(program, owner)
            .and_then(|subjects| subjects.first().copied());
        if let CheckedNaturalRankMeasure::DeclaredMeasure { measure, .. } = rank.measure {
            let definition = program
                .measures()
                .iter()
                .find(|definition| definition.symbol == measure);
            let Some((formal, subject)) = definition
                .and_then(|definition| definition.parameter.as_ref())
                .zip(subject)
            else {
                continue;
            };
            let Some(entry) = program.machine_states(owner).first() else {
                continue;
            };
            let Some(subject) = validation::canonical_place_from_expression_in_state(
                program,
                entry.symbol,
                0,
                subject,
            ) else {
                continue;
            };
            let facts::PlaceRoot::Symbol(parameter) = subject.root else {
                continue;
            };
            let template_bindings = bindings.declaration_bindings(
                formal.symbol,
                &validation::RankSubjectCoordinate {
                    parameter,
                    path: subject.segments,
                },
            );
            if template_bindings.is_empty() {
                continue;
            }
            declaration_bindings.extend(
                template_bindings
                    .iter()
                    .map(|binding| (place(&binding.entry), place(&binding.destination))),
            );
        }
        let parameters = inputs
            .iter()
            .filter(|parameter| crate::values::scalar::occupies_scalar_position(program, parameter))
            .cloned()
            .collect::<Vec<_>>();
        let Some(parameter_types) = parameters
            .iter()
            .map(|parameter| program.primitive_type_reference(parameter.type_reference))
            .collect::<Option<Vec<_>>>()
        else {
            continue;
        };
        let statement_nodes_start = plans.nodes.storage_slice().len();
        let mut builder = Builder {
            program,
            operators,
            semantic,
            flow,
            borrow,
            proof,
            pure,
            exact_integer_casts,
            machine: owner.symbol,
            state: state.symbol,
            statement_index: 0,
            parameters: &parameters,
            authored_parameters: &inputs,
            declaration_parameters: &inputs,
            declaration_bindings: &declaration_bindings,
            parameter_types: &parameter_types,
            locals: &[],
            plans,
            structural_values,
            statement_nodes_start,
            scalar_occurrences: Vec::new(),
            integer_occurrences: Vec::new(),
        };
        if let Some(root) = observation(&mut builder, rank, subject, &bindings) {
            let _ = builder.push_scalar_root(CheckedScalarExpressionRole::RankObservation, root);
        }
    }
}

fn input_coordinate(
    builder: &mut Builder<'_, '_>,
    rank: &CheckedStateNaturalRank,
    subject: Option<ExpressionHandle>,
    bindings: &validation::RankStateBindings,
    primitive_type: PrimitiveType,
) -> Option<CheckedScalarComputationHandle> {
    let (owner, _) = validation::find_state(builder.program, builder.state)?;
    let entry = builder.program.machine_states(owner).first()?;
    let source = if let Some(subject) = subject {
        validation::canonical_place_from_expression_in_state(
            builder.program,
            entry.symbol,
            0,
            subject,
        )?
    } else {
        // A joint component retains an exact primitive entry coordinate even
        // when this participant authors no separate ranking expression. Its
        // declaration and admitted identity telescope supply the read; no
        // source expression or enclosing invocation is reconstructed.
        let parameter = builder
            .program
            .state_parameters(entry)
            .get(usize::try_from(rank.parameter_position).ok()?)?;
        if parameter.symbol != rank.parameter
            || builder
                .program
                .primitive_type_reference(parameter.type_reference)
                != Some(primitive_type)
        {
            return None;
        }
        validation::CanonicalPlace {
            root: facts::PlaceRoot::Symbol(parameter.symbol),
            segments: Vec::new(),
        }
    };
    let facts::PlaceRoot::Symbol(parameter) = source.root else {
        return None;
    };
    let coordinate = bindings.destination(&validation::RankSubjectCoordinate {
        parameter,
        path: source.segments,
    })?;
    if coordinate.parameter != rank.parameter {
        return None;
    }
    let source = validation::CanonicalPlace {
        root: facts::PlaceRoot::Symbol(coordinate.parameter),
        segments: coordinate.path,
    };
    // Bare parameter coordinates require no authored expression; projected
    // coordinates retain their original occurrence for structural access.
    builder.scalar_coordinate(subject.unwrap_or_default(), &source, primitive_type)
}

fn observation(
    builder: &mut Builder<'_, '_>,
    rank: &CheckedStateNaturalRank,
    subject: Option<ExpressionHandle>,
    bindings: &validation::RankStateBindings,
) -> Option<CheckedScalarComputationHandle> {
    match rank.measure {
        CheckedNaturalRankMeasure::DeclaredMeasure {
            measure,
            primitive_type,
        } => {
            let definition = builder
                .program
                .measures()
                .iter()
                .find(|definition| definition.symbol == measure)?;
            let formal = definition.parameter.as_ref()?;
            let [body] = builder
                .program
                .expression_table
                .expression_handles(definition.body)
            else {
                return None;
            };
            let subject = subject?;
            if let Some(subject_type) = builder
                .program
                .primitive_type_reference(formal.type_reference)
            {
                let operand = builder.expression(subject, subject_type)?;
                let expression = crate::values::scalar::scalar_lowering::lower_return_expression(
                    builder.program,
                    builder.operators,
                    *body,
                    std::slice::from_ref(formal),
                    std::slice::from_ref(formal),
                    &[subject_type],
                    &[],
                    primitive_type,
                    builder.exact_integer_casts,
                )?;
                let operands = builder.plans.operands.insert_many([operand]);
                Some(builder.insert(
                    primitive_type,
                    CheckedScalarComputationKind::Apply {
                        source_expression: *body,
                        expression,
                        operands,
                    },
                ))
            } else {
                // The shared graph builder composes the declaration's entire
                // body under its explicit subject binding. Projection geometry
                // and arithmetic remain owned by their ordinary operations.
                builder.expression(*body, primitive_type)
            }
        }
        CheckedNaturalRankMeasure::IntegerParameter { primitive_type } => {
            input_coordinate(builder, rank, subject, bindings, primitive_type)
        }
        CheckedNaturalRankMeasure::SequenceLength { subject } => {
            let (owner, state) = validation::find_state(builder.program, builder.state)?;
            let (parameter, position, expression) = validation::ranking_slice_length_observation(
                builder.program,
                owner,
                state,
                bindings,
                subject,
            )?;
            (parameter == rank.parameter && position == rank.parameter_position).then_some(())?;
            Some(builder.insert(
                PrimitiveType::U64,
                CheckedScalarComputationKind::Value(expression),
            ))
        }
        CheckedNaturalRankMeasure::BoundedDistance { primitive_type, .. } => {
            let lower = input_coordinate(builder, rank, subject, bindings, primitive_type)?;
            distance(builder, lower, primitive_type)
        }
    }
}

/// Preserve the source distance's established total normalization, MAX-lower.
/// The source judgment proves the bound remains invariant, so this differs
/// from upper-lower by a constant and orders the same edges even at a crossed
/// loop header. No new lower<=upper precondition or saturation is introduced.
fn distance(
    builder: &mut Builder<'_, '_>,
    lower: CheckedScalarComputationHandle,
    primitive_type: PrimitiveType,
) -> Option<CheckedScalarComputationHandle> {
    let maximum = match primitive_type {
        PrimitiveType::U8 => u64::from(u8::MAX),
        PrimitiveType::U16 => u64::from(u16::MAX),
        PrimitiveType::U32 => u64::from(u32::MAX),
        PrimitiveType::U64 => u64::MAX,
        _ => return None,
    };
    let ceiling = builder.insert(
        primitive_type,
        CheckedScalarComputationKind::Value(facts::u64_integer_literal(maximum, primitive_type)?),
    );
    let template = |position| CheckedScalarExpression::Parameter {
        position,
        primitive_type,
    };
    let (expression, _) = crate::values::scalar::scalar_lowering::construct_integer_binary(
        BinaryOperator::Subtract,
        template(0),
        ArithmeticDomain::Exact,
        template(1),
        ArithmeticDomain::Exact,
        true,
    )?;
    let operands = builder.plans.operands.insert_many([ceiling, lower]);
    Some(builder.insert(
        primitive_type,
        CheckedScalarComputationKind::Apply {
            source_expression: ExpressionHandle::invalid(),
            expression,
            operands,
        },
    ))
}
