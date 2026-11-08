/*
Independent boundary result-policy verification (`verify_boundary_result_policies`)
and saturation-eligibility checks over lowered and Terminal Psi calls.
*/

use checked_trees::{CheckedArithmeticPolicyAdapter, CheckedTrees, NominalMachineUseSite};
use lowered_psi::LoweredSourceCustody;
use numerics::float_semantics::FloatFormat;
use semantic_vocabulary::IeeeFloatFormat;
use terminal_psi::{FloatResultPolicy, OperationKind};

pub(crate) mod saturation_eligibility {
    use semantic_vocabulary::{IeeeFloatFormat, IntegerSign, IntegerValue, ScalarType, ValueId};
    use terminal_psi::{Operation, OperationKind};

    #[derive(Debug, PartialEq, Eq)]
    enum Predicate {
        Finite(ValueId, IeeeFloatFormat),
        Nonzero(ValueId, IeeeFloatFormat),
    }

    pub(crate) fn verify(
        operations: &[&Operation],
        eligible: ValueId,
        arguments: &[ValueId],
        types: &[ScalarType],
        divisor: Option<usize>,
    ) -> bool {
        if arguments.len() != types.len() {
            return false;
        }
        let mut expected = Vec::new();
        for (position, (argument, kind)) in arguments.iter().zip(types).enumerate() {
            match kind {
                ScalarType::IeeeFloat(format) => {
                    expected.push(Predicate::Finite(*argument, *format));
                    if divisor == Some(position) {
                        expected.push(Predicate::Nonzero(*argument, *format));
                    }
                }
                ScalarType::Integer(_) if divisor != Some(position) => {}
                _ => return false,
            }
        }
        if divisor.is_some_and(|position| position >= arguments.len()) {
            return false;
        }
        let mut budget = 4096;
        let Some(root) = definition(operations, eligible, &mut budget) else {
            return false;
        };
        if root
            .result
            .scalar_ref()
            .is_none_or(|result| result.scalar_type != ScalarType::Boolean)
        {
            return false;
        }
        if let OperationKind::BooleanConstant { value } = root.kind {
            return constant_conjunction(operations, &expected, &mut budget) == Some(value);
        }
        let OperationKind::IntegerEqual { left, right } = root.kind else {
            return false;
        };
        let mut observed = Vec::new();
        let conjunction = if is_byte_one(operations, right, &mut budget) {
            left
        } else if is_byte_one(operations, left, &mut budget) {
            right
        } else {
            return false;
        };
        byte_predicates(operations, conjunction, &mut budget, &mut observed) && observed == expected
    }

    fn definition<'a>(
        operations: &[&'a Operation],
        value: ValueId,
        budget: &mut usize,
    ) -> Option<&'a Operation> {
        *budget = budget.checked_sub(1)?;
        let mut matches = operations.iter().filter(|operation| {
            operation
                .result
                .scalar_ref()
                .is_some_and(|result| result.id == value)
        });
        let operation = *matches.next()?;
        matches.next().is_none().then_some(operation)
    }
    fn unsigned_constant(
        operations: &[&Operation],
        value: ValueId,
        bits: u16,
        budget: &mut usize,
    ) -> Option<u128> {
        let operation = definition(operations, value, budget)?;
        let ScalarType::Integer(integer) = operation.result.scalar_ref()?.scalar_type else {
            return None;
        };
        if integer.sign() != IntegerSign::Unsigned || integer.bits() != bits {
            return None;
        }
        let OperationKind::IntegerConstant {
            value: IntegerValue::Unsigned(value),
        } = operation.kind
        else {
            return None;
        };
        Some(value)
    }
    fn is_byte_one(operations: &[&Operation], value: ValueId, budget: &mut usize) -> bool {
        unsigned_constant(operations, value, 8, budget) == Some(1)
    }
    fn byte_predicates(
        operations: &[&Operation],
        value: ValueId,
        budget: &mut usize,
        predicates: &mut Vec<Predicate>,
    ) -> bool {
        if is_byte_one(operations, value, budget) {
            return true;
        }
        let Some(operation) = definition(operations, value, budget) else {
            return false;
        };
        let Some(result) = operation.result.scalar_ref() else {
            return false;
        };
        if !matches!(result.scalar_type, ScalarType::Integer(integer) if integer.sign() == IntegerSign::Unsigned && integer.bits() == 8)
        {
            return false;
        }
        match operation.kind {
            OperationKind::IntegerBitwiseAnd { left, right } => {
                byte_predicates(operations, left, budget, predicates)
                    && byte_predicates(operations, right, budget, predicates)
            }
            OperationKind::ScalarRepresentationView {
                operand,
                landing_obligation: None,
            } => {
                let Some(predicate) = predicate(operations, operand, budget) else {
                    return false;
                };
                predicates.push(predicate);
                true
            }
            _ => false,
        }
    }
    fn predicate(
        operations: &[&Operation],
        value: ValueId,
        budget: &mut usize,
    ) -> Option<Predicate> {
        let negation = definition(operations, value, budget)?;
        if negation.result.scalar_ref()?.scalar_type != ScalarType::Boolean {
            return None;
        }
        let OperationKind::BooleanNot { operand } = negation.kind else {
            return None;
        };
        let comparison = definition(operations, operand, budget)?;
        if comparison.result.scalar_ref()?.scalar_type != ScalarType::Boolean {
            return None;
        }
        let OperationKind::IntegerEqual { left, right } = comparison.kind else {
            return None;
        };
        image_comparison(operations, left, right, budget)
            .or_else(|| image_comparison(operations, right, left, budget))
    }
    fn image_comparison(
        operations: &[&Operation],
        image: ValueId,
        comparison: ValueId,
        budget: &mut usize,
    ) -> Option<Predicate> {
        let masked = definition(operations, image, budget)?;
        let ScalarType::Integer(integer) = masked.result.scalar_ref()?.scalar_type else {
            return None;
        };
        if integer.sign() != IntegerSign::Unsigned {
            return None;
        }
        let (format, exponent, magnitude) = match integer.bits() {
            32 => (IeeeFloatFormat::Binary32, 0x7f80_0000, 0x7fff_ffff),
            64 => (
                IeeeFloatFormat::Binary64,
                0x7ff0_0000_0000_0000,
                0x7fff_ffff_ffff_ffff,
            ),
            _ => return None,
        };
        let comparison = unsigned_constant(operations, comparison, integer.bits(), budget)?;
        let OperationKind::IntegerBitwiseAnd { left, right } = masked.kind else {
            return None;
        };
        let (view, mask) =
            if let Some(mask) = unsigned_constant(operations, right, integer.bits(), budget) {
                (left, mask)
            } else {
                (
                    right,
                    unsigned_constant(operations, left, integer.bits(), budget)?,
                )
            };
        let view = definition(operations, view, budget)?;
        if view.result.scalar_ref()?.scalar_type != ScalarType::Integer(integer) {
            return None;
        }
        let OperationKind::ScalarRepresentationView {
            operand,
            landing_obligation: None,
        } = view.kind
        else {
            return None;
        };
        if mask == exponent && comparison == exponent {
            Some(Predicate::Finite(operand, format))
        } else if mask == magnitude && comparison == 0 {
            Some(Predicate::Nonzero(operand, format))
        } else {
            None
        }
    }

    fn constant_conjunction(
        operations: &[&Operation],
        predicates: &[Predicate],
        budget: &mut usize,
    ) -> Option<bool> {
        let mut unknown = false;
        for predicate in predicates {
            let (source, format) = match *predicate {
                Predicate::Finite(source, format) | Predicate::Nonzero(source, format) => {
                    (source, format)
                }
            };
            let truth = definition(operations, source, budget).and_then(|operation| {
                let OperationKind::IeeeFloatConstant { value } = operation.kind else {
                    return None;
                };
                if value.format() != format {
                    return None;
                }
                Some(match (predicate, value) {
                    (
                        Predicate::Finite(..),
                        semantic_vocabulary::IeeeFloatValue::Binary32(bits),
                    ) => f32::from_bits(bits).is_finite(),
                    (
                        Predicate::Finite(..),
                        semantic_vocabulary::IeeeFloatValue::Binary64(bits),
                    ) => f64::from_bits(bits).is_finite(),
                    (
                        Predicate::Nonzero(..),
                        semantic_vocabulary::IeeeFloatValue::Binary32(bits),
                    ) => f32::from_bits(bits) != 0.0,
                    (
                        Predicate::Nonzero(..),
                        semantic_vocabulary::IeeeFloatValue::Binary64(bits),
                    ) => f64::from_bits(bits) != 0.0,
                })
            });
            match truth {
                Some(false) => return Some(false),
                None => unknown = true,
                Some(true) => {}
            }
        }
        (!unknown).then_some(true)
    }
}

pub fn verify_boundary_result_policies(
    checked: &CheckedTrees,
    module: &terminal_psi::TerminalModule,
    source_custody: &LoweredSourceCustody,
) -> Result<(), &'static str> {
    let source_policies = source_custody
        .source_call_occurrences
        .iter()
        .filter(|source| {
            let Some(NominalMachineUseSite::Expression(expression)) = source.source_site else {
                return false;
            };
            let mut has_policy = false;
            checked
                .facts
                .operators
                .for_each_resolved_expression_selection(expression, |origin, target, policy, _| {
                    if !has_policy
                        && target == source.source_target
                        && policy != CheckedArithmeticPolicyAdapter::None
                        && source.matches_checked_origin(checked, origin)
                    {
                        has_policy = true;
                    }
                });
            has_policy
        })
        .count();
    let mut matched = 0;
    let mut actual = 0;
    for machine in &module.machines {
        for block in &machine.blocks {
            for (position, operation) in block.operations.iter().enumerate() {
                if matches!(operation.kind, OperationKind::FloatResultPolicy { .. }) {
                    actual += 1;
                }
                let OperationKind::BoundaryCall {
                    boundary,
                    arguments,
                    ..
                } = &operation.kind
                else {
                    continue;
                };
                let Some(raw) = operation.result.scalar() else {
                    continue;
                };
                let Some(raw_format) = raw.scalar_type.as_ieee_float() else {
                    continue;
                };
                let mut sources = source_custody
                    .source_call_occurrences
                    .iter()
                    .filter(|source| source.terminal_site.operation() == Some(operation.id));
                let (Some(source), None) = (sources.next(), sources.next()) else {
                    return Err("float boundary call has no unique source occurrence");
                };
                let Some(NominalMachineUseSite::Expression(expression)) = source.source_site else {
                    continue;
                };
                let mut first_selection = None;
                let mut multiple_selections = false;
                checked
                    .facts
                    .operators
                    .for_each_resolved_expression_selection(
                        expression,
                        |origin, target, policy, _| {
                            if source.matches_checked_origin(checked, origin) {
                                if first_selection.is_none() {
                                    first_selection = Some((target, policy));
                                } else {
                                    multiple_selections = true;
                                }
                            }
                        },
                    );
                let Some((target, policy)) = first_selection else {
                    if typed_trees::operator::declaration_by_symbol(
                        &checked.typed,
                        source.source_target,
                    )
                    .is_some()
                    {
                        return Err("float result policy lost its checked selection");
                    }
                    continue;
                };
                if multiple_selections || target != source.source_target {
                    return Err("float result policy changed its exact checked selection");
                }
                let control = &checked.facts.flow.control;
                let target_state = checked
                    .typed
                    .machine_by_symbol(target)
                    .and_then(|machine| checked.typed.machine_states(machine).first())
                    .map_or(target, |state| state.symbol);
                let coordinate = source
                    .origin
                    .statement()
                    .ok_or("float result policy has no statement call custody")?;
                let calls = control
                    .state_by_symbol(coordinate.state)
                    .into_iter()
                    .flat_map(|state| control.calls.span_or_empty(state.calls))
                    .filter(|call| {
                        call.statement_index == coordinate.statement_index
                            && call.call_ordinal == coordinate.call_ordinal
                            && call.authored_expression == expression
                            && call.target_symbol == target_state
                    })
                    .count();
                if calls != 1 {
                    return Err("float result policy changed its source call coordinate");
                }
                let declaration = module
                    .boundary_machine(*boundary)
                    .ok_or("float result policy lost its boundary declaration")?;
                if module.identities.text(declaration.identity)
                    != Some(requirement_identity(checked, target)?.as_str())
                {
                    return Err("float result policy changed its original boundary target");
                }
                let Some(format) = policy.float_format() else {
                    continue;
                };
                let expected_format = match raw_format {
                    IeeeFloatFormat::Binary32 => FloatFormat::BINARY32,
                    IeeeFloatFormat::Binary64 => FloatFormat::BINARY64,
                };
                if format != expected_format {
                    return Err("float result policy changed its selected format");
                }
                let expected = match policy {
                    CheckedArithmeticPolicyAdapter::None => continue,
                    CheckedArithmeticPolicyAdapter::FloatTrappingNonFinite { .. } => {
                        FloatResultPolicy::Trapping { raw_result: raw.id }
                    }
                    CheckedArithmeticPolicyAdapter::FloatSaturatingOverflowOnly { .. } => {
                        let signature =
                            typed_trees::operator::declaration_by_symbol(&checked.typed, target)
                                .ok_or("saturating result lost its exact declaration")?;
                        let divisor = validation::float_saturation_divisor_parameter(
                            &checked.typed,
                            signature,
                        )
                        .ok_or("saturating result lost its raw semantic equation")?;
                        let adapter = block
                            .operations
                            .get(position + 1)
                            .ok_or("float call lost its result adapter")?;
                        let OperationKind::FloatResultPolicy {
                            policy:
                                FloatResultPolicy::Saturating {
                                    eligible,
                                    raw_result,
                                },
                        } = adapter.kind
                        else {
                            return Err("saturating result lost its adapter");
                        };
                        if raw_result != raw.id
                            || !saturation_eligibility::verify(
                                &machine.operations().collect::<Vec<_>>(),
                                eligible,
                                arguments,
                                &declaration.scalar_parameters,
                                divisor,
                            )
                        {
                            return Err("saturating result changed its exact input eligibility");
                        }
                        FloatResultPolicy::Saturating {
                            raw_result: raw.id,
                            eligible,
                        }
                    }
                };
                let adapter = block
                    .operations
                    .get(position + 1)
                    .ok_or("float call lost its result adapter")?;
                if adapter.kind != (OperationKind::FloatResultPolicy { policy: expected }) {
                    return Err("float call changed or omitted its exact result adapter");
                }
                let adapted = adapter
                    .result
                    .scalar()
                    .ok_or("float result adapter has no scalar result")?;
                if adapted.id == raw.id
                    || adapted.scalar_type != raw.scalar_type
                    || !adapted.qualifications.is_empty()
                    || !raw.qualifications.is_empty()
                {
                    return Err("float result adapter changed its result carrier");
                }
                let mut bypass = false;
                for consumer in &block.operations {
                    if consumer.id == adapter.id {
                        continue;
                    }
                    consumer.kind.clone().map_scalar_uses(&mut |value| {
                        bypass |= value == raw.id;
                        value
                    });
                }
                block.terminator.clone().map_scalar_uses(&mut |value| {
                    bypass |= value == raw.id;
                    value
                });
                if bypass {
                    return Err("float boundary raw result bypasses its policy adapter");
                }
                matched += 1;
            }
        }
    }
    if matched != actual || matched != source_policies {
        return Err("float result adapters do not match the exact source call roster");
    }
    Ok(())
}

fn requirement_identity(
    checked: &CheckedTrees,
    target: symbols::SymbolHandle,
) -> Result<String, &'static str> {
    if let Some(operator) = checked
        .typed
        .operators()
        .iter()
        .find(|operator| operator.is_boundary && operator.symbol == target)
    {
        return Ok(
            typed_trees::operator::boundary_operator_requirement_identity(&checked.typed, operator),
        );
    }
    let machine = checked
        .typed
        .machine_by_symbol(target)
        .ok_or("float call lost its source requirement")?;
    if machine.supply_mode == language_semantics::MachineSupplyMode::TopLevelRequirement
        || (machine.supply_mode == language_semantics::MachineSupplyMode::Boundary
            && machine.service_reach_is_installation_bound)
    {
        return checked
            .typed
            .machine_callable_identity_text(machine)
            .ok_or("float call lost its normalized requirement identity");
    }
    Ok(machine.name.as_str().to_owned())
}
