/*
    Same-invocation state arrivals transport live completed facts. Ordinary
    machine applications contribute unknown inputs to callable non-entry states;
    their incidental actuals never become assumptions of a generic body.
*/

use crate::published::facts::boolean_predicates::BoundPredicate;
use crate::published::facts::boolean_predicates::EntryPaths;
use crate::published::facts::boolean_predicates::seed_entry_paths;
use crate::values::integer_bounds_at_place;
use validation::CallSite;

use crate::flow::{FlowBuildContext, active_context_handles, canonical_place_from_semantic_place};

use arena::HandleSpan;
use checked_trees::expression::{ExpressionHandle, ExpressionNode};
use checked_trees::{BorrowCallFact, FlowConstraintKind, FlowOperandFact, FlowSemanticContextRef};
use facts::ScalarValue;
use facts::{FactPayload, FactPlace, FactPlan, ProgramPoint};
use symbols::SymbolHandle;
use typed_trees::statement::{TransitionExit, TransitionTargetNode};
use validation::CanonicalPlace;

mod arguments {
    /*
        Save one completed jump argument while its operand facts are still live.
    */
    use validation::CanonicalPlace;

    use super::{ScalarValue, TransitionTargetNode};

    use crate::flow::FlowBuildContext;
    use crate::flow::literal;
    use arena::HandleSpan;
    use checked_trees::CheckedScalarExpressionRole;
    use checked_trees::FlowSemanticContextRef;
    use checked_trees::expression::ExpressionHandle;
    use checked_trees::statement::StatementNode;
    use facts::FactPlan;

    #[allow(clippy::too_many_arguments)]
    pub(in crate::flow) fn capture_argument(
        program: &dyn ::validation::CheckingInput,
        semantic: &FactPlan,
        context: &mut FlowBuildContext,
        machine: &typed_trees::machine::Machine,
        state: &typed_trees::state::State,
        statement_index: usize,
        target: typed_trees::statement::TransitionTargetHandle,
        ordinal: usize,
        argument: ExpressionHandle,
        contexts: HandleSpan<FlowSemanticContextRef>,
        copied_values: HandleSpan<checked_trees::FlowOperandFact>,
    ) -> ScalarValue {
        if !program.expression_table.expression_is_valid(argument) {
            return ScalarValue::Unknown;
        }
        let TransitionTargetNode::Named {
            path, arguments, ..
        } = program.statement_table.transition_target(target)
        else {
            return ScalarValue::Unknown;
        };
        if program
            .statement_table
            .expression_handles(*arguments)
            .get(ordinal)
            != Some(&argument)
        {
            return ScalarValue::Unknown;
        }
        let Some(destination) = context
            .state_index_in_machine(program, machine.symbol, path.symbol)
            .and_then(|index| program.machine_states(machine).get(index))
        else {
            return ScalarValue::Unknown;
        };
        let Some((position, parameter)) = program
            .state_parameters(destination)
            .iter()
            .enumerate()
            .nth(ordinal)
        else {
            return ScalarValue::Unknown;
        };
        let (Ok(statement_ordinal), Ok(argument_ordinal)) =
            (u32::try_from(statement_index), u32::try_from(position))
        else {
            return ScalarValue::Unknown;
        };
        let Some(StatementNode::Transition(transition)) =
            program.statement_table.statement_at(state, statement_index)
        else {
            return ScalarValue::Unknown;
        };
        let role = if target == transition.target {
            CheckedScalarExpressionRole::TransitionArgument { argument_ordinal }
        } else if target == transition.continuation {
            CheckedScalarExpressionRole::TransitionContinuationArgument { argument_ordinal }
        } else {
            return ScalarValue::Unknown;
        };
        match context.selected_scalar_at(state.symbol, statement_ordinal, |binding| {
            binding.role == role
        }) {
            None => {}
            Some(None) => return ScalarValue::Unknown,
            Some(Some((binding, expression))) => {
                if binding.expression != argument || binding.destination != parameter.symbol {
                    return ScalarValue::Unknown;
                }
                return crate::values::evaluate_checked_scalar(
                    expression,
                    &mut crate::values::PlaceScalarValues {
                        program,
                        parameters: program.state_parameters(state),
                        symbols: context
                            .scalar_expressions
                            .binding_symbols
                            .span_or_empty(binding.symbols),
                        value_at_place: |place: &CanonicalPlace, observation| {
                            let place = crate::flow::observations::completed_copy_read_place(
                                program,
                                &context.control,
                                copied_values,
                                state.symbol,
                                statement_index,
                                observation,
                                place,
                            )?;
                            context.scalar_value_at_place(program, semantic, contexts, &place)
                        },
                    },
                )
                .unwrap_or_default();
            }
        }
        if let Some(value) = literal(program, argument) {
            return value;
        }
        let Some(place) =
            context.canonical_place_at(program, state.symbol, statement_index, argument)
        else {
            return ScalarValue::Unknown;
        };
        let Some(place) = crate::flow::observations::completed_copy_read_place(
            program,
            &context.control,
            copied_values,
            state.symbol,
            statement_index,
            argument,
            &place,
        ) else {
            return ScalarValue::Unknown;
        };
        if !place
            .segments
            .iter()
            .all(crate::flow::fixed_selection_segment)
        {
            return ScalarValue::Unknown;
        }
        context
            .scalar_value_at_place(program, semantic, contexts, &place)
            .unwrap_or_default()
    }
}
mod callable_inputs {
    /*
        Callable selection supplies an independent entry under its declared inputs.
        Each complete applied signature is one alternative arrival. Ordinary flow
        joins it with every other arrival; requirements from different selections
        are never conjoined. Ensures describe completed calls and supply no entry fact.
    */
    use super::{ArgumentInputs, StateValues, fields, qualifications};
    use crate::flow::FlowBuildContext;
    use crate::published::facts::boolean_predicates::{self, BoundPredicate};
    use checked_trees::CheckedScalarExpression;
    use facts::{BooleanPredicateOperand, PlaceRoot};
    use typed_trees::domain::ProofFact;
    use typed_trees::signature::{SignatureContractKind, StateParameter, StateSignature};
    use validation::{CanonicalPlace, StaticMachineContractObligation};

    pub(in crate::flow) fn seed_contract_inputs(
        program: &dyn validation::CheckingInput,
        build: &mut FlowBuildContext<'_>,
        machines: &[usize],
    ) {
        for &position in machines {
            let machine = &program.machines()[position];
            let rows = validation::with_private_invocation_inputs(program, machine, |contracts| {
                contracts
                    .iter()
                    .filter_map(|input| {
                        let destination =
                            validation::call_target_state(program, input.selected_entry())?;
                        let state = input_state(program, build.operators, destination, input);
                        Some((
                            destination,
                            fields::BoundsSource::callable_input(input),
                            state,
                        ))
                    })
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
            for (destination, source, state) in rows {
                super::join(program, build, machine, destination, source, state);
            }
        }
    }

    fn input_state(
        program: &dyn validation::CheckingInput,
        operators: &checked_trees::CheckedOperatorFacts,
        destination: &typed_trees::state::State,
        input: &StaticMachineContractObligation,
    ) -> StateValues {
        let parameters = program.state_parameters(destination);
        let mut state = StateValues {
            state: destination.symbol,
            values: parameters
                .iter()
                .map(|parameter| (parameter.symbol, facts::ScalarValue::Unknown))
                .collect(),
            bounds: Vec::new(),
            fields: Vec::new(),
            field_sources: Vec::new(),
            qualifications: Vec::new(),
            predicates: Vec::new(),
            entry_paths: boolean_predicates::EntryPaths::unknown(),
            argument_edges: ArgumentInputs::Edges(Vec::new()),
            cases: Vec::new(),
        };
        let Ok(required) = input.required_signature(program) else {
            return state;
        };
        let formals = program.state_signature_parameters(required);
        if formals.len() != parameters.len() {
            return state;
        }
        for (formal, parameter) in formals.iter().zip(parameters) {
            for row in
                validation::field_domains::declared_type_domain_rows(program, formal.type_reference)
            {
                if fixed_path(&row.path) {
                    state
                        .qualifications
                        .push(qualifications::QualifiedInput::declared_contract(
                            CanonicalPlace {
                                root: PlaceRoot::Symbol(parameter.symbol),
                                segments: row.path,
                            },
                            row.domain_symbol,
                            row.semantic_domain,
                            destination.symbol,
                        ));
                }
            }
        }
        for contract in program.state_signature_contracts(required) {
            if contract.kind != SignatureContractKind::Requires {
                continue;
            }
            for fact in program.proof_facts.span_or_empty(contract.facts) {
                match fact {
                    ProofFact::Expression(expression) => {
                        /*
                            A stated case is this input's case edge: the meet
                            keeps a case only when every arrival states it.
                        */
                        if let Some(binary) = program.expression_table.binary_node(*expression)
                            && let Some(variant) =
                                validation::applied_declaration_case_membership_variant(
                                    program,
                                    required.symbol,
                                    formals,
                                    *expression,
                                    binary,
                                )
                            && let Some(place) =
                                validation::canonical_place_from_expression(program, binary.left)
                                    .and_then(|place| rebind_place(formals, parameters, &place))
                        {
                            state.cases.push((place, variant));
                        }
                        let Some(expression) = crate::values::lower_parameter_boolean_expression(
                            program,
                            operators,
                            required.symbol,
                            formals,
                            *expression,
                            &operators.exact_integer_casts,
                        ) else {
                            continue;
                        };
                        let Some(predicate) = boolean_predicates::bind_parameters(
                            program,
                            required.symbol,
                            required.symbol,
                            formals,
                            None,
                            expression,
                        ) else {
                            continue;
                        };
                        if let Some(predicate) =
                            rebind_predicate(program, required, destination, predicate)
                        {
                            state.predicates.push(predicate);
                        }
                    }
                    ProofFact::Membership(membership) => {
                        if let Some(place) =
                            validation::canonical_place_from_expression(program, membership.value)
                                .and_then(|place| rebind_place(formals, parameters, &place))
                        {
                            state.qualifications.push(
                                qualifications::QualifiedInput::declared_contract(
                                    place,
                                    membership.domain_symbol,
                                    membership.semantic_domain,
                                    destination.symbol,
                                ),
                            );
                        }
                    }
                    ProofFact::Proposition(_) => {}
                }
            }
        }
        for parameter in parameters {
            let place = CanonicalPlace {
                root: PlaceRoot::Symbol(parameter.symbol),
                segments: Vec::new(),
            };
            if let Some(value) = state
                .predicates
                .iter()
                .find_map(|predicate| predicate.constant_at_place(&place, |_| None))
                && let Some((_, recorded)) = state
                    .values
                    .iter_mut()
                    .find(|(symbol, _)| *symbol == parameter.symbol)
            {
                *recorded = value;
            }
            let bounds = validation::field_domains::readable_type_reference(
                program,
                parameter.type_reference,
            )
            .and_then(|reference| program.primitive_type_reference(reference))
            .and_then(|primitive| {
                let mut common: Option<facts::IntegerRange> = None;
                for predicate in &state.predicates {
                    let incoming =
                        predicate.integer_interval_at_place(&place, primitive, |_| None)?;
                    common = Some(match common {
                        None => incoming,
                        Some(previous) => facts::IntegerRange {
                            minimum: previous.minimum.max(incoming.minimum),
                            maximum: previous.maximum.min(incoming.maximum),
                        },
                    });
                }
                common.filter(|range| range.minimum <= range.maximum)
            });
            state
                .bounds
                .push(super::ParameterBounds::new(parameter.symbol, bounds));
        }
        /*
            A direct caller delivers a field's value as a field row, so a stated
            field bound must arrive in that same row for the meet to compare them.
        */
        let mut field_places: Vec<CanonicalPlace> = Vec::new();
        for binding in state.predicates.iter().flat_map(|row| &row.bindings) {
            if !binding.place.segments.is_empty() && !field_places.contains(&binding.place) {
                field_places.push(binding.place.clone());
            }
        }
        for place in field_places {
            let Some(primitive) =
                validation::canonical_place_type_reference(program, destination.symbol, 0, &place)
                    .and_then(|reference| {
                        validation::field_domains::readable_type_reference(program, reference)
                    })
                    .and_then(|reference| program.primitive_type_reference(reference))
            else {
                continue;
            };
            let bounds = state
                .predicates
                .iter()
                .filter_map(|row| row.integer_interval_at_place(&place, primitive, |_| None))
                .reduce(|previous, incoming| facts::IntegerRange {
                    minimum: previous.minimum.max(incoming.minimum),
                    maximum: previous.maximum.min(incoming.maximum),
                })
                .filter(|range| range.minimum <= range.maximum);
            if let Some(bounds) = bounds {
                state
                    .fields
                    .push(fields::FieldValue::declared_bounds(place, bounds));
            }
        }
        state
    }

    fn fixed_path(path: &[facts::PlaceSegment]) -> bool {
        path.iter()
            .all(|segment| !matches!(segment, facts::PlaceSegment::Index { .. }))
    }

    fn rebind_place(
        formals: &[StateParameter],
        parameters: &[StateParameter],
        place: &CanonicalPlace,
    ) -> Option<CanonicalPlace> {
        if !fixed_path(&place.segments) {
            return None;
        }
        let symbol = place.root.as_symbol()?;
        let ordinal = formals.iter().position(|formal| formal.symbol == symbol)?;
        Some(CanonicalPlace {
            root: PlaceRoot::Symbol(parameters.get(ordinal)?.symbol),
            segments: place.segments.clone(),
        })
    }

    fn rebind_predicate(
        program: &dyn validation::CheckingInput,
        required: &StateSignature,
        destination: &typed_trees::state::State,
        mut predicate: BoundPredicate,
    ) -> Option<BoundPredicate> {
        let formals = program.state_signature_parameters(required);
        let parameters = program.state_parameters(destination);
        let mut bindings = Vec::new();
        boolean_predicates::map_observations(&mut predicate.expression, &mut |operand| {
            let supplied = predicate
                .bindings
                .iter()
                .find(|binding| binding.operand == *operand)?;
            let place = rebind_place(formals, parameters, &supplied.place)?;
            let reference =
                validation::canonical_place_type_reference(program, destination.symbol, 0, &place)?;
            let reference = validation::field_domains::readable_type_reference(program, reference)?;
            if let Some(primitive) = operand.primitive_type() {
                if program.primitive_type_reference(reference) != Some(primitive) {
                    return None;
                }
            } else if program.normalized_type_identity(supplied.type_reference)
                != program.normalized_type_identity(reference)
            {
                return None;
            }
            if let BooleanPredicateOperand::Scalar(CheckedScalarExpression::StorageRead {
                symbol,
                ..
            }) = operand
            {
                let ordinal = formals.iter().position(|formal| formal.symbol == *symbol)?;
                *symbol = parameters.get(ordinal)?.symbol;
            }
            let mut binding = supplied.clone();
            binding.operand = operand.clone();
            binding.place = place;
            binding.type_reference = reference;
            if !bindings.contains(&binding) {
                bindings.push(binding);
            }
            Some(())
        })?;
        predicate.machine = program.machine_holding_state(destination.symbol)?.symbol;
        predicate.entry_state = destination.symbol;
        predicate.bindings = bindings;
        Some(predicate)
    }
}
pub(super) use callable_inputs::seed_contract_inputs;
pub(super) mod lexical_regions {
    /*
        Captured entry values of an extracted region, an authored nested machine,
        or a closed private machine in the same source activation. An open or
        public body stays checked only under its declared premises. A closed
        private entry receives the caller roster's completed scalars and cases.
    */
    use super::{ArgumentInputs, StateValues, fields};
    use arena::HandleSpan;
    use checked_trees::{BorrowCallFact, FlowOperandFact, FlowSemanticContextRef};
    use facts::{FactPlan, ScalarValue};

    #[allow(clippy::too_many_arguments)]
    pub(in crate::flow) fn record<'plans>(
        program: &'plans dyn validation::CheckingInput,
        semantic: &FactPlan,
        build: &mut crate::flow::FlowBuildContext<'plans>,
        machine: &typed_trees::machine::Machine,
        state: &typed_trees::state::State,
        call: &BorrowCallFact,
        captured: &[FlowOperandFact],
        invocation: HandleSpan<FlowSemanticContextRef>,
    ) {
        if !super::reachable(build, program, machine, state.symbol) {
            return;
        }
        let Some((owner_index, state_index)) = build.state_location(program, call.target_symbol)
        else {
            return;
        };
        let owner = &program.machines()[owner_index];
        let destination = &program.machine_states(owner)[state_index];
        let closed_private = super::caller_roster_supplies_entry(program, owner);
        // Nesting does not let callers strengthen a written entry contract.
        // Compiler-extracted assembly regions instead continue a checked source
        // location, identified by their retained lexical origin.
        if owner.lexical_region_origin.is_none() && !closed_private {
            return;
        }
        if super::entry_state(program, owner).is_none_or(|entry| entry.symbol != destination.symbol)
        {
            return;
        }
        let site = crate::flow::calls::memoized_find_call_site(
            program,
            build,
            machine.symbol,
            state.symbol,
            call.statement_index,
            call.call_ordinal,
        );
        if let Some(site) = site
            && validation::with_private_invocation_inputs(program, owner, |inputs| {
                inputs
                    .iter()
                    .any(|input| input.bounds_static_call(program, site))
            }) == Some(true)
        {
            return;
        }
        let parameters = program.state_parameters(destination);
        let authenticated = closed_private
            && site.is_some_and(|site| {
                // A machine's own body names itself, `write(...)` inside
                // `write`, by its machine symbol, while its declaring machine's
                // call resolves to the entry state. Both invoke that entry, and
                // the self edge's facts derive from the entry being joined.
                (site.target_symbol(program) == destination.symbol
                    || (owner.symbol == machine.symbol
                        && site.target_symbol(program) == owner.symbol))
                    && match owner.lexical_region_origin {
                        Some(origin) => {
                            origin.machine == machine.symbol
                                && origin.state == state.symbol
                                && validation::call_site_argument_expressions(program, &site)
                                    .iter()
                                    .copied()
                                    .eq(program
                                        .statement_table
                                        .expression_handles(origin.arguments)
                                        .iter()
                                        .copied())
                        }
                        None => {
                            let roster = validation::call_site_argument_expressions(program, &site)
                                .iter()
                                .copied()
                                .eq(captured.iter().map(|value| value.expression));
                            roster
                                && (super::lexical_scope_covers_call(
                                    program,
                                    machine.symbol,
                                    owner.symbol,
                                ) || !super::lexically_nested_machine(program, owner.symbol))
                        }
                    }
            })
            && !call.has_receiver
            && captured.len() == parameters.len()
            && captured.iter().all(|value| {
                value.state_symbol == state.symbol && value.statement_index == call.statement_index
            });
        let mut values = Vec::new();
        let mut bounds = Vec::new();
        let mut fields = Vec::new();
        let mut qualifications = Vec::new();
        for (ordinal, parameter) in parameters.iter().enumerate() {
            let capture = captured.get(ordinal);
            let completed = authenticated
                .then_some(capture)
                .flatten()
                .and_then(|capture| {
                    super::completed_argument_facts(program, build, capture, invocation)
                });
            let mut assignments = completed.as_ref().map(|completed| {
                crate::values::RootAssignmentObservations::new(
                    program,
                    semantic,
                    &completed.contexts,
                )
            });
            let observed = capture
                .zip(completed.as_ref())
                .and_then(|(capture, completed)| {
                    let assignments = assignments.as_mut()?;
                    let mut values = completed.places.iter().map(|place| {
                        assignments
                            .scalar_value(&place.projected)
                            .or_else(|| super::literal(program, capture.expression))
                    });
                    let first = values.next()??;
                    values
                        .all(|value| value.as_ref() == Some(&first))
                        .then_some(first)
                })
                .unwrap_or(ScalarValue::Unknown);
            let interval = match &observed {
                ScalarValue::Integer(value) => Some(facts::IntegerRange {
                    minimum: value.clone(),
                    maximum: value.clone(),
                }),
                _ => completed.as_ref().and_then(|completed| {
                    let assignments = assignments.as_mut()?;
                    let mut joined: Option<facts::IntegerRange> = None;
                    for place in &completed.places {
                        let incoming = assignments.integer_bounds(&place.projected)?;
                        joined = Some(match joined {
                            None => incoming,
                            Some(previous) => facts::IntegerRange {
                                minimum: previous.minimum.min(incoming.minimum),
                                maximum: previous.maximum.max(incoming.maximum),
                            },
                        });
                    }
                    joined
                }),
            };
            values.push((parameter.symbol, observed));
            bounds.push(super::ParameterBounds::new(parameter.symbol, interval));
            if let Some((capture, completed)) = capture.zip(completed.as_ref()) {
                fields.extend(super::fields::capture_completed_argument(
                    program,
                    semantic,
                    build.operators,
                    state,
                    call.statement_index,
                    completed,
                    parameter.symbol,
                ));
                qualifications.extend(super::qualifications::capture_completed_argument(
                    program,
                    semantic,
                    build,
                    capture,
                    completed,
                    destination.symbol,
                    parameter.symbol,
                ));
            }
        }
        let arguments: Vec<_> = captured.iter().map(|value| value.expression).collect();
        let cases = if authenticated {
            super::capture_argument_cases(
                program,
                semantic,
                build,
                state,
                destination,
                call.statement_index,
                &arguments,
                captured,
                invocation,
            )
        } else {
            Vec::new()
        };
        let source =
            fields::BoundsSource::invocation(state.symbol, call.statement_index, call.call_ordinal);
        super::join(
            program,
            build,
            owner,
            destination,
            source,
            StateValues {
                state: destination.symbol,
                field_sources: Vec::new(),
                values,
                bounds,
                fields,
                qualifications,
                predicates: Vec::new(),
                entry_paths: crate::published::facts::boolean_predicates::EntryPaths::unknown(),
                argument_edges: ArgumentInputs::Edges(Vec::new()),
                cases,
            },
        );
    }
}
mod parameter_bounds {
    /*
        Conservative, finite joins for scalar state parameters.

        Union alone enumerates a counter's values one by one: Squalr's byte-comparison
        loop kept the flow builder running for hundreds of thousands of sweeps. Preserve small
        joins exactly, then widen growing endpoints to authored thresholds or the
        integer carrier. Each result still contains every previously delivered range;
        this cannot prove a stronger proposition than the original union. Unknown
        input is absorbing, and widening never changes independent qualifications.

        These rows belong to the unpublished flow fixed point. Growth counts are
        scheduling scratch, not proof evidence; only a changed interval dirties a
        destination. Final materialization and the builder's fail-closed fallback
        remain responsible for publishing settled facts.
    */
    use facts::IntegerRange;
    use numerics::bignum::BigInt;
    use symbols::SymbolHandle;
    use typed_trees::state::State;

    const WIDENING_DELAY: u8 = 4;

    #[derive(Clone, Debug, PartialEq, Eq)]
    pub(super) struct ParameterBounds {
        pub(super) symbol: SymbolHandle,
        pub(super) bounds: Option<IntegerRange>,
        growth: u8,
    }

    impl ParameterBounds {
        pub(super) fn new(symbol: SymbolHandle, bounds: Option<IntegerRange>) -> Self {
            Self {
                symbol,
                bounds,
                growth: 0,
            }
        }

        pub(super) fn height(&self, threshold_count: usize) -> usize {
            if self.bounds.is_none() {
                return 0;
            }
            threshold_count
                .saturating_add(1)
                .saturating_mul(2)
                .saturating_add(usize::from(WIDENING_DELAY))
                .saturating_add(1)
        }

        pub(super) fn join(
            &mut self,
            program: &dyn ::validation::CheckingInput,
            destination: &State,
            incoming: Option<&IntegerRange>,
            thresholds: &[BigInt],
        ) -> bool {
            let Some(previous) = self.bounds.as_ref() else {
                return false;
            };
            let Some(incoming) = incoming else {
                self.bounds = None;
                return true;
            };
            let mut joined = IntegerRange {
                minimum: previous.minimum.clone().min(incoming.minimum.clone()),
                maximum: previous.maximum.clone().max(incoming.maximum.clone()),
            };
            if &joined == previous {
                return false;
            }
            self.growth = self.growth.saturating_add(1);
            if self.growth >= WIDENING_DELAY {
                let carrier = program
                    .state_parameter_by_symbol(destination, self.symbol)
                    .and_then(|parameter| {
                        program.primitive_type_reference(parameter.type_reference)
                    })
                    .and_then(validation::carrier_integer_range);
                let Some(carrier) = carrier.filter(|carrier| {
                    carrier.minimum <= joined.minimum && joined.maximum <= carrier.maximum
                }) else {
                    self.bounds = None;
                    return true;
                };
                if joined.minimum < previous.minimum {
                    joined.minimum = thresholds
                        .iter()
                        .rev()
                        .find(|threshold| {
                            carrier.minimum <= **threshold && **threshold <= joined.minimum
                        })
                        .cloned()
                        .unwrap_or(carrier.minimum);
                }
                if joined.maximum > previous.maximum {
                    joined.maximum = thresholds
                        .iter()
                        .find(|threshold| {
                            joined.maximum <= **threshold && **threshold <= carrier.maximum
                        })
                        .cloned()
                        .unwrap_or(carrier.maximum);
                }
            }
            self.bounds = Some(joined);
            true
        }
    }
}
mod predicates {
    /*
        Relational state inputs come from live completed edges. Scratch rows retain
        canonical places across sweeps; only reseeding allocates current fact handles.
    */
    use crate::checks::contracts::boolean_value_at_place;
    use crate::flow::active_context_handles;
    use crate::values::scalar_value_at_place;
    use validation::{CanonicalPlace, canonical_place_type_reference};

    use crate::flow::FlowBuildContext;
    use crate::published::facts::boolean_predicates::{self, BoundObservation, BoundPredicate};
    use arena::HandleSpan;
    use checked_trees::{CheckedBooleanExpression, FlowSemanticContextRef};
    use facts::{FactOrigin, FactPlan, PlaceRoot, ProgramPoint, ScalarValue};
    use typed_trees::{expression::ExpressionHandle, state::State};

    pub(super) fn meet(previous: &mut Vec<BoundPredicate>, incoming: &[BoundPredicate]) -> bool {
        let before = previous.len();
        previous.retain(|predicate| incoming.contains(predicate));
        previous.len() != before
    }

    #[allow(clippy::too_many_arguments)]
    pub(super) fn capture(
        program: &dyn ::validation::CheckingInput,
        semantic: &FactPlan,
        build: &mut FlowBuildContext,
        state: &State,
        destination: &State,
        statement_index: usize,
        arguments: &[ExpressionHandle],
        contexts: HandleSpan<FlowSemanticContextRef>,
    ) -> Vec<BoundPredicate> {
        let contexts = active_context_handles(&build.contexts, contexts);
        let mut transfers = Vec::new();
        let mut arguments = arguments.iter();
        for parameter in program.state_parameters(destination) {
            let source = {
                arguments.next().and_then(|argument| {
                    build.canonical_place_at(program, state.symbol, statement_index, *argument)
                })
            };
            if let Some(source) = source {
                transfers.push((source, parameter.symbol));
            }
        }
        let rebase = |binding: &BoundObservation| {
            let source = &binding.place;
            let mut candidate = None;
            for (argument, parameter) in &transfers {
                if argument.root != source.root || !source.segments.starts_with(&argument.segments)
                {
                    continue;
                }
                let place = CanonicalPlace {
                    root: PlaceRoot::Symbol(*parameter),
                    segments: source.segments[argument.segments.len()..].to_vec(),
                };
                let reference =
                    canonical_place_type_reference(program, destination.symbol, 0, &place)?;
                let reference =
                    validation::field_domains::readable_type_reference(program, reference)?;
                if program.normalized_type_identity(reference)
                    != program.normalized_type_identity(binding.type_reference)
                {
                    continue;
                }
                if candidate.is_none() {
                    candidate = Some(place);
                }
            }
            candidate
        };
        let mut result = Vec::new();
        let Some(machine) = program.machine_holding_state(state.symbol) else {
            return Vec::new();
        };
        let point = ProgramPoint::Statement {
            machine_symbol: machine.symbol,
            state_symbol: state.symbol,
            statement_index,
        };
        for predicate in
            boolean_predicates::retained(program, build.operators, semantic, &contexts, point)
        {
            let Some(mut predicate) = boolean_predicates::fold(&predicate, |place| {
                let binding = predicate
                    .bindings
                    .iter()
                    .find(|binding| binding.place == *place)?;
                if rebase(binding).is_some() {
                    return None;
                }
                boolean_value_at_place(program, build.operators, semantic, &contexts, point, place)
                    .map(ScalarValue::Boolean)
                    .or_else(|| {
                        scalar_value_at_place(
                            program,
                            semantic,
                            contexts
                                .iter()
                                .map(|context| semantic.contexts.get(*context)),
                            place,
                        )
                    })
            }) else {
                continue;
            };
            let mut complete = true;
            for binding in &mut predicate.bindings {
                let Some(place) = rebase(binding) else {
                    complete = false;
                    break;
                };
                binding.place = place;
            }
            if complete
                && !matches!(
                    predicate.expression,
                    CheckedBooleanExpression::Constant(true)
                )
                && !result.contains(&predicate)
            {
                result.push(predicate);
            }
        }
        result
    }

    pub(super) fn append(
        semantic: &mut FactPlan,
        predicates: &[BoundPredicate],
        point: ProgramPoint,
    ) {
        for predicate in predicates {
            boolean_predicates::seed(semantic, predicate, point, FactOrigin::StatementTransfer);
        }
    }
}
use parameter_bounds::ParameterBounds;
pub(super) mod fields {
    /*
        Live attached-field facts at ordinary state edges. No declaration seeds them.
    */
    use crate::flow::FlowBuildContext;
    use crate::flow::canonical_place_from_semantic_place;
    use crate::flow::expressions;
    use arena::HandleSpan;
    use checked_trees::FlowSemanticContextRef;
    use checked_trees::expression::{ExpressionHandle, ExpressionNode};
    use facts::{
        Fact, FactOrigin, FactPayload, FactPlace, FactPlan, ProgramPoint, QualificationEvidence,
    };
    use symbols::SymbolHandle;
    use validation::field_domains::ByteSequencePredicate;

    /*
        Identifies one incoming edge's contribution to a joined field row. A
        transition edge keys on its source state and arm target; a call-derived
        edge keys on its source state and call site.
    */
    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    pub(super) enum BoundsSource {
        Transition {
            state: SymbolHandle,
            target: typed_trees::statement::TransitionTargetHandle,
        },
        Invocation {
            state: SymbolHandle,
            statement: usize,
            ordinal: usize,
        },
        CallableInput {
            identity: validation::CallableInputIdentity,
        },
    }

    impl BoundsSource {
        pub(super) fn transition(
            state: SymbolHandle,
            target: typed_trees::statement::TransitionTargetHandle,
        ) -> Self {
            Self::Transition { state, target }
        }
        pub(super) fn invocation(state: SymbolHandle, statement: usize, ordinal: usize) -> Self {
            Self::Invocation {
                state,
                statement,
                ordinal,
            }
        }
        pub(super) fn callable_input(input: &validation::StaticMachineContractObligation) -> Self {
            Self::CallableInput {
                identity: input.input_identity(),
            }
        }
    }

    /*
        One predecessor edge's latest delivery for a field. An edge whose captured
        facts say nothing about the field records `empty()` -- an unconstrained
        path -- so every predecessor bounds the join, not just edges that happened
        to carry evidence.
    */
    #[derive(Clone, Debug, PartialEq, Eq)]
    struct EdgeDelivery {
        literal: ExpressionHandle,
        predicates: Vec<ByteSequencePredicate>,
        integer_bounds: Option<facts::IntegerRange>,
    }

    impl FieldValue {
        /* One arrival's stated integer bound for a named field. */
        pub(super) fn declared_bounds(
            place: validation::CanonicalPlace,
            bounds: facts::IntegerRange,
        ) -> Self {
            Self {
                root: place.root,
                segments: place.segments,
                literal: ExpressionHandle::invalid(),
                predicates: Vec::new(),
                integer_bounds: Some(bounds),
                deliveries: Vec::new(),
                bounds_growth: 0,
                bounds_shrink: 0,
            }
        }

        /*
            Record the edge this row was first built from, so later meets recompute
            the join over per-edge deliveries instead of treating this one as
            already joined.
        */
        pub(super) fn seed_delivery(&mut self, source: BoundsSource) {
            self.deliveries = vec![(source, EdgeDelivery::of(self))];
        }
    }

    impl EdgeDelivery {
        fn empty() -> Self {
            Self {
                literal: ExpressionHandle::invalid(),
                predicates: Vec::new(),
                integer_bounds: None,
            }
        }

        fn of(field: &FieldValue) -> Self {
            Self {
                literal: field.literal,
                predicates: field.predicates.clone(),
                integer_bounds: field.integer_bounds.clone(),
            }
        }
    }

    #[derive(Clone, Debug, PartialEq, Eq)]
    pub(super) struct FieldValue {
        /*
            The destination-side identity of the carrier storage: the machine
            symbol for an attached `self` field, or the destination parameter's
            own symbol for a `&mut`/shared transition argument. `&mut self` is an
            ordinary `&mut T` argument's receiver spelling; the two roots name the
            same kind of evidence at one hop apart.
        */
        root: facts::PlaceRoot,
        segments: Vec<facts::PlaceSegment>,
        literal: ExpressionHandle,
        predicates: Vec<ByteSequencePredicate>,
        integer_bounds: Option<facts::IntegerRange>,
        /*
            Each predecessor edge's latest delivery. Every joined channel is
            recomputed from these rows, so an edge that later proves richer
            evidence retightens the join; meeting directly into the stored channels
            could only shrink them, and a mid-fixpoint under-approximation (a bound
            still unconstrained on one pass) would wedge the row forever.
        */
        deliveries: Vec<(BoundsSource, EdgeDelivery)>,
        /*
            Joins that extended the accumulated interval. Repeated growth widens
            to authored thresholds or the primitive carrier. Unchanged deliveries
            preserve that widening; genuinely changed evidence can retighten it.
        */
        bounds_growth: u8,
        /*
            Joins that narrowed the accumulated interval. Repeated shrinking on a
            bound that is being recomputed one step per join -- a loop clamping a
            counter feeds each pass a slightly tighter endpoint -- holds the
            shrunken axis so the slide cannot take one pass per integer.
        */
        bounds_shrink: u8,
    }

    const BOUNDS_WIDENING_DELAY: u8 = 4;
    const BOUNDS_NARROWING_LIMIT: u8 = 8;

    pub(super) fn height(fields: &[FieldValue], threshold_count: usize) -> usize {
        let bounds_height = threshold_count
            .saturating_add(1)
            .saturating_mul(2)
            .saturating_mul(usize::from(BOUNDS_NARROWING_LIMIT) + 1)
            .saturating_add(usize::from(BOUNDS_WIDENING_DELAY))
            .saturating_add(usize::from(BOUNDS_NARROWING_LIMIT));
        fields
            .iter()
            .map(|field| {
                1 + field.predicates.len()
                    + if field.integer_bounds.is_some() {
                        bounds_height
                    } else {
                        0
                    }
            })
            .sum()
    }

    /*
        The data definition a captured row's root denotes: the attached record for
        the machine root, or the readable referent type behind the destination
        parameter's own symbol for a transition argument. `&mut self` and a named
        `&mut T` parameter are the same carrier here, one root spelling apart.
    */
    fn root_data_definition<'program>(
        program: &'program dyn ::validation::CheckingInput,
        machine: &typed_trees::machine::Machine,
        state: &typed_trees::state::State,
        root: facts::PlaceRoot,
    ) -> Option<&'program typed_trees::data::DataDefinition> {
        let symbol = root.as_symbol()?;
        if symbol == machine.symbol {
            let attached = machine.attached_data.as_ref()?;
            return program.data_definition_by_name_str(attached.as_str());
        }
        let parameter = program.state_parameter_by_symbol(state, symbol)?;
        validation::field_domains::readable_nominal_definition(program, parameter.type_reference)
    }

    /*
        The declared primitive extent of a field path, when it resolves to a
        primitive scalar: the widest interval a widening join can need.
    */
    fn carrier_integer_range(
        program: &dyn ::validation::CheckingInput,
        machine: &typed_trees::machine::Machine,
        state: &typed_trees::state::State,
        root: facts::PlaceRoot,
        segments: &[facts::PlaceSegment],
    ) -> Option<facts::IntegerRange> {
        let mut data = root_data_definition(program, machine, state, root)?;
        let mut reference = None;
        for (index, segment) in segments.iter().enumerate() {
            let facts::PlaceSegment::Field { symbol } = segment else {
                return None;
            };
            let next = program
                .data_members(data)
                .iter()
                .find_map(|member| match member {
                    typed_trees::data::DataMember::Field(field) if field.symbol == *symbol => field
                        .type_reference
                        .is_valid()
                        .then_some(field.type_reference),
                    _ => None,
                })?;
            reference = Some(next);
            if index + 1 < segments.len() {
                data = validation::field_domains::data_definition_for_field_type(program, next)?;
            }
        }
        let primitive = program.primitive_type_reference(reference?)?;
        validation::carrier_integer_range(primitive)
    }

    /*
        Integer literals the program compares against, sorted ascending. They are
        the widening thresholds: a still-extending bound settles where a guard or
        contract compares it with a constant (named constants are substituted by
        now, so `i < LIMIT` reads as a literal comparison). A growing bound climbs
        one threshold per flow pass, so every other literal -- an increment, an
        index, a byte value -- only added passes: std's 193 literals made one
        loop counter take 183 passes, where its 81 compared literals suffice.
        The collection is program-pure; `FlowBuildContext` memoizes it so the
        arena scan runs at most once per flow build rather than per widening
        rejoin.
    */
    pub(in crate::flow) fn integer_literal_thresholds(
        program: &dyn ::validation::CheckingInput,
    ) -> Vec<numerics::bignum::BigInt> {
        use typed_trees::expression::BinaryOperator;
        let mut literals: Vec<numerics::bignum::BigInt> = expressions(program)
            .iter_expressions()
            .filter_map(|(_, expression)| match expression {
                ExpressionNode::Binary(binary)
                    if matches!(
                        binary.operator,
                        BinaryOperator::Less
                            | BinaryOperator::LessOrEqual
                            | BinaryOperator::Greater
                            | BinaryOperator::GreaterOrEqual
                            | BinaryOperator::Equal
                            | BinaryOperator::NotEqual
                    ) =>
                {
                    Some([binary.left, binary.right])
                }
                _ => None,
            })
            .flatten()
            .filter_map(
                |operand| match program.expression_table.expression(operand) {
                    ExpressionNode::Integer(literal) => literal.value_bignum(),
                    _ => None,
                },
            )
            .collect();
        literals.sort();
        literals.dedup();
        literals
    }

    /*
        Missing evidence is absorbing for a field, but only per edge: each join
        records what THIS edge delivered and recomputes the row from every edge's
        latest delivery, so a field an edge stops mentioning loses that edge's
        evidence yet can be re-delivered by a later pass.

        Channels join differently over the deliveries. Integer bounds union: a
        value entering the state may come from any edge, so only the union of the
        edges' ranges covers it; an edge that captured no bound contributes the
        declared carrier. Byte predicates intersect: a predicate holds only when
        every edge proves it, so an edge that delivered none empties the join.
        A literal survives only when every edge delivered that same literal.
        A bound that keeps extending across joins -- a counter backedge that adds
        one each pass -- widens to authored thresholds or the declared primitive
        carrier. Identical arrivals retain that widening rather than shrinking
        back to the raw union without new evidence.
    */
    pub(super) fn meet(
        program: &dyn ::validation::CheckingInput,
        machine: &typed_trees::machine::Machine,
        destination: &typed_trees::state::State,
        previous: &mut Vec<FieldValue>,
        incoming: &[FieldValue],
        source: BoundsSource,
        known_sources: &[BoundsSource],
        thresholds: &[numerics::bignum::BigInt],
    ) -> bool {
        let mut changed = false;
        previous.retain_mut(|field| {
            let delivery = incoming
                .iter()
                .find(|next| next.root == field.root && next.segments == field.segments)
                .map(EdgeDelivery::of)
                .unwrap_or_else(EdgeDelivery::empty);
            if let Some(slot) = field.deliveries.iter_mut().find(|(key, _)| *key == source) {
                if slot.1 == delivery {
                    return true;
                }
                slot.1 = delivery;
            } else {
                field.deliveries.push((source, delivery));
            }
            changed |= field.rejoin(program, machine, destination, thresholds);
            true
        });
        for next in incoming {
            if previous
                .iter()
                .any(|field| field.root == next.root && field.segments == next.segments)
            {
                continue;
            }
            let mut deliveries: Vec<(BoundsSource, EdgeDelivery)> = known_sources
                .iter()
                .filter(|key| **key != source)
                .map(|key| (*key, EdgeDelivery::empty()))
                .collect();
            deliveries.push((source, EdgeDelivery::of(next)));
            let mut field = FieldValue {
                root: next.root,
                segments: next.segments.clone(),
                literal: ExpressionHandle::invalid(),
                predicates: Vec::new(),
                integer_bounds: None,
                deliveries,
                bounds_growth: 0,
                bounds_shrink: 0,
            };
            field.rejoin(program, machine, destination, thresholds);
            previous.push(field);
            changed = true;
        }
        changed
    }

    impl FieldValue {
        /*
            Recompute the joined literal, predicate set, and integer bound from the
            per-edge deliveries. `thresholds` is the memoized program literal set
            widening consults when a bound keeps extending.
        */
        fn rejoin(
            &mut self,
            program: &dyn ::validation::CheckingInput,
            machine: &typed_trees::machine::Machine,
            state: &typed_trees::state::State,
            thresholds: &[numerics::bignum::BigInt],
        ) -> bool {
            let mut changed = false;
            let literal = self
                .deliveries
                .first()
                .filter(|(_, first)| {
                    first.literal.is_valid()
                        && self
                            .deliveries
                            .iter()
                            .all(|(_, delivery)| delivery.literal == first.literal)
                })
                .map(|(_, first)| first.literal)
                .unwrap_or_else(ExpressionHandle::invalid);
            let literal_unchanged = match (self.literal.is_valid(), literal.is_valid()) {
                (true, true) => self.literal == literal,
                (false, false) => true,
                _ => false,
            };
            if !literal_unchanged {
                self.literal = literal;
                changed = true;
            }
            let predicates: Vec<ByteSequencePredicate> = ByteSequencePredicate::ALL
                .into_iter()
                .filter(|predicate| {
                    self.deliveries
                        .iter()
                        .all(|(_, delivery)| delivery.predicates.contains(predicate))
                })
                .collect();
            if self.predicates != predicates {
                self.predicates = predicates;
                changed = true;
            }
            let mut joined: Option<facts::IntegerRange> = None;
            let mut unbounded_edge = false;
            for (_, delivery) in &self.deliveries {
                match &delivery.integer_bounds {
                    Some(bound) => {
                        joined = Some(match joined {
                            Some(accumulated) => facts::IntegerRange {
                                minimum: accumulated.minimum.min(bound.minimum.clone()),
                                maximum: accumulated.maximum.max(bound.maximum.clone()),
                            },
                            None => bound.clone(),
                        });
                    }
                    None => {
                        unbounded_edge = true;
                        break;
                    }
                }
            }
            let mut joined = if unbounded_edge {
                carrier_integer_range(program, machine, state, self.root, &self.segments)
            } else {
                joined
            };
            if let (Some(stored), Some(fresh)) = (&self.integer_bounds, &joined) {
                let extends_minimum = fresh.minimum < stored.minimum;
                let extends_maximum = fresh.maximum > stored.maximum;
                if extends_minimum || extends_maximum {
                    self.bounds_growth = self.bounds_growth.saturating_add(1);
                    if self.bounds_growth >= BOUNDS_WIDENING_DELAY
                        && let Some(carrier) = carrier_integer_range(
                            program,
                            machine,
                            state,
                            self.root,
                            &self.segments,
                        )
                        && let Some(widened) = joined.as_mut()
                    {
                        if extends_minimum {
                            widened.minimum = thresholds
                                .iter()
                                .rev()
                                .find(|literal| **literal <= widened.minimum)
                                .cloned()
                                .unwrap_or_else(|| carrier.minimum.clone());
                        }
                        if extends_maximum {
                            widened.maximum = thresholds
                                .iter()
                                .find(|literal| **literal >= widened.maximum)
                                .cloned()
                                .unwrap_or_else(|| carrier.maximum.clone());
                        }
                    }
                }
            }
            if let (Some(stored), Some(fresh)) = (&self.integer_bounds, &mut joined)
                && (fresh.minimum > stored.minimum || fresh.maximum < stored.maximum)
            {
                self.bounds_shrink = self.bounds_shrink.saturating_add(1);
                if self.bounds_shrink >= BOUNDS_NARROWING_LIMIT {
                    if fresh.minimum > stored.minimum {
                        fresh.minimum = stored.minimum.clone();
                    }
                    if fresh.maximum < stored.maximum {
                        fresh.maximum = stored.maximum.clone();
                    }
                }
            }
            if self.integer_bounds != joined {
                self.integer_bounds = joined;
                changed = true;
            }
            changed
        }
    }

    /*
        A Named edge's explicit arguments bind the destination's non-self
        parameters. Each argument's canonical place names the caller storage the
        parameter aliases, so a captured fact rooted at that place -- with the
        argument's own segment prefix stripped -- is evidence about the
        parameter's referent at entry. `&mut self` rides the machine root through
        the same join; a named `&mut T` argument is no different one symbol over.
    */
    #[allow(clippy::too_many_arguments)]
    /*
        The (value handle, byte predicates, integer bounds) row an integer point
        carries: no place, no predicates, and the degenerate min == max interval.
    */
    fn point_range_point(
        value: numerics::bignum::BigInt,
    ) -> (
        ExpressionHandle,
        Vec<ByteSequencePredicate>,
        Option<facts::IntegerRange>,
    ) {
        (
            ExpressionHandle::invalid(),
            Vec::new(),
            Some(facts::IntegerRange {
                minimum: value.clone(),
                maximum: value,
            }),
        )
    }

    pub(super) fn capture(
        program: &dyn ::validation::CheckingInput,
        semantic: &FactPlan,
        build: &mut FlowBuildContext,
        state: &typed_trees::state::State,
        destination: &typed_trees::state::State,
        statement_index: usize,
        arguments: &[typed_trees::expression::ExpressionHandle],
        contexts: HandleSpan<FlowSemanticContextRef>,
    ) -> Vec<FieldValue> {
        let mut argument_roots: Vec<(facts::PlaceRoot, Vec<facts::PlaceSegment>, SymbolHandle)> =
            Vec::new();
        for (ordinal, parameter) in program.state_parameters(destination).iter().enumerate() {
            let Some(&argument) = arguments.get(ordinal) else {
                break;
            };
            let Some(place) =
                build.canonical_place_at(program, state.symbol, statement_index, argument)
            else {
                continue;
            };

            if place.root.as_symbol().is_some() {
                argument_roots.push((place.root, place.segments, parameter.symbol));
            }
        }
        if argument_roots.is_empty() {
            return Vec::new();
        }
        let contexts = crate::flow::active_context_handles(&build.contexts, contexts);
        capture_storage_fields(
            program,
            semantic,
            build.operators,
            state,
            statement_index,
            &argument_roots,
            &contexts,
        )
    }

    /*
        One completed argument supplies its own value epoch and every possible
        referent. Only facts common to all retained alternatives reach the formal.
    */
    pub(super) fn capture_completed_argument(
        program: &dyn ::validation::CheckingInput,
        semantic: &FactPlan,
        operators: &checked_trees::CheckedOperatorFacts,
        state: &typed_trees::state::State,
        statement_index: usize,
        completed: &super::CompletedArgumentFacts,
        destination: SymbolHandle,
    ) -> Vec<FieldValue> {
        let mut common: Option<Vec<FieldValue>> = None;
        for place in &completed.places {
            let roots = [(
                place.projected.root,
                place.projected.segments.clone(),
                destination,
            )];
            let found = capture_storage_fields(
                program,
                semantic,
                operators,
                state,
                statement_index,
                &roots,
                &completed.contexts,
            );
            common = Some(match common {
                None => found,
                Some(mut previous) => {
                    previous.retain_mut(|field| {
                        let Some(next) = found.iter().find(|next| {
                            next.root == field.root && next.segments == field.segments
                        }) else {
                            return false;
                        };
                        if field.literal != next.literal {
                            field.literal = ExpressionHandle::invalid();
                        }
                        field
                            .predicates
                            .retain(|predicate| next.predicates.contains(predicate));

                        field.integer_bounds = field
                            .integer_bounds
                            .take()
                            .zip(next.integer_bounds.as_ref())
                            .map(|(previous, next)| facts::IntegerRange {
                                minimum: previous.minimum.min(next.minimum.clone()),
                                maximum: previous.maximum.max(next.maximum.clone()),
                            });
                        true
                    });
                    previous
                }
            });
        }
        common.unwrap_or_default()
    }

    fn capture_storage_fields(
        program: &dyn ::validation::CheckingInput,
        semantic: &FactPlan,
        operators: &checked_trees::CheckedOperatorFacts,
        state: &typed_trees::state::State,
        statement_index: usize,
        argument_roots: &[(facts::PlaceRoot, Vec<facts::PlaceSegment>, SymbolHandle)],
        contexts: &[facts::FactContextHandle],
    ) -> Vec<FieldValue> {
        fn bound_point_rank(point: ProgramPoint) -> (usize, u8) {
            match point {
                ProgramPoint::Statement {
                    statement_index, ..
                } => (statement_index + 1, 0),
                facts::call_point_triple_pattern!(statement_index, ..) => (statement_index + 1, 1),
                ProgramPoint::TransitionArm {
                    statement_index, ..
                }
                | ProgramPoint::Exit {
                    statement_index, ..
                } => (statement_index + 1, 2),
                _ => (0, 0),
            }
        }
        let mut fields: Vec<FieldValue> = Vec::new();
        let mut bounds_ranks: Vec<(usize, u8)> = Vec::new();
        for context in contexts {
            for fact in semantic
                .context_view(semantic.contexts.get(*context))
                .facts()
            {
                let (literal, predicates, integer_bounds) = match fact.payload {
                    FactPayload::AssignedValue { value }
                        if program.expression_table.expression_is_valid(value) =>
                    {
                        if let Some(bytes) =
                            typed_trees::byte_predicates::literal_bytes(program, value)
                        {
                            (
                                value,
                                ByteSequencePredicate::ALL
                                    .into_iter()
                                    .filter(|predicate| predicate.holds_for(&bytes))
                                    .collect::<Vec<_>>(),
                                None,
                            )
                        } else {
                            match program.expression_table.expression(value) {
                                ExpressionNode::Integer(literal) => {
                                    let Some(value) = literal.value_bignum() else {
                                        continue;
                                    };
                                    point_range_point(value.clone())
                                }
                                _ if crate::values::is_assignment_literal(program, value) => {
                                    (value, Vec::new(), None)
                                }
                                _ => continue,
                            }
                        }
                    }
                    FactPayload::AssignedScalarValue { value } => {
                        let facts::ScalarValue::Integer(value) = semantic.scalar_values.get(value)
                        else {
                            continue;
                        };
                        point_range_point(value.clone())
                    }
                    FactPayload::BytePredicate { predicate: proved } => (
                        ExpressionHandle::invalid(),
                        ByteSequencePredicate::ALL
                            .into_iter()
                            .filter(|predicate| proved.implies(*predicate))
                            .collect(),
                        None,
                    ),
                    FactPayload::AssignedIntegerBounds { bounds }
                        if semantic.integer_ranges.is_valid(bounds) =>
                    {
                        let bounds = semantic.integer_ranges.get(bounds);
                        if bounds.minimum > bounds.maximum {
                            continue;
                        }
                        (
                            ExpressionHandle::invalid(),
                            Vec::new(),
                            Some(bounds.clone()),
                        )
                    }
                    _ => continue,
                };
                let mut predicates = predicates;
                if let Some(bounds) = &integer_bounds
                    && let (Some(minimum), Some(maximum)) =
                        (bounds.minimum.to_i64(), bounds.maximum.to_i64())
                    && let (Ok(minimum), Ok(maximum)) =
                        (u8::try_from(minimum), u8::try_from(maximum))
                {
                    for predicate in ByteSequencePredicate::ALL.into_iter().filter(|predicate| {
                        predicate.is_subslice_preserving()
                            && predicate.holds_for(&[minimum, maximum])
                    }) {
                        crate::shared::collections::push_unique(&mut predicates, predicate);
                    }
                }
                let Some(place) = fact.place.as_place() else {
                    continue;
                };
                let Some(place) = canonical_place_from_semantic_place(
                    program,
                    semantic,
                    semantic.places.get(place),
                ) else {
                    continue;
                };

                let field_segments = |segments: &[facts::PlaceSegment]| {
                    segments.iter().all(|segment| match segment {
                        facts::PlaceSegment::Field { symbol } => symbol.is_valid(),
                        facts::PlaceSegment::Case { variant } => variant.is_valid(),
                        facts::PlaceSegment::FixedIndex { .. } => true,
                        _ => false,
                    })
                };
                let mut targets: Vec<(facts::PlaceRoot, &[facts::PlaceSegment])> = Vec::new();
                for (root, prefix, parameter) in argument_roots {
                    if place.root != *root
                        || !place.segments.starts_with(prefix)
                        || !field_segments(&place.segments[prefix.len()..])
                    {
                        continue;
                    }
                    targets.push((
                        facts::PlaceRoot::Symbol(*parameter),
                        &place.segments[prefix.len()..],
                    ));
                }
                for (root, segments) in targets {
                    if let Some(index) = fields
                        .iter()
                        .position(|field| field.root == root && field.segments == segments)
                    {
                        let field = &mut fields[index];
                        if literal.is_valid() {
                            field.literal = literal;
                        }
                        if let Some(incoming) = integer_bounds.clone() {
                            let rank = bound_point_rank(fact.point);
                            field.integer_bounds =
                                match (bounds_ranks[index], field.integer_bounds.take()) {
                                    (stored, Some(previous)) if stored > rank => Some(previous),
                                    (stored, Some(previous)) if stored == rank => {
                                        Some(facts::IntegerRange {
                                            minimum: previous.minimum.min(incoming.minimum),
                                            maximum: previous.maximum.max(incoming.maximum),
                                        })
                                    }
                                    _ => {
                                        bounds_ranks[index] = rank;
                                        Some(incoming)
                                    }
                                };
                        }
                        for predicate in predicates.iter().copied() {
                            crate::shared::collections::push_unique(
                                &mut field.predicates,
                                predicate,
                            );
                        }
                    } else {
                        bounds_ranks.push(if integer_bounds.is_some() {
                            bound_point_rank(fact.point)
                        } else {
                            (0, 0)
                        });
                        fields.push(FieldValue {
                            root,
                            segments: segments.to_vec(),
                            literal,
                            predicates: predicates.clone(),
                            integer_bounds: integer_bounds.clone(),
                            deliveries: Vec::new(),
                            bounds_growth: 0,
                            bounds_shrink: 0,
                        });
                    }
                }
            }
        }
        capture_predicate_bounds(
            program,
            semantic,
            operators,
            state,
            statement_index,
            argument_roots,
            contexts,
            &mut fields,
        );
        fields
    }

    /*
        A completed guard can bound a field without an assigned-bounds payload.
        Query its live numeric consequence before rebinding the captured argument;
        no predicate expression or caller-local operand crosses the call boundary.
    */
    #[allow(clippy::too_many_arguments)]
    fn capture_predicate_bounds(
        program: &dyn validation::CheckingInput,
        semantic: &FactPlan,
        operators: &checked_trees::CheckedOperatorFacts,
        state: &typed_trees::state::State,
        statement_index: usize,
        argument_roots: &[(facts::PlaceRoot, Vec<facts::PlaceSegment>, SymbolHandle)],
        contexts: &[facts::FactContextHandle],
        fields: &mut Vec<FieldValue>,
    ) {
        let mut places = Vec::new();
        for handle in semantic.unique_context_facts(contexts.iter().copied()) {
            let FactPayload::BooleanPredicate { predicate } = semantic.facts.get(handle).payload
            else {
                continue;
            };
            if !semantic.boolean_predicates.is_valid(predicate) {
                continue;
            }
            let predicate = semantic.boolean_predicates.get(predicate);
            if predicate.role != facts::BooleanPredicateRole::CurrentObservation {
                continue;
            }
            for binding in semantic
                .boolean_predicate_bindings
                .span_or_empty(predicate.bindings)
            {
                if semantic.places.is_valid(binding.place)
                    && let Some(place) = canonical_place_from_semantic_place(
                        program,
                        semantic,
                        semantic.places.get(binding.place),
                    )
                {
                    places.push(place);
                }
            }
        }
        places.retain(|place| {
            argument_roots
                .iter()
                .any(|(root, prefix, _)| place.root == *root && place.segments.starts_with(prefix))
        });
        places.sort_by(validation::CanonicalPlace::compare_identity);
        places.dedup();
        if places.is_empty() {
            return;
        }
        let Some(mut observations) = crate::values::CurrentIntegerBounds::at_statement(
            program,
            operators,
            semantic,
            contexts,
            state.symbol,
            statement_index,
        ) else {
            return;
        };
        let mut indexed = fields
            .iter()
            .enumerate()
            .map(|(index, field)| {
                (
                    validation::CanonicalPlace {
                        root: field.root,
                        segments: field.segments.clone(),
                    },
                    index,
                )
            })
            .collect::<Vec<_>>();
        indexed.sort_by(|(left, _), (right, _)| left.compare_identity(right));
        let mut additional = Vec::new();
        for place in places {
            let Some(primitive) = validation::canonical_place_type_reference(
                program,
                state.symbol,
                statement_index,
                &place,
            )
            .and_then(|reference| {
                validation::field_domains::readable_type_reference(program, reference)
            })
            .and_then(|reference| program.primitive_type_reference(reference)) else {
                continue;
            };
            let Some(carrier) = validation::carrier_integer_range(primitive) else {
                continue;
            };
            let Some(bounds) = observations.at_place(&place, primitive) else {
                continue;
            };
            if validation::integer_range_contains(&bounds, &carrier) {
                continue;
            }
            for (root, prefix, parameter) in argument_roots {
                if place.root != *root || !place.segments.starts_with(prefix) {
                    continue;
                }
                let suffix = &place.segments[prefix.len()..];
                if !suffix.iter().all(|segment| match segment {
                    facts::PlaceSegment::Field { symbol } => symbol.is_valid(),
                    facts::PlaceSegment::Case { variant } => variant.is_valid(),
                    facts::PlaceSegment::FixedIndex { .. } => true,
                    _ => false,
                }) {
                    continue;
                }
                let destination = validation::CanonicalPlace {
                    root: facts::PlaceRoot::Symbol(*parameter),
                    segments: suffix.to_vec(),
                };
                if let Ok(position) =
                    indexed.binary_search_by(|(place, _)| place.compare_identity(&destination))
                {
                    let field = &mut fields[indexed[position].1];
                    field.integer_bounds = Some(match field.integer_bounds.take() {
                        Some(previous) => facts::IntegerRange {
                            minimum: previous.minimum.max(bounds.minimum.clone()),
                            maximum: previous.maximum.min(bounds.maximum.clone()),
                        },
                        None => bounds.clone(),
                    })
                    .filter(|bounds| bounds.minimum <= bounds.maximum);
                } else {
                    additional.push(FieldValue {
                        root: destination.root,
                        segments: destination.segments,
                        literal: ExpressionHandle::invalid(),
                        predicates: Vec::new(),
                        integer_bounds: Some(bounds.clone()),
                        deliveries: Vec::new(),
                        bounds_growth: 0,
                        bounds_shrink: 0,
                    });
                }
            }
        }
        fields.extend(additional);
    }

    pub(super) fn append(
        program: &dyn ::validation::CheckingInput,
        semantic: &mut FactPlan,
        fields: &[FieldValue],
        machine: &typed_trees::machine::Machine,
        state: &typed_trees::state::State,
        point: ProgramPoint,
    ) {
        for field in fields {
            let mut segments = HandleSpan::empty();
            for segment in &field.segments {
                semantic
                    .place_segments
                    .append_to_span(&mut segments, *segment);
            }
            let place = semantic.append_place(facts::Place {
                root: field.root,
                segments,
            });
            let mut references = HandleSpan::empty();
            let bounds_payload = field
                .integer_bounds
                .as_ref()
                .filter(|bounds| {
                    carrier_integer_range(program, machine, state, field.root, &field.segments)
                        .is_none_or(|carrier| !validation::integer_range_contains(bounds, &carrier))
                })
                .map(|bounds| FactPayload::AssignedIntegerBounds {
                    bounds: semantic.integer_ranges.append(bounds.clone()),
                });
            let payloads = field
                .literal
                .is_valid()
                .then_some(FactPayload::AssignedValue {
                    value: field.literal,
                })
                .into_iter()
                .chain(
                    field
                        .predicates
                        .iter()
                        .map(|predicate| FactPayload::BytePredicate {
                            predicate: *predicate,
                        }),
                )
                .chain(bounds_payload);
            for payload in payloads {
                let fact = semantic.append_fact(Fact {
                    place: FactPlace::Place(place),
                    point,
                    origin: FactOrigin::StatementTransfer,
                    evidence: QualificationEvidence::default(),
                    payload,
                });
                semantic.append_ref(&mut references, fact);
            }
            semantic.append_context(point, references);
        }
    }
}
pub(in crate::flow) mod qualifications {
    /*
        Finite live predicates carried by exact state arguments and original
        invocation referents. Domain memberships and normalized IEEE observations
        use the same custody path, including when no executable binding survives.

        Origins alone supply no row. Every row is captured from an available fact;
        incoming joins can only weaken claims, and root entry is never narrowed.
    */
    use super::TransitionTargetNode;
    use crate::flow::FlowBuildContext;
    use crate::flow::canonical_place_from_semantic_place;
    use crate::flow::expressions;
    use arena::HandleSpan;
    use checked_trees::FlowSemanticContextRef;
    use checked_trees::expression::ExpressionHandle;
    use checked_trees::name::Identifier;
    use facts::{
        Fact, FactOrigin, FactPayload, FactPlace, FactPlan, ProgramPoint, QualificationEvidence,
    };
    use symbols::SymbolHandle;
    use typed_trees::types::TypeReferenceHandle;

    #[derive(Clone, Debug, PartialEq, Eq)]
    pub(in crate::flow) struct QualifiedInput {
        subject: facts::PlaceRoot,
        segments: Vec<facts::PlaceSegment>,
        predicate: CarriedPredicate,
        evidence: QualificationEvidence,
    }

    /*
        The finite transportable predicate lattice: a declared domain membership or
        a normalized IEEE observation. One implementation serves both the exact
        state-argument carriers and the match-result predicate join; the two
        variants compare and combine identically, and only the published row
        differs in whether it names an evaluated value or an authored domain.
    */
    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    pub(in crate::flow) enum CarriedPredicate {
        Domain {
            domain: HandleSpan<Identifier>,
            domain_symbol: SymbolHandle,
            semantic_domain: language_semantics::SemanticDomainId,
        },
        Float(facts::FloatPredicate),
    }

    impl CarriedPredicate {
        fn from_fact(fact: &Fact) -> Option<Self> {
            if matches!(fact.origin, FactOrigin::CallRequires) {
                return None;
            }
            match fact.payload {
                facts::domain_membership_pattern!(domain, domain_symbol, semantic_domain)
                    if domain_symbol.is_valid() =>
                {
                    Some(Self::Domain {
                        domain,
                        domain_symbol,
                        semantic_domain,
                    })
                }
                FactPayload::FloatPredicate { predicate } => Some(Self::Float(predicate)),
                _ => None,
            }
        }

        pub(in crate::flow) fn implies(self, other: Self) -> bool {
            match (self, other) {
                (
                    Self::Domain {
                        domain_symbol,
                        semantic_domain,
                        ..
                    },
                    Self::Domain {
                        domain_symbol: other_symbol,
                        semantic_domain: other_domain,
                        ..
                    },
                ) => domain_symbol == other_symbol && semantic_domain == other_domain,
                (Self::Float(left), Self::Float(right)) => left.implies(right),
                _ => false,
            }
        }

        pub(in crate::flow) fn common_consequence(self, other: Self) -> Option<Self> {
            match (self, other) {
                (Self::Float(left), Self::Float(right)) => {
                    left.common_consequence(right).map(Self::Float)
                }
                _ => self.implies(other).then_some(self),
            }
        }

        /*
            The published fact row. `value` names the evaluated expression whose
            place is observed when the predicate was derived from a value; a
            predicate captured from a fact row keeps that row's authored domain
            spelling and publishes no value.
        */
        pub(in crate::flow) fn payload(self, value: ExpressionHandle) -> FactPayload {
            match self {
                Self::Domain {
                    domain,
                    domain_symbol,
                    semantic_domain,
                } => FactPayload::DomainMembership {
                    value,
                    domain,
                    domain_symbol,
                    semantic_domain,
                },
                Self::Float(predicate) => FactPayload::FloatPredicate { predicate },
            }
        }
    }

    impl QualifiedInput {
        /*
            A selected callable's admitted entry condition, already substituted to
            the selected body's parameter. It inherits authority; it mints none.
        */
        pub(super) fn declared_contract(
            place: validation::CanonicalPlace,
            domain_symbol: SymbolHandle,
            semantic_domain: language_semantics::SemanticDomainId,
            entry: SymbolHandle,
        ) -> Self {
            Self {
                subject: place.root,
                segments: place.segments,
                predicate: CarriedPredicate::Domain {
                    domain: HandleSpan::empty(),
                    domain_symbol,
                    semantic_domain,
                },
                evidence: crate::published::facts::propagated_evidence(entry),
            }
        }

        /*
            One integer per storage place, folded from the root's and segments'
            integer keys: equal subjects get equal keys. Distinct places may share
            a key, so `same_subject` still decides every match.
        */
        fn subject_key(&self) -> u64 {
            let mix = |key: u64, part: u64| {
                (key.rotate_left(5) ^ part).wrapping_mul(0x9E37_79B9_7F4A_7C15)
            };
            let (variant, index, generation) = self.subject.group_key();
            let root = mix(
                u64::from(variant),
                (u64::from(index) << 32) | u64::from(generation),
            );
            self.segments.iter().fold(root, |key, segment| {
                let (variant, first, second) = segment.order_key();
                mix(mix(mix(key, u64::from(variant)), first), second)
            })
        }

        fn same_subject(&self, other: &Self) -> bool {
            self.subject == other.subject
                && self.segments == other.segments
                && self.evidence.same_authority(other.evidence)
        }

        fn same_claim(&self, other: &Self) -> bool {
            self.same_subject(other)
                && self.evidence == other.evidence
                && self.predicate.implies(other.predicate)
                && other.predicate.implies(self.predicate)
        }
    }

    /*
        Scratch positions group a batch by its storage place without reordering
        published evidence. Keys only narrow comparisons: the ordinary
        subject/evidence checks still decide every match, including shared keys.
    */
    fn subject_positions(inputs: &[QualifiedInput]) -> Vec<(u64, usize)> {
        let mut positions: Vec<_> = inputs
            .iter()
            .enumerate()
            .map(|(position, input)| (input.subject_key(), position))
            .collect();
        positions.sort_unstable();
        positions
    }

    fn matching_positions(positions: &[(u64, usize)], key: u64) -> &[(u64, usize)] {
        let start = positions.partition_point(|(candidate, _)| *candidate < key);
        let end = positions.partition_point(|(candidate, _)| *candidate <= key);
        &positions[start..end]
    }

    fn deduplicate(inputs: &mut Vec<QualifiedInput>) {
        let positions = subject_positions(inputs);
        let retained: Vec<_> = inputs
            .iter()
            .enumerate()
            .map(|(position, input)| {
                !matching_positions(&positions, input.subject_key())
                    .iter()
                    .take_while(|(_, previous)| *previous < position)
                    .any(|(_, previous)| inputs[*previous].same_claim(input))
            })
            .collect();
        let mut retained = retained.into_iter();
        inputs.retain(|_| retained.next().unwrap_or(false));
    }

    pub(in crate::flow) struct CapturedQualification {
        input: QualifiedInput,
        context: facts::FactContextHandle,
        isolated: bool,
    }

    impl CapturedQualification {
        /*
            An isolated claim stands on its own storage and never consults the
            caller-side binding-stability verdict -- the transition caller uses
            this to skip resolving stability when nothing could consume it.
        */
        pub(in crate::flow) fn is_isolated(&self) -> bool {
            self.isolated
        }
    }

    pub(super) fn meet(previous: &mut Vec<QualifiedInput>, incoming: &[QualifiedInput]) -> bool {
        let before = previous.len();
        let mut changed = false;
        let positions = subject_positions(incoming);
        previous.retain_mut(|claim| {
            let mut common: Option<(CarriedPredicate, QualificationEvidence)> = None;
            for next in matching_positions(&positions, claim.subject_key())
                .iter()
                .map(|(_, position)| &incoming[*position])
                .filter(|next| claim.same_subject(next))
            {
                let Some(candidate) = claim.predicate.common_consequence(next.predicate) else {
                    continue;
                };
                let Some(evidence) = claim.evidence.common_consequence(next.evidence) else {
                    continue;
                };
                match common {
                    Some((retained, retained_evidence))
                        if candidate.implies(retained) && retained.implies(candidate) =>
                    {
                        if let Some(evidence) = retained_evidence.available_alternative(evidence) {
                            common = Some((retained, evidence));
                        }
                    }
                    _ if common.is_none_or(|(retained, _)| candidate.implies(retained)) => {
                        common = Some((candidate, evidence));
                    }
                    _ => {}
                }
            }
            let Some((common, evidence)) = common else {
                return false;
            };
            changed |= !claim.predicate.implies(common)
                || !common.implies(claim.predicate)
                || claim.evidence != evidence;
            claim.predicate = common;
            claim.evidence = evidence;
            true
        });
        changed || previous.len() != before
    }

    /*
        A membership can only disappear. A float predicate can additionally
        weaken among the five positive comparisons at authored endpoints, then
        non-NaN, then absence. Provenance can additionally weaken from independent
        to one input, mixed inputs, then unknown. No join creates an endpoint or receipt.
    */
    pub(super) fn height(
        program: &dyn ::validation::CheckingInput,
        inputs: &[QualifiedInput],
    ) -> usize {
        let float_height = expressions(program)
            .expression_count()
            .saturating_mul(5)
            .saturating_add(2);
        inputs.iter().fold(0usize, |height, input| {
            height.saturating_add(
                match input.predicate {
                    CarriedPredicate::Domain { .. } => 1,
                    CarriedPredicate::Float(_) => float_height,
                }
                .saturating_mul(4),
            )
        })
    }

    /*
        A projection may pass through owned aggregates, but never another reference.
        The final qualified value must itself be isolated from referenced storage.
    */
    fn owned_projection(
        program: &dyn ::validation::CheckingInput,
        frames: &validation::CallFrameResolver<'_>,
        mut reference: TypeReferenceHandle,
        segments: &[facts::PlaceSegment],
    ) -> bool {
        reference = program
            .type_reference_table
            .unconstrained_type_reference(reference);
        reference = program
            .type_reference_table
            .dereferenced_type_reference(reference);
        let root_type = reference;
        for length in 0..segments.len() {
            let Some(projected) = validation::project_type_reference_from_segments(
                program,
                root_type,
                &segments[..length],
            ) else {
                return false;
            };
            if validation::reference_shell(program, projected).is_some() {
                return false;
            }
        }
        validation::project_type_reference_from_segments(program, root_type, segments)
            .is_some_and(|projected| frames.proof_value_is_caller_isolated(projected))
    }

    /*
        Capture live predicates whose place is the argument's own denoted
        place or a projection below it. `source_segments` names the argument value
        itself, so a fact deeper in the same place describes a selected field of
        the value the state parameter will carry; the captured claim republishes
        only the part below the argument.
    */
    #[allow(clippy::too_many_arguments)]
    fn capture_place(
        program: &dyn ::validation::CheckingInput,
        semantic: &FactPlan,
        flow_context: &FlowBuildContext,
        source_root: facts::PlaceRoot,
        source_segments: &[facts::PlaceSegment],
        source_type: TypeReferenceHandle,
        destination: SymbolHandle,
        contexts: HandleSpan<FlowSemanticContextRef>,
    ) -> Vec<CapturedQualification> {
        let Some(frames) = flow_context.call_frames else {
            return Vec::new();
        };
        let isolated = frames.proof_value_is_caller_isolated(source_type);
        let mut captured = Vec::new();
        for context in flow_context
            .contexts
            .semantic_context_refs
            .span_or_empty(contexts)
        {
            for fact in semantic
                .context_view(semantic.contexts.get(context.context))
                .facts()
            {
                let Some(predicate) = CarriedPredicate::from_fact(fact) else {
                    continue;
                };
                let Some(place) = fact.place.as_place() else {
                    continue;
                };
                let Some(place) = canonical_place_from_semantic_place(
                    program,
                    semantic,
                    semantic.places.get(place),
                ) else {
                    continue;
                };
                if place.root != source_root || !place.segments.starts_with(source_segments) {
                    continue;
                }
                let segments = &place.segments[source_segments.len()..];
                if !owned_projection(program, frames, source_type, segments) {
                    continue;
                }
                captured.push(CapturedQualification {
                    input: QualifiedInput {
                        subject: facts::PlaceRoot::Symbol(destination),
                        segments: segments.to_vec(),
                        predicate,
                        evidence: fact.evidence,
                    },
                    context: context.context,
                    isolated,
                });
            }
        }
        captured
    }

    /*
        Capture live predicates below every place one completed invocation argument
        denotes, re-rooted at its destination parameter. The operand's own evidence
        epoch supplies every row — the same custody judgment the exact state-argument
        carrier uses, replayed through the recorded operand observation rather than
        a re-read of the caller's places. Each denoted alternative must carry the
        same claim independently: a select operand whose alternatives disagree
        delivers nothing.
    */
    pub(super) fn capture_completed_argument(
        program: &dyn ::validation::CheckingInput,
        semantic: &FactPlan,
        flow_context: &FlowBuildContext,
        operand: &checked_trees::FlowOperandFact,
        completed: &super::CompletedArgumentFacts,
        destination_state: SymbolHandle,
        destination: SymbolHandle,
    ) -> Vec<QualifiedInput> {
        let Some(frames) = flow_context.call_frames else {
            return Vec::new();
        };
        let mut common: Option<Vec<QualifiedInput>> = None;
        for place in &completed.places {
            let Some(source_type) = validation::canonical_place_type_reference(
                program,
                operand.state_symbol,
                operand.statement_index,
                &place.projected,
            ) else {
                return Vec::new();
            };
            let mut found = Vec::new();
            for context in &completed.contexts {
                for fact in semantic
                    .context_view(semantic.contexts.get(*context))
                    .facts()
                {
                    let Some(predicate) = CarriedPredicate::from_fact(fact) else {
                        continue;
                    };
                    let Some(candidate) = fact.place.as_place().and_then(|place| {
                        canonical_place_from_semantic_place(
                            program,
                            semantic,
                            semantic.places.get(place),
                        )
                    }) else {
                        continue;
                    };
                    if candidate.root != place.projected.root
                        || !candidate.segments.starts_with(&place.projected.segments)
                    {
                        continue;
                    }
                    let segments = &candidate.segments[place.projected.segments.len()..];
                    if !owned_projection(program, frames, source_type, segments) {
                        continue;
                    }
                    found.push(QualifiedInput {
                        subject: facts::PlaceRoot::Symbol(destination),
                        segments: segments.to_vec(),
                        predicate,
                        evidence: fact
                            .evidence
                            .at_invocation_input(destination_state, destination),
                    });
                }
            }
            deduplicate(&mut found);
            common = Some(match common {
                None => found,
                Some(previous) => previous
                    .into_iter()
                    .filter(|claim| found.iter().any(|next| claim.same_claim(next)))
                    .collect(),
            });
            if common.as_ref().is_some_and(Vec::is_empty) {
                break;
            }
        }
        common.unwrap_or_default()
    }

    /*
        Capture the predicates carried by one transition argument into its
        destination parameter. The argument is read at its authored evaluation
        point, so every denoted place counts — a renamed state parameter, a staged
        local, a selected field, or an evaluated call result — rather than only a
        bare parameter name. Facts must already be live in `contexts`; nothing is
        rederived from the destination signature, and a missing place or value type
        fails closed.
    */
    #[allow(clippy::too_many_arguments)]
    pub(in crate::flow) fn capture_argument(
        program: &dyn ::validation::CheckingInput,
        semantic: &FactPlan,
        flow_context: &mut FlowBuildContext,
        machine: &typed_trees::machine::Machine,
        state: &typed_trees::state::State,
        statement_index: usize,
        target: typed_trees::statement::TransitionTargetHandle,
        ordinal: usize,
        argument: ExpressionHandle,
        contexts: HandleSpan<FlowSemanticContextRef>,
        copied_values: HandleSpan<checked_trees::FlowOperandFact>,
    ) -> Vec<CapturedQualification> {
        let TransitionTargetNode::Named { path, .. } =
            program.statement_table.transition_target(target)
        else {
            return Vec::new();
        };
        let Some(destination) = flow_context
            .state_index_in_machine(program, machine.symbol, path.symbol)
            .and_then(|index| program.machine_states(machine).get(index))
        else {
            return Vec::new();
        };
        let Some(parameter) = program.state_parameters(destination).get(ordinal) else {
            return Vec::new();
        };
        let Some(authored) =
            flow_context.canonical_place_at(program, state.symbol, statement_index, argument)
        else {
            return Vec::new();
        };
        let Some(source_type) =
            flow_context.expression_result_type_at(program, machine, state, argument)
        else {
            return Vec::new();
        };
        let source = crate::flow::observations::completed_copy_read_place(
            program,
            &flow_context.control,
            copied_values,
            state.symbol,
            statement_index,
            argument,
            &authored,
        );
        let source = match source {
            Some(place) => place,
            None if validation::reference_shell(program, source_type).is_some() => authored,
            None => return Vec::new(),
        };
        let denoted = if validation::reference_shell(program, source_type).is_some() {
            match crate::flow::argument_storage_places(
                program,
                state.symbol,
                statement_index,
                &source,
                flow_context.call_frames,
            ) {
                Some(places) if !places.is_empty() => places,
                _ => return Vec::new(),
            }
        } else {
            vec![source]
        };
        let mut common: Option<Vec<CapturedQualification>> = None;
        for place in denoted {
            let found = capture_place(
                program,
                semantic,
                flow_context,
                place.root,
                &place.segments,
                source_type,
                parameter.symbol,
                contexts,
            );
            common = Some(match common {
                None => found,
                Some(previous) => previous
                    .into_iter()
                    .filter(|claim| found.iter().any(|next| claim.input.same_claim(&next.input)))
                    .collect(),
            });
            if common.as_ref().is_some_and(Vec::is_empty) {
                break;
            }
        }
        common.unwrap_or_default()
    }

    pub(in crate::flow) fn finish(
        flow_context: &FlowBuildContext,
        contexts: HandleSpan<FlowSemanticContextRef>,
        captured: Vec<CapturedQualification>,
        bindings_stable: bool,
    ) -> Vec<QualifiedInput> {
        let live = flow_context
            .contexts
            .semantic_context_refs
            .span_or_empty(contexts);
        let mut live_membership = None;
        let mut inputs: Vec<QualifiedInput> = Vec::new();
        for captured in captured {
            if captured.isolated
                || (bindings_stable
                    && live_membership
                        .get_or_insert_with(|| {
                            crate::flow::reference_spans::SemanticContextMembership::new(live)
                        })
                        .contains(captured.context))
            {
                inputs.push(captured.input);
            }
        }
        deduplicate(&mut inputs);
        inputs
    }

    /*
        Capture original input storage independently of the next state's runtime
        argument roster. These are current live observations, not a replay of the
        machine requires. The same meet as explicit arguments withdraws a claim
        when any reachable predecessor lacks it.
    */
    pub(super) fn capture_invocation_referents(
        program: &dyn ::validation::CheckingInput,
        semantic: &FactPlan,
        build: &mut FlowBuildContext,
        machine: &typed_trees::machine::Machine,
        state: &typed_trees::state::State,
        statement_index: usize,
        contexts: HandleSpan<FlowSemanticContextRef>,
    ) -> Vec<QualifiedInput> {
        let Some(entry) = program.machine_states(machine).first() else {
            return Vec::new();
        };
        if !program.state_parameters(entry).iter().any(|parameter| {
            crate::flow::entry_origins::invocation_referent(program, machine, parameter.symbol)
                .is_some()
        }) {
            return Vec::new();
        }
        let context_refs = build
            .contexts
            .semantic_context_refs
            .span_or_empty(contexts)
            .to_vec();
        let mut inputs = Vec::new();
        for context in context_refs {
            for fact in semantic
                .context_view(semantic.contexts.get(context.context))
                .facts()
            {
                let Some(predicate) = CarriedPredicate::from_fact(fact) else {
                    continue;
                };
                let Some(place) = fact.place.as_place().and_then(|place| {
                    canonical_place_from_semantic_place(
                        program,
                        semantic,
                        semantic.places.get(place),
                    )
                }) else {
                    continue;
                };
                let Some(projected) = crate::flow::entry_origins::project_invocation_place(
                    program,
                    build,
                    machine,
                    state,
                    statement_index,
                    &place,
                    true,
                ) else {
                    continue;
                };
                let [projected] = projected.as_slice() else {
                    continue;
                };
                let facts::PlaceRoot::InvocationReferent { type_reference, .. } = projected.root
                else {
                    continue;
                };
                let Some(frames) = build.call_frames else {
                    continue;
                };
                if !owned_projection(program, frames, type_reference, &projected.segments) {
                    continue;
                }
                let same_carrier = || {
                    let source = validation::canonical_place_type_reference(
                        program,
                        state.symbol,
                        statement_index,
                        &place,
                    )?;
                    let destination = validation::canonical_place_type_reference(
                        program,
                        state.symbol,
                        statement_index,
                        projected,
                    )?;
                    let carrier = |reference| {
                        let reference = program
                            .type_reference_table
                            .unconstrained_type_reference(reference);
                        let reference = program
                            .type_reference_table
                            .dereferenced_type_reference(reference);
                        program.normalized_type_identity(
                            program
                                .type_reference_table
                                .unconstrained_type_reference(reference),
                        )
                    };
                    Some(carrier(source) == carrier(destination))
                };
                if same_carrier() != Some(true) {
                    continue;
                }
                let input = QualifiedInput {
                    subject: projected.root,
                    segments: projected.segments.clone(),
                    predicate,
                    evidence: fact.evidence,
                };
                inputs.push(input);
            }
        }
        deduplicate(&mut inputs);
        inputs
    }

    pub(super) fn append(semantic: &mut FactPlan, inputs: &[QualifiedInput], point: ProgramPoint) {
        for input in inputs {
            let place = semantic.places.append(facts::Place {
                root: input.subject,
                segments: HandleSpan::empty(),
            });
            for segment in &input.segments {
                semantic.push_place_segment(place, *segment);
            }
            let fact = semantic.append_fact(Fact {
                place: FactPlace::Place(place),
                point,
                origin: FactOrigin::StatementTransfer,
                evidence: input.evidence,
                payload: input.predicate.payload(ExpressionHandle::invalid()),
            });
            let mut references = HandleSpan::empty();
            semantic.append_ref(&mut references, fact);
            semantic.append_context(point, references);
        }
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        fn claim(element: usize) -> QualifiedInput {
            QualifiedInput {
                subject: facts::PlaceRoot::Symbol(SymbolHandle::invalid()),
                segments: vec![facts::PlaceSegment::FixedIndex { index: element }],
                predicate: CarriedPredicate::Domain {
                    domain: HandleSpan::empty(),
                    domain_symbol: SymbolHandle::invalid(),
                    semantic_domain: language_semantics::SemanticDomainId::NULL,
                },
                evidence: QualificationEvidence::default(),
            }
        }

        #[test]
        fn subject_batches_preserve_order_and_evidence() {
            let originals: Vec<_> = (0..128).rev().map(claim).collect();
            let mut inputs = originals.clone();
            inputs.extend(originals.iter().rev().cloned());
            let mut distinct_evidence = claim(64);
            distinct_evidence.evidence.origin =
                language_semantics::QualificationEvidenceOrigin::Propagated;
            inputs.push(distinct_evidence.clone());
            deduplicate(&mut inputs);
            let mut expected = originals.clone();
            expected.push(distinct_evidence);
            assert_eq!(inputs, expected);
            assert!(meet(&mut inputs, &originals));
            assert_eq!(inputs, originals);
            assert!(!meet(&mut inputs, &originals));
            let incoming: Vec<_> = originals.iter().step_by(2).cloned().collect();
            assert!(meet(&mut inputs, &incoming));
            assert_eq!(inputs, incoming);
            assert!(meet(&mut inputs, &[]));
            assert!(inputs.is_empty());
        }
    }
}
pub(super) use arguments::capture_argument;

/*
    A missing row is unreachable. A present unknown value must
    participate in the incoming meet; it is never an omitted predecessor.
*/
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct StateValues {
    pub(super) state: SymbolHandle,
    values: Vec<(SymbolHandle, ScalarValue)>,
    /*
        The integer interval every edge proves for the parameter, `None` when
        this edge delivers none. The join keeps the UNION: the destination
        only knows the argument lies inside the widest delivered range.
        `put(35) | put(46)` joins `ch` to `[35, 46]`, which is how an
        all-ASCII glyph table keeps a `u8` byte store provably in-class.
    */
    bounds: Vec<ParameterBounds>,
    fields: Vec<fields::FieldValue>,
    /*
        Every observed input edge, including ones carrying no field evidence.
        A later field cannot forget an earlier unknown predecessor.
    */
    field_sources: Vec<fields::BoundsSource>,
    qualifications: Vec<qualifications::QualifiedInput>,
    predicates: Vec<BoundPredicate>,
    entry_paths: EntryPaths,
    /* The selected cases each arrival delivered, one edge per predecessor. */
    argument_edges: ArgumentInputs,
    /*
        On a stored row, the cases every recorded edge agrees on. On an arriving
        value, that one edge's cases. Keyed by exact destination place.
    */
    cases: Vec<SelectedCase>,
}

type SelectedCase = (CanonicalPlace, SymbolHandle);

/* One arrival's completed argument cases, replaced when that arrival revisits. */
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct ArgumentEdge {
    source: fields::BoundsSource,
    cases: Vec<SelectedCase>,
}

/*
    Unreached inputs collect edges; a conservative fallback already represents
    every possible predecessor. Unknown is absorbing, not an empty edge list
    that a later visit may narrow to the predecessors encountered so far.
*/
#[derive(Clone, Debug, PartialEq, Eq)]
enum ArgumentInputs {
    Unknown,
    Edges(Vec<ArgumentEdge>),
}

pub(super) fn entry_state_index(
    program: &dyn ::validation::CheckingInput,
    machine: &typed_trees::machine::Machine,
) -> Option<usize> {
    validation::activation_target_state_index(program, machine, machine.symbol)
}

fn entry_state<'program>(
    program: &'program dyn ::validation::CheckingInput,
    machine: &typed_trees::machine::Machine,
) -> Option<&'program typed_trees::state::State> {
    program
        .machine_states(machine)
        .get(entry_state_index(program, machine)?)
}

fn reachable(
    build: &FlowBuildContext,
    program: &dyn ::validation::CheckingInput,
    machine: &typed_trees::machine::Machine,
    state: SymbolHandle,
) -> bool {
    let declared_entry = entry_state(program, machine).is_some_and(|entry| entry.symbol == state)
        && !validation::private_invocation_entry_is_closed(program, machine);
    declared_entry || build.reached_state_index(state).is_some()
}

fn join(
    program: &dyn ::validation::CheckingInput,
    build: &mut FlowBuildContext,
    machine: &typed_trees::machine::Machine,
    destination: &typed_trees::state::State,
    source: fields::BoundsSource,
    incoming: StateValues,
) {
    if !delivers_here(program, build, machine, destination, || Delivery::Join {
        destination: destination.symbol,
        source,
        incoming: incoming.clone(),
    }) {
        return;
    }
    let mut incoming = incoming;
    let edge_cases = std::mem::take(&mut incoming.cases);
    let state = incoming.state;
    let mut changed = false;
    let thresholds = build.integer_literal_thresholds_at(program);
    if let Some(previous) = build
        .reached_state_index(incoming.state)
        .map(|index| &mut build.state_value_inputs[index])
    {
        if !previous.field_sources.contains(&source) {
            previous.field_sources.push(source);
            build.new_state_field_input_height += 1;
            changed = true;
        }
        if !matches!(previous.argument_edges, ArgumentInputs::Unknown) {
            changed |= fields::meet(
                program,
                machine,
                destination,
                &mut previous.fields,
                &incoming.fields,
                source,
                &previous.field_sources,
                thresholds.as_slice(),
            );
        }
        changed |= qualifications::meet(&mut previous.qualifications, &incoming.qualifications);
        changed |= predicates::meet(&mut previous.predicates, &incoming.predicates);
        if previous.entry_paths.union(&incoming.entry_paths) {
            changed = true;
            build.new_state_field_input_height += 1;
        }
        for (parameter, value) in &mut previous.values {
            let next = incoming
                .values
                .iter()
                .find(|row| row.0 == *parameter)
                .map(|row| row.1.clone())
                .unwrap_or_default();
            if *value != next {
                changed |= *value != ScalarValue::Unknown;
                *value = ScalarValue::Unknown;
            }
        }
        for bound in &mut previous.bounds {
            let next = incoming
                .bounds
                .iter()
                .find(|row| row.symbol == bound.symbol)
                .and_then(|row| row.bounds.as_ref());
            changed |= bound.join(program, destination, next, thresholds.as_slice());
        }
        if let ArgumentInputs::Edges(edges) = &mut previous.argument_edges {
            match edges.iter_mut().find(|edge| edge.source == source) {
                Some(edge) if same_cases(&edge.cases, &edge_cases) => {}
                Some(edge) => edge.cases = edge_cases,
                None => {
                    edges.push(ArgumentEdge {
                        source,
                        cases: edge_cases,
                    });
                    build.new_state_field_input_height =
                        build.new_state_field_input_height.saturating_add(1);
                    changed = true;
                }
            }
            let joined = joined_cases(edges);
            if !same_cases(&joined, &previous.cases) {
                previous.cases = joined;
                changed = true;
            }
        }
    } else {
        changed = true;
        build.new_state_field_input_height = build.new_state_field_input_height.saturating_add(1);
        build.new_state_field_input_height += fields::height(&incoming.fields, thresholds.len());
        build.new_state_field_input_height += incoming.predicates.len();
        build.new_state_field_input_height = build
            .new_state_field_input_height
            .saturating_add(qualifications::height(program, &incoming.qualifications));
        build.new_state_field_input_height = incoming
            .bounds
            .iter()
            .fold(build.new_state_field_input_height, |height, bound| {
                height.saturating_add(bound.height(thresholds.len()))
            });
        incoming.field_sources.push(source);
        for field in &mut incoming.fields {
            field.seed_delivery(source);
        }
        incoming.cases = edge_cases.clone();
        incoming.argument_edges = ArgumentInputs::Edges(vec![ArgumentEdge {
            source,
            cases: edge_cases,
        }]);
        build.push_state_value_input(incoming);
    }
    if changed && !build.dirty_state_value_inputs.contains(&state) {
        build.dirty_state_value_inputs.push(state);
    }
}

/* Unknown inputs for every state of the machines at roster `positions`. */
pub(super) fn unknown_inputs(
    program: &dyn ::validation::CheckingInput,
    positions: &[usize],
) -> Vec<StateValues> {
    positions
        .iter()
        .map(|position| &program.machines()[*position])
        .flat_map(|machine| {
            program
                .machine_states(machine)
                .iter()
                .map(|state| StateValues {
                    state: state.symbol,
                    field_sources: Vec::new(),
                    fields: Vec::new(),
                    qualifications: Vec::new(),
                    predicates: Vec::new(),
                    entry_paths: EntryPaths::unknown(),
                    values: program
                        .state_parameters(state)
                        .iter()
                        .map(|parameter| (parameter.symbol, ScalarValue::Unknown))
                        .collect(),
                    bounds: Vec::new(),
                    argument_edges: ArgumentInputs::Unknown,
                    cases: Vec::new(),
                })
        })
        .collect()
}

pub(crate) fn literal(
    program: &dyn ::validation::CheckingInput,
    expression: ExpressionHandle,
) -> Option<ScalarValue> {
    if !program.expression_table.expression_is_valid(expression) {
        return None;
    }
    match program.expression_table.expression(expression) {
        ExpressionNode::Integer(value) => value.value_bignum().map(ScalarValue::Integer),
        ExpressionNode::Boolean(value) => Some(ScalarValue::Boolean(*value)),
        _ => None,
    }
}

#[allow(clippy::too_many_arguments)]
pub(super) fn record_transition(
    program: &dyn ::validation::CheckingInput,
    semantic: &mut FactPlan,
    build: &mut FlowBuildContext,
    machine: &typed_trees::machine::Machine,
    state: &typed_trees::state::State,
    statement_index: usize,
    transition: &typed_trees::statement::TableTransition,
    target: typed_trees::statement::TransitionTargetHandle,
    contexts: HandleSpan<FlowSemanticContextRef>,
    captured_operands: &[FlowOperandFact],
    argument_values: &[ScalarValue],
    argument_qualifications: Vec<qualifications::QualifiedInput>,
) {
    if transition.exit != TransitionExit::Ordinary
        || !reachable(build, program, machine, state.symbol)
    {
        return;
    }
    let (destination, arguments) = match program.statement_table.transition_target(target) {
        TransitionTargetNode::Named {
            path, arguments, ..
        } => {
            let Some(destination) = build
                .state_index_in_machine(program, machine.symbol, path.symbol)
                .and_then(|index| program.machine_states(machine).get(index))
            else {
                return;
            };
            (
                destination,
                program.statement_table.expression_handles(*arguments),
            )
        }
        _ => return,
    };
    if entry_state(program, machine).is_some_and(|entry| entry.symbol == destination.symbol) {
        return;
    }
    let mut values = Vec::new();
    let mut bounds = Vec::new();
    for (ordinal, parameter) in program.state_parameters(destination).iter().enumerate() {
        let captured = argument_values.get(ordinal);
        let operand = captured_operands.get(ordinal).filter(|operand| {
            program.state_parameters(destination).len() == arguments.len()
                && arguments.len() == captured_operands.len()
                && argument_values.len() == arguments.len()
                && arguments.get(ordinal).is_some_and(|argument| {
                    completed_argument_matches(operand, state.symbol, statement_index, *argument)
                })
        });
        let scalar = program
            .primitive_type_reference(parameter.type_reference)
            .is_some_and(|primitive| {
                primitive.accepts_integer_literal()
                    || primitive == typed_trees::types::PrimitiveType::Bool
            });
        let value = if !scalar || operand.is_none() {
            ScalarValue::Unknown
        } else {
            captured.cloned().unwrap_or_default()
        };
        let bound = match &value {
            ScalarValue::Integer(value) => Some(facts::IntegerRange {
                minimum: value.clone(),
                maximum: value.clone(),
            }),
            _ => operand
                .and_then(|operand| completed_argument_facts(program, build, operand, contexts))
                .and_then(|captured| {
                    let mut joined: Option<facts::IntegerRange> = None;
                    for place in &captured.places {
                        let incoming = integer_bounds_at_place(
                            program,
                            semantic,
                            captured
                                .contexts
                                .iter()
                                .map(|context| semantic.contexts.get(*context)),
                            &place.projected,
                        )?;
                        joined = Some(match joined {
                            Some(previous) => facts::IntegerRange {
                                minimum: previous.minimum.min(incoming.minimum),
                                maximum: previous.maximum.max(incoming.maximum),
                            },
                            None => incoming,
                        });
                    }
                    joined
                }),
        };
        values.push((parameter.symbol, value));
        bounds.push(ParameterBounds::new(parameter.symbol, bound));
    }
    let fields = fields::capture(
        program,
        semantic,
        build,
        state,
        destination,
        statement_index,
        arguments,
        contexts,
    );
    let predicates = predicates::capture(
        program,
        semantic,
        build,
        state,
        destination,
        statement_index,
        arguments,
        contexts,
    );
    let path_contexts = active_context_handles(&build.contexts, contexts);
    let entry_paths = EntryPaths::retained(semantic, &path_contexts, machine.symbol);
    let mut qualifications = argument_qualifications;
    qualifications.extend(qualifications::capture_invocation_referents(
        program,
        semantic,
        build,
        machine,
        state,
        statement_index,
        contexts,
    ));
    let cases = capture_argument_cases(
        program,
        semantic,
        build,
        state,
        destination,
        statement_index,
        arguments,
        captured_operands,
        contexts,
    );
    join(
        program,
        build,
        machine,
        destination,
        fields::BoundsSource::transition(state.symbol, target),
        StateValues {
            state: destination.symbol,
            field_sources: Vec::new(),
            values,
            bounds,
            fields,
            qualifications,
            predicates,
            entry_paths,
            argument_edges: ArgumentInputs::Edges(Vec::new()),
            cases,
        },
    );
}

/* Ordinary invocations are not narrowed by the internal transition proof. */
pub(super) fn record_invocation<'plans>(
    program: &'plans dyn ::validation::CheckingInput,
    build: &mut FlowBuildContext<'plans>,
    machine: &typed_trees::machine::Machine,
    state: &typed_trees::state::State,
    call: &BorrowCallFact,
) {
    if !reachable(build, program, machine, state.symbol) {
        return;
    }
    let Some((owner, destination)) =
        build
            .state_location(program, call.target_symbol)
            .map(|(machine_index, state_index)| {
                let owner = &program.machines()[machine_index];
                (owner, &program.machine_states(owner)[state_index])
            })
    else {
        return;
    };
    if entry_state(program, owner).is_some_and(|entry| entry.symbol == destination.symbol) {
        return;
    }
    if owner.symbol == machine.symbol
        && matches!(
            super::calls::memoized_find_call_site(
                program,
                build,
                machine.symbol,
                state.symbol,
                call.statement_index,
                call.call_ordinal
            ),
            Some(CallSite::TransitionNamed { .. })
        )
    {
        return;
    }
    let source =
        fields::BoundsSource::invocation(state.symbol, call.statement_index, call.call_ordinal);
    join(
        program,
        build,
        owner,
        destination,
        source,
        StateValues {
            state: destination.symbol,
            field_sources: Vec::new(),
            fields: Vec::new(),
            qualifications: Vec::new(),
            predicates: Vec::new(),
            entry_paths: EntryPaths::unknown(),
            bounds: Vec::new(),
            values: program
                .state_parameters(destination)
                .iter()
                .map(|parameter| (parameter.symbol, ScalarValue::Unknown))
                .collect(),
            argument_edges: ArgumentInputs::Edges(Vec::new()),
            cases: Vec::new(),
        },
    );
}

/*
    Capture one exact argument tuple. Owned values retain their completion
    contexts; views use invocation facts only at their captured referents.
    Source spelling never supplies a later storage version for an earlier value.
*/
#[allow(clippy::too_many_arguments)]
fn capture_argument_cases(
    program: &dyn ::validation::CheckingInput,
    semantic: &FactPlan,
    build: &FlowBuildContext,
    state: &typed_trees::state::State,
    destination: &typed_trees::state::State,
    statement_index: usize,
    arguments: &[ExpressionHandle],
    captured_operands: &[FlowOperandFact],
    contexts: HandleSpan<FlowSemanticContextRef>,
) -> Vec<SelectedCase> {
    let parameters = program.state_parameters(destination);
    let mut cases = Vec::new();
    if parameters.len() != arguments.len()
        || captured_operands.len() != arguments.len()
        || !captured_operands
            .iter()
            .zip(arguments)
            .all(|(operand, argument)| {
                completed_argument_matches(operand, state.symbol, statement_index, *argument)
            })
    {
        return cases;
    }
    for (parameter, operand) in parameters.iter().zip(captured_operands) {
        let Some(captured) = completed_argument_facts(program, build, operand, contexts) else {
            continue;
        };
        let mut places = captured.places.iter().map(|place| {
            selected_cases_under(program, semantic, &captured.contexts, &place.projected)
        });
        let Some(mut agreed) = places.next() else {
            continue;
        };
        for other in places {
            agreed.retain(|case| other.contains(case));
        }
        cases.extend(agreed.into_iter().map(|(segments, variant)| {
            (
                CanonicalPlace {
                    root: facts::PlaceRoot::Symbol(parameter.symbol),
                    segments,
                },
                variant,
            )
        }));
    }
    cases
}

fn completed_argument_matches(
    operand: &FlowOperandFact,
    state: SymbolHandle,
    statement_index: usize,
    argument: ExpressionHandle,
) -> bool {
    operand.expression == argument
        && operand.state_symbol == state
        && operand.statement_index == statement_index
        && operand.type_reference.is_valid()
}

/*
    Invocation scratch over the existing completed operand graph. Facts about
    an owned value keep its completion epoch; readable live views use only the
    invocation epoch at their captured referents. Neither path grants access.
*/
struct CompletedArgumentFacts {
    contexts: Vec<facts::FactContextHandle>,
    places: Vec<super::operand_values::CapturedValuePlace>,
}

fn completed_argument_facts(
    program: &dyn ::validation::CheckingInput,
    build: &FlowBuildContext,
    operand: &FlowOperandFact,
    invocation_contexts: HandleSpan<FlowSemanticContextRef>,
) -> Option<CompletedArgumentFacts> {
    let live_view = super::operand_binds_live_view(program, operand.type_reference);
    if live_view {
        validation::field_domains::readable_type_reference(program, operand.type_reference)?;
        if operand.referents.is_empty()
            && !matches!(
                operand.completion,
                checked_trees::FlowOperandCompletion::Copied { .. }
            )
        {
            return None;
        }
    }
    let contexts = if live_view {
        active_context_handles(&build.contexts, invocation_contexts)
    } else {
        build
            .contexts
            .constraint_refs
            .span_or_empty(operand.constraints)
            .iter()
            .filter_map(|reference| match reference.kind {
                FlowConstraintKind::SemanticContext { context } => Some(context),
                _ => None,
            })
            .collect()
    };
    let places = super::operand_values::captured_places(
        program,
        &build.control,
        operand,
        operand.type_reference,
        &super::LiteralExpressionProjection {
            expression: operand.expression,
            remaining: Vec::new(),
            destination: Vec::new(),
        },
    )?;
    if places.is_empty() {
        return None;
    }
    Some(CompletedArgumentFacts { contexts, places })
}

/*
    Selected cases at `root` or a named field or case payload under it, relative
    to `root`. Two live facts that disagree at one place select nothing there.
    An index or range is meaningful only in the caller, so it is not transported.
*/
fn selected_cases_under(
    program: &dyn ::validation::CheckingInput,
    semantic: &FactPlan,
    contexts: &[facts::FactContextHandle],
    root: &CanonicalPlace,
) -> Vec<(Vec<facts::PlaceSegment>, SymbolHandle)> {
    let mut found: Vec<(Vec<facts::PlaceSegment>, Option<SymbolHandle>)> = Vec::new();
    for fact in contexts.iter().flat_map(|context| {
        semantic
            .context_view(semantic.contexts.get(*context))
            .facts()
    }) {
        let FactPayload::CaseValue {
            variant,
            value: true,
        } = fact.payload
        else {
            continue;
        };
        if !variant.is_valid() || program.symbols.get(variant).kind != symbols::SymbolKind::Variant
        {
            continue;
        }
        let Some(handle) = fact.place.as_place() else {
            continue;
        };
        let Some(fact_place) =
            canonical_place_from_semantic_place(program, semantic, semantic.places.get(handle))
        else {
            continue;
        };
        if fact_place.root != root.root || !fact_place.segments.starts_with(&root.segments) {
            continue;
        }
        let relative = &fact_place.segments[root.segments.len()..];
        if !relative.iter().all(|segment| {
            matches!(
                segment,
                facts::PlaceSegment::Field { .. } | facts::PlaceSegment::Case { .. }
            )
        }) {
            continue;
        }
        match found.iter_mut().find(|row| row.0 == relative) {
            Some(row) if row.1 != Some(variant) => row.1 = None,
            Some(_) => {}
            None => found.push((relative.to_vec(), Some(variant))),
        }
    }
    found
        .into_iter()
        .filter_map(|(segments, variant)| Some((segments, variant?)))
        .collect()
}

fn same_cases(left: &[SelectedCase], right: &[SelectedCase]) -> bool {
    left.len() == right.len() && left.iter().all(|case| right.contains(case))
}

/* A case survives only when every arrival edge selects it at the same place. */
fn joined_cases(edges: &[ArgumentEdge]) -> Vec<SelectedCase> {
    let Some(first) = edges.first() else {
        return Vec::new();
    };
    first
        .cases
        .iter()
        .filter(|case| edges.iter().all(|edge| edge.cases.contains(case)))
        .cloned()
        .collect()
}

/*
    Caller facts can enter a body only when every possible invocation is known.
    A written precondition is complete. Public entries, escaped callables and
    provider bodies also use declared inputs, even when known callers supply
    stronger facts. Compiler-extracted regions retain their source continuation.
*/
pub(super) fn caller_roster_supplies_entry(
    program: &dyn ::validation::CheckingInput,
    machine: &typed_trees::machine::Machine,
) -> bool {
    !machine.is_public
        && !machine.target_family_interface
        && machine.supplies_checked_body()
        && validation::with_private_invocation_inputs(program, machine, |_| ()).is_some()
}
/*
    An authored machine nested inside an enclosing machine's scope belongs to
    that activation: its declaration-scope parent is a machine lexical scope.
*/
pub(super) fn lexically_nested_machine(
    program: &dyn ::validation::CheckingInput,
    machine: symbols::SymbolHandle,
) -> bool {
    let scope = program.declaration_scope_parent(machine);
    scope.is_valid()
        && program
            .machine_declaration_for_lexical_scope(scope)
            .is_some()
}

/*
    The callee's authored lexical scope encloses the caller machine: the
    callee's declaring machine is the caller itself or one of its lexical
    ancestors, so the invocation replays within the same source activation.
*/
pub(super) fn lexical_scope_covers_call(
    program: &dyn ::validation::CheckingInput,
    caller: symbols::SymbolHandle,
    callee: symbols::SymbolHandle,
) -> bool {
    let scope = program.declaration_scope_parent(callee);
    let Some(declaring) = scope
        .is_valid()
        .then(|| program.machine_declaration_for_lexical_scope(scope))
        .flatten()
    else {
        return false;
    };
    if declaring.symbol == caller {
        return true;
    }
    let mut enclosing = program.declaration_scope_parent(caller);
    while enclosing.is_valid() {
        if let Some(declaration) = program.machine_declaration_for_lexical_scope(enclosing)
            && declaration.symbol == declaring.symbol
        {
            return true;
        }
        enclosing = program.declaration_scope_parent(enclosing);
    }
    false
}

pub(super) fn append_entry_context(
    program: &dyn ::validation::CheckingInput,
    semantic: &mut FactPlan,
    build: &mut FlowBuildContext,
    machine: &typed_trees::machine::Machine,
    state: &typed_trees::state::State,
) {
    let declared_entry = machine.lexical_region_origin.is_none()
        && entry_state(program, machine).is_some_and(|entry| entry.symbol == state.symbol);
    if declared_entry && !caller_roster_supplies_entry(program, machine) {
        return;
    }
    let Some(input) = build
        .reached_state_index(state.symbol)
        .map(|index| &build.state_value_inputs[index])
    else {
        return;
    };
    let point = ProgramPoint::State {
        machine_symbol: machine.symbol,
        state_symbol: state.symbol,
    };
    fields::append(program, semantic, &input.fields, machine, state, point);
    qualifications::append(semantic, &input.qualifications, point);
    predicates::append(semantic, &input.predicates, point);
    if let Some(entry) = validation::activation_target_state_index(program, machine, machine.symbol)
        .and_then(|index| program.machine_states(machine).get(index))
    {
        seed_entry_paths(
            semantic,
            machine.symbol,
            entry.symbol,
            point,
            &input.entry_paths,
        );
    }
    for (parameter, value) in &input.values {
        if *value == ScalarValue::Unknown {
            continue;
        }
        let mut refs = HandleSpan::empty();
        let place = semantic.append_symbol_place(*parameter);
        let value = semantic.scalar_values.append(value.clone());
        let fact = semantic.append_transfer_at_point(
            FactPlace::Place(place),
            point,
            FactPayload::AssignedScalarValue { value },
        );
        semantic.append_ref(&mut refs, fact);
        semantic.append_context(point, refs);
    }
    for parameter in &input.bounds {
        let Some(bounds) = &parameter.bounds else {
            continue;
        };
        let mut refs = HandleSpan::empty();
        let place = semantic.append_symbol_place(parameter.symbol);
        let bounds = semantic.integer_ranges.append(bounds.clone());
        let fact = semantic.append_transfer_at_point(
            FactPlace::Place(place),
            point,
            FactPayload::AssignedIntegerBounds { bounds },
        );
        semantic.append_ref(&mut refs, fact);
        semantic.append_context(point, refs);
    }
    for (selected, variant) in &input.cases {
        let mut refs = HandleSpan::empty();
        let mut segments = HandleSpan::empty();
        for segment in &selected.segments {
            semantic
                .place_segments
                .append_to_span(&mut segments, *segment);
        }
        let place = semantic.append_place(facts::Place {
            root: selected.root,
            segments,
        });
        let fact = semantic.append_transfer_at_point(
            FactPlace::Place(place),
            point,
            FactPayload::CaseValue {
                variant: *variant,
                value: true,
            },
        );
        semantic.append_ref(&mut refs, fact);
        semantic.append_context(point, refs);
    }
}

/*
    One value-input delivery a state visit made, kept with the exact arguments
    `join` received so a group run can repeat it.
*/
#[derive(Clone, Debug)]
pub(super) struct InputDelivery(Delivery);

#[derive(Clone, Debug)]
enum Delivery {
    Join {
        destination: SymbolHandle,
        source: fields::BoundsSource,
        incoming: StateValues,
    },
    /* The destination does not resolve back to the state that received it. */
    Unreplayable,
}

/* The deliveries of one state visit, tagged with its pass. */
#[derive(Debug)]
pub(super) struct RecordedVisit {
    pub(super) state: SymbolHandle,
    pub(super) pass: usize,
    pub(super) deliveries: Vec<InputDelivery>,
}

/*
    A part of a group that converges alone: no machine outside it writes its
    inputs. `machines` is sorted by roster position.
*/
#[derive(Debug, Default)]
pub(super) struct InputRecording {
    pub(super) machines: Vec<usize>,
    pub(super) visits: Vec<RecordedVisit>,
}

/*
    Records the delivery while a part converges alone and reports whether to
    apply it here: rows outside the part belong to the group's later run, which
    replays every recorded delivery in the group's own pass and roster order.
*/
fn delivers_here(
    program: &dyn ::validation::CheckingInput,
    build: &mut FlowBuildContext,
    machine: &typed_trees::machine::Machine,
    destination: &typed_trees::state::State,
    delivery: impl FnOnce() -> Delivery,
) -> bool {
    if build.replay_guard.is_some() {
        let owner = build
            .state_location(program, destination.symbol)
            .map(|(owner, _)| owner);
        if let Some(guard) = build.replay_guard.as_mut() {
            guard.observe(owner);
        }
    }
    if build.input_recording.is_none() {
        return true;
    }
    let location = build.state_location(program, destination.symbol);
    let resolves = location.is_some_and(|(owner, state)| {
        let owner = &program.machines()[owner];
        std::ptr::eq(owner, machine)
            && std::ptr::eq(&program.machine_states(owner)[state], destination)
    });
    let recording = build
        .input_recording
        .as_mut()
        .expect("recording checked above");
    let delivery = if resolves {
        delivery()
    } else {
        Delivery::Unreplayable
    };
    if let Some(visit) = recording.visits.last_mut() {
        visit.deliveries.push(InputDelivery(delivery));
    } else {
        recording.visits.push(RecordedVisit {
            state: SymbolHandle::invalid(),
            pass: usize::MAX,
            deliveries: vec![InputDelivery(Delivery::Unreplayable)],
        });
    }
    location.is_some_and(|(owner, _)| recording.machines.binary_search(&owner).is_ok())
}

/* Repeat one recorded delivery; `false` when it no longer resolves. */
pub(super) fn replay_delivery(
    program: &dyn ::validation::CheckingInput,
    build: &mut FlowBuildContext,
    delivery: &InputDelivery,
) -> bool {
    let Delivery::Join {
        destination,
        source,
        incoming,
    } = &delivery.0
    else {
        return false;
    };
    let Some((owner, state)) = build.state_location(program, *destination) else {
        return false;
    };
    let machine = &program.machines()[owner];
    let destination = &program.machine_states(machine)[state];
    join(
        program,
        build,
        machine,
        destination,
        *source,
        incoming.clone(),
    );
    true
}

/*
    While a split group replays its independent parts, every input row of a
    part state must change only through that part: its replayed visits or its
    own seeds. The solo run that recorded the part saw no other writer, so any
    other delivery into a part row means the recording no longer describes the
    serial run and the group must be built serially.
*/
#[derive(Debug)]
pub(super) struct ReplayGuard {
    /* The replayed part of each roster position, or `None` outside every part. */
    part_of: Vec<Option<u32>>,
    /* The roster position whose visit or seeds are delivering now. */
    writer: Option<usize>,
    broken: bool,
}

impl ReplayGuard {
    pub(super) fn new(machine_count: usize, parts: &[Vec<usize>]) -> Self {
        let mut part_of = vec![None; machine_count];
        for (part, machines) in parts.iter().enumerate() {
            let part = u32::try_from(part).expect("replayed parts exceed u32");
            for &machine in machines {
                part_of[machine] = Some(part);
            }
        }
        Self {
            part_of,
            writer: None,
            broken: false,
        }
    }

    pub(super) fn set_writer(&mut self, writer: usize) {
        self.writer = Some(writer);
    }

    /*
        One delivery into a row owned by roster position `destination`; an
        unresolved destination cannot be shown to stay outside every part.
    */
    pub(super) fn observe(&mut self, destination: Option<usize>) {
        let Some(destination) = destination else {
            self.broken = true;
            return;
        };
        let Some(part) = self.part_of.get(destination).copied().flatten() else {
            return;
        };
        let writer_part = self
            .writer
            .and_then(|writer| self.part_of.get(writer).copied().flatten());
        if writer_part != Some(part) {
            self.broken = true;
        }
    }

    pub(super) fn holds(&self) -> bool {
        !self.broken
    }
}

#[cfg(test)]
mod replay_guard_tests {
    use super::ReplayGuard;

    #[test]
    fn outside_delivery_into_a_replayed_part_breaks_the_replay() {
        let parts = [vec![0, 1], vec![3]];
        let mut guard = ReplayGuard::new(4, &parts);
        guard.set_writer(0);
        guard.observe(Some(1));
        guard.observe(Some(2));
        assert!(
            guard.holds(),
            "a part writing its own rows and outside rows is replayable"
        );
        guard.set_writer(2);
        guard.observe(Some(2));
        assert!(
            guard.holds(),
            "an outside machine writing outside rows is replayable"
        );
        guard.observe(Some(0));
        assert!(
            !guard.holds(),
            "an outside machine wrote a replayed part's row"
        );

        let mut guard = ReplayGuard::new(4, &parts);
        guard.set_writer(1);
        guard.observe(Some(3));
        assert!(!guard.holds(), "one part wrote another part's row");

        let mut guard = ReplayGuard::new(4, &parts);
        guard.observe(Some(3));
        assert!(
            !guard.holds(),
            "a delivery with no writer reached a part row"
        );

        let mut guard = ReplayGuard::new(4, &parts);
        guard.set_writer(0);
        guard.observe(None);
        assert!(
            !guard.holds(),
            "an unresolved destination is not shown outside the parts"
        );
    }
}
