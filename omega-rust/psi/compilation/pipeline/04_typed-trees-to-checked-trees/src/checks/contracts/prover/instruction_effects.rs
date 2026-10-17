//! Match sealed instruction-model propositions to completed flow events.
//! Values are compared at their own operand-completion frontiers. Unknown
//! scalars can retain the same place only through the flow owner's complete
//! write footprints in the admitted control order; no source expression or
//! call initializer is replayed to recover a value.
use checked_trees::CheckFacts;
use facts::{FactPayload, ProgramPoint};
use symbols::SymbolHandle;
use typed_trees::expression::ExpressionHandle;
use typed_trees::proposition::PropositionApplication;
use typed_trees::types::PrimitiveType;
mod atomic;
mod operands;
use operands::operand_matches;

pub(super) fn proves_contract(
    program: &dyn validation::CheckingInput,
    facts: &CheckFacts,
    point: ProgramPoint,
    contexts: &[facts::FactContextHandle],
    fact: &facts::Fact,
) -> bool {
    let (FactPayload::PropositionApplication { fact: source, .. }
    | FactPayload::ContractPropositionApplication { fact: source, .. }) = fact.payload
    else {
        return false;
    };
    let Some(application) = program.proof_facts.get(source).as_proposition() else {
        return false;
    };
    let ProgramPoint::Statement {
        state_symbol,
        statement_index,
        ..
    } = point
    else {
        return false;
    };
    proves(
        program,
        facts,
        state_symbol,
        statement_index,
        contexts,
        application,
    )
}
pub(in crate::checks::contracts) fn proves(
    program: &dyn validation::CheckingInput,
    facts: &CheckFacts,
    state_symbol: SymbolHandle,
    statement: usize,
    contexts: &[facts::FactContextHandle],
    application: &PropositionApplication,
) -> bool {
    evidence(
        program,
        facts,
        state_symbol,
        statement,
        contexts,
        application,
        &[],
    )
    .is_some()
}

/// An invocation path distinguishes a helper's new instruction from a premise
/// it merely preserves. Every normal callee exit contributes its own child;
/// mutually exclusive exits may select different instructions. Two receipts
/// overlap when any execution alternative would spend the same physical event.
#[derive(Clone)]
pub(in crate::checks) struct InstructionEffectEvidence {
    pub(in crate::checks) event: facts::FactHandle,
    contract: arena::Handle<checked_trees::ContractProofFact>,
    pub(in crate::checks) children: Vec<InstructionEffectEvidence>,
    observation: ProgramPoint,
}
impl InstructionEffectEvidence {
    pub(in crate::checks) fn publish(
        &self,
        arena: &mut arena::Arena<checked_trees::flow::FlowInstructionEffectReceiptFact>,
    ) -> arena::Handle<checked_trees::flow::FlowInstructionEffectReceiptFact> {
        let record = self.record(arena);
        arena.append(record)
    }
    pub(in crate::checks) fn observe_at(&mut self, point: ProgramPoint) {
        self.observation = point;
    }

    fn record(
        &self,
        arena: &mut arena::Arena<checked_trees::flow::FlowInstructionEffectReceiptFact>,
    ) -> checked_trees::flow::FlowInstructionEffectReceiptFact {
        let children = self
            .children
            .iter()
            .map(|child| child.record(arena))
            .collect::<Vec<_>>();
        let children = arena.insert_many(children);
        checked_trees::flow::FlowInstructionEffectReceiptFact {
            event: self.event,
            contract: self.contract,
            observation: self.observation,
            children,
        }
    }

    pub(in crate::checks) fn overlaps(&self, facts: &CheckFacts, other: &Self) -> bool {
        if facts.semantic.facts.get(self.event).point != facts.semantic.facts.get(other.event).point
        {
            return false;
        }
        match (self.children.is_empty(), other.children.is_empty()) {
            (true, true) => true,
            (false, false) => self.children.iter().any(|left| {
                other.children.iter().any(|right| {
                    left.observation == right.observation && left.overlaps(facts, right)
                })
            }),
            _ => false,
        }
    }
}
pub(in crate::checks) fn evidence(
    program: &dyn validation::CheckingInput,
    facts: &CheckFacts,
    state_symbol: SymbolHandle,
    statement: usize,
    contexts: &[facts::FactContextHandle],
    application: &PropositionApplication,
    spent: &[InstructionEffectEvidence],
) -> Option<InstructionEffectEvidence> {
    evidence_inner(
        program,
        facts,
        state_symbol,
        statement,
        contexts,
        application,
        spent,
        &mut Vec::new(),
    )
}
fn evidence_inner(
    program: &dyn validation::CheckingInput,
    facts: &CheckFacts,
    state_symbol: SymbolHandle,
    statement: usize,
    contexts: &[facts::FactContextHandle],
    application: &PropositionApplication,
    spent: &[InstructionEffectEvidence],
    active: &mut Vec<SymbolHandle>,
) -> Option<InstructionEffectEvidence> {
    let contract = validation::instruction_effect_contract(program, application)?;
    let operation = contract.operation;
    let (_, flow) = facts
        .flow
        .control
        .states
        .iter()
        .find(|(_, flow)| flow.state_symbol == state_symbol)?;
    let (_, state) = program.state_flow_site(flow.machine_symbol, state_symbol)?;
    let order = program
        .statement_table
        .admitted_execution_order(state.statement_nodes);
    let observation = ProgramPoint::Statement {
        machine_symbol: flow.machine_symbol,
        state_symbol,
        statement_index: statement,
    };
    let arguments = QueryArguments::new(program, &contract)?;
    let required = arguments.frames.last()?.arguments.clone();
    let carriers = match operation {
        facts::InstructionEffectOperation::Register(operation) => operation.input_carriers(),
        facts::InstructionEffectOperation::AtomicReleaseStore => &[],
    };
    let arity = if operation == facts::InstructionEffectOperation::AtomicReleaseStore {
        2
    } else {
        carriers.len()
    };
    if required.len() != arity {
        return None;
    }
    for context in contexts {
        for reference in facts
            .semantic
            .refs
            .span_or_empty(facts.semantic.contexts.get(*context).facts)
        {
            let event = facts.semantic.facts.get(reference.fact);
            if operation == facts::InstructionEffectOperation::AtomicReleaseStore
                && matches!(
                    event.payload,
                    FactPayload::InstructionEffect {
                        operation: facts::InstructionEffectOperation::AtomicReleaseStore
                    }
                )
            {
                if atomic::matches_store(
                    program, facts, flow, state, statement, contexts, event, &arguments, &required,
                ) {
                    let receipt = InstructionEffectEvidence {
                        event: reference.fact,
                        contract: Default::default(),
                        children: Vec::new(),
                        observation,
                    };
                    if !spent.iter().any(|other| receipt.overlaps(facts, other)) {
                        return Some(receipt);
                    }
                }
                continue;
            }
            let ProgramPoint::CallEnsures {
                machine_symbol,
                state_symbol: event_state,
                statement_index,
                call_ordinal,
            } = event.point
            else {
                continue;
            };
            if machine_symbol != flow.machine_symbol
                || event_state != state_symbol
                || !(order.precedes(statement_index, statement)
                    || (statement
                        == program
                            .statement_table
                            .statements(state.statement_nodes)
                            .len()
                        && order.position(statement_index).is_some()))
            {
                continue;
            }
            let Some(call) = facts
                .flow
                .control
                .calls
                .span_or_empty(flow.calls)
                .iter()
                .find(|call| {
                    call.statement_index == statement_index && call.call_ordinal == call_ordinal
                })
            else {
                continue;
            };
            if matches!(event.payload, FactPayload::CallCompletion { target } if target == call.target_symbol)
            {
                for ensured in facts.proof.contract_fact_refs.span_or_empty(call.ensures) {
                    let guarantee = facts.proof.contract_facts.get(ensured.fact);
                    if guarantee.kind != checked_trees::ContractProofFactKind::Ensures {
                        continue;
                    }
                    let Some(candidate) = program.proof_facts.get(guarantee.fact).as_proposition()
                    else {
                        continue;
                    };
                    let matches = match operation {
                        facts::InstructionEffectOperation::Register(register_operation) => {
                            operands::guarantee_matches(
                                program,
                                facts,
                                flow,
                                state,
                                statement,
                                contexts,
                                call,
                                candidate,
                                &arguments,
                                &required,
                                register_operation,
                            )
                        }
                        facts::InstructionEffectOperation::AtomicReleaseStore => {
                            atomic::guarantee_matches(
                                program, facts, flow, state, statement, contexts, call, candidate,
                                &arguments, &required,
                            )
                        }
                    };
                    if !matches {
                        continue;
                    }
                    let Some(callee_flow) = facts
                        .flow
                        .control
                        .states
                        .iter()
                        .map(|(_, flow)| flow)
                        .find(|flow| flow.state_symbol == call.target_symbol)
                    else {
                        continue;
                    };
                    let Some(callee) = program.machine_by_symbol(callee_flow.machine_symbol) else {
                        continue;
                    };
                    if active.contains(&callee.symbol) {
                        continue;
                    }
                    let exits = facts
                        .flow
                        .control
                        .exits
                        .iter()
                        .filter_map(|(_, exit)| {
                            (exit.machine_symbol == callee.symbol).then_some(exit)
                        })
                        .collect::<Vec<_>>();
                    if exits.is_empty() {
                        continue;
                    }
                    active.push(callee.symbol);
                    let children = exits
                        .iter()
                        .map(|exit| {
                            let contexts = facts
                                .flow
                                .semantic_constraint_contexts(exit.entry_constraints)
                                .collect::<Vec<_>>();
                            let observation = ProgramPoint::Exit {
                                machine_symbol: exit.machine_symbol,
                                state_symbol: exit.state_symbol,
                                statement_index: exit.statement_index,
                                transition_target: exit.transition_target,
                            };
                            let spent_here = spent
                                .iter()
                                .filter(|receipt| {
                                    facts.semantic.facts.get(receipt.event).point == event.point
                                })
                                .flat_map(|receipt| &receipt.children)
                                .filter(|receipt| receipt.observation == observation)
                                .cloned()
                                .collect::<Vec<_>>();
                            evidence_inner(
                                program,
                                facts,
                                exit.state_symbol,
                                exit.statement_index,
                                &contexts,
                                candidate,
                                &spent_here,
                                active,
                            )
                            .map(|mut receipt| {
                                receipt.observation = observation;
                                receipt
                            })
                        })
                        .collect::<Option<Vec<_>>>();
                    active.pop();
                    if let Some(children) = children {
                        let receipt = InstructionEffectEvidence {
                            event: reference.fact,
                            contract: ensured.fact,
                            children,
                            observation,
                        };
                        if !spent.iter().any(|other| receipt.overlaps(facts, other)) {
                            return Some(receipt);
                        }
                    }
                }
                continue;
            }
            if !matches!(event.payload, FactPayload::InstructionEffect { operation: recorded } if recorded == operation)
            {
                continue;
            }
            let operands = facts.flow.control.operands.span_or_empty(call.operands);
            if operands.len() == required.len()
                && operands.iter().zip(&required).zip(carriers).all(
                    |((operand, required), carrier)| {
                        let Some(primitive) = PrimitiveType::from_name(carrier.name()) else {
                            return false;
                        };
                        operand_matches(
                            program, facts, flow, state, statement, contexts, operand, &arguments,
                            *required, primitive,
                        )
                    },
                )
            {
                let receipt = InstructionEffectEvidence {
                    event: reference.fact,
                    contract: Default::default(),
                    children: Vec::new(),
                    observation,
                };
                if !spent.iter().any(|other| receipt.overlaps(facts, other)) {
                    return Some(receipt);
                }
            }
        }
    }
    None
}

#[derive(Clone, Copy)]
enum QueryArgument {
    Source(ExpressionHandle),
    Parameter {
        frame: usize,
        expression: ExpressionHandle,
    },
}
struct QueryFrame<'program> {
    declaration: &'program typed_trees::proposition::PropositionDefinition,
    application: &'program PropositionApplication,
    arguments: Vec<QueryArgument>,
}
struct QueryArguments<'program> {
    frames: Vec<QueryFrame<'program>>,
}
impl<'program> QueryArguments<'program> {
    fn new(
        program: &'program dyn validation::CheckingInput,
        contract: &validation::InstructionEffectContract<'program>,
    ) -> Option<Self> {
        let mut frames: Vec<QueryFrame<'program>> = Vec::new();
        for application in &contract.applications {
            let declaration = program.proposition_definition_by_symbol(application.proposition)?;
            let actuals = program
                .expression_table
                .expression_handles(application.arguments);
            if actuals.len() != program.proposition_parameters(declaration).len() {
                return None;
            }
            let mut arguments = Vec::with_capacity(actuals.len());
            for expression in actuals {
                let argument = if let Some(previous) = frames.last() {
                    let selected = program
                        .expression_table
                        .name_path_node(*expression)
                        .and_then(|name| {
                            program
                                .proposition_parameters(previous.declaration)
                                .iter()
                                .position(|parameter| parameter.symbol == name.symbol)
                        })
                        .and_then(|position| previous.arguments.get(position))
                        .copied();
                    selected.unwrap_or(QueryArgument::Parameter {
                        frame: frames.len() - 1,
                        expression: *expression,
                    })
                } else {
                    QueryArgument::Source(*expression)
                };
                arguments.push(argument);
            }
            frames.push(QueryFrame {
                declaration,
                application,
                arguments,
            });
        }
        Some(Self { frames })
    }
    fn scoped_value(
        &self,
        program: &dyn validation::CheckingInput,
        facts: &CheckFacts,
        argument: QueryArgument,
        projection: &[facts::PlaceSegment],
        primitive: PrimitiveType,
        source: &mut impl FnMut(
            ExpressionHandle,
            &[facts::PlaceSegment],
            PrimitiveType,
        ) -> Option<validation::ScopedArithmeticValue>,
    ) -> Option<validation::ScopedArithmeticValue> {
        let QueryArgument::Parameter { frame, expression } = argument else {
            let QueryArgument::Source(expression) = argument else {
                unreachable!()
            };
            return source(expression, projection, primitive);
        };
        let frame = self.frames.get(frame)?;
        let parameters = program.proposition_parameters(frame.declaration);
        let project = |occurrence| {
            let (position, path, _) =
                crate::values::structural_parameter_place(program, parameters, occurrence)?;
            let (_, segments, _, _) = crate::values::resolve_structural_parameter_path(
                program, parameters, position, &path,
            )?;
            Some((position, segments))
        };
        if let Some((position, mut segments)) = project(expression) {
            segments.extend_from_slice(projection);
            return self.scoped_value(
                program,
                facts,
                *frame.arguments.get(position as usize)?,
                &segments,
                primitive,
                source,
            );
        }
        super::call_guarantees::arithmetic::scalar_term(
            program,
            facts,
            &super::call_guarantees::callable::Callable::Proposition {
                declaration: frame.declaration,
            },
            expression,
            |occurrence, primitive| {
                if let Some((position, segments)) = project(occurrence) {
                    self.scoped_value(
                        program,
                        facts,
                        *frame.arguments.get(position as usize)?,
                        &segments,
                        primitive,
                        source,
                    )
                } else {
                    program
                        .closed_integer_value_in(occurrence, frame.declaration.symbol)
                        .map(|value| validation::ScopedArithmeticValue::Integer(value.value))
                }
            },
        )
    }
}

impl QueryArguments<'_> {
    fn source_projection(
        &self,
        program: &dyn validation::CheckingInput,
        argument: QueryArgument,
        suffix: &[facts::PlaceSegment],
    ) -> Option<(ExpressionHandle, Vec<facts::PlaceSegment>)> {
        match argument {
            QueryArgument::Source(expression) => Some((expression, suffix.to_vec())),
            QueryArgument::Parameter { frame, expression } => {
                let frame = self.frames.get(frame)?;
                let parameters = program.proposition_parameters(frame.declaration);
                let (position, path, _) =
                    crate::values::structural_parameter_place(program, parameters, expression)?;
                let (_, mut segments, _, _) = crate::values::resolve_structural_parameter_path(
                    program, parameters, position, &path,
                )?;
                segments.extend_from_slice(suffix);
                self.source_projection(program, *frame.arguments.get(position as usize)?, &segments)
            }
        }
    }
    fn binder_symbol(
        &self,
        program: &dyn validation::CheckingInput,
        frame: usize,
        position: usize,
    ) -> Option<SymbolHandle> {
        let current = self.frames.get(frame)?;
        let argument = current.application.binder_arguments.get(position)?;
        if argument.kind != typed_trees::proposition::PropositionBinderArgumentKind::Type {
            return None;
        }
        if let Some(previous) = frame.checked_sub(1) {
            let declaration = self.frames.get(previous)?.declaration;
            if let Some(position) = program
                .proposition_binders(declaration)
                .iter()
                .position(|binder| binder.symbol == argument.symbol)
            {
                return self.binder_symbol(program, previous, position);
            }
        }
        argument.symbol.is_valid().then_some(argument.symbol)
    }
}
