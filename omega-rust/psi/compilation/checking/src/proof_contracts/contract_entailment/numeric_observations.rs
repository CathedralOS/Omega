//! Batch numeric observations from the existing normal-value path owner.
//!
//! Field syntax is not a value identity. The completed operand owns its read
//! epoch, including reads before a later sibling call writes the same storage.
//! This owner's semantic keys preserve completed values across query prefixes;
//! disagreeing normal paths publish no observation. Completed result equations
//! reach scalar copies only through actual reads at the current write epoch,
//! never through a saved expression handle reinterpreted as current storage.
//! Descriptor measures remain with the collection observation owner.
//!
//! Consumers request the facts they use. Arrival/value transfer consumes
//! orderings and result associations; the structural checker consumes captured
//! scalar reads. Their demands compose in this one traversal and remain part
//! of its memo identity, so a partial request cannot supply another consumer's
//! omitted facts. Unknown prefixes remain uncacheable for every demand.

use super::structural_judgment::{StructuralJudge, StructuralTerm};
use super::structural_prefix::{
    PrefixObservation, StructuralPrefixSite, StructuralPrefixVisitor, observe_runtime_state_prefix,
    observe_runtime_state_prefixes,
};
use crate::ScopedArithmeticValue;
use crate::proof_contracts::arithmetic_domains::{
    CopiedIntegerValue, ScalarOperand, scalar_operand,
};
use symbols::SymbolHandle;
use typed_trees::expression::{BinaryOperator, ExpressionHandle};
use typed_trees::types::TypeReferenceHandle;
use typed_trees::{machine::Machine, state::State};

/// One actual declared-field read, not a current-place or initializer lookup.
#[derive(Clone, PartialEq)]
pub struct CapturedScalarObservation {
    pub expression: ExpressionHandle,
    pub value: ScopedArithmeticValue,
}

/// Captured order and frame evidence before one operation's result exists.
/// These rows belong to this statement's proof environment, never a state edge.
#[derive(Clone, Debug, PartialEq)]
pub struct CapturedIntegerOperation {
    pub state: SymbolHandle,
    pub expression: ExpressionHandle,
    pub minimum_difference: Option<i64>,
    /// Existing complete frame proves operand evaluation cannot revise storage.
    pub preserves_storage: bool,
}

/// An equality of one completed result and an actually read current place.
/// The result expression is scoped to this exact statement invocation; only
/// the destination binding introduced after completion may transport the fact.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct CapturedScalarAssociation {
    pub state: SymbolHandle,
    pub expression: ExpressionHandle,
    pub current: ScalarOperand,
}

#[derive(Clone, Default, PartialEq)]
pub struct CapturedNumericObservations {
    pub scalar_values: Vec<CapturedScalarObservation>,
    pub integer_operations: Vec<CapturedIntegerOperation>,
    pub(crate) scalar_associations: Vec<CapturedScalarAssociation>,
}

/// Independent outputs of one exact normal-prefix query. Selecting an output
/// does not change its proof, effect or path-joining rules.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct NumericObservationDemand {
    pub scalar_values: bool,
    pub integer_operations: bool,
    pub scalar_associations: bool,
}

type NumericSite = (
    SymbolHandle,
    SymbolHandle,
    usize,
    bool,
    NumericObservationDemand,
);
/// Capture the finite field-read roster once at this executed prefix. The
/// normal-value owner supplies source ordering, branch joins and call effects;
/// this adapter neither evaluates a second operand language nor unfolds callees.
pub fn captured_numeric_observations(
    program: &dyn crate::CheckingInput,
    machine: &Machine,
    state: &State,
    statement_index: usize,
    in_transition_target: bool,
    demand: NumericObservationDemand,
) -> CapturedNumericObservations {
    let memos = crate::frozen_program::frozen_program_memos(program).filter(|_| {
        program
            .machine(machine.symbol)
            .is_some_and(|stored| std::ptr::eq(stored, machine))
            && program
                .state_in_machine(machine, state.symbol)
                .is_some_and(|stored| std::ptr::eq(stored, state))
    });
    let site: NumericSite = (
        machine.symbol,
        state.symbol,
        statement_index,
        in_transition_target,
        demand,
    );
    if let Some(memos) = memos
        && let Ok(completed) = memos.completed_numeric_observations.lock()
        && let Some(observations) = completed.get(&site)
    {
        return observations.clone();
    }
    let statement_count = program
        .statement_table
        .statements(state.statement_nodes)
        .len();
    if let Some(memos) = memos
        && !in_transition_target
        && !demand.scalar_values
        && statement_index <= statement_count
    {
        let batch = collect_state_numeric_observations_batch(program, machine, state, demand);
        let result = batch.get(statement_index).cloned().unwrap_or_default();
        if let Ok(mut completed) = memos.completed_numeric_observations.lock() {
            for (index, observations) in batch.into_iter().enumerate() {
                completed.insert(
                    (machine.symbol, state.symbol, index, false, demand),
                    observations,
                );
            }
        }
        return result;
    }
    // This query's callers are checking orchestration and scalar lowering;
    // the normal-value owner never re-enters either adapter. Optional memos
    // change reuse only: one-shot views retain their original selections.
    let (observations, _complete) = collect_numeric_observations(
        program,
        machine,
        state,
        statement_index,
        in_transition_target,
        demand,
    );
    if let Some(memos) = memos
        && let Ok(mut completed) = memos.completed_numeric_observations.lock()
    {
        completed.insert(site, observations.clone());
    }
    observations
}

fn finalize_visitor_observations(
    state: SymbolHandle,
    demand: NumericObservationDemand,
    visitor: Observations,
) -> CapturedNumericObservations {
    let scalar_values = if demand.scalar_values {
        visitor
            .values
            .into_iter()
            .filter_map(|(expression, value)| {
                Some(CapturedScalarObservation {
                    expression,
                    value: value?,
                })
            })
            .collect()
    } else {
        Vec::new()
    };
    let integer_operations = visitor
        .orderings
        .into_iter()
        .map(
            |(expression, minimum, preserves_storage)| CapturedIntegerOperation {
                state,
                expression,
                minimum_difference: minimum,
                preserves_storage,
            },
        )
        .collect();
    let scalar_associations = visitor
        .associations
        .into_iter()
        .flat_map(|(expression, places)| {
            places
                .into_iter()
                .map(move |current| CapturedScalarAssociation {
                    state,
                    expression,
                    current,
                })
        })
        .collect();
    CapturedNumericObservations {
        scalar_values,
        integer_operations,
        scalar_associations,
    }
}

fn collect_state_numeric_observations_batch(
    program: &dyn crate::CheckingInput,
    machine: &Machine,
    state: &State,
    demand: NumericObservationDemand,
) -> Vec<CapturedNumericObservations> {
    let statements = program.statement_table.statements(state.statement_nodes);
    let total_sites = statements.len() + 1;
    let needs_relations = demand.integer_operations || demand.scalar_associations;
    let mut statement_candidates = vec![Vec::new(); statements.len()];
    let mut statement_operations = vec![Vec::new(); statements.len()];
    let mut statement_results = vec![Vec::new(); statements.len()];
    let mut state_candidates = Vec::new();

    let mut statement_has_integer_read = vec![false; statements.len()];
    for (index, statement) in statements.iter().enumerate() {
        if !demand.scalar_values && !needs_relations {
            continue;
        }
        for (_, root) in program.tables.expression_roots(statement) {
            let mut expressions = Vec::new();
            crate::expression_traversal::collect_expression_tree(program, root, &mut expressions);
            for expression in expressions {
                if super::observed_projections::integer_storage_read(
                    program, machine, state, expression,
                )
                .is_some()
                {
                    statement_has_integer_read[index] = true;
                    if demand.scalar_values && !state_candidates.contains(&expression) {
                        state_candidates.push(expression);
                    }
                    if !demand.scalar_values
                        && needs_relations
                        && !statement_candidates[index].contains(&expression)
                    {
                        statement_candidates[index].push(expression);
                    }
                }
                if demand.integer_operations
                    && integer_subtraction(program, machine, state, expression).is_some()
                    && !statement_operations[index].contains(&expression)
                {
                    statement_operations[index].push(expression);
                }
            }
        }
    }

    if demand.scalar_associations && statement_has_integer_read.iter().any(|&has_read| has_read) {
        let mut can_have_prior_read = vec![true; statements.len()];
        if let Ok(Some(execution)) = program
            .statement_table
            .finite_execution_order(state.statement_nodes)
        {
            can_have_prior_read.fill(false);
            let mut seen_read = false;
            for statement_index in execution {
                if statement_has_integer_read[statement_index] {
                    seen_read = true;
                }
                can_have_prior_read[statement_index] = seen_read;
            }
        }
        for (index, statement) in statements.iter().enumerate() {
            if !can_have_prior_read[index] {
                continue;
            }
            let delivered = match statement {
                typed_trees::statement::StatementNode::LocalData(local) => {
                    Some(local.initial_value)
                }
                typed_trees::statement::StatementNode::Assignment(assignment) => {
                    Some(assignment.value)
                }
                _ => None,
            };
            if let Some(delivered) = delivered {
                CopiedIntegerValue::visit_delivered_values(program, delivered, &mut |expression| {
                    if crate::expression_result_type_reference(program, machine, state, expression)
                        .and_then(|reference| program.primitive_type_reference(reference))
                        .is_some_and(|primitive| primitive.is_fixed_integer())
                        && !statement_results[index].contains(&expression)
                    {
                        statement_results[index].push(expression);
                    }
                });
            }
        }
    }

    let mut visitors = Vec::new();
    let mut visitor_indices = Vec::new();
    for statement_index in 0..total_sites {
        let candidates = if demand.scalar_values {
            state_candidates.clone()
        } else if needs_relations && statement_index < statements.len() {
            std::mem::take(&mut statement_candidates[statement_index])
        } else {
            Vec::new()
        };
        let operations = if statement_index < statements.len() {
            std::mem::take(&mut statement_operations[statement_index])
        } else {
            Vec::new()
        };
        let results = if statement_index < statements.len() {
            std::mem::take(&mut statement_results[statement_index])
        } else {
            Vec::new()
        };
        if (!demand.scalar_values || candidates.is_empty())
            && operations.is_empty()
            && results.is_empty()
        {
            continue;
        }
        visitor_indices.push(statement_index);
        visitors.push(Observations {
            state: state.symbol,
            statement_index,
            in_transition_target: false,
            wants_scalar_values: demand.scalar_values,
            candidates,
            values: Vec::new(),
            operations,
            orderings: Vec::new(),
            results,
            associations: Vec::new(),
            complete: true,
        });
    }

    let mut results = vec![CapturedNumericObservations::default(); total_sites];
    if visitors.is_empty() {
        return results;
    }
    let valid = {
        let mut refs: Vec<&mut dyn StructuralPrefixVisitor> = visitors
            .iter_mut()
            .map(|visitor| visitor as &mut dyn StructuralPrefixVisitor)
            .collect();
        observe_runtime_state_prefixes(program, machine, state, &mut refs)
    };
    for ((statement_index, visitor), is_valid) in
        visitor_indices.into_iter().zip(visitors).zip(valid)
    {
        if is_valid {
            results[statement_index] = finalize_visitor_observations(state.symbol, demand, visitor);
        }
    }
    results
}

fn collect_numeric_observations(
    program: &dyn crate::CheckingInput,
    machine: &Machine,
    state: &State,
    statement_index: usize,
    in_transition_target: bool,
    demand: NumericObservationDemand,
) -> (CapturedNumericObservations, bool) {
    let mut candidates = Vec::new();
    let mut operations = Vec::new();
    let mut results = Vec::new();
    let needs_relations = demand.integer_operations || demand.scalar_associations;
    // Association consumers bind only a local/store's delivered value and its
    // supplied record fields. Other descendants still execute, but requesting
    // their results would construct facts no receiving binding can consume.
    if demand.scalar_associations
        && let Some(statement) = program
            .statement_table
            .statements(state.statement_nodes)
            .get(statement_index)
    {
        let delivered = match statement {
            typed_trees::statement::StatementNode::LocalData(local) => Some(local.initial_value),
            typed_trees::statement::StatementNode::Assignment(assignment) => Some(assignment.value),
            _ => None,
        };
        if let Some(delivered) = delivered {
            CopiedIntegerValue::visit_delivered_values(program, delivered, &mut |expression| {
                if crate::expression_result_type_reference(program, machine, state, expression)
                    .and_then(|reference| program.primitive_type_reference(reference))
                    .is_some_and(|primitive| primitive.is_fixed_integer())
                    && !results.contains(&expression)
                {
                    results.push(expression);
                }
            });
        }
    }
    for (index, statement) in program
        .statement_table
        .statements(state.statement_nodes)
        .iter()
        .enumerate()
    {
        if !demand.scalar_values && !(needs_relations && index == statement_index) {
            continue;
        }
        for (_, root) in program.tables.expression_roots(statement) {
            let mut expressions = Vec::new();
            crate::expression_traversal::collect_expression_tree(program, root, &mut expressions);
            for expression in expressions {
                // A relation still demands current-statement reads so the
                // normal-value owner authenticates them at their exact epoch.
                // Only scalar snapshot consumers need the whole state's read
                // roster, including earlier completed observations.
                if super::observed_projections::integer_storage_read(
                    program, machine, state, expression,
                )
                .is_some()
                    && !candidates.contains(&expression)
                {
                    candidates.push(expression);
                }
                if demand.integer_operations
                    && index == statement_index
                    && integer_subtraction(program, machine, state, expression).is_some()
                    && !operations.contains(&expression)
                {
                    operations.push(expression);
                }
            }
        }
    }
    if (!demand.scalar_values || candidates.is_empty())
        && operations.is_empty()
        && results.is_empty()
    {
        return (CapturedNumericObservations::default(), true);
    }
    let mut visitor = Observations {
        state: state.symbol,
        statement_index,
        in_transition_target,
        wants_scalar_values: demand.scalar_values,
        candidates,
        values: Vec::new(),
        operations,
        orderings: Vec::new(),
        results,
        associations: Vec::new(),
        complete: true,
    };
    if observe_runtime_state_prefix(program, machine, state, &mut visitor).is_none() {
        return (CapturedNumericObservations::default(), false);
    }
    let scalar_values = if demand.scalar_values {
        visitor
            .values
            .into_iter()
            .filter_map(|(expression, value)| {
                Some(CapturedScalarObservation {
                    expression,
                    value: value?,
                })
            })
            .collect()
    } else {
        Vec::new()
    };
    let integer_operations = visitor
        .orderings
        .into_iter()
        .map(
            |(expression, minimum, preserves_storage)| CapturedIntegerOperation {
                state: state.symbol,
                expression,
                minimum_difference: minimum,
                preserves_storage,
            },
        )
        .collect();
    let scalar_associations = visitor
        .associations
        .into_iter()
        .flat_map(|(expression, places)| {
            places
                .into_iter()
                .map(move |current| CapturedScalarAssociation {
                    state: state.symbol,
                    expression,
                    current,
                })
        })
        .collect();
    (
        CapturedNumericObservations {
            scalar_values,
            integer_operations,
            scalar_associations,
        },
        visitor.complete,
    )
}

fn integer_subtraction(
    program: &dyn crate::CheckingInput,
    machine: &Machine,
    state: &State,
    expression: ExpressionHandle,
) -> Option<[TypeReferenceHandle; 2]> {
    let binary = program.expression_table.binary_node(expression)?;
    if binary.operator != BinaryOperator::Subtract
        || !crate::has_builtin_binary_expression_meaning(program, machine, Some(state), expression)
    {
        return None;
    }
    let references = [binary.left, binary.right]
        .map(|operand| crate::expression_result_type_reference(program, machine, state, operand));
    let [Some(left), Some(right)] = references else {
        return None;
    };
    [left, right]
        .iter()
        .all(|reference| {
            program
                .primitive_type_reference(*reference)
                .is_some_and(|primitive| primitive.is_fixed_integer())
        })
        .then_some([left, right])
}

struct Observations {
    state: SymbolHandle,
    statement_index: usize,
    in_transition_target: bool,
    wants_scalar_values: bool,
    candidates: Vec<ExpressionHandle>,
    values: Vec<(ExpressionHandle, Option<ScopedArithmeticValue>)>,
    operations: Vec<ExpressionHandle>,
    orderings: Vec<(ExpressionHandle, Option<i64>, bool)>,
    results: Vec<ExpressionHandle>,
    associations: Vec<(ExpressionHandle, Vec<ScalarOperand>)>,
    complete: bool,
}

impl Observations {
    fn invalidate_root(&mut self, program: &dyn crate::CheckingInput, root: ExpressionHandle) {
        self.complete = false;
        for expression in self.results.clone() {
            if crate::expression_owners::expression_contains(program, root, expression) {
                self.intersect_associations(expression, Vec::new());
            }
        }
        for expression in &self.candidates {
            if !crate::expression_owners::expression_contains(program, root, *expression) {
                continue;
            }
            if let Some((_, value)) = self
                .values
                .iter_mut()
                .find(|(source, _)| source == expression)
            {
                *value = None;
            } else {
                self.values.push((*expression, None));
            }
        }
        for expression in &self.operations {
            if !crate::expression_owners::expression_contains(program, root, *expression) {
                continue;
            }
            if let Some((_, value, preserves_storage)) = self
                .orderings
                .iter_mut()
                .find(|(source, _, _)| source == expression)
            {
                *value = None;
                *preserves_storage = false;
            } else {
                self.orderings.push((*expression, None, false));
            }
        }
    }

    fn intersect_associations(
        &mut self,
        expression: ExpressionHandle,
        current: Vec<ScalarOperand>,
    ) {
        if let Some((_, previous)) = self
            .associations
            .iter_mut()
            .find(|(source, _)| *source == expression)
        {
            previous.retain(|place| current.contains(place));
        } else {
            self.associations.push((expression, current));
        }
    }

    fn collect_associations(
        &mut self,
        program: &dyn crate::CheckingInput,
        machine: &Machine,
        state: &State,
        expressions: &[ExpressionHandle],
        judge: &StructuralJudge<'_>,
    ) {
        if judge.hypotheses_contradictory
            || !expressions.iter().any(|expression| {
                self.results.contains(expression)
                    && !self
                        .associations
                        .iter()
                        .any(|(source, previous)| source == expression && previous.is_empty())
            })
        {
            return;
        }
        let mut current_reads = Vec::new();
        for (read, value) in judge.current_scalar_reads() {
            let Some(carrier) =
                super::observed_projections::integer_storage_read(program, machine, state, read)
            else {
                continue;
            };
            let Some(place) =
                scalar_operand(program, machine, state, read, judge.frame_resolver())
            else {
                continue;
            };
            let entry = (place, value, carrier.reference);
            if !current_reads.contains(&entry) {
                current_reads.push(entry);
            }
        }
        if current_reads.is_empty() {
            for &expression in expressions {
                self.intersect_associations(expression, Vec::new());
            }
            return;
        }
        let mut current = vec![Vec::new(); expressions.len()];
        let mut predicates = Vec::new();
        let mut destinations = Vec::new();
        for (position, expression) in expressions.iter().enumerate() {
            if !self.results.contains(expression) {
                continue;
            }
            let prior = self
                .associations
                .iter()
                .find(|(source, _)| source == expression)
                .map(|(_, previous)| previous);
            if matches!(prior, Some(previous) if previous.is_empty()) {
                continue;
            }
            let Some((_, result)) = judge
                .completed_expression_values()
                .iter()
                .find(|(source, _)| source == expression)
            else {
                continue;
            };
            let Some(result_reference) =
                crate::expression_result_type_reference(program, machine, state, *expression)
            else {
                continue;
            };
            for (read_index, (place, value, reference)) in current_reads.iter().enumerate() {
                if prior.is_some_and(|previous| !previous.contains(place)) {
                    continue;
                }
                predicates.push(StructuralTerm::MathematicalBinary {
                    operator: BinaryOperator::Equal,
                    left: Box::new(StructuralTerm::IntegerEmbedding {
                        reference: result_reference,
                        value: Box::new(result.clone()),
                    }),
                    right: Box::new(StructuralTerm::IntegerEmbedding {
                        reference: *reference,
                        value: Box::new((*value).clone()),
                    }),
                });
                destinations.push((position, read_index));
            }
        }
        if !predicates.is_empty() {
            for ((position, read_index), proven) in destinations
                .into_iter()
                .zip(judge.prove_observed_predicates(&predicates, 0))
            {
                let place = &current_reads[read_index].0;
                if proven && !current[position].contains(place) {
                    current[position].push(place.clone());
                }
            }
        }
        // A reachable path with no authenticated current read contributes the
        // empty set. Batch only within this judge; arm outcomes still intersect.
        for (expression, current) in expressions.iter().zip(current) {
            self.intersect_associations(*expression, current);
        }
    }

    fn collect_ordering<'program>(
        &mut self,
        program: &'program dyn crate::CheckingInput,
        machine: &Machine,
        state: &State,
        expression: ExpressionHandle,
        arguments: &[StructuralTerm],
        judge: &StructuralJudge<'program>,
    ) {
        if judge.hypotheses_contradictory || !self.operations.contains(&expression) {
            return;
        }
        let minimum = (|| {
            let [left_reference, right_reference] =
                integer_subtraction(program, machine, state, expression)?;
            let [left, right] = arguments else {
                return None;
            };
            let comparison = |operator| StructuralTerm::MathematicalBinary {
                operator,
                left: Box::new(StructuralTerm::IntegerEmbedding {
                    reference: left_reference,
                    value: Box::new(left.clone()),
                }),
                right: Box::new(StructuralTerm::IntegerEmbedding {
                    reference: right_reference,
                    value: Box::new(right.clone()),
                }),
            };
            // These predicates mention only the completed operands. The
            // subtraction's own result and formation never enter this judge.
            if matches!(
                judge.judge_observed_predicate(comparison(BinaryOperator::Greater), 0),
                super::structural_judgment::StructuralJudgment::Proven
            ) {
                Some(1)
            } else if matches!(
                judge.judge_observed_predicate(comparison(BinaryOperator::GreaterOrEqual), 0),
                super::structural_judgment::StructuralJudgment::Proven
            ) {
                Some(0)
            } else {
                None
            }
        })();
        let preserves_storage = program.machine(machine.symbol).is_some_and(|machine| {
            judge.frame_resolver().is_some_and(|frames| {
                frames
                    .expression_write_frame(machine, expression)
                    .complete_paths()
                    .is_some_and(|paths| paths.is_empty())
            })
        });
        if let Some((_, previous, preserved)) = self
            .orderings
            .iter_mut()
            .find(|(source, _, _)| *source == expression)
        {
            *previous = previous.zip(minimum).map(|(left, right)| left.min(right));
            *preserved &= preserves_storage;
        } else {
            self.orderings
                .push((expression, minimum, preserves_storage));
        }
    }

    fn collect(
        &mut self,
        program: &dyn crate::CheckingInput,
        machine: &Machine,
        state: &State,
        judge: &StructuralJudge<'_>,
    ) {
        for (expression, value) in judge.completed_expression_values() {
            if !self.candidates.contains(expression) {
                continue;
            }
            let Some(carrier) = super::observed_projections::declared_integer_field_read(
                program,
                machine,
                state,
                *expression,
            ) else {
                continue;
            };
            let value = judge.completed_value_identity(value.clone());

            let value = match value.semantic_value() {
                StructuralTerm::Integer(integer) => ScopedArithmeticValue::Integer(integer.clone()),
                _ => {
                    let Some(key) = value.observation_identity_key() else {
                        self.invalidate_root(program, *expression);
                        continue;
                    };
                    ScopedArithmeticValue::Atom {
                        identity: format!("state:{:?}:completed-scalar:{key}", self.state),
                        unsigned: !carrier.primitive.is_signed_integer(),
                    }
                }
            };
            if let Some((_, prior)) = self
                .values
                .iter_mut()
                .find(|(source, _)| source == expression)
            {
                if prior.as_ref() != Some(&value) {
                    *prior = None;
                }
            } else {
                self.values.push((*expression, Some(value)));
            }
        }
    }
}

impl StructuralPrefixVisitor for Observations {
    fn site(&self) -> StructuralPrefixSite {
        StructuralPrefixSite {
            state: self.state,
            statement_index: self.statement_index,
            in_transition_target: self.in_transition_target,
        }
    }

    fn observe<'program>(&mut self, observation: PrefixObservation<'_, 'program>) {
        let (program, machine, state, judge, environment) = observation;
        if self.wants_scalar_values {
            self.collect(program, machine, state, &judge);
        }
        let Some(statement) = program
            .statement_table
            .statement_at(state, self.statement_index)
        else {
            return;
        };
        if !self.wants_scalar_values
            && self.operations.iter().all(|operation| {
                self.orderings.iter().any(|(source, minimum, preserved)| {
                    source == operation && minimum.is_none() && !*preserved
                })
            })
            && self.results.iter().all(|expression| {
                self.associations
                    .iter()
                    .any(|(source, previous)| source == expression && previous.is_empty())
            })
        {
            return;
        }
        if !self.wants_scalar_values
            && self.operations.is_empty()
            && self.candidates.is_empty()
            && !judge.current_scalar_reads().any(|(read, _)| {
                super::observed_projections::integer_storage_read(program, machine, state, read)
                    .is_some()
                    && scalar_operand(program, machine, state, read, judge.frame_resolver())
                        .is_some()
            })
        {
            for index in 0..self.results.len() {
                let expression = self.results[index];
                self.intersect_associations(expression, Vec::new());
            }
            return;
        }
        // Requested identities do not change when an unknown root invalidates
        // its captures. Keep this exact roster for every root in the visit.
        let mut requested = self.candidates.clone();
        requested.extend(
            self.results
                .iter()
                .copied()
                .filter(|result| !self.candidates.contains(result)),
        );
        let roots: Vec<_> = program
            .tables
            .expression_roots(statement)
            .into_iter()
            .map(|(_, expression)| expression)
            .filter(|expression| expression.is_valid())
            .collect();
        if roots.len() > 1
            && !matches!(
                statement,
                typed_trees::statement::StatementNode::Transition(_)
            )
            && judge.frame_resolver().is_some_and(|frames| {
                roots.iter().all(|&root| {
                    frames
                        .expression_write_frame(machine, root)
                        .complete_paths()
                        .is_some_and(|paths| paths.is_empty())
                })
            })
        {
            let last_root = *roots.last().unwrap();
            let reference =
                crate::expression_result_type_reference(program, machine, state, last_root)
                    .unwrap_or(state.return_type);
            if let Some(observations) = judge.completed_observations(
                self.statement_index,
                last_root,
                environment,
                (machine, state, reference),
                &requested,
                &self.operations,
            ) {
                for entry in observations.operations {
                    let Some(arguments) = entry.observed_arguments() else {
                        continue;
                    };
                    self.collect_ordering(
                        program,
                        machine,
                        state,
                        entry.expression,
                        &arguments,
                        &entry.judge,
                    );
                }
                let results = self
                    .results
                    .iter()
                    .copied()
                    .filter(|result| {
                        roots.iter().any(|&root| {
                            crate::expression_owners::expression_contains(program, root, *result)
                        })
                    })
                    .collect::<Vec<_>>();
                for outcome in observations.paths {
                    if self.wants_scalar_values {
                        self.collect(program, machine, state, &outcome);
                    }
                    self.collect_associations(program, machine, state, &results, &outcome);
                }
                return;
            }
        }
        // The normal-value owner installs each root's completed leading roots
        // itself, including ordered call arguments and guard/target context.
        for (_, expression) in program.tables.expression_roots(statement) {
            let reference =
                crate::expression_result_type_reference(program, machine, state, expression)
                    .unwrap_or(state.return_type);
            let Some(observations) = judge.completed_observations(
                self.statement_index,
                expression,
                environment,
                (machine, state, reference),
                &requested,
                &self.operations,
            ) else {
                // Unknown completion of a potentially evaluated root cannot
                // leave a convenient sibling-path capture behind. Earlier
                // snapshots outside this root retain their completed custody.
                self.invalidate_root(program, expression);
                continue;
            };
            for entry in observations.operations {
                let Some(arguments) = entry.observed_arguments() else {
                    continue;
                };
                self.collect_ordering(
                    program,
                    machine,
                    state,
                    entry.expression,
                    &arguments,
                    &entry.judge,
                );
            }
            // Root containment depends only on the requested expression
            // identities, not an individual normal path's captured values.
            let results = self
                .results
                .iter()
                .copied()
                .filter(|result| {
                    crate::expression_owners::expression_contains(program, expression, *result)
                })
                .collect::<Vec<_>>();
            for outcome in observations.paths {
                if self.wants_scalar_values {
                    self.collect(program, machine, state, &outcome);
                }
                self.collect_associations(program, machine, state, &results, &outcome);
            }
        }
    }
}
