//! Result classification observes the exact normal return, not value equality
//! against a fabricated case payload. Constructors provide their nominal tag;
//! saved places consume only live flow predicates. Initializers and calls are
//! never replayed to recover a tag after their evaluation point.

use checked_trees::{CheckFacts, ContractProofFactKind, ContractProofFactOwner, FlowExitFact};
use facts::{FactContextHandle, FactPayload, PlaceRoot, PlaceSegment};
use symbols::SymbolHandle;
use typed_trees::{
    TypedTrees,
    expression::{BinaryOperator, ExpressionHandle, ExpressionNode, UnaryOperator},
    machine::Machine,
    state::State,
};

use super::super::return_values::exit_return_expression;
use crate::flow::{CanonicalPlace, canonical_place_from_expression_in_state};

impl<'a> CaseObservation<'a> {
    pub(super) fn for_requirement(
        program: &'a TypedTrees,
        facts: &'a CheckFacts,
        exit: &'a FlowExitFact,
        contexts: &'a [FactContextHandle],
        requirement: &facts::Fact,
        call_frames: Option<&validation::CallFrameResolver<'_>>,
    ) -> Option<Self> {
        let FactPayload::ContractBooleanExpression {
            fact, expression, ..
        } = requirement.payload
        else {
            return None;
        };
        if !matches!(program.proof_facts.get(fact), typed_trees::domain::ProofFact::Expression(source) if *source == expression)
        {
            return None;
        }
        // The expression's reserved result belongs to an authored ensures clause;
        // the exit must independently retain that exact clause as an obligation.
        if !facts
            .proof
            .contract_fact_refs
            .span_or_empty(exit.ensures)
            .iter()
            .any(|reference| {
                let contract = facts.proof.contract_facts.get(reference.fact);
                contract.fact == fact
                    && contract.kind == ContractProofFactKind::Ensures
                    && match contract.owner {
                        ContractProofFactOwner::Machine { machine_symbol } => {
                            machine_symbol == exit.machine_symbol
                        }
                        ContractProofFactOwner::MachineState {
                            machine_symbol,
                            state_symbol,
                        } => {
                            machine_symbol == exit.machine_symbol
                                && state_symbol == exit.state_symbol
                        }
                        _ => false,
                    }
            })
        {
            return None;
        }
        let machine = program
            .machines()
            .iter()
            .find(|machine| machine.symbol == exit.machine_symbol)?;
        let state = program
            .machine_states(machine)
            .iter()
            .find(|state| state.symbol == exit.state_symbol)?;
        let returned = exit_return_expression(program, exit);
        let mut returned_nodes = Vec::new();
        crate::monomorphization::collect_expression_tree(program, returned, &mut returned_nodes);
        // The call-frame query covers calls, not atomic destinations or
        // selected operator implementations. Those must not be mistaken for
        // an empty write frame when rereading an earlier constructor operand.
        let has_unframed_writes = returned_nodes.iter().any(|expression| {
            matches!(
                program.expression_table.expression(*expression),
                ExpressionNode::Atomic(_)
            )
        }) || facts.operators.uses.iter().any(|(_, operator)| {
            operator.status != checked_trees::CheckedOperatorResolutionStatus::BuiltinFallback
                && returned_nodes.contains(&operator.expression)
        });
        Some(CaseObservation {
            program,
            facts,
            exit,
            contexts,
            machine,
            state,
            return_has_no_writes: !has_unframed_writes
                && call_frames.is_some_and(|frames| {
                    frames
                        .expression_write_frame(machine, returned)
                        .into_complete_paths()
                        .is_some_and(|paths| paths.is_empty())
                }),
        })
    }
}

pub(super) struct CaseObservation<'a> {
    program: &'a TypedTrees,
    facts: &'a CheckFacts,
    exit: &'a FlowExitFact,
    contexts: &'a [FactContextHandle],
    machine: &'a Machine,
    state: &'a State,
    return_has_no_writes: bool,
}

impl CaseObservation<'_> {
    pub(super) fn observe(&self, expression: ExpressionHandle) -> Option<bool> {
        let ExpressionNode::Binary(binary) = self.program.expression_table.expression(expression)
        else {
            return None;
        };
        if !validation::has_exact_case_membership_meaning(
            self.program,
            self.machine,
            Some(self.state),
            expression,
            binary,
        ) {
            return None;
        }
        let subject = validation::reserved_result_place(self.program, binary.left)?;
        if subject.machine_symbol != self.machine.symbol {
            return None;
        }
        let ExpressionNode::Name(case) = self.program.expression_table.expression(binary.right)
        else {
            return None;
        };
        let returned = exit_return_expression(self.program, self.exit);
        if self.returned_case(returned, &subject.segments, case.symbol, true, true, 0) {
            Some(true)
        } else if self.returned_case(returned, &subject.segments, case.symbol, false, true, 0) {
            Some(false)
        } else {
            None
        }
    }

    fn returned_case(
        &self,
        expression: ExpressionHandle,
        projection: &[PlaceSegment],
        case: SymbolHandle,
        required: bool,
        can_read_live: bool,
        depth: usize,
    ) -> bool {
        if depth >= 128
            || !self
                .program
                .expression_table
                .expression_is_valid(expression)
        {
            return false;
        }
        if let ExpressionNode::Match(dispatch) =
            self.program.expression_table.expression(expression)
        {
            // Join the same observation across all independently produced
            // alternatives without importing an unselected arm's predicates.
            let arms = self.program.expression_table.match_arms(dispatch.arms);
            return !arms.is_empty()
                && arms.iter().all(|arm| {
                    self.returned_case(
                        arm.value,
                        projection,
                        case,
                        required,
                        can_read_live,
                        depth + 1,
                    )
                });
        }
        if let Some((segment, rest)) = projection.split_first() {
            if let ExpressionNode::StructLiteral(literal) =
                self.program.expression_table.expression(expression)
            {
                let PlaceSegment::Field { symbol } = segment else {
                    return false;
                };
                if self.program.symbols.get(*symbol).parent != literal.type_symbol {
                    return false;
                }
                let mut fields = self
                    .program
                    .expression_table
                    .struct_fields(literal.fields)
                    .iter()
                    .filter(|field| field.field_symbol == *symbol);
                let Some(field) = fields.next() else {
                    return false;
                };
                if fields.next().is_some() {
                    return false;
                }
                // A constructor retains each field's evaluated value. Later
                // fields can mutate a source read by an earlier field, so exit
                // predicates may justify that earlier read only with a complete
                // no-write frame. A directly constructed nominal tag needs no
                // such storage premise.
                return self.returned_case(
                    field.value,
                    rest,
                    case,
                    required,
                    can_read_live && self.return_has_no_writes,
                    depth + 1,
                );
            }
            return can_read_live && self.live_case(expression, projection, case, required, depth);
        }
        let owner = self.program.symbols.get(case).parent;
        let Some(data) = self
            .program
            .data_definitions()
            .iter()
            .find(|data| data.symbol == owner)
        else {
            return false;
        };
        match self.program.expression_table.expression(expression) {
            ExpressionNode::StructLiteral(literal) => {
                let Some(actual) = literal.case_symbol else {
                    return false;
                };
                let declared = self.program.data_members(data).iter().any(|member| {
                    matches!(member, typed_trees::data::DataMember::Variant(variant) if variant.symbol == actual)
                });
                literal.type_symbol == owner && declared && (actual == case) == required
            }
            ExpressionNode::Name(path)
                if self.program.symbols.get(path.symbol).kind == symbols::SymbolKind::Variant =>
            {
                let Some(actual_owner) =
                    validation::exact_case_reference_owner(self.program, expression)
                else {
                    return false;
                };
                let payload_free = self.program.data_members(data).iter().any(|member| {
                    matches!(member, typed_trees::data::DataMember::Variant(variant) if variant.symbol == path.symbol && variant.payload.is_empty())
                });
                actual_owner.symbol == owner && payload_free && (path.symbol == case) == required
            }
            _ => can_read_live && self.live_case(expression, &[], case, required, depth),
        }
    }

    fn live_case(
        &self,
        expression: ExpressionHandle,
        projection: &[PlaceSegment],
        case: SymbolHandle,
        required: bool,
        depth: usize,
    ) -> bool {
        let Some(mut subject) = self.stable_place(expression) else {
            return false;
        };
        subject.segments.extend_from_slice(projection);
        self.contexts.iter().any(|context| {
            self.facts
                .semantic
                .context_view(self.facts.semantic.contexts.get(*context))
                .facts()
                .any(|fact| {
                    let (expression, value) = match fact.payload {
                        FactPayload::BooleanValue { expression, value } => (expression, value),
                        FactPayload::BooleanExpression(expression)
                        | FactPayload::ContractBooleanExpression { expression, .. } => {
                            (expression, true)
                        }
                        _ => return false,
                    };
                    self.live_predicate(expression, value, &subject, case, required, depth + 1)
                })
        })
    }

    fn stable_place(&self, expression: ExpressionHandle) -> Option<CanonicalPlace> {
        let place = canonical_place_from_expression_in_state(
            self.program,
            self.state.symbol,
            self.exit.statement_index,
            expression,
        )?;
        (matches!(place.root, PlaceRoot::Symbol(symbol) if symbol.is_valid())
            && place.segments.iter().all(|segment| {
                matches!(segment, PlaceSegment::Field { symbol } if symbol.is_valid())
                    || matches!(segment, PlaceSegment::Case { variant } if variant.is_valid())
                    || matches!(segment, PlaceSegment::FixedIndex { .. })
            }))
        .then_some(place)
    }

    fn live_predicate(
        &self,
        expression: ExpressionHandle,
        value: bool,
        subject: &CanonicalPlace,
        case: SymbolHandle,
        required: bool,
        depth: usize,
    ) -> bool {
        if depth >= 128
            || !self
                .program
                .expression_table
                .expression_is_valid(expression)
        {
            return false;
        }
        match self.program.expression_table.expression(expression) {
            ExpressionNode::Unary(unary) if unary.operator == UnaryOperator::LogicalNot => {
                self.live_predicate(unary.operand, !value, subject, case, required, depth + 1)
            }
            ExpressionNode::Binary(binary)
                if (binary.operator == BinaryOperator::And && value)
                    || (binary.operator == BinaryOperator::Or && !value) =>
            {
                self.live_predicate(binary.left, value, subject, case, required, depth + 1)
                    || self.live_predicate(binary.right, value, subject, case, required, depth + 1)
            }
            ExpressionNode::Binary(binary)
                if validation::has_exact_case_membership_meaning(
                    self.program,
                    self.machine,
                    Some(self.state),
                    expression,
                    binary,
                ) =>
            {
                let ExpressionNode::Name(observed) =
                    self.program.expression_table.expression(binary.right)
                else {
                    return false;
                };
                self.stable_place(binary.left).as_ref() == Some(subject)
                    && self.program.symbols.get(observed.symbol).parent
                        == self.program.symbols.get(case).parent
                    && if value {
                        (observed.symbol == case) == required
                    } else {
                        observed.symbol == case && !required
                    }
            }
            _ => false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn result_field_observation_requires_a_complete_return_write_frame() {
        let source = "data Message { case Empty; case Data(value: u8); }
            data Wrapper { message: Message; later: bool; }
            machine make(value: Message) -> Wrapper requires value in Message::Data;
            ensures result.message in Message::Data;
            { Wrapper { message: value, later: 1 == 1 } }";
        let tokens = source_files_to_tokens::Lexer::new(source)
            .tokenize()
            .unwrap();
        let syntax = tokens_to_syntax_trees::parse_syntax_trees(&tokens).unwrap();
        let resolved = syntax_trees_to_symbol_resolved_trees::lower_syntax_trees(&syntax).unwrap();
        let typed =
            symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees(&resolved).unwrap();
        let checked = crate::lower_typed_trees(typed).unwrap();
        let exit = checked
            .facts
            .flow
            .control
            .exits
            .iter()
            .next()
            .unwrap()
            .1
            .clone();
        let contract = checked
            .facts
            .proof
            .contract_fact_refs
            .span_or_empty(exit.ensures)[0]
            .fact;
        let source_fact = checked.facts.proof.contract_facts.get(contract).fact;
        let requirement = *checked.facts.semantic.facts.iter().find(|(_, fact)| {
            matches!(fact.payload, FactPayload::ContractBooleanExpression { fact, .. } if fact == source_fact)
        }).unwrap().1;
        let preserves_live_reads = |checked: &checked_trees::CheckedTrees| {
            let frames = validation::CallFrameResolver::new(&checked.typed).unwrap();
            CaseObservation::for_requirement(
                &checked.typed,
                &checked.facts,
                &exit,
                &[],
                &requirement,
                Some(&frames),
            )
            .unwrap()
            .return_has_no_writes
        };
        assert!(preserves_live_reads(&checked));
        let ExpressionNode::StructLiteral(literal) = checked
            .typed
            .expression_table
            .expression(exit_return_expression(&checked.typed, &exit))
        else {
            panic!("returned aggregate")
        };
        let later = checked
            .typed
            .expression_table
            .struct_fields(literal.fields)
            .iter()
            .find(|field| field.name.as_str() == "later")
            .unwrap()
            .value;
        let mut selected = checked.clone();
        selected
            .facts
            .operators
            .uses
            .append(checked_trees::CheckedOperatorUseFact {
                expression: later,
                status: checked_trees::CheckedOperatorResolutionStatus::Resolved,
                ..Default::default()
            });
        assert!(!preserves_live_reads(&selected));
        let mut atomic = checked.clone();
        let operand = atomic
            .typed
            .expression_table
            .insert(ExpressionNode::Boolean(false));
        *atomic.typed.expression_table.expression_mut(later) =
            ExpressionNode::Atomic(typed_trees::expression::TableAtomicExpression {
                value: operand,
                result: operand,
                ordering: language_core::atomic::AtomicOrderingPlan::Swap(
                    language_core::atomic::MemoryOrdering::NoOrdering,
                ),
                result_custody: language_core::atomic::AtomicExpressionResultCustody::Scalar,
            });
        assert!(!preserves_live_reads(&atomic));
    }

    #[test]
    fn result_case_observation_rejects_reassociated_fact_and_exit_owners() {
        let source = "data Message { case Empty; case Data(value: u8); }
            machine first() -> Message ensures result in Message::Data; { Message::Data { value: 1 } }
            machine second() -> Message ensures result in Message::Empty; { Message::Empty }";
        let tokens = source_files_to_tokens::Lexer::new(source)
            .tokenize()
            .unwrap();
        let syntax = tokens_to_syntax_trees::parse_syntax_trees(&tokens).unwrap();
        let resolved = syntax_trees_to_symbol_resolved_trees::lower_syntax_trees(&syntax).unwrap();
        let typed =
            symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees(&resolved).unwrap();
        let checked = crate::lower_typed_trees(typed).unwrap();
        let exits = checked
            .facts
            .flow
            .control
            .exits
            .iter()
            .map(|(_, exit)| exit.clone())
            .collect::<Vec<_>>();
        let [first, second] = exits.as_slice() else {
            panic!("two normal exits");
        };
        let first_contract = checked
            .facts
            .proof
            .contract_fact_refs
            .span_or_empty(first.ensures)[0]
            .fact;
        let source_fact = checked.facts.proof.contract_facts.get(first_contract).fact;
        let requirement = checked.facts.semantic.facts.iter().find_map(|(_, fact)| {
            matches!(fact.payload, FactPayload::ContractBooleanExpression { fact, .. } if fact == source_fact).then_some(*fact)
        }).unwrap();
        let FactPayload::ContractBooleanExpression { expression, .. } = requirement.payload else {
            panic!("Boolean requirement");
        };
        let observe = |exit: &FlowExitFact, requirement: &facts::Fact| {
            CaseObservation::for_requirement(
                &checked.typed,
                &checked.facts,
                exit,
                &[],
                requirement,
                None,
            )
            .and_then(|observer| observer.observe(expression))
        };
        assert_eq!(observe(first, &requirement), Some(true));
        let mut changed = first.clone();
        changed.ensures = second.ensures;
        assert_eq!(observe(&changed, &requirement), None);
        assert_eq!(observe(second, &requirement), None);
        changed = first.clone();
        changed.statement_index = changed.statement_index.saturating_add(1);
        assert_eq!(observe(&changed, &requirement), None);
        let mut changed_requirement = requirement;
        let FactPayload::ContractBooleanExpression {
            ref mut expression, ..
        } = changed_requirement.payload
        else {
            panic!("Boolean requirement");
        };
        *expression = exit_return_expression(&checked.typed, first);
        assert_eq!(observe(first, &changed_requirement), None);
    }
}
