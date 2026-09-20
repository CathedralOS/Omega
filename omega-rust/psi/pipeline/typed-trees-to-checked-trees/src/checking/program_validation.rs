use diagnostics::Diagnostic;
use flow_effects::{OperationalPlan, ServiceReachInferencePlan};
use proof::obligations::ProofPlan;
use typed_trees::TypedTrees;
use typed_trees::data::DataMember;
use typed_trees::expression::ExpressionNode;
use typed_trees::types::{TypeReferenceHandle, TypeReferenceNode};

pub(crate) struct ValidatedTypedProgram<'program> {
    pub(crate) proof_plan: ProofPlan<'program>,
    pub(crate) operational: OperationalPlan,
    pub(crate) service_reaches: ServiceReachInferencePlan,
    pub(crate) validation_facts: validation::ProgramValidationFacts,
}

pub(crate) fn validate_typed_program<'program>(
    program: &'program TypedTrees,
    opaque_property_receipts: &[validation::OpaqueDataPropertyReceipt],
    allow_pending_opaque_copy: bool,
) -> Result<ValidatedTypedProgram<'program>, Vec<Diagnostic>> {
    validate_atomic_result_custody(program)?;

    let opaque_properties = if allow_pending_opaque_copy {
        validation::OpaquePropertyValidation::PendingBuildSelection
    } else {
        validation::OpaquePropertyValidation::Required(opaque_property_receipts)
    };
    let validated = validation::validate_specialized_program(program, opaque_properties)?;

    let proof_plan = proof::obligations::build_proof_plan(program);
    proof::checker::check_proof_plan(&proof_plan)?;

    let operational = validated.operational;
    validation::validate_behavior_plan(program, &operational)?;
    crate::checking::call_acknowledgements::validate_call_acknowledgements(program, &operational)?;
    validate_no_bare_boundary_trait_values(program)?;

    Ok(ValidatedTypedProgram {
        proof_plan,
        operational,
        validation_facts: validated.facts,
        service_reaches: validated.service_reaches,
    })
}

fn validate_atomic_result_custody(program: &TypedTrees) -> Result<(), Vec<Diagnostic>> {
    let diagnostics = program
        .expression_table
        .iter_expressions()
        .filter_map(|(_, expression)| {
            let ExpressionNode::Atomic(atomic) = expression else {
                return None;
            };
            (!atomic.result_custody.is_valid_for(atomic.ordering)
                || (atomic.result_custody.requires_result_destination()
                    && !atomic.result.is_valid()))
            .then(|| {
                Diagnostic::error(
                    "atomic expression result custody does not match its operation axis",
                )
            })
        })
        .collect::<Vec<_>>();
    if diagnostics.is_empty() {
        Ok(())
    } else {
        Err(diagnostics)
    }
}

/// A bare boundary trait in value position does not denote a service carrier:
/// fields, parameters, signature slots and returns name `Service<R>` instead.
/// Borrows (`&T`, `&mut T`) and container members are other positions and stay
/// out of this gate; only the outermost `Named`/`DynamicTrait`/`Generic` head
/// (through `Constrained` shells) is inspected, so `Service<Console>` itself is
/// never mistaken for a bare trait.
fn validate_no_bare_boundary_trait_values(program: &TypedTrees) -> Result<(), Vec<Diagnostic>> {
    let mut diagnostics = Vec::new();
    let mut check = |position: String, type_reference: TypeReferenceHandle| {
        if let Some(trait_name) = bare_boundary_trait_name(program, type_reference) {
            diagnostics.push(Diagnostic::error(format!(
                "{position} names bare boundary trait `{trait_name}` in value position; the intrinsic `Service<R>` carrier is the only service value spelling"
            )));
        }
    };
    for definition in program.data_definitions() {
        for member in program.data_members(definition) {
            match member {
                DataMember::Field(field) => check(
                    format!(
                        "field `{}` on data `{}`",
                        field.name.as_str(),
                        definition.name.as_str()
                    ),
                    field.type_reference,
                ),
                DataMember::Variant(variant) => {
                    for payload in program.data_payload_fields(variant) {
                        check(
                            format!(
                                "payload field `{}` on case `{}` of data `{}`",
                                payload.name.as_str(),
                                variant.name.as_str(),
                                definition.name.as_str()
                            ),
                            payload.type_reference,
                        );
                    }
                }
            }
        }
    }
    for machine in program.machines() {
        // A `satisfies` adapter on a boundary requirement may take the
        // satisfied trait itself as one extra leading parameter — the
        // self-forwarding receiver conformance slicing removes before
        // arity/refinement. That slot is dispatch plumbing rather than a
        // carried service, so its bare spelling stays admitted here.
        let forwarding_receiver_symbols = program
            .machine_trait_conformances(machine)
            .iter()
            .map(|conformance| conformance.symbol)
            .filter(|symbol| {
                program
                    .traits()
                    .iter()
                    .any(|definition| definition.symbol == *symbol && definition.is_boundary)
            })
            .collect::<Vec<_>>();
        for state in program.machine_states(machine) {
            for (parameter_index, parameter) in program.state_parameters(state).iter().enumerate() {
                if parameter_index == 0
                    && !forwarding_receiver_symbols.is_empty()
                    && bare_boundary_trait_symbol(program, parameter.type_reference)
                        .is_some_and(|symbol| forwarding_receiver_symbols.contains(&symbol))
                {
                    continue;
                }
                check(
                    format!(
                        "parameter `{}` on `{}.{}`",
                        parameter.name.as_str(),
                        machine.name.as_str(),
                        state.name.as_str()
                    ),
                    parameter.type_reference,
                );
            }
            check(
                format!(
                    "return type of `{}.{}`",
                    machine.name.as_str(),
                    state.name.as_str()
                ),
                state.return_type,
            );
        }
    }
    for trait_definition in program.traits() {
        for signature in program.trait_machine_signatures(trait_definition) {
            for parameter in program.state_signature_parameters(signature) {
                check(
                    format!(
                        "parameter `{}` on signature `{}.{}`",
                        parameter.name.as_str(),
                        trait_definition.name.as_str(),
                        signature.name.as_str()
                    ),
                    parameter.type_reference,
                );
            }
            check(
                format!(
                    "return type of signature `{}.{}`",
                    trait_definition.name.as_str(),
                    signature.name.as_str()
                ),
                signature.return_type,
            );
        }
    }
    if diagnostics.is_empty() {
        Ok(())
    } else {
        Err(diagnostics)
    }
}

fn bare_boundary_trait_name(
    program: &TypedTrees,
    type_reference: TypeReferenceHandle,
) -> Option<String> {
    bare_boundary_trait_symbol_and_name(program, type_reference).map(|(_, name)| name)
}

fn bare_boundary_trait_symbol(
    program: &TypedTrees,
    type_reference: TypeReferenceHandle,
) -> Option<symbols::SymbolHandle> {
    bare_boundary_trait_symbol_and_name(program, type_reference).map(|(symbol, _)| symbol)
}

fn bare_boundary_trait_symbol_and_name(
    program: &TypedTrees,
    mut type_reference: TypeReferenceHandle,
) -> Option<(symbols::SymbolHandle, String)> {
    loop {
        if !program
            .type_reference_table
            .contains_type_reference(type_reference)
        {
            return None;
        }
        let symbol_and_name = match program.type_reference_table.type_reference(type_reference) {
            TypeReferenceNode::Constrained { base_type, .. } => {
                type_reference = *base_type;
                continue;
            }
            TypeReferenceNode::Named { symbol, name }
            | TypeReferenceNode::DynamicTrait { symbol, name, .. } => (*symbol, name.as_str()),
            TypeReferenceNode::Generic {
                base_symbol,
                base_name,
                ..
            } => (*base_symbol, base_name.as_str()),
            _ => return None,
        };
        let (symbol, name) = symbol_and_name;
        return program
            .traits()
            .iter()
            .any(|definition| definition.symbol == symbol && definition.is_boundary)
            .then(|| (symbol, name.to_owned()));
    }
}

#[cfg(test)]
mod tests {
    use language_core::atomic::{
        AtomicCompareExchangeOnceResultCustody as OnceCustody,
        AtomicCompareExchangeOutcomeIdentity as Outcome,
        AtomicExpressionResultCustody as ResultCustody,
        AtomicObservingCompareExchangeOperation as Operation,
        AtomicObservingCompareExchangeResultShape as Shape, AtomicOrderingPlan, MemoryOrdering,
    };
    use typed_trees::TypedTrees;
    use typed_trees::expression::{ExpressionHandle, ExpressionNode, TableAtomicExpression};

    use super::validate_atomic_result_custody;

    fn program_with_atomic(
        ordering: AtomicOrderingPlan,
        result_custody: ResultCustody,
        has_result: bool,
    ) -> TypedTrees {
        let mut program = TypedTrees::default();
        let value = program
            .expression_table
            .insert(ExpressionNode::Boolean(false));
        program
            .expression_table
            .insert(ExpressionNode::Atomic(TableAtomicExpression {
                value,
                result: if has_result {
                    value
                } else {
                    ExpressionHandle::invalid()
                },
                ordering,
                result_custody,
            }));
        program
    }

    #[test]
    fn checked_boundary_accepts_exact_single_attempt_result_custody() {
        let program = program_with_atomic(
            AtomicOrderingPlan::CompareExchangeOnce {
                success: MemoryOrdering::ReceivePublish,
                failure: MemoryOrdering::Receive,
            },
            ResultCustody::ObservingCompareExchangeOnce(OnceCustody::CANONICAL),
            true,
        );

        assert!(validate_atomic_result_custody(&program).is_ok());
    }

    #[test]
    fn checked_boundary_rejects_incomplete_or_substituted_single_attempt_custody() {
        let once_ordering = AtomicOrderingPlan::CompareExchangeOnce {
            success: MemoryOrdering::ReceivePublish,
            failure: MemoryOrdering::Receive,
        };
        let decisive_ordering = AtomicOrderingPlan::CompareExchange {
            success: MemoryOrdering::ReceivePublish,
            failure: MemoryOrdering::Receive,
        };
        for (ordering, custody, has_result) in [
            (
                once_ordering,
                ResultCustody::ObservingCompareExchangeOnce(OnceCustody::CANONICAL),
                false,
            ),
            (
                decisive_ordering,
                ResultCustody::ObservingCompareExchangeOnce(OnceCustody::CANONICAL),
                true,
            ),
            (
                once_ordering,
                ResultCustody::ObservingCompareExchangeOnce(OnceCustody {
                    operation: Operation::Decisive,
                    ..OnceCustody::CANONICAL
                }),
                true,
            ),
            (
                once_ordering,
                ResultCustody::ObservingCompareExchangeOnce(OnceCustody {
                    result_shape: Shape::ExchangedOrMismatchedObserved,
                    ..OnceCustody::CANONICAL
                }),
                true,
            ),
            (
                once_ordering,
                ResultCustody::ObservingCompareExchangeOnce(OnceCustody {
                    outcome_identity: Outcome::AtomicCompareExchangeOutcome,
                    ..OnceCustody::CANONICAL
                }),
                true,
            ),
            (
                once_ordering,
                ResultCustody::ObservingCompareExchangeOnce(OnceCustody {
                    outcome_identity: Outcome::AtomicTryExchangeOnceOutcome,
                    ..OnceCustody::CANONICAL
                }),
                true,
            ),
            (once_ordering, ResultCustody::Scalar, true),
        ] {
            let program = program_with_atomic(ordering, custody, has_result);
            assert!(validate_atomic_result_custody(&program).is_err());
        }
    }

    #[test]
    fn checked_boundary_keeps_decisive_scalar_custody_unchanged() {
        let program = program_with_atomic(
            AtomicOrderingPlan::CompareExchange {
                success: MemoryOrdering::ReceivePublish,
                failure: MemoryOrdering::Receive,
            },
            ResultCustody::Scalar,
            true,
        );

        assert!(validate_atomic_result_custody(&program).is_ok());
    }
}
