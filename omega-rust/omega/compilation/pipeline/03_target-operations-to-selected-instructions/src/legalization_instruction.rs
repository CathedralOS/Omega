//! Select the execution carried by a target instruction, without reopening its producer's body.

use crate::LegalizationError as Error;
use crate::legalization::runtime_indices::retained_operands;
use crate::legalization_source::scalar_graph::instruction::{
    call_instructions, project_reference, storage_instructions as storage,
};
use legalized_operations::LegalizedScalarInstructionKind as Kind;
use semantic_vocabulary::{MachineId, OperationId, ScalarType, ValueId};
use target_operations::{
    TargetBooleanExpression as Boolean, TargetIntegerExpression as Integer,
    TargetScalarExpression as Scalar, TargetUnitOperation as Operation,
};

pub(crate) fn project(
    machine: MachineId,
    facts: &[optimization_unit::AcceptedObligationFact],
    native: &target_operations::TargetOperationPlan,
    geometry: &crate::native_geometry::NativeGeometry<'_>,
    instruction: &target_operations::TargetInstruction,
) -> Result<legalized_operations::LegalizedScalarInstruction, Error> {
    let source = instruction.source.as_ref().ok_or_else(|| {
        Error::instruction_metadata("ordinary target instruction has no retained source metadata")
    })?;
    let operation = source.operation;
    let row = &instruction.kind;
    let indices = |rows: &[target_operations::TargetStructuralRuntimeIndex]| {
        retained_operands(facts, machine, operation, rows)
    };
    let fact = |obligation| {
        crate::legalization::obligations::retained_fact(facts, machine, operation, obligation)
    };
    let kind = match row {
        Operation::StructuralByteSequenceFieldLength { .. } => {
            storage::project_sequence_length(machine, facts, operation, row)?
        }
        Operation::EstablishElementView { .. } => {
            storage::project_established_view(machine, facts, row)?
        }
        Operation::ByteSequenceSubslice { .. } | Operation::ElementViewSubslice { .. } => {
            storage::project_subslice(machine, facts, row)?
        }
        Operation::EstablishReference { .. } | Operation::EstablishAccessReference { .. } => {
            project_reference(machine, facts, row)?
        }
        Operation::PrimitiveScalarRead { .. } => {
            storage::project_primitive_scalar_read(machine, facts, row)?
        }
        Operation::WriteOnlyPrimitiveStore { .. } => {
            storage::project_write_only_primitive_store(machine, facts, row)?
        }
        Operation::StructuralScalarFieldStore { .. } => {
            storage::project_structural_scalar_field_store(machine, facts, row)?
        }
        Operation::Call { .. } => call_instructions::project_call(machine, facts, row)?,
        Operation::StructuralLeafCopy { .. } => {
            storage::project_structural_leaf_copy(machine, facts, row)?
        }
        Operation::StoreStructuralField { .. } => {
            storage::project_structural_field_store(machine, facts, geometry, row)?
        }
        Operation::StructuralCaseMembership { .. } => {
            storage::project_structural_case_membership(machine, facts, row)?
        }
        Operation::ByteSequenceWrite { .. } => {
            storage::project_byte_access(machine, facts, operation, row)?
        }
        Operation::StructuralByteSequenceFieldRead { .. } => {
            storage::project_structural_byte_sequence_field_read(machine, facts, row)?
        }
        Operation::StructuralByteViewFieldStore { .. } => {
            storage::project_byte_view_store(machine, facts, row)?
        }
        Operation::ScalarDefinition {
            result_home,
            expression,
        } => {
            if let Some(read) = storage::project_element_view_read(machine, facts, operation, row)?
            {
                read
            } else {
                scalar(
                    machine,
                    facts,
                    operation,
                    result_home.scalar_type,
                    expression,
                    row,
                )?
            }
        }
        Operation::InstallStaticInterrupt {
            boundary,
            arguments,
            indices: selectors,
            calls,
            ..
        } => {
            let indices = argument_indices(machine, facts, operation, selectors)?;
            let call = |position: usize| {
                let call = &calls[position];
                let mut semantic = arguments[if position == 0 { 0 } else { 5 }].clone();
                semantic.access = terminal_psi::StructuralAccess::SharedBorrow;
                Ok::<_, Error>(legalized_operations::LegalizedScalarCall {
                    source: target_operations::NativeCallOrigin::Authored,
                    target: target_operations::NativeCallTarget::Machine(call.callee),
                    call_plan: call.call_plan.clone(),
                    arguments: vec![legalized_operations::LegalizedScalarArgument::Structural {
                        semantic,
                        target: call.argument.clone(),
                        indices: retained_operands(
                            facts,
                            machine,
                            operation,
                            &call.argument.indices,
                        )?,
                    }],
                    result_placement: call.call_plan.result.clone(),
                    structural_result: None,
                    descriptor_origins: Vec::new(),
                    access_bindings: Vec::new(),
                    claim_transfers: Vec::new(),
                    requirement_obligations: Vec::new(),
                    crash_continuations: Vec::new(),
                })
            };
            let calls = [call(0)?, call(1)?];
            Kind::InstallStaticInterrupt {
                boundary: *boundary,
                arguments: arguments.clone(),
                indices,
                calls,
            }
        }
        Operation::StaticInterrupt {
            boundary,
            operation,
            token,
            result_home,
            indices: selectors,
            ..
        } => Kind::StaticInterrupt {
            boundary: *boundary,
            operation: *operation,
            token: token.clone(),
            indices: indices(selectors)?,
            result: result_home.as_ref().map(result).transpose()?,
        },
        Operation::AccessLifecycle {
            boundary,
            operands,
            indices: selectors,
            result_home,
            ..
        } => Kind::AccessLifecycle {
            boundary: *boundary,
            operands: operands.clone(),
            indices: indices(selectors)?,
            result: result(result_home)?,
        },
        Operation::ProgramImageAddress { boundary, .. } => Kind::ProgramImageAddress {
            boundary: *boundary,
        },
        Operation::AccessorScalarAccess {
            boundary,
            realization,
            location,
            operand,
            indices: selectors,
            ..
        } => Kind::AccessorScalarAccess {
            boundary: *boundary,
            realization: *realization,
            location: location.clone(),
            indices: indices(selectors)?,
            operand: operand.map(|operand| operand.source_value()),
        },
        Operation::RegisterAccess { service, event, .. } => Kind::RegisterAccess {
            service: *service,
            event: event.try_map_inputs(|operand| Ok::<_, Error>(operand.source_value()))?,
        },
        Operation::PortWrite {
            service,
            port,
            value,
            ..
        } => Kind::PortWrite {
            service: *service,
            port: *port,
            value: *value,
        },
        Operation::SelectImplementation { table, .. } => {
            Kind::SelectImplementation { table: *table }
        }
        Operation::AtomicFence { ordering, .. } => Kind::AtomicFence {
            ordering: *ordering,
        },
        Operation::AtomicAccess {
            location,
            scalar_type,
            field_byte_offset,
            event,
            outcome_home,
            ..
        } => Kind::AtomicAccess {
            location: location.clone(),
            scalar_type: *scalar_type,
            field_byte_offset: *field_byte_offset,
            event: *event,
            outcome: outcome_home
                .as_ref()
                .map(|home| Ok((result(home)?, sum(home)?)))
                .transpose()?,
        },
        Operation::StructuralScalarFieldRead {
            source,
            case,
            field,
            ..
        } => Kind::StructuralScalarFieldRead {
            source: source.clone(),
            case: *case,
            field: *field,
        },
        Operation::FloatResultPolicy { format, policy, .. } => Kind::FloatResultPolicy {
            format: *format,
            policy: policy.semantic_policy(),
        },
        Operation::ScalarRepresentationView { operand, .. } => Kind::ScalarRepresentationView {
            operand: operand.source_value(),
            source_type: operand.scalar_type(),
        },
        Operation::IeeeFloatCompare {
            comparison,
            format,
            left,
            right,
            ..
        } => Kind::IeeeFloatCompare {
            comparison: *comparison,
            format: *format,
            left: left.source_value(),
            right: right.source_value(),
        },
        Operation::ReleaseReference { source, path, .. } => Kind::ReleaseReference {
            source: *source,
            path: path.clone(),
        },
        Operation::EstablishZeroedStructural { result_home, .. } => {
            Kind::EstablishZeroedStructural {
                result: result(result_home)?,
                shape: result_home.layout.shape(),
            }
        }
        Operation::EstablishStructuralLocal { result_home, .. } => Kind::EstablishStructuralLocal {
            result: result(result_home)?,
            shape: result_home.layout.shape(),
        },
        Operation::EstablishStructuralArray {
            result_home,
            elements,
            ..
        } => Kind::EstablishStructuralArray {
            result: result(result_home)?,
            elements: elements.clone(),
            shape: result_home.layout.shape(),
        },
        Operation::EstablishActivationClaim {
            result_home,
            bound,
            committed,
            backing_type,
            established_case,
            rejected_case,
            ..
        } => Kind::EstablishActivationClaim {
            result: result(result_home)?,
            bound: *bound,
            committed: committed.source_value(),
            backing_type: *backing_type,
            established_case: *established_case,
            rejected_case: *rejected_case,
            layout: sum(result_home)?,
        },
        Operation::ReleaseActivationClaim { source, claim, .. } => Kind::ReleaseActivationClaim {
            source: source.clone(),
            claim: *claim,
        },
        Operation::SplitActivationClaim {
            result_home,
            source,
            claim,
            bound_at,
            committed,
            covered_case,
            rejected_case,
            ..
        } => Kind::SplitActivationClaim {
            result: result(result_home)?,
            source: source.clone(),
            claim: *claim,
            bound_at: *bound_at,
            committed_at: committed.source_value(),
            covered_case: *covered_case,
            rejected_case: *rejected_case,
            layout: sum(result_home)?,
        },
        Operation::MergeActivationClaim {
            result_home,
            lower,
            upper,
            lower_claim,
            upper_claim,
            covered_case,
            rejected_case,
            ..
        } => Kind::MergeActivationClaim {
            result: result(result_home)?,
            lower: lower.clone(),
            upper: upper.clone(),
            lower_claim: *lower_claim,
            upper_claim: *upper_claim,
            covered_case: *covered_case,
            rejected_case: *rejected_case,
            layout: sum(result_home)?,
        },
        Operation::EstablishNativeCodeLayout {
            result_home,
            machine,
            ..
        } => Kind::EstablishNativeCodeLayout {
            result: result(result_home)?,
            machine: *machine,
            shape: result_home.layout.shape(),
        },
        Operation::EstablishRecord {
            result_home,
            fields,
            ..
        } => Kind::EstablishRecord {
            result: result(result_home)?,
            fields: fields.clone(),
            shape: result_home.layout.shape(),
        },
        Operation::EstablishScalarArray {
            result_home,
            elements,
            ..
        } => Kind::EstablishScalarArray {
            result: result(result_home)?,
            elements: elements.clone(),
            shape: result_home.layout.shape(),
        },
        Operation::EstablishCase {
            result_home,
            result_case,
            fields,
            ..
        } => Kind::EstablishCase {
            result: result(result_home)?,
            result_case: *result_case,
            fields: fields.clone(),
            layout: sum(result_home)?,
        },
        Operation::EstablishPrimitiveLocal {
            result,
            value,
            shape,
            ..
        } => Kind::EstablishPrimitiveLocal {
            result: result.clone(),
            value: *value,
            shape: *shape,
        },
        Operation::PrimitiveLocalStore {
            destination, value, ..
        } => Kind::PrimitiveLocalStore {
            destination: *destination,
            value: *value,
        },
        Operation::DecomposeStructuralValue { subject, .. } => {
            if source.result.is_some() {
                return Err(Error::instruction_metadata(
                    "decomposition unexpectedly defines a scalar result",
                ));
            }
            Kind::DecomposeStructuralValue {
                subject: subject.clone(),
            }
        }
        Operation::EstablishTrivialAffineLocal {
            place,
            structural_type,
            ..
        } => Kind::EstablishTrivialAffineLocal {
            place: *place,
            structural_type: structural_type.clone(),
        },
        Operation::ActivateTaskRegister {
            service,
            entries,
            gdt,
            task_state,
            selector,
            scratch,
            indices: selectors,
            ..
        } => Kind::ActivateTaskRegister {
            service: *service,
            entries: *entries,
            gdt: gdt.clone(),
            task_state: task_state.clone(),
            selector: selector.source_value(),
            scratch: scratch.clone(),
            indices: argument_indices(machine, facts, operation, selectors)?,
        },
        Operation::ReloadSegmentDescriptors {
            service,
            entries,
            gdt,
            kind,
            selector,
            scratch,
            indices: selectors,
            ..
        } => Kind::ReloadSegmentDescriptors {
            service: *service,
            entries: *entries,
            gdt: gdt.clone(),
            kind: *kind,
            selector: selector.source_value(),
            scratch: scratch.clone(),
            indices: argument_indices(machine, facts, operation, selectors)?,
        },
        Operation::ReadTaskRegister { service, .. } => Kind::ReadTaskRegister { service: *service },
        Operation::ReadSegmentSelector {
            service, register, ..
        } => Kind::ReadSegmentSelector {
            service: *service,
            register: *register,
        },
        Operation::GlobalDescriptorTable {
            service,
            operation,
            source,
            indices: selectors,
            ..
        } => Kind::GlobalDescriptorTable {
            service: *service,
            operation: *operation,
            source: source.place,
            path: source.path.clone(),
            indices: indices(selectors)?,
        },
        Operation::StorageAddress {
            source,
            path,
            indices: selectors,
            ..
        } => Kind::StorageAddress {
            source: *source,
            path: path.clone(),
            indices: indices(selectors)?,
        },
        Operation::IeeeFloatClassify {
            boundary,
            realization,
            result_home,
            argument,
            ..
        } => Kind::IeeeFloatClassify {
            boundary: *boundary,
            format: realization.format,
            argument: argument.source_value(),
            result: result(result_home)?,
            layout: sum(result_home)?,
        },
        Operation::NearestIeeeFloatArithmetic {
            boundary,
            realization,
            arguments,
            ..
        } => Kind::NearestIeeeFloatArithmetic {
            boundary: *boundary,
            realization: *realization,
            arguments: arguments
                .iter()
                .map(|argument| argument.source_value())
                .collect(),
        },
        Operation::NormalizedForeignCall { .. } => {
            call_instructions::project_normalized_foreign_call(
                machine, facts, native, operation, row,
            )?
        }
        Operation::BoundarySettlement {
            boundary,
            result: boundary_result,
            realization,
            runtime_scalar_arguments,
            ..
        } => match realization {
            target_operations::BoundaryRealization::HostedReadByte(_) => {
                let target_operations::TargetBoundaryResult::Structural(home) = boundary_result
                else {
                    return Err(Error::custody());
                };
                Kind::HostedReadByte {
                    boundary: *boundary,
                    result: result(home)?,
                    layout: sum(home)?,
                }
            }
            target_operations::BoundaryRealization::HostedWriteByteI32(_) => {
                let [argument] = runtime_scalar_arguments.as_slice() else {
                    return Err(Error::custody());
                };
                Kind::HostedWriteByteI32 {
                    boundary: *boundary,
                    source: argument.source.source_value(),
                }
            }
            _ => return Err(Error::custody()),
        },
        Operation::EstablishByteSequenceLiteral {
            place,
            structural_type,
            bytes,
            ..
        } => Kind::EstablishByteSequenceLiteral {
            destination: *place,
            structural_type: structural_type.clone(),
            bytes: bytes.clone(),
        },
        Operation::StructuralByteSequenceFieldByteStore {
            destination,
            field,
            index,
            value,
            length,
            obligation,
            ..
        } => Kind::StructuralByteSequenceFieldByteStore {
            destination: destination.clone(),
            field: *field,
            index: index.source_value(),
            value: value.source_value(),
            length: *length,
            obligation: *obligation,
            accepted_fact: fact(*obligation)?,
        },
        Operation::EstablishByteSequenceConcatenation {
            result,
            left,
            right,
            capacity_obligation,
            ..
        } => Kind::EstablishByteSequenceConcatenation {
            result: result.clone(),
            left: left.clone(),
            right: right.clone(),
            capacity_obligation: *capacity_obligation,
            accepted_fact: fact(*capacity_obligation)?,
        },
        Operation::ByteSequenceStore {
            destination,
            source,
            length,
            obligation,
            ..
        } => Kind::ByteSequenceStore {
            destination: *destination,
            source: *source,
            length: length.source_value(),
            obligation: *obligation,
            accepted_fact: fact(*obligation)?,
        },
        Operation::StructuralByteSequenceFieldStore {
            destination,
            field,
            source,
            length,
            obligation,
            ..
        } => Kind::StructuralByteSequenceFieldStore {
            destination: destination.clone(),
            field: *field,
            source: *source,
            length: length.source_value(),
            obligation: *obligation,
            accepted_fact: fact(*obligation)?,
        },
        Operation::ElementViewWrite {
            path,
            destination,
            indices: selectors,
            index,
            value,
            length,
            obligation,
            ..
        } => Kind::ElementViewWrite {
            path: path.clone(),
            destination: destination.clone(),
            indices: indices(selectors)?,
            index: index.source_value(),
            value: value.source_value(),
            length: *length,
            obligation: *obligation,
            accepted_fact: fact(*obligation)?,
        },
        Operation::IntegerConstant { value, .. } => Kind::Constant(*value),
        Operation::BooleanConstant { value, .. } => Kind::Constant(
            semantic_vocabulary::IntegerValue::Unsigned(u128::from(*value)),
        ),
        Operation::IeeeFloatConstant { value, .. } => {
            Kind::Constant(semantic_vocabulary::IntegerValue::Unsigned(match value {
                semantic_vocabulary::IeeeFloatValue::Binary32(bits) => u128::from(*bits),
                semantic_vocabulary::IeeeFloatValue::Binary64(bits) => u128::from(*bits),
            }))
        }
        Operation::CleanupCall { .. } | Operation::Continue { .. } | Operation::Return { .. } => {
            return Err(Error::custody());
        }
    };
    Ok(crate::legalization_source::scalar_graph::instruction::with_metadata(source, kind))
}

fn result(
    home: &target_operations::TargetStructuralHomeRequirement,
) -> Result<terminal_psi::StructuralOperationResult, Error> {
    home.operation_result()
        .map(|(_, result)| result.clone())
        .ok_or_else(Error::custody)
}

fn argument_indices<const COUNT: usize>(
    machine: MachineId,
    facts: &[optimization_unit::AcceptedObligationFact],
    operation: OperationId,
    selectors: &[Vec<target_operations::TargetStructuralRuntimeIndex>; COUNT],
) -> Result<[Vec<legalized_operations::LegalizedRuntimeIndexOperand>; COUNT], Error> {
    let mut indices = std::array::from_fn(|_| Vec::new());
    for (selectors, indices) in selectors.iter().zip(&mut indices) {
        *indices = retained_operands(facts, machine, operation, selectors)?;
    }
    Ok(indices)
}

fn sum(
    home: &target_operations::TargetStructuralHomeRequirement,
) -> Result<calling_conventions::ConventionalSumLayout, Error> {
    home.layout.sum().cloned().ok_or_else(Error::custody)
}

fn integer_operand(expression: &Integer) -> Result<ValueId, Error> {
    expression.operand_value().ok_or_else(Error::custody)
}

fn boolean_operand(expression: &Boolean) -> Result<ValueId, Error> {
    match expression {
        Boolean::BlockParameter(parameter) => Ok(parameter.value),
        Boolean::ScalarHome(home) => Ok(home.source_value),
        Boolean::Immediate { source_value, .. } | Boolean::Parameter { source_value, .. } => {
            Ok(*source_value)
        }
        _ => Err(Error::custody()),
    }
}

fn scalar(
    machine: MachineId,
    facts: &[optimization_unit::AcceptedObligationFact],
    operation: OperationId,
    scalar_type: ScalarType,
    expression: &Scalar,
    row: &Operation,
) -> Result<Kind, Error> {
    let fact = |obligation| {
        crate::legalization::obligations::retained_fact(facts, machine, operation, obligation)
    };
    Ok(match expression {
        Scalar::Integer {
            expression: Integer::ByteSequenceRead { .. },
            ..
        } => storage::project_byte_access(machine, facts, operation, row)?,
        Scalar::Integer {
            expression: Integer::ByteSequenceLength { .. } | Integer::ElementViewLength { .. },
            ..
        } => storage::project_sequence_length(machine, facts, operation, row)?,
        Scalar::Integer {
            expression: Integer::Immediate { value, .. },
            ..
        } => Kind::Constant(*value),
        Scalar::Boolean(Boolean::Immediate { value, .. }) => Kind::Constant(
            semantic_vocabulary::IntegerValue::Unsigned(u128::from(*value)),
        ),
        Scalar::Boolean(Boolean::Not { operand, .. }) => Kind::BooleanNot {
            operand: boolean_operand(operand)?,
        },
        Scalar::Boolean(Boolean::Equal { left, right, .. }) => Kind::Compare {
            predicate: legalized_operations::LegalizedScalarComparison::Equal,
            operand_type: ScalarType::Boolean,
            right_operand_type: ScalarType::Boolean,
            left: boolean_operand(left)?,
            right: boolean_operand(right)?,
        },
        Scalar::Boolean(
            Boolean::IntegerEqual {
                scalar_type,
                right_scalar_type,
                left,
                right,
                ..
            }
            | Boolean::IntegerLessThan {
                scalar_type,
                right_scalar_type,
                left,
                right,
                ..
            }
            | Boolean::IntegerLessOrEqual {
                scalar_type,
                right_scalar_type,
                left,
                right,
                ..
            },
        ) => Kind::Compare {
            predicate: match expression {
                Scalar::Boolean(Boolean::IntegerEqual { .. }) => {
                    legalized_operations::LegalizedScalarComparison::Equal
                }
                Scalar::Boolean(Boolean::IntegerLessThan { .. }) => {
                    legalized_operations::LegalizedScalarComparison::LessThan
                }
                _ => legalized_operations::LegalizedScalarComparison::LessOrEqual,
            },
            operand_type: ScalarType::Integer(*scalar_type),
            right_operand_type: ScalarType::Integer(*right_scalar_type),
            left: integer_operand(left)?,
            right: integer_operand(right)?,
        },
        Scalar::Boolean(Boolean::ByteSequencePredicate {
            operand, predicate, ..
        }) => Kind::ByteSequencePredicate {
            operand: operand.clone(),
            predicate: *predicate,
        },
        Scalar::Boolean(Boolean::ByteSequenceEqual { left, right, .. }) => {
            Kind::ByteSequenceEqual {
                left: left.clone(),
                right: right.clone(),
            }
        }
        Scalar::Integer { expression, .. } => match expression {
            Integer::BitwiseNot { operand, .. } => Kind::BitwiseNot {
                operand: integer_operand(operand)?,
            },
            Integer::BitwiseAnd { left, right, .. } => Kind::BitwiseAnd {
                left: integer_operand(left)?,
                right: integer_operand(right)?,
            },
            Integer::BitwiseOr { left, right, .. } => Kind::BitwiseOr {
                left: integer_operand(left)?,
                right: integer_operand(right)?,
            },
            Integer::BitwiseXor { left, right, .. } => Kind::BitwiseXor {
                left: integer_operand(left)?,
                right: integer_operand(right)?,
            },
            Integer::IntegerWiden {
                operand,
                source_type,
                ..
            } => Kind::IntegerWiden {
                operand: integer_operand(operand)?,
                source_type: *source_type,
            },
            Integer::IntegerExactCast {
                operand,
                source_type,
                obligation,
                ..
            } => Kind::IntegerExactCast {
                operand: integer_operand(operand)?,
                source_type: *source_type,
                obligation: *obligation,
                accepted_fact: fact(*obligation)?,
            },
            Integer::Shift {
                value,
                count,
                policy,
                direction,
                ..
            } => {
                use abstract_operations::{
                    IntegerShiftDirection as Direction, IntegerShiftPolicy as Policy,
                };
                let (value, count) = (integer_operand(value)?, integer_operand(count)?);
                match (policy, direction) {
                    (Policy::Wrapping, Direction::Left) => Kind::WrappingShiftLeft { value, count },
                    (Policy::Wrapping, Direction::Right) => {
                        Kind::WrappingShiftRight { value, count }
                    }
                    (Policy::Exact { obligation }, Direction::Left) => Kind::ExactShiftLeft {
                        value,
                        count,
                        obligation: *obligation,
                        accepted_fact: fact(*obligation)?,
                    },
                    (Policy::Exact { obligation }, Direction::Right) => Kind::ExactShiftRight {
                        value,
                        count,
                        obligation: *obligation,
                        accepted_fact: fact(*obligation)?,
                    },
                }
            }
            Integer::Arithmetic {
                arithmetic,
                left,
                right,
                ..
            } => arithmetic_instruction(
                machine,
                facts,
                operation,
                scalar_type,
                *arithmetic,
                integer_operand(left)?,
                integer_operand(right)?,
            )?,
            Integer::Trapping {
                primitive,
                operand_type,
                left,
                right,
                ..
            } => {
                let ScalarType::Integer(integer) = scalar_type else {
                    return Err(Error::custody());
                };
                let form = crate::legalization::scalar_graph_input::trapping_form(
                    integer,
                    *operand_type,
                    *primitive,
                )
                .ok_or_else(Error::custody)?;
                if *primitive == terminal_psi::TrappingIntegerPrimitive::Convert {
                    Kind::TrappingConvert {
                        form,
                        source: crate::legalization::scalar_graph_input::saturating_carrier(
                            *operand_type,
                        )
                        .ok_or_else(Error::custody)?,
                        operand: integer_operand(left)?,
                    }
                } else {
                    Kind::TrappingBinary {
                        form,
                        left: integer_operand(left)?,
                        right: integer_operand(right.as_ref().ok_or_else(Error::custody)?)?,
                    }
                }
            }
            _ => return Err(Error::custody()),
        },
        _ => return Err(Error::custody()),
    })
}

#[allow(clippy::too_many_arguments)]
fn arithmetic_instruction(
    machine: MachineId,
    facts: &[optimization_unit::AcceptedObligationFact],
    operation: OperationId,
    scalar_type: ScalarType,
    arithmetic: abstract_operations::IntegerArithmetic,
    left: ValueId,
    right: ValueId,
) -> Result<Kind, Error> {
    use abstract_operations::{
        IntegerArithmeticOperator as Operator, IntegerOverflowPolicy as Policy,
    };
    let fact = || {
        let obligation = arithmetic.obligation().ok_or_else(Error::custody)?;
        Ok::<_, Error>((
            obligation,
            crate::legalization::obligations::retained_fact(facts, machine, operation, obligation)?,
        ))
    };
    let carrier = || {
        let ScalarType::Integer(integer) = scalar_type else {
            return Err(Error::custody());
        };
        crate::legalization::scalar_graph_input::saturating_carrier(integer)
            .ok_or_else(Error::custody)
    };
    Ok(match (arithmetic.policy(), arithmetic.operator()) {
        (Policy::Wrapping, Operator::Add) => Kind::WrappingAdd { left, right },
        (Policy::Wrapping, Operator::Subtract) => Kind::WrappingSubtract { left, right },
        (Policy::Wrapping, Operator::Multiply) => Kind::WrappingMultiply { left, right },
        (Policy::Wrapping, operator @ (Operator::Divide | Operator::Remainder)) => {
            let (obligation, accepted_fact) = fact()?;
            let ScalarType::Integer(integer) = scalar_type else {
                return Err(Error::custody());
            };
            if !crate::legalization::scalar_graph_input::supports_wrapping_division(integer) {
                return Err(Error::custody());
            }
            if operator == Operator::Divide {
                Kind::WrappingDivide {
                    left,
                    right,
                    obligation,
                    accepted_fact,
                }
            } else {
                Kind::WrappingRemainder {
                    left,
                    right,
                    obligation,
                    accepted_fact,
                }
            }
        }
        (Policy::Saturating, Operator::Add) => Kind::SaturatingAdd {
            carrier: carrier()?,
            left,
            right,
        },
        (Policy::Saturating, Operator::Subtract) => Kind::SaturatingSubtract {
            carrier: carrier()?,
            left,
            right,
        },
        (Policy::Saturating, Operator::Multiply) => Kind::SaturatingMultiply {
            carrier: carrier()?,
            left,
            right,
        },
        (Policy::Saturating, operator @ (Operator::Divide | Operator::Remainder)) => {
            let (obligation, accepted_fact) = fact()?;
            if operator == Operator::Divide {
                Kind::SaturatingDivide {
                    carrier: carrier()?,
                    left,
                    right,
                    obligation,
                    accepted_fact,
                }
            } else {
                Kind::SaturatingRemainder {
                    carrier: carrier()?,
                    left,
                    right,
                    obligation,
                    accepted_fact,
                }
            }
        }
        (Policy::Exact, operator) => {
            let (obligation, accepted_fact) = fact()?;
            Kind::ExactBinary {
                operator: match operator {
                    Operator::Add => legalized_operations::LegalizedExactIntegerOperator::Add,
                    Operator::Subtract => {
                        legalized_operations::LegalizedExactIntegerOperator::Subtract
                    }
                    Operator::Multiply => {
                        legalized_operations::LegalizedExactIntegerOperator::Multiply
                    }
                    Operator::Divide => legalized_operations::LegalizedExactIntegerOperator::Divide,
                    Operator::Remainder => {
                        legalized_operations::LegalizedExactIntegerOperator::Remainder
                    }
                },
                left,
                right,
                obligation,
                accepted_fact,
            }
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use abstract_operations::{
        IntegerArithmetic, IntegerArithmeticOperator, IntegerOverflowPolicy,
    };
    use semantic_vocabulary::{IntegerSign, IntegerType, ObligationId};

    #[test]
    fn retained_scalar_execution_keeps_operands_and_requires_the_exact_fact() {
        let machine = MachineId::new(1).unwrap();
        let operation = OperationId::new(2).unwrap();
        let left = ValueId::new(3).unwrap();
        let right = ValueId::new(4).unwrap();
        let obligation = ObligationId::new(5).unwrap();
        let scalar_type = ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 64).unwrap());
        let wrapping = IntegerArithmetic::new(
            IntegerOverflowPolicy::Wrapping,
            IntegerArithmeticOperator::Subtract,
            None,
        )
        .unwrap();
        assert_eq!(
            arithmetic_instruction(machine, &[], operation, scalar_type, wrapping, left, right)
                .unwrap(),
            Kind::WrappingSubtract { left, right }
        );
        let exact = IntegerArithmetic::new(
            IntegerOverflowPolicy::Exact,
            IntegerArithmeticOperator::Subtract,
            Some(obligation),
        )
        .unwrap();
        assert!(
            arithmetic_instruction(machine, &[], operation, scalar_type, exact, left, right)
                .is_err()
        );
        let fact = optimization_unit::AcceptedObligationFact::new(
            terminal_psi::TerminalPsiIdentity {
                vocabulary_marker: terminal_psi::VocabularyMarker::CURRENT,
                program_fingerprint: terminal_psi::SemanticFingerprint::from_bytes([6; 32]),
            },
            [7; 32],
            machine,
            operation,
            obligation,
            vec![8],
        );
        assert_eq!(
            arithmetic_instruction(
                machine,
                std::slice::from_ref(&fact),
                operation,
                scalar_type,
                exact,
                left,
                right
            )
            .unwrap(),
            Kind::ExactBinary {
                operator: legalized_operations::LegalizedExactIntegerOperator::Subtract,
                left,
                right,
                obligation,
                accepted_fact: fact.identity
            }
        );
        let foreign_operation = OperationId::new(9).unwrap();
        assert!(
            arithmetic_instruction(
                machine,
                &[fact],
                foreign_operation,
                scalar_type,
                exact,
                left,
                right
            )
            .is_err()
        );
    }
}
