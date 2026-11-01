//! Exact operand carrier fit before the consuming operation exists.
//!
//! The normal-value owner supplies the actual operation-entry paths, including
//! ordered siblings and selected guards. Typed cast inputs use their captured
//! embedding; anonymous comparison inputs use their completed mathematical
//! integer value. Address ceilings remain the symbolic model bound.

use super::structural_judgment::StructuralTerm;
use super::structural_judgment::normal_returns::OperationOperandDemand;
use super::structural_prefix::{PrefixObservation, StructuralPrefixSite, StructuralPrefixVisitor};
use crate::proof_contracts::arithmetic_domains::BigIntInterval;
use numerics::arithmetic::ArithmeticDomain;
use numerics::bignum::BigInt;
use numerics::float_semantics_catalog::FloatSemanticKernel;
use typed_trees::expression::{BinaryOperator, ExpressionHandle};
use typed_trees::machine::Machine;
use typed_trees::state::State;
use typed_trees::types::{PrimitiveType, TypeReferenceHandle};

/// Formation may be conditional on one still-symbolic target model. Pending
/// bounds are captured mathematical values, never permission to observe a
/// carrier that the same operation has not formed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum CarrierFit {
    Proven,
    Refuted,
    Unknown,
    Pending { interval: BigIntInterval },
}

impl CarrierFit {
    fn across_paths(self, other: Self) -> Self {
        match (self, other) {
            (Self::Refuted, _) | (_, Self::Refuted) => Self::Refuted,
            (Self::Unknown, _) | (_, Self::Unknown) => Self::Unknown,
            (Self::Proven, other) | (other, Self::Proven) => other,
            (Self::Pending { interval }, Self::Pending { interval: other }) => Self::Pending {
                interval: interval.union(&other),
            },
        }
    }

    pub(crate) fn retain(
        self,
        expression: ExpressionHandle,
        owner: symbols::SymbolHandle,
        description: &str,
        obligations: &mut Vec<checked_trees::IntegerRangeObligation>,
    ) -> bool {
        match self {
            Self::Proven => true,
            Self::Pending { interval } => {
                if !owner.is_valid() || !expression.is_valid() {
                    return false;
                }
                let (Some(low), Some(high)) = (interval.low, interval.high) else {
                    return false;
                };
                let obligation = checked_trees::IntegerRangeObligation::retain_address_fit(
                    expression,
                    owner,
                    low,
                    high,
                    description.to_owned(),
                );
                if !obligations.contains(&obligation) {
                    obligations.push(obligation);
                }
                true
            }
            Self::Refuted | Self::Unknown => false,
        }
    }
}

#[derive(Clone, Copy)]
enum OperandSource {
    Completed(TypeReferenceHandle),
    TruncatedFloat(TypeReferenceHandle),
    AnonymousInteger,
    AnonymousValue,
    CompletedIntegerExclusion {
        references: [TypeReferenceHandle; 2],
        excluded: [Option<i64>; 2],
    },
    CompletedOperation {
        operator: BinaryOperator,
        references: [TypeReferenceHandle; 2],
        established_carriers: [bool; 2],
    },
}

#[derive(Clone, Copy)]
struct OperandFitRequest {
    expression: ExpressionHandle,
    position: usize,
    source: OperandSource,
    target: PrimitiveType,
    context_reference: TypeReferenceHandle,
}

/// The selected named conversion consumes a completed float, not a place
/// reconstructed from its spelling. Reuse the ordinary operation-entry owner;
/// its result and guarantees cannot establish its own operand's admission.
pub(crate) fn float_integer_conversion_fits_at_occurrence(
    program: &dyn crate::CheckingInput,
    machine: &Machine,
    state: &State,
    expression: ExpressionHandle,
) -> bool {
    let Some(call) = program.expression_table.call_node(expression) else {
        return false;
    };
    let Some(operator) = typed_trees::operator::resolve_named_expression_call(program, call) else {
        return false;
    };
    let Some((source, semantic)) = crate::float_integer_conversion_contract(program, operator)
    else {
        return false;
    };
    if !matches!(semantic.kernel(), FloatSemanticKernel::ToIntegerExact)
        || program.arithmetic_domain_for_type_reference(operator.return_type)
            != ArithmeticDomain::Exact
    {
        return false;
    }
    let Some(target) = program.primitive_type_reference(operator.return_type) else {
        return false;
    };
    let [operand] = program.expression_table.expression_handles(call.arguments) else {
        return false;
    };
    let Some(reference) =
        crate::expression_result_type_reference(program, machine, state, *operand)
    else {
        return false;
    };
    if !target.is_fixed_integer() || program.primitive_type_reference(reference) != Some(source) {
        return false;
    }
    matches!(
        operand_fits_at_occurrence(
            program,
            machine,
            state,
            OperandFitRequest {
                expression,
                position: 0,
                source: OperandSource::TruncatedFloat(reference),
                target,
                context_reference: reference,
            },
        ),
        CarrierFit::Proven
    )
}

pub(crate) fn exact_cast_fits_at_occurrence(
    program: &dyn crate::CheckingInput,
    machine: &Machine,
    state: &State,
    expression: ExpressionHandle,
) -> bool {
    let Some(cast) = program.expression_table.cast_node(expression) else {
        return false;
    };
    let Some(target) = program.primitive_type_reference(cast.target_type) else {
        return false;
    };
    let Some(source_reference) =
        crate::expression_result_type_reference(program, machine, state, cast.value)
    else {
        return false;
    };
    matches!(
        operand_fits_at_occurrence(
            program,
            machine,
            state,
            OperandFitRequest {
                expression,
                position: 0,
                source: OperandSource::Completed(source_reference),
                target,
                context_reference: source_reference,
            },
        ),
        CarrierFit::Proven
    )
}

/// Both operands have completed, but this operation has not. Their own carrier
/// bounds and live observations may prove the result; its proposed carrier may
/// not supply an assumption about that same result.
#[derive(Default)]
pub(crate) struct CompletedIntegerFormation {
    pub(crate) proven: bool,
    pub(crate) recipe: Option<checked_trees::IntegerRangeRecipe>,
}

/// The operand-fit demand one exact binary formation carries, or `None` when
/// the expression cannot select a builtin integer operation. Both operands
/// have completed while this operation has not; their carriers and live
/// observations must prove the result without donating a premise.
fn exact_binary_fit_request(
    program: &dyn crate::CheckingInput,
    machine: &Machine,
    state: &State,
    expression: ExpressionHandle,
) -> Option<OperandFitRequest> {
    let binary = program.expression_table.binary_node(expression)?;
    if !crate::has_builtin_binary_expression_meaning(program, machine, Some(state), expression) {
        return None;
    }
    let mut references = [binary.left, binary.right]
        .map(|operand| crate::expression_result_type_reference(program, machine, state, operand));
    // A contextual reference selects an anonymous operand's destination; it
    // does not establish that operand's landing. Preserve that distinction
    // when a completed, independently typed value resolves to a constant.
    let established_carriers = references.map(|reference| reference.is_some());
    for position in 0..2 {
        if references[position].is_some() {
            continue;
        }
        let operand = [binary.left, binary.right][position];
        if !crate::value_custody::literals::has_anonymous_numeric_results(program, operand) {
            return None;
        }
        let Some(peer) = references[1 - position].and_then(|reference| {
            crate::value_custody::expression_types::scalar_observation_meaning(program, reference)
                .map(|(primitive, _)| primitive)
        }) else {
            return None;
        };
        let carrier = typed_trees::operator::integer_peer_operand_carrier(
            binary.operator,
            peer,
            position == 1,
        );
        let Some(atom) = carrier
            .landed_landing()
            .map(typed_trees::types::builtin_type_atom)
        else {
            return None;
        };
        references[position] =
            crate::proof_contracts::bound_expression_meaning::builtin_type_reference(program, atom);
    }
    let [Some(left), Some(right)] = references else {
        return None;
    };
    let primitives = [left, right].map(|reference| {
        crate::value_custody::expression_types::scalar_observation_meaning(program, reference)
            .map(|(primitive, _)| primitive)
    });
    let [Some(left_primitive), Some(right_primitive)] = primitives else {
        return None;
    };
    let target = typed_trees::operator::builtin_integer_result_carrier(
        binary.operator,
        [left_primitive, right_primitive],
    )?;
    Some(OperandFitRequest {
        expression,
        position: 0,
        source: OperandSource::CompletedOperation {
            operator: binary.operator,
            references: [left, right],
            established_carriers,
        },
        target,
        context_reference: left,
    })
}

pub(crate) fn exact_binary_formation_at_occurrence(
    program: &dyn crate::CheckingInput,
    machine: &Machine,
    state: &State,
    expression: ExpressionHandle,
) -> CompletedIntegerFormation {
    let Some(request) = exact_binary_fit_request(program, machine, state, expression) else {
        return CompletedIntegerFormation::default();
    };
    let Some(observed) = observe_operand_fit_at_occurrence(program, machine, state, request) else {
        return CompletedIntegerFormation::default();
    };
    CompletedIntegerFormation {
        proven: matches!(observed.judgment, Some(CarrierFit::Proven)),
        recipe: observed
            .range_complete
            .then_some(observed.range_recipe)
            .flatten()
            .filter(checked_trees::IntegerRangeRecipe::is_target_dependent),
    }
}

/// The caller independently checks anonymous integrality/representation and
/// builtin selection. The requested peer is an actual operand occurrence;
/// no expected destination invents a source type or completed result.
pub(crate) fn anonymous_comparison_operand_fits_at_occurrence(
    program: &dyn crate::CheckingInput,
    machine: &Machine,
    state: &State,
    expression: ExpressionHandle,
    peer: ExpressionHandle,
    target: PrimitiveType,
) -> CarrierFit {
    let Some(binary) = program.expression_table.binary_node(expression) else {
        return CarrierFit::Unknown;
    };
    let Some(position) = [binary.left, binary.right]
        .iter()
        .position(|operand| *operand == peer)
    else {
        return CarrierFit::Unknown;
    };
    if !crate::value_custody::literals::has_anonymous_numeric_results(program, peer) {
        return CarrierFit::Unknown;
    }
    let Some(context_reference) =
        crate::expression_result_type_reference(program, machine, state, expression)
    else {
        return CarrierFit::Unknown;
    };
    operand_fits_at_occurrence(
        program,
        machine,
        state,
        OperandFitRequest {
            expression,
            position,
            source: OperandSource::AnonymousInteger,
            target,
            context_reference,
        },
    )
}

/// Storage and parameter landings consume the same completed-value and model
/// judgment as comparison peers. No expected type becomes a carrier premise.
pub(crate) fn anonymous_value_fit_at_occurrence(
    program: &dyn crate::CheckingInput,
    machine: &Machine,
    state: Option<&State>,
    expression: ExpressionHandle,
    target: PrimitiveType,
) -> CarrierFit {
    if !crate::value_custody::literals::has_anonymous_numeric_results(program, expression) {
        return CarrierFit::Unknown;
    }
    let Some(state) = state else {
        // A declaration value has no runtime frame. It uses the same exact
        // symbol-scoped mathematical fit, with no invocation/body premises.
        return super::scoped_arithmetic::scoped_arithmetic_value_fits_for_declaration(
            program,
            machine.symbol,
            &[],
            &super::ScopedArithmeticValue::Term(super::ScopedArithmeticExpression {
                owner: machine.symbol,
                expression,
                bindings: Vec::new(),
            }),
            target,
        );
    };
    // There is no finite receiving carrier while this mathematical anonymous
    // value completes. ZII absence prevents the destination from qualifying
    // its internal arithmetic or donating carrier membership.
    let reference = TypeReferenceHandle::invalid();
    operand_fits_at_occurrence(
        program,
        machine,
        state,
        OperandFitRequest {
            expression,
            position: 0,
            source: OperandSource::AnonymousValue,
            target,
            context_reference: reference,
        },
    )
}

/// Exclude one simultaneous tuple of integer operand values at the operation
/// cut. Earlier operands are captured before later operands' writes; neither
/// the requested operation nor its normal result contributes a premise.
pub(crate) fn integer_operands_exclude_at_occurrence(
    program: &dyn crate::CheckingInput,
    machine: &Machine,
    state: &State,
    expression: ExpressionHandle,
    primitive: PrimitiveType,
    excluded: [Option<i64>; 2],
) -> bool {
    let Some(binary) = program.expression_table.binary_node(expression) else {
        return false;
    };
    let Some(position) = excluded.iter().position(Option::is_some) else {
        return false;
    };
    let references = [binary.left, binary.right].map(|operand| {
        crate::expression_result_type_reference(program, machine, state, operand)
            .unwrap_or_default()
    });
    if !primitive.is_fixed_integer()
        || references.iter().zip(excluded).any(|(reference, value)| {
            value.is_some() && program.primitive_type_reference(*reference) != Some(primitive)
        })
    {
        return false;
    }
    let context_reference =
        crate::expression_result_type_reference(program, machine, state, expression)
            .unwrap_or(state.return_type);
    matches!(
        operand_fits_at_occurrence(
            program,
            machine,
            state,
            OperandFitRequest {
                expression,
                position,
                source: OperandSource::CompletedIntegerExclusion {
                    references,
                    excluded
                },
                target: primitive,
                context_reference,
            }
        ),
        CarrierFit::Proven
    )
}

fn operand_fits_at_occurrence(
    program: &dyn crate::CheckingInput,
    machine: &Machine,
    state: &State,
    request: OperandFitRequest,
) -> CarrierFit {
    observe_operand_fit_at_occurrence(program, machine, state, request)
        .and_then(|observed| observed.judgment)
        .unwrap_or(CarrierFit::Unknown)
}

/// The recorded answer for one exact binary-fit expression: either the
/// visitor's surviving fields (`Some`) or the fact that no observation
/// delivered (`None`, retained so a later query does not walk again). All
/// fields are owned; `CheckingMemos` stores no `'program` borrows.
#[derive(Clone)]
pub(crate) struct OperandFitOutcome {
    seen: bool,
    judgment: Option<CarrierFit>,
    range_recipe: Option<checked_trees::IntegerRangeRecipe>,
    range_complete: bool,
}

// A population walk can itself consult this same query for another
// expression or machine. Track the machines already populating on this
// thread; a nested request for one of them keeps the ordinary
// per-occurrence path, never a provisional verdict from an unfinished
// population.
std::thread_local! {
    static ACTIVE_OPERAND_FIT_POPULATIONS: std::cell::RefCell<Vec<symbols::SymbolHandle>> =
        const { std::cell::RefCell::new(Vec::new()) };
}

struct OperandFitPopulation;

impl OperandFitPopulation {
    fn enter(machine: symbols::SymbolHandle) -> Option<Self> {
        ACTIVE_OPERAND_FIT_POPULATIONS.with(|active| {
            let mut active = active.borrow_mut();
            if active.contains(&machine) {
                return None;
            }
            active.push(machine);
            Some(Self)
        })
    }
}

impl Drop for OperandFitPopulation {
    fn drop(&mut self) {
        ACTIVE_OPERAND_FIT_POPULATIONS.with(|active| {
            active.borrow_mut().pop();
        });
    }
}

/// Rebuild the solo-path visitor from a recorded outcome. The statement
/// coordinate is recomputed exactly as the solo path computes it, so a
/// stored verdict can never outlive the occurrence check it came from.
fn replay_operand_fit(
    program: &dyn crate::CheckingInput,
    state: &State,
    request: OperandFitRequest,
    outcome: OperandFitOutcome,
) -> Option<OperandFit> {
    let statement_index =
        super::completed_values::expression_statement_index(program, state, request.expression)?;
    Some(OperandFit {
        site: StructuralPrefixSite {
            state: state.symbol,
            statement_index,
            in_transition_target: super::structural_prefix::expression_is_transition_target(
                program,
                state,
                statement_index,
                request.expression,
            ),
        },
        request,
        seen: outcome.seen,
        judgment: outcome.judgment,
        range_recipe: outcome.range_recipe,
        range_complete: outcome.range_complete,
    })
}

/// Every exact-formation binary expression in `machine`'s states is an
/// operand-fit demand site. One prefix pass per state and one shared
/// private-arrivals pass answer them all; each verdict (and each
/// no-verdict) is recorded by expression so later occurrences replay
/// without walking again. Transition-target occurrences stay on the
/// per-occurrence walk: the batched forward pass evaluates statement-entry
/// sites only, and rerouting them to the arrivals frontier would change
/// which premises their judgment observes.
fn populate_machine_operand_fits(
    program: &dyn crate::CheckingInput,
    machine: &Machine,
    memos: &crate::frozen_program::CheckingMemos,
) {
    let mut grouped: Vec<(&State, Vec<OperandFit>)> = Vec::new();
    for state in program.machine_states(machine) {
        let mut expressions = Vec::new();
        for statement in program.statement_table.statements(state.statement_nodes) {
            crate::expression_traversal::collect_statement_expression_trees(
                program,
                statement,
                &mut expressions,
            );
        }
        let mut fits = Vec::new();
        for expression in expressions {
            let Some(request) = exact_binary_fit_request(program, machine, state, expression)
            else {
                continue;
            };
            let Some(statement_index) =
                super::completed_values::expression_statement_index(program, state, expression)
            else {
                continue;
            };
            if super::structural_prefix::expression_is_transition_target(
                program,
                state,
                statement_index,
                expression,
            ) {
                continue;
            }
            fits.push(OperandFit {
                site: StructuralPrefixSite {
                    state: state.symbol,
                    statement_index,
                    in_transition_target: false,
                },
                request,
                seen: false,
                judgment: None,
                range_recipe: None,
                range_complete: true,
            });
        }
        if !fits.is_empty() {
            grouped.push((state, fits));
        }
    }
    let accepted = observe_operand_fits_batch(program, machine, &mut grouped);
    if let Ok(mut outcomes) = memos.operand_fit_outcomes.lock() {
        for ((_, fits), flags) in grouped.iter().zip(accepted) {
            for (fit, accepted) in fits.iter().zip(flags) {
                outcomes.insert(
                    (machine.symbol, fit.request.expression),
                    accepted.then(|| OperandFitOutcome {
                        seen: fit.seen,
                        judgment: fit.judgment.clone(),
                        range_recipe: fit.range_recipe.clone(),
                        range_complete: fit.range_complete,
                    }),
                );
            }
        }
    }
    if let Ok(mut populated) = memos.operand_fit_populated.lock() {
        populated.insert(machine.symbol, ());
    }
}

/// One state-prefix pass per state and one shared private-arrivals pass
/// for every still-undecided request. Each visitor keeps the verdict its
/// solo walk would have produced; the returned flags parallel `grouped`
/// and name which visitors survived either pass.
fn observe_operand_fits_batch(
    program: &dyn crate::CheckingInput,
    machine: &Machine,
    grouped: &mut [(&State, Vec<OperandFit>)],
) -> Vec<Vec<bool>> {
    let mut accepted: Vec<Vec<bool>> = grouped
        .iter()
        .map(|(_, fits)| vec![false; fits.len()])
        .collect();
    for (group_index, (state, fits)) in grouped.iter_mut().enumerate() {
        let recognized = {
            let mut visitors: Vec<&mut dyn StructuralPrefixVisitor> = fits
                .iter_mut()
                .map(|fit| fit as &mut dyn StructuralPrefixVisitor)
                .collect();
            super::structural_prefix::observe_runtime_state_prefixes(
                program,
                machine,
                *state,
                &mut visitors,
            )
        };
        for (index, fit) in fits.iter_mut().enumerate() {
            if recognized.get(index).copied().unwrap_or(false)
                && fit.seen
                && (fit.has_positive_proof()
                    || matches!(
                        fit.judgment,
                        Some(CarrierFit::Refuted | CarrierFit::Pending { .. })
                    ))
            {
                accepted[group_index][index] = true;
                continue;
            }
            // Inferred private requirements owe every actual arrival. The
            // same reset the solo path performs applies before the shared
            // coordinator publishes a proof or a conditional recipe.
            fit.seen = false;
            fit.judgment = None;
            fit.range_recipe = None;
            fit.range_complete = true;
        }
    }
    let mut member_indices = Vec::new();
    let mut members: Vec<&mut OperandFit> = Vec::new();
    for (group_index, (_, fits)) in grouped.iter_mut().enumerate() {
        for (index, fit) in fits.iter_mut().enumerate() {
            if !accepted[group_index][index] {
                member_indices.push((group_index, index));
                members.push(fit);
            }
        }
    }
    if !members.is_empty() {
        let verdicts = super::structural_prefix::observe_private_arrivals_batch(
            program,
            machine,
            members.as_mut_slice(),
            false,
        );
        for (((group_index, index), verdict), member) in
            member_indices.iter().zip(verdicts).zip(members.iter())
        {
            if verdict == Some(true) && member.seen {
                accepted[*group_index][*index] = true;
            }
        }
    }
    accepted
}

fn observe_operand_fit_at_occurrence(
    program: &dyn crate::CheckingInput,
    machine: &Machine,
    state: &State,
    request: OperandFitRequest,
) -> Option<OperandFit> {
    // The exact-formation demand is identical for every binary expression
    // it qualifies: one population walks each of this machine's state
    // prefixes and its private arrivals once, and later occurrences replay
    // the recorded verdict. Other request kinds carry occurrence-specific
    // sources, so they bypass the memo and keep the per-occurrence walk.
    let memos = if matches!(request.source, OperandSource::CompletedOperation { .. }) {
        crate::frozen_program::frozen_program_memos(program).filter(|_| {
            program
                .machine(machine.symbol)
                .is_some_and(|stored| std::ptr::eq(stored, machine))
                && program
                    .state_in_machine(machine, state.symbol)
                    .is_some_and(|stored| std::ptr::eq(stored, state))
        })
    } else {
        None
    };
    if let Some(memos) = memos {
        let key = (machine.symbol, request.expression);
        let mut cached = memos
            .operand_fit_outcomes
            .lock()
            .ok()
            .and_then(|outcomes| outcomes.get(&key).cloned());
        if cached.is_none()
            && !memos
                .operand_fit_populated
                .lock()
                .map(|populated| populated.contains_key(&machine.symbol))
                .unwrap_or(false)
            && let Some(_population) = OperandFitPopulation::enter(machine.symbol)
        {
            populate_machine_operand_fits(program, machine, memos);
            cached = memos
                .operand_fit_outcomes
                .lock()
                .ok()
                .and_then(|outcomes| outcomes.get(&key).cloned());
        }
        if let Some(outcome) = cached {
            return outcome
                .and_then(|outcome| replay_operand_fit(program, state, request, outcome));
        }
    }
    let statement_index =
        super::completed_values::expression_statement_index(program, state, request.expression)?;
    let mut visitor = OperandFit {
        site: StructuralPrefixSite {
            state: state.symbol,
            statement_index,
            in_transition_target: super::structural_prefix::expression_is_transition_target(
                program,
                state,
                statement_index,
                request.expression,
            ),
        },
        request,
        seen: false,
        judgment: None,
        range_recipe: None,
        range_complete: true,
    };
    let recognized = super::structural_prefix::observe_runtime_state_prefix(
        program,
        machine,
        state,
        &mut visitor,
    )
    .is_some();
    if recognized
        && visitor.seen
        && (visitor.has_positive_proof()
            || matches!(
                visitor.judgment,
                Some(CarrierFit::Refuted | CarrierFit::Pending { .. })
            ))
    {
        return Some(visitor);
    }
    // Inferred private requirements owe every actual arrival. The same
    // coordinator closes callable uses and recursion before publishing either
    // a proof or a conditional recipe; an omitted input never establishes one.
    visitor.seen = false;
    visitor.judgment = None;
    visitor.range_recipe = None;
    visitor.range_complete = true;
    (super::structural_prefix::observe_private_arrivals(program, machine, &mut visitor, false)
        == Some(true)
        && visitor.seen)
        .then_some(visitor)
}

#[derive(Clone)]
struct OperandFit {
    site: StructuralPrefixSite,
    request: OperandFitRequest,
    seen: bool,
    judgment: Option<CarrierFit>,
    range_recipe: Option<checked_trees::IntegerRangeRecipe>,
    range_complete: bool,
}

impl StructuralPrefixVisitor for OperandFit {
    fn has_positive_proof(&self) -> bool {
        self.seen
            && (matches!(self.judgment, Some(CarrierFit::Proven))
                || (self.range_complete
                    && self
                        .range_recipe
                        .as_ref()
                        .is_some_and(checked_trees::IntegerRangeRecipe::is_target_dependent)))
    }

    fn site(&self) -> StructuralPrefixSite {
        self.site
    }

    fn entered(&mut self) {
        self.seen = true;
    }

    fn unresolved(&mut self, _machine: &Machine) {
        self.judgment = Some(CarrierFit::Unknown);
        self.range_complete = false;
    }

    fn observe<'program>(&mut self, observation: PrefixObservation<'_, 'program>) {
        let (program, machine, state, judge, environment) = observation;
        let entries = match self.request.source {
            OperandSource::AnonymousValue => judge.normal_value_entries(
                self.site.statement_index,
                self.request.expression,
                environment,
                (machine, state, self.request.context_reference),
            ),
            _ => judge.normal_operation_entries(
                self.site.statement_index,
                self.request.expression,
                OperationOperandDemand::All,
                environment,
                (machine, state, self.request.context_reference),
            ),
        };
        let Some(entries) = entries else {
            self.judgment = Some(CarrierFit::Unknown);
            self.range_complete = false;
            return;
        };
        if entries.is_empty() {
            self.judgment = Some(CarrierFit::Unknown);
            self.range_complete = false;
        }
        for entry in entries {
            let Some(value) = entry
                .arguments
                .get(self.request.position)
                .and_then(Option::as_ref)
            else {
                self.judgment = Some(CarrierFit::Unknown);
                self.range_complete = false;
                continue;
            };
            let judgment = match self.request.source {
                OperandSource::TruncatedFloat(reference) => {
                    if entry
                        .judge
                        .observed_float_bounds(value, reference)
                        .is_some_and(|bounds| bounds.proves_integer_conversion(self.request.target))
                    {
                        CarrierFit::Proven
                    } else {
                        CarrierFit::Unknown
                    }
                }
                OperandSource::Completed(reference) => {
                    if entry.judge.observed_integer_fits(
                        value.clone(),
                        reference,
                        self.request.target,
                    ) {
                        CarrierFit::Proven
                    } else {
                        CarrierFit::Unknown
                    }
                }
                OperandSource::AnonymousInteger | OperandSource::AnonymousValue => entry
                    .judge
                    .integer_term_fit_judgment(value.clone(), self.request.target),
                OperandSource::CompletedIntegerExclusion {
                    references,
                    excluded,
                } => {
                    let predicates = excluded
                        .into_iter()
                        .enumerate()
                        .filter_map(|(position, excluded)| {
                            excluded.map(|excluded| {
                                let value = entry.arguments.get(position)?.as_ref()?.clone();
                                Some(StructuralTerm::ScalarBinary {
                                    references: [references[position]; 2],
                                    operator: BinaryOperator::NotEqual,
                                    meaning: [(self.request.target, ArithmeticDomain::Exact); 2],
                                    left: Box::new(value),
                                    right: Box::new(StructuralTerm::Integer(BigInt::from_i64(
                                        excluded,
                                    ))),
                                })
                            })
                        })
                        .collect::<Option<Vec<_>>>();
                    let predicate = predicates.and_then(|mut predicates| {
                        let first = predicates.pop()?;
                        if let Some(other) = predicates.pop() {
                            let boolean = crate::proof_contracts::bound_expression_meaning::builtin_type_reference(
                                program, symbols::BuiltinTypeAtom::Bool,
                            )?;
                            Some(StructuralTerm::ScalarBinary {
                                references: [boolean; 2],
                                operator: BinaryOperator::Or,
                                meaning: [(PrimitiveType::Bool, ArithmeticDomain::Exact); 2],
                                left: Box::new(other), right: Box::new(first),
                            })
                        } else { Some(first) }
                    });
                    if predicate.is_some_and(|predicate| {
                        matches!(
                            entry.judge.judge_observed_predicate(predicate, 0),
                            super::structural_judgment::StructuralJudgment::Proven
                        )
                    }) {
                        CarrierFit::Proven
                    } else {
                        CarrierFit::Unknown
                    }
                }
                OperandSource::CompletedOperation {
                    operator,
                    references,
                    established_carriers,
                } => {
                    let [Some(left), Some(right)] = entry.arguments.as_slice() else {
                        self.judgment = Some(CarrierFit::Unknown);
                        self.range_complete = false;
                        continue;
                    };
                    let values = [left, right];
                    // Selecting a readable carrier is not evidence that its
                    // contents were observed. In particular, a returned pointer
                    // cannot acquire scalar bounds from this wrapper alone.
                    if values.iter().zip(references).any(|(value, reference)| {
                        !super::structural_terms::scalar_operand_is_observed(
                            program, reference, value,
                        )
                    }) {
                        self.judgment = Some(CarrierFit::Unknown);
                        continue;
                    }
                    let observed = references.map(|reference| {
                        crate::value_custody::expression_types::readable_observation_type_reference(
                            program, reference,
                        )
                    });
                    let [Some(observed_left), Some(observed_right)] = observed else {
                        self.judgment = Some(CarrierFit::Unknown);
                        continue;
                    };
                    let observed = [observed_left, observed_right];
                    let [left, right] = std::array::from_fn(|position| {
                        let value = values[position].clone();
                        let value = if established_carriers[position] {
                            StructuralTerm::ObservedValue {
                                observed_type: observed[position],
                                value: Box::new(value),
                                descriptor: None,
                            }
                        } else {
                            value
                        };
                        StructuralTerm::IntegerEmbedding {
                            reference: references[position],
                            value: Box::new(value),
                        }
                    });
                    let result = StructuralTerm::MathematicalBinary {
                        operator,
                        left: Box::new(left),
                        right: Box::new(right),
                    };
                    let judgment = entry
                        .judge
                        .integer_term_fit_judgment(result.clone(), self.request.target);
                    let recipe = if matches!(judgment, CarrierFit::Proven) {
                        // A proved path already establishes the result carrier.
                        // Re-expanding loose operand intervals must not undo it.
                        self.request.target.integer_bounds().map(|(low, high)| {
                            checked_trees::IntegerRangeRecipe::bounds(
                                BigInt::from_i128(low),
                                BigInt::from_i128(high),
                            )
                        })
                    } else {
                        entry.judge.completed_integer_range_recipe(&result)
                    };
                    if let Some(recipe) = recipe {
                        self.range_recipe = Some(
                            self.range_recipe
                                .take()
                                .map_or_else(|| recipe.clone(), |prior| prior.union(&recipe)),
                        );
                    } else {
                        self.range_complete = false;
                    }
                    judgment
                }
            };
            self.judgment = Some(match self.judgment.take() {
                None => judgment,
                Some(previous) => previous.across_paths(judgment),
            });
        }
    }
}
