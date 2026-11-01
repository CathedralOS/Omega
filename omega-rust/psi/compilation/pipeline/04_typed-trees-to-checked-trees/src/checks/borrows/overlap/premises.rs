//! Integer relations available at the exact compatibility judgment.
//!
//! A premised compatibility derivation consumes the same establishment point
//! as the range checker: authored preconditions and surviving incoming
//! guards. Guard queries are transported back through immutable scalar
//! parameter bindings to the guard's evaluation scope. This direction also
//! handles multiple destination parameters carrying the same source value.
//! Availability and transport are established from the typed program.
//! Premises prove bound ordering, equality or disequality the structural
//! judgment could not. Disequality distinguishes singleton elements without
//! choosing an ordering; it says nothing about overlap of wider windows.
//! When no single premise names both bounds, one ordered premise pair may
//! compose transitively through a shared middle bound (`i < n` with
//! `n <= j` proves `i < j`); `==` legs substitute from either side and `!=`
//! legs compose only with exact equality substitution.
//! Checked predicate calls contribute the relations the shared symbolic body
//! prover establishes on every true return, substituted onto exact actuals.
//! No relation can form a loan, extend a lifetime, or widen access.
//! The arithmetic-domain oracle owns pure ordering composition; this client
//! establishes which exact source premises are available to that oracle.
//! Required domain membership uses the same decomposition with a definition
//! scope: its reserved self binds the immutable membership subject. The
//! definition-owned meaning reader rejects foreign self and selected operators.
//! Call guarantees join the contract checker's shared availability reader at
//! each statement entry. They never enter the stable state-entry premise set.
//! The exact callee contract and immutable captured result survive separately;
//! both must rejoin before a reserved result can license separation.
//! A `requires` row spelled as a transparent proposition application
//! decomposes the instantiated formula under the same meaning gate, with each
//! declared parameter bound to its actual argument's normalized bound;
//! primitive and witness propositions carry no formula and supply nothing.

use checked_trees::{CheckFacts, ContractProofFactKind, FlowStateFact};
use facts::FactContextHandle;
use facts::PlaceRoot;
use facts::PlaceSegment;
use symbols::SymbolHandle;
use typed_trees::domain::DomainDefinition;
use typed_trees::domain::ProofFact;
use typed_trees::expression::{BinaryOperator, ExpressionHandle, ExpressionNode, UnaryOperator};
use typed_trees::machine::Machine;
use typed_trees::signature::StateParameter;
use typed_trees::signature::state_parameter_position;
use typed_trees::state::State;
use validation::CallFrameResolver;
use validation::ImmutableBoundLookup;
use validation::PredicateIntegerOperand;
use validation::canonical_place_from_expression;
use validation::exact_domain_subject_type;
use validation::find_state_in_machine;
use validation::has_builtin_decomposed_guard_meaning;
use validation::has_builtin_domain_decomposed_guard_meaning;
use validation::instantiate_immutable_entry_indexed_read_identity;
use validation::predicate_ordering_consequences;

use super::indexes::{NormalizedBound, normalized_bound};
use crate::checks::contracts::prover::call_guarantees::{self, AvailableGuarantee};
use crate::checks::ranges::incoming_guards::IncomingGuardIndex;
use crate::checks::ranges::requirements::state_requires_facts;

pub(super) use validation::IntegerOrderingRelation as BorrowCompatibilityPremiseRelation;
pub use validation::StatedOrderingPremise;

mod domains;
mod propositions;
mod stored_values;

#[derive(Clone)]
enum PremiseScope<'program> {
    State {
        machine: &'program Machine,
        state: &'program State,
    },
    Domain {
        definition: &'program DomainDefinition,
        subject: NormalizedBound,
    },
    Call {
        guarantee: &'program AvailableGuarantee<'program>,
        result: SymbolHandle,
        /// Whether the bound storage is mutable: premise operands then mint
        /// the storage bound, matching the query coordinate a mutable
        /// selector contributes at this scope.
        result_mutable: bool,
    },
    /// A transparent proposition formula evaluated in the machine scope of
    /// the requires row: `parameters`/`arguments` pair the proposition's
    /// declared value parameters with the application's actual expressions.
    Proposition {
        machine: &'program Machine,
        state: &'program State,
        parameters: &'program [StateParameter],
        arguments: &'program [ExpressionHandle],
    },
}

impl PremiseScope<'_> {
    fn has_builtin_meaning(
        &self,
        program: &dyn ::validation::CheckingInput,
        expression: ExpressionHandle,
    ) -> bool {
        match self {
            Self::Call { guarantee, .. } => guarantee.has_builtin_meaning(program, expression),
            Self::State { machine, state } => {
                has_builtin_decomposed_guard_meaning(program, machine, Some(state), expression)
            }
            Self::Domain { definition, .. } => {
                has_builtin_domain_decomposed_guard_meaning(program, definition, expression)
            }
            Self::Proposition { machine, state, .. } => {
                has_builtin_decomposed_guard_meaning(program, machine, Some(state), expression)
            }
        }
    }

    fn projected_bound(
        &self,
        program: &dyn ::validation::CheckingInput,
        lookup: &ImmutableBoundLookup<'_>,
        expression: ExpressionHandle,
        projection: &[PlaceSegment],
    ) -> Option<NormalizedBound> {
        if projection.is_empty() {
            return self.bound(program, lookup, expression);
        }
        if let Some(base) = self.bound(program, lookup, expression) {
            return super::indexes::append_immutable_projection(program, lookup, base, projection);
        }
        let (expression, mut prefix) = match self {
            Self::State { .. } => (expression, Vec::new()),
            Self::Call { guarantee, .. } => guarantee.actual_projection(program, expression)?,
            Self::Proposition {
                parameters,
                arguments,
                ..
            } => {
                let place = canonical_place_from_expression(program, expression)?;
                let PlaceRoot::Symbol(symbol) = place.root else {
                    return None;
                };
                let position = state_parameter_position(parameters, symbol)?;
                (arguments[position], place.segments)
            }
            Self::Domain { .. } => return None,
        };
        let place = canonical_place_from_expression(program, expression)?;
        let PlaceRoot::Symbol(symbol) = place.root else {
            return None;
        };
        let mut segments = place.segments;
        segments.append(&mut prefix);
        let base = NormalizedBound::Projected { symbol, segments };
        super::indexes::append_immutable_projection(program, lookup, base, projection)
    }

    fn bound(
        &self,
        program: &dyn ::validation::CheckingInput,
        lookup: &ImmutableBoundLookup<'_>,
        expression: ExpressionHandle,
    ) -> Option<NormalizedBound> {
        match self {
            Self::Call {
                guarantee,
                result,
                result_mutable,
            } => {
                if let Some(segments) = guarantee.result_segments(program, expression) {
                    match segments.as_slice() {
                        [] => {
                            if result.is_valid() {
                                Some(if *result_mutable {
                                    NormalizedBound::Storage { symbol: *result }
                                } else {
                                    NormalizedBound::Symbol {
                                        symbol: *result,
                                        offset: 0,
                                    }
                                })
                            } else {
                                // A nested call produces no binding: the
                                // result operand names the call occurrence
                                // itself, so the guarantee is consumable
                                // inside its own statement.
                                guarantee.call_expression().map(|expression| {
                                    NormalizedBound::CallResult {
                                        expression,
                                        segments: Vec::new(),
                                    }
                                })
                            }
                        }
                        // `result.first` — and longer resolved projections
                        // like `result.items[2]` — binds the pinned binding's
                        // projected place; the binding's own pin covers the
                        // projection's root.
                        [_, ..]
                            if segments.iter().all(|segment| {
                                matches!(
                                    segment,
                                    PlaceSegment::Field { .. } | PlaceSegment::FixedIndex { .. }
                                )
                            }) =>
                        {
                            if result.is_valid() {
                                let segments = segments.to_vec();
                                Some(if *result_mutable {
                                    NormalizedBound::StorageProjected {
                                        symbol: *result,
                                        segments,
                                    }
                                } else {
                                    NormalizedBound::Projected {
                                        symbol: *result,
                                        segments,
                                    }
                                })
                            } else {
                                // A nested call's projected result binds the
                                // occurrence at that segment path — the same
                                // intra-statement context the bare-result arm
                                // supplies.
                                guarantee.call_expression().map(|expression| {
                                    NormalizedBound::CallResult {
                                        expression,
                                        segments: segments.to_vec(),
                                    }
                                })
                            }
                        }
                        _ => None,
                    }
                } else {
                    let (actual, remaining) = guarantee.actual_projection(program, expression)?;
                    match remaining.as_slice() {
                        [] => normalized_bound(program, lookup, actual).or_else(|| {
                            // An exclusive-borrow actual hands the callee the
                            // caller's storage: the operand resolves to that
                            // place's post-call contents, not an immutable
                            // value.
                            if !matches!(
                                program.expression_table.expression(actual),
                                ExpressionNode::Borrow(borrow)
                                    if borrow.access.is_exclusive()
                            ) {
                                return None;
                            }
                            let place = canonical_place_from_expression(program, actual)?;
                            match (place.root, place.segments.as_slice()) {
                                (PlaceRoot::Symbol(symbol), []) => {
                                    Some(NormalizedBound::Storage { symbol })
                                }
                                // A projected actual (`&mut self.cut`,
                                // `&mut items[2]`, `&mut self.pivot[2]`)
                                // writes the projected coordinate: the bound
                                // is that coordinate's post-call contents. A
                                // runtime `Index` expression or unresolved
                                // segment is handle identity and stays
                                // unbound.
                                (PlaceRoot::Symbol(symbol), segments @ [_, ..])
                                    if segments.iter().all(|segment| {
                                        matches!(
                                            segment,
                                            PlaceSegment::Field { .. }
                                                | PlaceSegment::FixedIndex { .. }
                                        )
                                    }) =>
                                {
                                    Some(NormalizedBound::StorageProjected {
                                        symbol,
                                        segments: segments.to_vec(),
                                    })
                                }
                                _ => None,
                            }
                        }),
                        [_, ..]
                            if remaining.iter().all(|segment| {
                                matches!(
                                    segment,
                                    PlaceSegment::Field { .. } | PlaceSegment::FixedIndex { .. }
                                )
                            }) =>
                        {
                            match normalized_bound(program, lookup, actual)? {
                                // `value.first` (and deeper projections)
                                // bind the actual's projected place;
                                // non-identity bases stay unboundable.
                                NormalizedBound::Symbol { symbol, offset: 0 } => {
                                    Some(NormalizedBound::Projected {
                                        symbol,
                                        segments: remaining.to_vec(),
                                    })
                                }
                                _ => None,
                            }
                        }
                        _ => None,
                    }
                }
            }
            Self::State { .. } => normalized_bound(program, lookup, expression)
                .or_else(|| projected_immutable_bound(program, lookup, expression)),
            Self::Domain {
                definition,
                subject,
            } => {
                if exact_domain_subject_type(program, definition, expression).is_some() {
                    Some(subject.clone())
                } else if let ExpressionNode::Integer(literal) =
                    program.expression_table.expression(expression)
                {
                    literal.value_i64().map(NormalizedBound::Integer)
                } else {
                    None
                }
            }

            Self::Proposition {
                parameters,
                arguments,
                ..
            } => {
                if let Some(path) = program.expression_table.name_path_node(expression)
                    && program.expression_table.name_path_is_single_member(path)
                    && let Some(index) = state_parameter_position(parameters, path.symbol)
                {
                    return normalized_bound(program, lookup, arguments[index]);
                }
                normalized_bound(program, lookup, expression)
                    .or_else(|| projected_immutable_bound(program, lookup, expression))
                    .and_then(|bound| {
                        propositions::substitute_bound(
                            program, lookup, bound, parameters, arguments,
                        )
                    })
            }
        }
    }
}

/// The bound of one receiver-rooted field projection as a stated premise
/// operand. Only an immutable receiver's frozen projection identity can
/// claim a stated relation; a mutable receiver's projected storage would
/// assert a premise about its current contents, which stated evidence has
/// no version pin for — the same contract `normalized_bound` already
/// applies to whole mutable names.
fn projected_immutable_bound(
    program: &dyn ::validation::CheckingInput,
    lookup: &ImmutableBoundLookup<'_>,
    expression: ExpressionHandle,
) -> Option<NormalizedBound> {
    match super::indexes::projected_bound(program, lookup, expression)? {
        bound @ NormalizedBound::Projected { .. } => Some(bound),
        _ => None,
    }
}

/// Collect the ordering premises available at one formation scope.
///
/// Entry availability and incoming guards are shared with range checking.
/// Required domain predicates additionally bind their own definition scope.
/// Facts that do not decompose into builtin integer comparisons over immutable
/// bounds contribute no premise; mutable storage needs version evidence first.
pub fn stated_ordering_premises<'p>(
    program: &'p dyn ::validation::CheckingInput,
    facts: &CheckFacts,
    machine: &Machine,
    state: &State,
    incoming_guards: &IncomingGuardIndex,
    bound_lookup: &mut Option<ImmutableBoundLookup<'p>>,
) -> Vec<StatedOrderingPremise> {
    // The bound index scans the whole program once; the caller shares one
    // cell across every formation scope this pass visits.
    let mut premises = Vec::new();
    for (_, row) in facts.proof.contract_facts.iter() {
        if row.kind != ContractProofFactKind::Requires || row.inherited_scope.is_some() {
            continue;
        }
        if !state_requires_facts(program, machine, state).any(|candidate| candidate == row.fact) {
            continue;
        }
        let bound_lookup = bound_lookup.get_or_insert_with(|| ImmutableBoundLookup::new(program));
        match program.proof_facts.get(row.fact) {
            ProofFact::Expression(expression) => {
                decompose_premise_expression(
                    program,
                    bound_lookup,
                    PremiseScope::State { machine, state },
                    *expression,
                    false,
                    &None,
                    &mut premises,
                );
            }
            ProofFact::Membership(membership) => {
                domains::append_membership_premises(
                    program,
                    bound_lookup,
                    machine,
                    state,
                    membership,
                    &mut premises,
                );
            }
            ProofFact::Proposition(application) => {
                propositions::append_proposition_premises(
                    program,
                    bound_lookup,
                    machine,
                    state,
                    application,
                    &mut premises,
                );
            }
        }
    }
    for guard in incoming_guards
        .for_machine(machine.symbol)
        .iter()
        .filter(|guard| guard.applies_at(state.symbol))
    {
        let Some(evaluation_state) = program
            .machine_states(machine)
            .iter()
            .find(|candidate| candidate.symbol == guard.evaluation_state())
        else {
            continue;
        };
        let bindings = Some(
            program
                .state_parameters(state)
                .iter()
                .filter_map(|parameter| {
                    guard
                        .immutable_argument_symbol_for_parameter(parameter.symbol)
                        .map(|argument| (parameter.symbol, argument))
                })
                .collect(),
        );
        decompose_premise_expression(
            program,
            bound_lookup.get_or_insert_with(|| ImmutableBoundLookup::new(program)),
            PremiseScope::State {
                machine,
                state: evaluation_state,
            },
            guard.guard(),
            guard.is_negated(),
            &bindings,
            &mut premises,
        );
    }
    premises
}

/// Extend entry premises with completed immutable values and call guarantees
/// at the exact statement entry. Later calls and invalidated captures cannot
/// license earlier loan formation or mutation.
pub(in crate::checks::borrows) fn append_statement_premises<'p>(
    program: &'p dyn ::validation::CheckingInput,
    facts: &CheckFacts,
    state_flow: &FlowStateFact,
    statement: usize,
    call_frames: Option<&CallFrameResolver<'_>>,
    premises: &mut Vec<StatedOrderingPremise>,
    bound_lookup: &mut Option<ImmutableBoundLookup<'p>>,
) {
    let Some(state) =
        find_state_in_machine(program, state_flow.machine_symbol, state_flow.state_symbol)
    else {
        return;
    };
    let Some(statement_flow) = facts
        .flow
        .control
        .statements
        .span_or_empty(state_flow.statements)
        .iter()
        .find(|row| row.statement_index == statement)
    else {
        return;
    };
    // The bound index scans the whole program once; the caller shares one
    // cell across every statement entry this pass visits.
    let contexts: Vec<_> = facts
        .flow
        .semantic_constraint_contexts(statement_flow.entry_constraints)
        .collect();
    stored_values::append(program, facts, state, statement, &contexts, premises);
    let Some(frames) = call_frames else {
        return;
    };
    for guarantee in call_guarantees::available(
        program,
        facts,
        state_flow,
        call_guarantees::GuaranteeUse::StatementEntry(statement),
        &contexts,
        frames,
    ) {
        let (result, result_mutable) = guarantee
            .result_binding(program, &facts.semantic, &contexts, state)
            .unwrap_or_default();
        decompose_premise_expression(
            program,
            bound_lookup.get_or_insert_with(|| ImmutableBoundLookup::new(program)),
            PremiseScope::Call {
                guarantee: &guarantee,
                result,
                result_mutable,
            },
            guarantee.expression,
            false,
            &None,
            premises,
        );
    }
}

/// Captured paths may use immutable result facts established during operand
/// evaluation, never facts about the current contents of mutable storage.
/// Bind results to their invocation even when a local originally received
/// them: reassignment cannot retarget the captured result occurrence.
pub(super) fn captured_call_premises(
    program: &dyn ::validation::CheckingInput,
    facts: &CheckFacts,
    state: &FlowStateFact,
    statement: usize,
    contexts: &[FactContextHandle],
    frames: &CallFrameResolver<'_>,
    lookup: &ImmutableBoundLookup<'_>,
) -> Vec<StatedOrderingPremise> {
    let mut premises = Vec::new();
    for guarantee in call_guarantees::available(
        program,
        facts,
        state,
        call_guarantees::GuaranteeUse::EvaluatedContext(statement),
        contexts,
        frames,
    ) {
        decompose_premise_expression(
            program,
            lookup,
            PremiseScope::Call {
                guarantee: &guarantee,
                result: Default::default(),
                result_mutable: false,
            },
            guarantee.expression,
            false,
            &None,
            &mut premises,
        );
    }
    let captured = |bound: &NormalizedBound| {
        !matches!(
            bound,
            NormalizedBound::Storage { .. } | NormalizedBound::StorageProjected { .. }
        )
    };
    premises.retain(|premise| captured(&premise.left) && captured(&premise.right));
    premises
}

/// Decompose one `requires` expression into atomic ordering premises.
///
/// Conjunctions contribute each conjunct independently; `>`/`>=` normalize to
/// the flipped `<`/`<=` premise so stored relations stay canonical. Every
/// node is gated by its scope's builtin-meaning reader, so an overloaded
/// comparison cannot masquerade as integer ordering.
fn decompose_premise_expression(
    program: &dyn ::validation::CheckingInput,
    lookup: &ImmutableBoundLookup<'_>,
    scope: PremiseScope<'_>,
    expression: ExpressionHandle,
    negated: bool,
    parameter_arguments: &Option<Vec<(SymbolHandle, SymbolHandle)>>,
    premises: &mut Vec<StatedOrderingPremise>,
) {
    if !expression.is_valid() {
        return;
    }
    let node = program.expression_table.expression(expression);
    if matches!(node, ExpressionNode::Call(_)) {
        // Positive predicate truth is available here, but only its checked
        // body's consequences may justify compatibility. Keep exact actual
        // normalization in this formation scope; the shared prover's private
        // formal atoms cannot become caller loan or storage identities.
        if !negated {
            for consequence in predicate_ordering_consequences(program, expression) {
                let operand_bound = |operand| match operand {
                    PredicateIntegerOperand::Argument(subject) => scope
                        .has_builtin_meaning(program, subject.expression)
                        .then(|| {
                            scope.projected_bound(
                                program,
                                lookup,
                                subject.expression,
                                &subject.projection,
                            )
                        })
                        .flatten(),
                    PredicateIntegerOperand::Read(read) => match &scope {
                        PremiseScope::State { machine, state } => {
                            instantiate_immutable_entry_indexed_read_identity(
                                program,
                                read.predicate,
                                read.expression,
                                &read.arguments,
                                machine,
                                state,
                            )
                            .map(NormalizedBound::EntryRead)
                        }
                        // Completed/local observations need their own exact
                        // prefix/capture owner, not an entry namespace guess.
                        _ => None,
                    },
                    PredicateIntegerOperand::Integer(value) => {
                        i64::try_from(value).ok().map(NormalizedBound::Integer)
                    }
                };
                let (Some(left), Some(right)) = (
                    operand_bound(consequence.left),
                    operand_bound(consequence.right),
                ) else {
                    continue;
                };
                use BorrowCompatibilityPremiseRelation as Relation;
                let (relation, left, right) = match consequence.operator {
                    BinaryOperator::Less => (Relation::StrictlyBefore, left, right),
                    BinaryOperator::LessOrEqual => (Relation::LessOrEqual, left, right),
                    BinaryOperator::Greater => (Relation::StrictlyBefore, right, left),
                    BinaryOperator::GreaterOrEqual => (Relation::LessOrEqual, right, left),
                    BinaryOperator::Equal => (Relation::Equal, left, right),
                    BinaryOperator::NotEqual => (Relation::NotEqual, left, right),
                    _ => continue,
                };
                premises.push(StatedOrderingPremise {
                    relation,
                    left,
                    right,
                    parameter_arguments: parameter_arguments.clone(),
                });
            }
        }
        return;
    }
    if !scope.has_builtin_meaning(program, expression) {
        return;
    }
    if let ExpressionNode::Unary(unary) = node
        && unary.operator == UnaryOperator::LogicalNot
    {
        decompose_premise_expression(
            program,
            lookup,
            scope,
            unary.operand,
            !negated,
            parameter_arguments,
            premises,
        );
        return;
    }
    let ExpressionNode::Binary(binary) = node else {
        return;
    };
    use BorrowCompatibilityPremiseRelation as Relation;
    if matches!(
        binary.operator,
        BinaryOperator::Equal | BinaryOperator::NotEqual
    ) {
        let boolean_operand = match (
            program.expression_table.expression(binary.left),
            program.expression_table.expression(binary.right),
        ) {
            (ExpressionNode::Boolean(value), _) => Some((binary.right, *value)),
            (_, ExpressionNode::Boolean(value)) => Some((binary.left, *value)),
            _ => None,
        };
        if let Some((operand, value)) = boolean_operand {
            let equality_negated = negated ^ (binary.operator == BinaryOperator::NotEqual);
            decompose_premise_expression(
                program,
                lookup,
                scope,
                operand,
                equality_negated == value,
                parameter_arguments,
                premises,
            );
            return;
        }
    }
    let (relation, left, right) = match (binary.operator, negated) {
        (BinaryOperator::And, false) | (BinaryOperator::Or, true) => {
            decompose_premise_expression(
                program,
                lookup,
                scope.clone(),
                binary.left,
                negated,
                parameter_arguments,
                premises,
            );
            decompose_premise_expression(
                program,
                lookup,
                scope,
                binary.right,
                negated,
                parameter_arguments,
                premises,
            );
            return;
        }
        (BinaryOperator::Less, false) | (BinaryOperator::GreaterOrEqual, true) => {
            (Relation::StrictlyBefore, binary.left, binary.right)
        }
        (BinaryOperator::LessOrEqual, false) | (BinaryOperator::Greater, true) => {
            (Relation::LessOrEqual, binary.left, binary.right)
        }
        (BinaryOperator::Greater, false) | (BinaryOperator::LessOrEqual, true) => {
            (Relation::StrictlyBefore, binary.right, binary.left)
        }
        (BinaryOperator::GreaterOrEqual, false) | (BinaryOperator::Less, true) => {
            (Relation::LessOrEqual, binary.right, binary.left)
        }
        (BinaryOperator::Equal, false) | (BinaryOperator::NotEqual, true) => {
            (Relation::Equal, binary.left, binary.right)
        }
        (BinaryOperator::NotEqual, false) | (BinaryOperator::Equal, true) => {
            (Relation::NotEqual, binary.left, binary.right)
        }
        _ => return,
    };
    let (Some(left), Some(right)) = (
        scope.bound(program, lookup, left),
        scope.bound(program, lookup, right),
    ) else {
        return;
    };
    premises.push(StatedOrderingPremise {
        relation,
        left,
        right,
        parameter_arguments: parameter_arguments.clone(),
    });
}
