//! Exact carrier fit at the executed operation, not at declaration entry.
//!
//! The normal-return owner evaluates preceding operands and captures the cast
//! inputs before observing its result. This retains call guarantees and retires
//! stale storage facts in the same way as ordinary call requirements. Address
//! bounds stay symbolic; a native representation choice supplies no premise.

use super::structural_judgment::StructuralTerm;
use super::structural_prefix::{PrefixObservation, StructuralPrefixSite, StructuralPrefixVisitor};
use typed_trees::expression::{BinaryOperator, ExpressionHandle};
use typed_trees::machine::Machine;
use typed_trees::state::State;
use typed_trees::types::{PrimitiveType, TypeReferenceHandle};

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
    fits_at_occurrence(
        program,
        machine,
        state,
        expression,
        IntegerFit::Cast {
            source_reference,
            target,
        },
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
        let atom = match carrier {
            PrimitiveType::Addr => symbols::BuiltinTypeAtom::Address,
            PrimitiveType::U64 => symbols::BuiltinTypeAtom::U64,
            _ => return false,
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
    fits_at_occurrence(
        program,
        machine,
        state,
        expression,
        IntegerFit::Binary {
            operator: binary.operator,
            references: [left, right],
            established_carriers,
            target,
        },
    )
}

fn fits_at_occurrence(
    program: &dyn crate::CheckingInput,
    machine: &Machine,
    state: &State,
    expression: ExpressionHandle,
    operation: IntegerFit,
) -> bool {
    let Some(statement_index) =
        super::completed_values::expression_statement_index(program, state, expression)
    else {
        return false;
    };
    let mut visitor = OccurrenceFit {
        site: StructuralPrefixSite {
            state: state.symbol,
            statement_index,
            in_transition_target: super::structural_prefix::expression_is_transition_target(
                program,
                state,
                statement_index,
                expression,
            ),
        },
        expression,
        operation,
        seen: false,
        proven: true,
    };
    let recognized = super::structural_prefix::observe_runtime_state_prefix(
        program,
        machine,
        state,
        &mut visitor,
    )
    .is_some();
    recognized && visitor.seen && visitor.proven
}

enum IntegerFit {
    Cast {
        source_reference: TypeReferenceHandle,
        target: PrimitiveType,
    },
    Binary {
        operator: BinaryOperator,
        references: [TypeReferenceHandle; 2],
        established_carriers: [bool; 2],
        target: PrimitiveType,
    },
}

struct OccurrenceFit {
    site: StructuralPrefixSite,
    expression: ExpressionHandle,
    operation: IntegerFit,
    seen: bool,
    proven: bool,
}

impl StructuralPrefixVisitor for OccurrenceFit {
    fn site(&self) -> StructuralPrefixSite {
        self.site
    }

    fn entered(&mut self) {
        self.seen = true;
    }

    fn unresolved(&mut self, _machine: &Machine) {
        self.proven = false;
    }

    fn observe<'program>(&mut self, observation: PrefixObservation<'_, 'program>) {
        let (_, machine, state, judge, environment) = observation;
        let Some(entries) = judge.normal_operation_entries(
            self.site.statement_index,
            self.expression,
            environment,
            (
                machine,
                state,
                match self.operation {
                    IntegerFit::Cast {
                        source_reference, ..
                    } => source_reference,
                    IntegerFit::Binary { references, .. } => references[0],
                },
            ),
        ) else {
            self.proven = false;
            return;
        };
        self.proven &= !entries.is_empty();
        for entry in entries {
            let fits = match (&self.operation, entry.arguments.as_slice()) {
                (
                    IntegerFit::Cast {
                        source_reference,
                        target,
                    },
                    [value],
                ) => entry
                    .judge
                    .observed_integer_fits(value.clone(), *source_reference, *target),
                (
                    IntegerFit::Binary {
                        operator,
                        references,
                        established_carriers,
                        target,
                    },
                    [left, right],
                ) => entry.judge.observed_integer_operation_fits(
                    *operator,
                    std::array::from_fn(|position| {
                        let value = [left, right][position].clone();
                        if established_carriers[position] {
                            StructuralTerm::ObservedValue {
                                observed_type: references[position],
                                value: Box::new(value),
                                descriptor: None,
                            }
                        } else {
                            value
                        }
                    }),
                    *references,
                    *target,
                ),
                _ => false,
            };
            self.proven &= fits;
        }
    }
}
