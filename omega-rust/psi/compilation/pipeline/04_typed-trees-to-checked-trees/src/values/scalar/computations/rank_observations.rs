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
    flow: &checked_trees::FlowFacts,
    borrow: &checked_trees::BorrowFacts,
    pure: &checked_trees::CheckedScalarExpressionPlans,
    exact_integer_casts: &[validation::ExactIntegerCastFact],
    call_frames: Option<&validation::CallFrameResolver<'_>>,
    plans: &mut checked_trees::CheckedScalarComputationPlans,
    structural_values: &mut checked_trees::CheckedStructuralValuePlans,
) {
    // Fusion may expose the same authored state through several dispatch
    // machines. A state may share its root only when the complete selected
    // descriptor agrees; incompatible rosters leave no executable observation.
    let mut roster = Vec::<CheckedStateNaturalRank>::new();
    for machine in program.machines() {
        if let Some((ranks, _)) = crate::execution::terminal_unit::state_graph::natural_rank_plans(
            program,
            machine,
            call_frames,
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
        let Some((owner, state)) = validation::find_state_with_machine(program, rank.state) else {
            continue;
        };
        let mut inputs = program.state_parameters(state).to_vec();
        // These bindings describe entry snapshots. Body planning still
        // uses the original mutable declarations and their storage homes.
        for parameter in &mut inputs {
            parameter.is_mutable = false;
        }
        let subject = match rank.measure {
            CheckedNaturalRankMeasure::DeclaredMeasure { .. } => {
                let Some(subjects) =
                    typed_trees::ranking::resolve_machine_witness_subjects(program, owner)
                else {
                    continue;
                };
                let [subject] = subjects.as_slice() else {
                    continue;
                };
                let Some(entry) = program.machine_states(owner).first() else {
                    continue;
                };
                let Some(place) = validation::canonical_place_from_expression_in_state(
                    program,
                    entry.symbol,
                    0,
                    *subject,
                ) else {
                    continue;
                };
                let facts::PlaceRoot::Symbol(symbol) = place.root else {
                    continue;
                };
                let Some(input) = inputs.get_mut(rank.parameter_position as usize) else {
                    continue;
                };
                // Exact source role supplied by the rank owner, retaining
                // this actual formal's complete type and namespace slot.
                input.symbol = symbol;
                Some(*subject)
            }
            _ => None,
        };
        let declaration_binding = match rank.measure {
            CheckedNaturalRankMeasure::DeclaredMeasure { measure, .. } => {
                let definition = program
                    .measures()
                    .iter()
                    .find(|definition| definition.symbol == measure);
                definition
                    .and_then(|definition| definition.parameter.as_ref())
                    .filter(|formal| {
                        program
                            .primitive_type_reference(formal.type_reference)
                            .is_none()
                    })
                    .zip(subject)
                    .map(|(formal, subject)| (formal.symbol, subject))
            }
            _ => None,
        };
        let declaration_bindings = declaration_binding.as_slice();
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
        let mut builder = Builder {
            program,
            operators,
            flow,
            borrow,
            pure,
            exact_integer_casts,
            machine: owner.symbol,
            state: state.symbol,
            statement_index: 0,
            parameters: &parameters,
            authored_parameters: &inputs,
            declaration_parameters: &inputs,
            declaration_bindings,
            parameter_types: &parameter_types,
            locals: &[],
            plans,
            structural_values,
            integer_occurrences: Vec::new(),
        };
        if let Some(root) = observation(&mut builder, rank, subject) {
            let _ = builder.push_scalar_root(CheckedScalarExpressionRole::RankObservation, root);
        }
    }
}

fn parameter(
    builder: &mut Builder<'_, '_>,
    authored_position: u32,
    primitive_type: PrimitiveType,
) -> Option<CheckedScalarComputationHandle> {
    let input = builder
        .authored_parameters
        .get(authored_position as usize)?;
    let position = builder
        .parameters
        .iter()
        .position(|parameter| parameter.symbol == input.symbol)?;
    Some(builder.insert(
        primitive_type,
        CheckedScalarComputationKind::Value(CheckedScalarExpression::Parameter {
            position,
            primitive_type,
        }),
    ))
}

fn observation(
    builder: &mut Builder<'_, '_>,
    rank: &CheckedStateNaturalRank,
    subject: Option<ExpressionHandle>,
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
            parameter(builder, rank.parameter_position, primitive_type)
        }
        CheckedNaturalRankMeasure::SequenceLength { subject } => {
            let (owner, state) =
                validation::find_state_with_machine(builder.program, builder.state)?;
            let root = builder.program.machine_states(owner).first()?;
            let place = validation::canonical_place_from_expression_in_state(
                builder.program,
                root.symbol,
                0,
                subject,
            )?;
            let facts::PlaceRoot::Symbol(entry) = place.root else {
                return None;
            };
            let required = match builder
                .program
                .ranking_expression_custody_for(owner.symbol)
                .and_then(|custody| custody.rank_range)
            {
                Some(range) => validation::ranking_range_required_symbols(
                    builder.program,
                    owner,
                    range,
                    validation::RankingRangeMeasure::SliceLength(subject),
                    validation::RankingRangePremises::RankInvariant,
                )?,
                None => vec![entry],
            };
            let mappings = validation::discover_state_entry_mappings(
                builder.program,
                owner,
                entry,
                &required,
            )?;
            let state_position = builder
                .program
                .machine_states(owner)
                .iter()
                .position(|candidate| candidate.symbol == state.symbol)?;
            let (parameter, position, expression) = validation::ranking_slice_length_observation(
                builder.program,
                owner,
                state,
                mappings.get(state_position)?,
                subject,
            )?;
            (parameter == rank.parameter && position == rank.parameter_position).then_some(())?;
            Some(builder.insert(
                PrimitiveType::U64,
                CheckedScalarComputationKind::Value(expression),
            ))
        }
        CheckedNaturalRankMeasure::UnsignedDistance { primitive_type, .. }
        | CheckedNaturalRankMeasure::ConstantDistance { primitive_type, .. } => {
            let lower = parameter(builder, rank.parameter_position, primitive_type)?;
            distance(builder, lower, primitive_type)
        }
        CheckedNaturalRankMeasure::CollectionLengthDistance { .. } => {
            let lower = parameter(builder, rank.parameter_position, PrimitiveType::U64)?;
            distance(builder, lower, PrimitiveType::U64)
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
