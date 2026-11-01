//! One completed call prefix supplies ordered actuals and current reference
//! subjects. Every rank component reads those same operand observations; the
//! tuple judgment forms them all before choosing a strict prefix.

use super::{CapturedRankComponent, Machine, State};
use crate::proof_contracts::contract_entailment::{
    specification_calls::{call_parameter_scope, selected_call_site_contract},
    structural_judgment::normal_returns::OperationOperandDemand,
    structural_judgment::{CapturedIntegerInput, CapturedIntegerRank, FormedIntegerRank},
    structural_prefix::{PrefixObservation, StructuralPrefixSite, StructuralPrefixVisitor},
};
use symbols::SymbolHandle;

pub(super) struct CallProgress {
    pub(super) state: SymbolHandle,
    pub(super) statement_index: usize,
    pub(super) target: typed_trees::statement::TransitionTargetHandle,
    pub(super) callee: SymbolHandle,
    pub(super) callee_body: arena::HandleSpan<State>,
    pub(super) source: Vec<FormedIntegerRank>,
    pub(super) destination: Vec<CapturedRankComponent>,
    pub(super) leading_operands: usize,
    pub(super) seen: bool,
    pub(super) strict: bool,
    pub(super) failure: Option<&'static str>,
}

impl StructuralPrefixVisitor for CallProgress {
    fn site(&self) -> StructuralPrefixSite {
        StructuralPrefixSite {
            state: self.state,
            statement_index: self.statement_index,
            in_transition_target: true,
        }
    }
    fn entered(&mut self) {
        self.seen = true;
    }
    fn unresolved(&mut self, _: &Machine) {
        self.failure = Some("the scalar rank call has an unresolved prefix state");
    }
    fn is_complete(&self) -> bool {
        self.failure.is_some()
    }
    fn observe<'program>(&mut self, observation: PrefixObservation<'_, 'program>) {
        if self.failure.is_some() {
            return;
        }
        let result = self.progress(observation);
        match result {
            Ok(strict) => self.strict &= strict,
            Err(reason) => self.failure = Some(reason),
        }
    }
}

impl CallProgress {
    fn progress(&self, observation: PrefixObservation<'_, '_>) -> Result<bool, &'static str> {
        let (program, machine, state, judge, environment) = observation;
        let callee = program
            .machines()
            .iter()
            .find(|machine| machine.symbol == self.callee && machine.states == self.callee_body)
            .ok_or("the captured rank call lost its exact destination body")?;
        let entry = program
            .machine_states(callee)
            .first()
            .ok_or("the captured rank destination has no entry")?;
        let parameters = program
            .state_parameters(entry)
            .iter()
            .cloned()
            .collect::<Vec<_>>();
        let site =
            crate::collect_call_sites_in_statement(program, machine, state, self.statement_index)
                .into_iter()
                .find(|site| site.completes_transition(program, self.target))
                .ok_or("the rank observation lost its exact call occurrence")?;
        let entries = match site {
            crate::CallSite::Expression { expression, .. } => judge.normal_operation_entries(
                self.statement_index,
                expression,
                OperationOperandDemand::All,
                environment,
                (machine, state, state.return_type),
            ),
            crate::CallSite::TransitionNamed { .. } => {
                let arguments = crate::call_site_argument_expressions(program, &site);
                let formal = site
                    .parameters(program, machine.symbol)
                    .ok_or("the rank call has no selected argument signature")?;
                judge.normal_argument_entries(
                    arguments,
                    formal,
                    environment,
                    (machine, state, state.return_type),
                    Some(site),
                )
            }
            crate::CallSite::Statement(_) => {
                return Err("a scalar rank edge is not a completing invocation");
            }
        }
        .ok_or("the scalar rank call operands have no complete ordered observations")?;
        if entries.is_empty() {
            return Err("the scalar rank call has no completed operand path");
        }
        let mut strict = true;
        for entry_observation in entries {
            let actuals = entry_observation
                .observed_arguments()
                .ok_or("a scalar rank call operand has no value observation")?;
            let actuals = actuals
                .get(self.leading_operands..)
                .filter(|values| values.len() == parameters.len())
                .ok_or("the scalar rank call has an incomplete captured telescope")?;
            let bindings = parameters
                .iter()
                .zip(actuals)
                .map(|(formal, value)| (formal.symbol, value.clone()))
                .collect::<Vec<_>>();
            let mut destinations = Vec::with_capacity(self.destination.len());
            for component in &self.destination {
                let place = crate::canonical_place_from_expression(program, component.subject)
                    .ok_or("the arriving rank input has no exact declared coordinate")?;
                let position = parameters
                    .iter()
                    .position(|parameter| Some(parameter.symbol) == place.root.as_symbol())
                    .ok_or("the arriving rank input is outside its destination telescope")?;
                let read = |projection: &[facts::PlaceSegment]| -> Result<_, &'static str> {
                    let mut path = place.segments.clone();
                    path.extend_from_slice(projection);
                    let input = if crate::reference_shell(
                        program,
                        parameters[position].type_reference,
                    )
                    .is_some()
                    {
                        let public_parameters = match site {
                            crate::CallSite::Expression { .. } => {
                                let owner = selected_call_site_contract(
                                    program,
                                    machine.symbol,
                                    site,
                                )
                                .ok_or(
                                    "the arriving rank reference has no selected public contract",
                                )?;
                                call_parameter_scope(program, machine.symbol, &owner, site).ok_or(
                                    "the arriving rank reference has no applied public telescope",
                                )?
                            }
                            crate::CallSite::TransitionNamed { .. } => {
                                std::borrow::Cow::Borrowed(parameters.as_slice())
                            }
                            crate::CallSite::Statement(_) => {
                                return Err("the rank call has no completing invocation");
                            }
                        };
                        let public_parameter = public_parameters
                        .get(position)
                        .filter(|parameter| {
                            crate::declared_carriers_match(
                                program,
                                parameter.type_reference,
                                parameters[position].type_reference,
                            )
                        })
                        .ok_or(
                            "the arriving rank reference public and executable carriers disagree",
                        )?;
                        let caller = program
                            .machines()
                            .iter()
                            .find(|candidate| {
                                candidate.symbol == machine.symbol
                                    && candidate.states == machine.states
                            })
                            .ok_or("the arriving rank reference lost its exact caller body")?;
                        let frames = entry_observation
                            .judge
                            .frame_resolver()
                            .ok_or("the arriving rank reference has no frame resolver")?;
                        let subjects = match site {
                            crate::CallSite::Expression { expression, .. } => frames
                                .expression_entry_reference_subjects(
                                    caller,
                                    expression,
                                    &public_parameters,
                                ),
                            crate::CallSite::TransitionNamed { .. } => frames
                                .transition_entry_reference_subjects(
                                    caller,
                                    self.target,
                                    &public_parameters,
                                ),
                            crate::CallSite::Statement(_) => {
                                return Err("the rank call has no completing invocation");
                            }
                        }
                        .ok_or("the arriving rank reference has no captured entry subjects")?;
                        let subject = subjects
                            .iter()
                            .find(|subject| subject.parameter == public_parameter.symbol)
                            .ok_or(
                                "the arriving rank reference is missing its exact public subject",
                            )?;
                        entry_observation.judge.argument_scalar_value(
                            state,
                            subject,
                            &path,
                            |selector| {
                                entry_observation.judge.bound_contract_term(
                                    selector,
                                    entry.symbol,
                                    &parameters,
                                    callee.symbol,
                                    &bindings,
                                    &[],
                                    None,
                                    0,
                                )
                            },
                        )?
                    } else {
                        let value = entry_observation.judge.bound_contract_term(
                    component.subject, entry.symbol, &parameters, callee.symbol,
                    &bindings, &[], None, 0,
                ).ok_or("the arriving rank input has no authenticated projection from its captured actuals")?;
                        let reference =
                            crate::value_custody::places::project_type_reference_from_segments(
                                program,
                                parameters[position].type_reference,
                                &place.segments,
                            )
                            .ok_or("the arriving rank base lost its exact declared carrier")?;
                        crate::proof_contracts::contract_entailment::observed_projections::project_observed_value_in_scope(
                    program, &entry_observation.judge, reference, value, projection,
                ).ok_or("the arriving rank component has no authenticated typed projection")?
                    };
                    Ok(input)
                };
                let input = if let Some(primitive) = component.primitive {
                    CapturedIntegerInput::Scalar {
                        value: read(&component.projection)?,
                        primitive,
                    }
                } else {
                    let declaration = program
                        .measures()
                        .iter()
                        .find(|declaration| declaration.symbol == component.measure)
                        .ok_or("the arriving record rank lost its supplying declaration")?;
                    let shape = super::super::declared_input_shape(program, declaration)
                        .ok_or("the arriving record rank has no complete typed read shape")?;
                    let path = place
                        .segments
                        .iter()
                        .chain(&component.projection)
                        .cloned()
                        .collect::<Vec<_>>();
                    let actual = crate::project_type_reference_from_segments(
                        program,
                        parameters[position].type_reference,
                        &path,
                    )
                    .ok_or("the arriving record rank lost its nominal input type")?;
                    if !super::super::declared_input_type_matches(
                        program,
                        actual,
                        shape.type_reference,
                    ) {
                        return Err(
                            "the arriving record rank input does not match its declared nominal type",
                        );
                    }
                    let mut reads = Vec::new();
                    for required in &shape.reads {
                        let projection = component
                            .projection
                            .iter()
                            .chain(&required.coordinate.path)
                            .cloned()
                            .collect::<Vec<_>>();
                        let value = read(&projection)?;
                        reads.push((required.coordinate.clone(), value, required.primitive_type));
                    }
                    let snapshot = entry_observation.judge.bound_contract_term(
                        component.subject, entry.symbol, &parameters, callee.symbol,
                        &bindings, &[], None, 0,
                    ).and_then(|value| {
                        let reference = crate::project_type_reference_from_segments(
                            program, parameters[position].type_reference, &place.segments,
                        )?;
                        super::super::super::observed_projections::project_observed_value_in_scope(
                            program, &entry_observation.judge, reference, value, &component.projection,
                        )
                    }).and_then(|value| {
                        super::super::super::structural_judgment::owned_snapshots::CapturedOwnedSnapshot::completed(
                            &entry_observation.judge, state.symbol, actual, &value,
                        )
                    });
                    CapturedIntegerInput::Record {
                        type_reference: actual,
                        snapshot,
                        reads,
                    }
                };
                destinations.push(CapturedIntegerRank {
                    input,
                    minimum: component.minimum.clone(),
                    measure: component.measure,
                });
            }
            strict &= entry_observation
                .judge
                .compare_captured_integer_rank_sequences(&self.source, &destinations)?;
        }
        Ok(strict)
    }
}
