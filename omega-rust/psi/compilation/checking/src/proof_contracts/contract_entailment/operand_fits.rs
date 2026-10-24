//! Exact operand carrier fit before the consuming operation exists.
//!
//! The normal-value owner supplies the actual operation-entry paths, including
//! ordered siblings and selected guards. Typed cast inputs use their captured
//! embedding; anonymous comparison inputs use their completed mathematical
//! integer value. Address ceilings remain the symbolic model bound.

use super::citations::CitationFacts;
use super::structural_case_arms::structural_arm_judge;
use super::structural_judgment::normal_returns::OperationOperandDemand;
use super::structural_judgment::{StructuralJudge, StructuralTerm};
use super::structural_prefix::{StructuralPrefixSite, StructuralPrefixVisitor};
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
    Pending {
        low: numerics::bignum::BigInt,
        high: numerics::bignum::BigInt,
    },
}

impl CarrierFit {
    fn across_paths(self, other: Self) -> Self {
        match (self, other) {
            (Self::Refuted, _) | (_, Self::Refuted) => Self::Refuted,
            (Self::Unknown, _) | (_, Self::Unknown) => Self::Unknown,
            (Self::Proven, other) | (other, Self::Proven) => other,
            (
                Self::Pending { low, high },
                Self::Pending {
                    low: other_low,
                    high: other_high,
                },
            ) => Self::Pending {
                low: low.min(other_low),
                high: high.max(other_high),
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
            Self::Pending { low, high } => {
                if !owner.is_valid() || !expression.is_valid() {
                    return false;
                }
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
    if !matches!(
        semantic.kernel(),
        numerics::float_semantics_catalog::FloatSemanticKernel::ToIntegerExact
    ) || program.arithmetic_domain_for_type_reference(operator.return_type)
        != numerics::arithmetic::ArithmeticDomain::Exact
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
pub(crate) fn exact_binary_fits_at_occurrence(
    program: &dyn crate::CheckingInput,
    machine: &Machine,
    state: &State,
    expression: ExpressionHandle,
) -> bool {
    let Some(binary) = program.expression_table.binary_node(expression) else {
        return false;
    };
    if !crate::has_builtin_binary_expression_meaning(program, machine, Some(state), expression) {
        return false;
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
            return false;
        }
        let Some(peer) = references[1 - position]
            .and_then(|reference| program.primitive_type_reference(reference))
        else {
            return false;
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
            return false;
        };
        references[position] =
            crate::proof_contracts::bound_expression_meaning::builtin_type_reference(program, atom);
    }
    let [Some(left), Some(right)] = references else {
        return false;
    };
    let primitives = [left, right].map(|reference| program.primitive_type_reference(reference));
    let [Some(left_primitive), Some(right_primitive)] = primitives else {
        return false;
    };
    let Some(target) = typed_trees::operator::builtin_integer_result_carrier(
        binary.operator,
        [left_primitive, right_primitive],
    ) else {
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
                source: OperandSource::CompletedOperation {
                    operator: binary.operator,
                    references: [left, right],
                    established_carriers,
                },
                target,
                context_reference: left,
            },
        ),
        CarrierFit::Proven
    )
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

fn operand_fits_at_occurrence(
    program: &dyn crate::CheckingInput,
    machine: &Machine,
    state: &State,
    request: OperandFitRequest,
) -> CarrierFit {
    let Some(statement_index) =
        super::completed_values::expression_statement_index(program, state, request.expression)
    else {
        return CarrierFit::Unknown;
    };
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
    };
    let recognized = super::structural_prefix::observe_runtime_state_prefix(
        program,
        machine,
        state,
        &mut visitor,
    )
    .is_some();
    if recognized && visitor.seen {
        visitor.judgment.unwrap_or(CarrierFit::Unknown)
    } else {
        CarrierFit::Unknown
    }
}

struct OperandFit {
    site: StructuralPrefixSite,
    request: OperandFitRequest,
    seen: bool,
    judgment: Option<CarrierFit>,
}

impl StructuralPrefixVisitor for OperandFit {
    fn site(&self) -> StructuralPrefixSite {
        self.site
    }

    fn visit<'program>(
        &mut self,
        program: &'program dyn crate::CheckingInput,
        machine: &Machine,
        judge: &StructuralJudge<'program>,
        environment: &[(String, StructuralTerm)],
        hypotheses: &[(String, StructuralTerm)],
        equations: &[(StructuralTerm, StructuralTerm)],
        guarantees: &CitationFacts,
    ) {
        self.seen = true;
        let mut judge = structural_arm_judge(program, judge, hypotheses, equations);
        if judge.hypotheses_contradictory {
            return;
        }
        guarantees.intake(&mut judge);
        let Some(state) = program.state_in_machine(machine, self.site.state) else {
            self.judgment = Some(CarrierFit::Unknown);
            return;
        };
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
            return;
        };
        if entries.is_empty() {
            self.judgment = Some(CarrierFit::Unknown);
        }
        for entry in entries {
            let Some(value) = entry.arguments.get(self.request.position) else {
                self.judgment = Some(CarrierFit::Unknown);
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
                OperandSource::CompletedOperation {
                    operator,
                    references,
                    established_carriers,
                } => {
                    let [left, right] = entry.arguments.as_slice() else {
                        self.judgment = Some(CarrierFit::Unknown);
                        continue;
                    };
                    let values = [left, right];
                    let [left, right] = std::array::from_fn(|position| {
                        let value = values[position].clone();
                        let value = if established_carriers[position] {
                            StructuralTerm::ObservedValue {
                                observed_type: references[position],
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
                    entry.judge.integer_term_fit_judgment(
                        StructuralTerm::MathematicalBinary {
                            operator,
                            left: Box::new(left),
                            right: Box::new(right),
                        },
                        self.request.target,
                    )
                }
            };
            self.judgment = Some(match self.judgment.take() {
                None => judgment,
                Some(previous) => previous.across_paths(judgment),
            });
        }
    }
}
