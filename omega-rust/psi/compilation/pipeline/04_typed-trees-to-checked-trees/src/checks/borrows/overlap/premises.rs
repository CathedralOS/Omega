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
//! legs never compose.
//! No relation can form a loan, extend a lifetime, or widen access.
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

use checked_trees::ContractProofFactKind;
use typed_trees::expression::{BinaryOperator, ExpressionHandle, ExpressionNode, UnaryOperator};
use typed_trees::machine::Machine;
use typed_trees::state::State;

use super::indexes::{NormalizedBound, normalized_bound};
use crate::checks::contracts::prover::call_guarantees::{self, AvailableGuarantee};
use crate::checks::ranges::incoming_guards::IncomingGuardIndex;
use crate::checks::ranges::requirements::state_requires_facts;

/// Integer ordering relations; disequality separates points, not windows.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum BorrowCompatibilityPremiseRelation {
    LessOrEqual,
    StrictlyBefore,
    Equal,
    NotEqual,
}

mod domains;
mod propositions;

#[derive(Clone)]
enum PremiseScope<'program> {
    State {
        machine: &'program Machine,
        state: &'program State,
    },
    Domain {
        definition: &'program typed_trees::domain::DomainDefinition,
        subject: NormalizedBound,
    },
    Call {
        guarantee: &'program AvailableGuarantee<'program>,
        result: symbols::SymbolHandle,
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
        parameters: &'program [typed_trees::signature::StateParameter],
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
            Self::State { machine, state } => validation::has_builtin_decomposed_guard_meaning(
                program,
                machine,
                Some(state),
                expression,
            ),
            Self::Domain { definition, .. } => {
                validation::has_builtin_domain_decomposed_guard_meaning(
                    program, definition, expression,
                )
            }
            Self::Proposition { machine, state, .. } => {
                validation::has_builtin_decomposed_guard_meaning(
                    program,
                    machine,
                    Some(state),
                    expression,
                )
            }
        }
    }

    fn bound(
        &self,
        program: &dyn ::validation::CheckingInput,
        lookup: &validation::ImmutableBoundLookup<'_>,
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
                                    facts::PlaceSegment::Field { .. }
                                        | facts::PlaceSegment::FixedIndex { .. }
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
                            let place =
                                validation::canonical_place_from_expression(program, actual)?;
                            match (place.root, place.segments.as_slice()) {
                                (facts::PlaceRoot::Symbol(symbol), []) => {
                                    Some(NormalizedBound::Storage { symbol })
                                }
                                // A projected actual (`&mut self.cut`,
                                // `&mut items[2]`, `&mut self.pivot[2]`)
                                // writes the projected coordinate: the bound
                                // is that coordinate's post-call contents. A
                                // runtime `Index` expression or unresolved
                                // segment is handle identity and stays
                                // unbound.
                                (facts::PlaceRoot::Symbol(symbol), segments @ [_, ..])
                                    if segments.iter().all(|segment| {
                                        matches!(
                                            segment,
                                            facts::PlaceSegment::Field { .. }
                                                | facts::PlaceSegment::FixedIndex { .. }
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
                                    facts::PlaceSegment::Field { .. }
                                        | facts::PlaceSegment::FixedIndex { .. }
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
                if validation::exact_domain_subject_type(program, definition, expression).is_some()
                {
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
                    && program
                        .expression_table
                        .name_path_members(path.members)
                        .len()
                        == 1
                    && path.head_symbol == path.symbol
                    && let Some(index) =
                        typed_trees::signature::state_parameter_position(parameters, path.symbol)
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
    lookup: &validation::ImmutableBoundLookup<'_>,
    expression: ExpressionHandle,
) -> Option<NormalizedBound> {
    match super::indexes::projected_bound(program, lookup, expression)? {
        bound @ NormalizedBound::Projected { .. } => Some(bound),
        _ => None,
    }
}

/// Normalized immutable relation and its reconstructed query transport.
/// Transport is local checking state, never serialized proof authority.
#[derive(Debug, Clone)]
pub struct StatedOrderingPremise {
    relation: BorrowCompatibilityPremiseRelation,
    left: NormalizedBound,
    right: NormalizedBound,
    parameter_arguments: Option<Vec<(symbols::SymbolHandle, symbols::SymbolHandle)>>,
}

impl StatedOrderingPremise {
    /// The caller has established equality of the detached values at their
    /// actual evaluation epochs, not equality of current storage coordinates.
    pub(super) fn captured_equality(left: NormalizedBound, right: NormalizedBound) -> Self {
        Self {
            relation: BorrowCompatibilityPremiseRelation::Equal,
            left,
            right,
            parameter_arguments: None,
        }
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
    facts: &checked_trees::CheckFacts,
    machine: &Machine,
    state: &State,
    incoming_guards: &IncomingGuardIndex,
    bound_lookup: &mut Option<validation::ImmutableBoundLookup<'p>>,
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
        let bound_lookup =
            bound_lookup.get_or_insert_with(|| validation::ImmutableBoundLookup::new(program));
        match program.proof_facts.get(row.fact) {
            typed_trees::domain::ProofFact::Expression(expression) => {
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
            typed_trees::domain::ProofFact::Membership(membership) => {
                domains::append_membership_premises(
                    program,
                    bound_lookup,
                    machine,
                    state,
                    membership,
                    &mut premises,
                );
            }
            typed_trees::domain::ProofFact::Proposition(application) => {
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
            bound_lookup.get_or_insert_with(|| validation::ImmutableBoundLookup::new(program)),
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

/// Extend entry premises at the exact statement entry. Later calls and
/// invalidated captures cannot license earlier loan formation or mutation.
pub(in crate::checks::borrows) fn append_call_premises<'p>(
    program: &'p dyn ::validation::CheckingInput,
    facts: &checked_trees::CheckFacts,
    state_flow: &checked_trees::FlowStateFact,
    statement: usize,
    call_frames: Option<&validation::CallFrameResolver<'_>>,
    premises: &mut Vec<StatedOrderingPremise>,
    bound_lookup: &mut Option<validation::ImmutableBoundLookup<'p>>,
) {
    let Some(frames) = call_frames else {
        return;
    };
    let Some(state) = validation::find_state_in_machine(
        program,
        state_flow.machine_symbol,
        state_flow.state_symbol,
    ) else {
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
            bound_lookup.get_or_insert_with(|| validation::ImmutableBoundLookup::new(program)),
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
    facts: &checked_trees::CheckFacts,
    state: &checked_trees::FlowStateFact,
    statement: usize,
    contexts: &[facts::FactContextHandle],
    frames: &validation::CallFrameResolver<'_>,
    lookup: &validation::ImmutableBoundLookup<'_>,
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
    lookup: &validation::ImmutableBoundLookup<'_>,
    scope: PremiseScope<'_>,
    expression: ExpressionHandle,
    negated: bool,
    parameter_arguments: &Option<Vec<(symbols::SymbolHandle, symbols::SymbolHandle)>>,
    premises: &mut Vec<StatedOrderingPremise>,
) {
    if !expression.is_valid() || !scope.has_builtin_meaning(program, expression) {
        return;
    }
    let node = program.expression_table.expression(expression);
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

/// Whether `premise` proves `left <query> right`.
///
/// The query shifts against the premise's endpoints only within one symbol's
/// constant-offset line or the integer line; unrelated bound pairs stay
/// unproven. Equality and disequality are symmetric, so their endpoints are
/// consulted in both orientations.
pub fn premise_proves(
    premise: &StatedOrderingPremise,
    left: NormalizedBound,
    query: BorrowCompatibilityPremiseRelation,
    right: NormalizedBound,
) -> bool {
    let (Some(left), Some(right)) = (
        transport_query_bound(premise, left),
        transport_query_bound(premise, right),
    ) else {
        return false;
    };
    premise_orientation_proves(
        premise.left.clone(),
        premise.relation,
        premise.right.clone(),
        left.clone(),
        query,
        right.clone(),
    ) || (matches!(
        premise.relation,
        BorrowCompatibilityPremiseRelation::Equal | BorrowCompatibilityPremiseRelation::NotEqual
    ) && premise_orientation_proves(
        premise.right.clone(),
        premise.relation,
        premise.left.clone(),
        left,
        query,
        right,
    ))
}

fn transport_query_bound(
    premise: &StatedOrderingPremise,
    bound: NormalizedBound,
) -> Option<NormalizedBound> {
    let Some(bindings) = &premise.parameter_arguments else {
        return Some(bound);
    };
    let argument = |symbol| {
        bindings
            .iter()
            .find_map(|(parameter, argument)| (*parameter == symbol).then_some(*argument))
    };
    Some(match bound {
        NormalizedBound::Integer(_) => bound,
        NormalizedBound::Symbol { symbol, offset } => NormalizedBound::Symbol {
            symbol: argument(symbol)?,
            offset,
        },
        NormalizedBound::Storage { symbol } => NormalizedBound::Storage {
            symbol: argument(symbol)?,
        },
        NormalizedBound::Projected { symbol, segments } => NormalizedBound::Projected {
            symbol: argument(symbol)?,
            segments,
        },
        NormalizedBound::StorageProjected { symbol, segments } => {
            NormalizedBound::StorageProjected {
                symbol: argument(symbol)?,
                segments,
            }
        }
        // A call-occurrence bound carries no parameter symbol to
        // transport; the occurrence identity crosses scopes unchanged.
        NormalizedBound::CallResult { .. } => bound,
        NormalizedBound::SymbolSum {
            first,
            second,
            offset,
        } => {
            let first = argument(first)?;
            let second = argument(second)?;
            // Two aliases of one argument mean coefficient two, not one.
            // That coefficient is outside the current normalized vocabulary.
            if first == second {
                return None;
            }
            let (first, second) = if (first.arena_index(), first.generation())
                < (second.arena_index(), second.generation())
            {
                (first, second)
            } else {
                (second, first)
            };
            NormalizedBound::SymbolSum {
                first,
                second,
                offset,
            }
        }
    })
}

/// Whether `premise_left <premise> premise_right` proves
/// `left <query> right`, with `left = premise_left + d1` and
/// `right = premise_right + d2` under the constant-offset algebra.
fn premise_orientation_proves(
    premise_left: NormalizedBound,
    premise: BorrowCompatibilityPremiseRelation,
    premise_right: NormalizedBound,
    left: NormalizedBound,
    query: BorrowCompatibilityPremiseRelation,
    right: NormalizedBound,
) -> bool {
    let (Some(left_shift), Some(right_shift)) = (
        bound_shift(left, premise_left),
        bound_shift(right, premise_right),
    ) else {
        return false;
    };
    use BorrowCompatibilityPremiseRelation as Relation;
    match (premise, query) {
        // `L <= R` gives `L + d1 <= R + d2` when `d1 <= d2`, and the strict
        // `L + d1 < R + d2` when `d1 < d2`. One direction carries no equality.
        (Relation::LessOrEqual, Relation::LessOrEqual) => left_shift <= right_shift,
        (Relation::LessOrEqual, Relation::StrictlyBefore) => left_shift < right_shift,
        (Relation::LessOrEqual, Relation::Equal) => false,
        (Relation::LessOrEqual, Relation::NotEqual) => left_shift < right_shift,
        // `L < R` is `L + 1 <= R`, so `L + d1 <= R + d2` holds when
        // `d1 <= d2 + 1`, and `L + d1 < R + d2` already when `d1 <= d2`.
        (Relation::StrictlyBefore, Relation::LessOrEqual) => right_shift
            .checked_add(1)
            .is_none_or(|bound| left_shift <= bound),
        (Relation::StrictlyBefore, Relation::StrictlyBefore) => left_shift <= right_shift,
        (Relation::StrictlyBefore, Relation::Equal) => false,
        (Relation::StrictlyBefore, Relation::NotEqual) => left_shift <= right_shift,
        // `L == R` shifts either endpoint by the same constant in every
        // comparison direction.
        (Relation::Equal, Relation::LessOrEqual) => left_shift <= right_shift,
        (Relation::Equal, Relation::StrictlyBefore) => left_shift < right_shift,
        (Relation::Equal, Relation::Equal) => left_shift == right_shift,
        (Relation::Equal, Relation::NotEqual) => left_shift != right_shift,
        // Translation by the same mathematical integer preserves inequality.
        // Unequal shifts could collapse two distinct points onto one; `!=`
        // never determines which point precedes the other.
        (Relation::NotEqual, Relation::NotEqual) => left_shift == right_shift,
        (Relation::NotEqual, _) => false,
    }
}

/// Whether `first` and `second` transitively prove `left <query> right`
/// through one shared middle bound: `first` supplies `a r1 m1`, `second`
/// supplies `m2 r2 b`, and the pair composes when `m1 <= m2` on a common term
/// line. A strictly separating middle (`m1 < m2`) makes the composed relation
/// strict; at an equal middle the relation is strict when either leg is
/// strict and loose when both are loose, with `==` absorbing a leg outright.
/// `!=` legs never compose: a distinct middle could reorder or collapse the
/// endpoints. Each query endpoint transports through its own premise's
/// parameter bindings, so a guard-scope premise may chain with a state-scope
/// one. The composed outer endpoints still participate in the ordinary
/// constant-offset algebra, so `i < n && n <= j` proves `i < j + 1`.
pub fn premise_chain_proves(
    first: &StatedOrderingPremise,
    second: &StatedOrderingPremise,
    left: NormalizedBound,
    query: BorrowCompatibilityPremiseRelation,
    right: NormalizedBound,
) -> bool {
    first_legs(first).into_iter().flatten().any(|first_leg| {
        second_legs(second).into_iter().flatten().any(|second_leg| {
            chained_legs_prove(
                first_leg.clone(),
                second_leg,
                first,
                second,
                left.clone(),
                query,
                right.clone(),
            )
        })
    })
}

/// One usable orientation of a premise in a chain. The first leg is
/// `outer <relation> middle`; the second is `middle <relation> outer`.
#[derive(Clone)]
struct ChainLeg {
    outer: NormalizedBound,
    relation: BorrowCompatibilityPremiseRelation,
    middle: NormalizedBound,
}

/// Orientations of `premise` usable as the left leg of a chain: the shared
/// middle is its right endpoint. `==` contributes both orientations.
fn legs(
    premise: &StatedOrderingPremise,
    outer: &NormalizedBound,
    middle: &NormalizedBound,
) -> [Option<ChainLeg>; 2] {
    let stored = Some(ChainLeg {
        outer: outer.clone(),
        relation: premise.relation,
        middle: middle.clone(),
    });
    let flipped =
        matches!(premise.relation, BorrowCompatibilityPremiseRelation::Equal).then_some(ChainLeg {
            outer: middle.clone(),
            relation: premise.relation,
            middle: outer.clone(),
        });
    [stored, flipped]
}

fn first_legs(premise: &StatedOrderingPremise) -> [Option<ChainLeg>; 2] {
    legs(premise, &premise.left, &premise.right)
}

fn second_legs(premise: &StatedOrderingPremise) -> [Option<ChainLeg>; 2] {
    legs(premise, &premise.right, &premise.left)
}

fn chained_legs_prove(
    first_leg: ChainLeg,
    second_leg: ChainLeg,
    first: &StatedOrderingPremise,
    second: &StatedOrderingPremise,
    left: NormalizedBound,
    query: BorrowCompatibilityPremiseRelation,
    right: NormalizedBound,
) -> bool {
    use BorrowCompatibilityPremiseRelation as Relation;
    if matches!(
        (first_leg.relation, second_leg.relation),
        (Relation::NotEqual, _) | (_, Relation::NotEqual)
    ) {
        return false;
    }
    // The legs meet only when the first middle provably precedes or equals
    // the second on one shared term line.
    let Some(middle_gap) = bound_shift(second_leg.middle, first_leg.middle) else {
        return false;
    };
    if middle_gap < 0 {
        return false;
    }
    let relation = if middle_gap > 0 {
        Relation::StrictlyBefore
    } else {
        match (first_leg.relation, second_leg.relation) {
            (Relation::Equal, relation) | (relation, Relation::Equal) => relation,
            (Relation::StrictlyBefore, _) | (_, Relation::StrictlyBefore) => {
                Relation::StrictlyBefore
            }
            (Relation::LessOrEqual, Relation::LessOrEqual) => Relation::LessOrEqual,
            _ => return false,
        }
    };
    let (Some(left), Some(right)) = (
        transport_query_bound(first, left),
        transport_query_bound(second, right),
    ) else {
        return false;
    };
    premise_orientation_proves(
        first_leg.outer,
        relation,
        second_leg.outer,
        left,
        query,
        right,
    )
}

/// `value - base` when both bounds sit on one symbol's offset line, on one
/// canonical two-symbol sum's offset line, or both are integers. Distinct
/// term sets stay unrelated, never negative evidence.
fn bound_shift(value: NormalizedBound, base: NormalizedBound) -> Option<i64> {
    match (value, base) {
        (NormalizedBound::Integer(value), NormalizedBound::Integer(base)) => {
            value.checked_sub(base)
        }
        (
            NormalizedBound::Symbol {
                symbol: value_symbol,
                offset: value_offset,
            },
            NormalizedBound::Symbol {
                symbol: base_symbol,
                offset: base_offset,
            },
        ) if value_symbol == base_symbol => value_offset.checked_sub(base_offset),
        // Two storage rows in one premise evaluation name the same pinned
        // occurrence: the minting binding verified the storage's version at
        // this scope, so the rows share one zero-offset line.
        (
            NormalizedBound::Storage {
                symbol: value_symbol,
            },
            NormalizedBound::Storage {
                symbol: base_symbol,
            },
        ) if value_symbol == base_symbol => Some(0),
        // One projected member place of the same symbol sits on the
        // zero-offset line of that projection: `pair.first >= 2` shifts the
        // query's own `pair.first` by zero.
        (
            NormalizedBound::Projected {
                symbol: value_symbol,
                segments: value_segments,
            },
            NormalizedBound::Projected {
                symbol: base_symbol,
                segments: base_segments,
            },
        ) if value_symbol == base_symbol && value_segments == base_segments => Some(0),
        (
            NormalizedBound::StorageProjected {
                symbol: value_symbol,
                segments: value_segments,
            },
            NormalizedBound::StorageProjected {
                symbol: base_symbol,
                segments: base_segments,
            },
        ) if value_symbol == base_symbol && value_segments == base_segments => Some(0),
        (
            NormalizedBound::SymbolSum {
                first: value_first,
                second: value_second,
                offset: value_offset,
            },
            NormalizedBound::SymbolSum {
                first: base_first,
                second: base_second,
                offset: base_offset,
            },
        ) if value_first == base_first && value_second == base_second => {
            value_offset.checked_sub(base_offset)
        }
        // The same call occurrence at the same projected path names one
        // produced value: a premise minted at the call sits on the query's
        // zero-offset line.
        (
            NormalizedBound::CallResult {
                expression: value_expression,
                segments: value_segments,
            },
            NormalizedBound::CallResult {
                expression: base_expression,
                segments: base_segments,
            },
        ) if value_expression == base_expression && value_segments == base_segments => Some(0),
        _ => None,
    }
}
