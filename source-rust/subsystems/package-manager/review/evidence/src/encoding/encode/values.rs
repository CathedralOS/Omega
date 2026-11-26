// Canonical semantic-value encoders grouped by the value families they encode.

pub(super) mod conformance_policy;
pub(super) mod contracts;
pub(super) mod declarations;
pub(super) mod effects;
pub(super) mod expressions;
pub(super) mod external_policy;
pub(super) mod identity;
pub(super) mod providers;

use super::encoder::Encoder;
use crate::encoding::PackageReviewEncodingError;
use crate::encoding::encode::declarations::{
    calling_policy_tag, encode_machine_register, encode_value_placement,
};
use crate::record::{
    PackagePolicyEntryControl, PackagePolicyEntryStack, PackagePolicyMachineRegime,
    PackagePolicyMachineState, PackagePolicyMachineStateSet, PackagePolicyPhysicalCallingContract,
    PackagePolicyPreemption, PackagePolicyStatePlan, PackageReviewArithmeticDomain,
    PackageReviewBooleanExpression, PackageReviewBoundaryCallingPolicy,
    PackageReviewIeeeFloatComparisonKind, PackageReviewIntegerBinaryKind,
    PackageReviewIntegerComparisonKind, PackageReviewPrimitiveType, PackageReviewRuntimeIndex,
    PackageReviewScalarExpression, PackageReviewStructuralParameterField,
    PackageReviewStructuralPredicatePathSegment,
};

// Canonical encoding for crash-route structural fields, predicate paths, and runtime indices.

pub(crate) fn encode_structural_field(
    encoder: &mut Encoder,
    field: &PackageReviewStructuralParameterField,
) -> Result<(), PackageReviewEncodingError> {
    encoder.u32_field("parameter_position", field.parameter_position)?;
    encoder.field_by("path", field.path.as_slice(), encode_structural_path)
}

pub(crate) fn encode_structural_path(
    encoder: &mut Encoder,
    path: &[PackageReviewStructuralPredicatePathSegment],
) -> Result<(), PackageReviewEncodingError> {
    encoder.sequence(path, |encoder, segment| {
        match segment {
            PackageReviewStructuralPredicatePathSegment::Referent => encoder.tag("referent", 4),
            PackageReviewStructuralPredicatePathSegment::Field(field) => {
                encoder.tag("field", 0);
                encoder.field("field", |encoder| encoder.string(field))?;
            }
            PackageReviewStructuralPredicatePathSegment::Case(case) => {
                encoder.tag("case", 1);
                encoder.field("case", |encoder| encoder.string(case))?;
            }
            PackageReviewStructuralPredicatePathSegment::FixedIndex(element_index) => {
                encoder.tag("fixed_index", 2);
                encoder.u64_field("index", *element_index)?;
            }
            PackageReviewStructuralPredicatePathSegment::RuntimeIndex(index) => {
                encoder.tag("runtime_index", 3);
                encoder.field("index", |encoder| encode_runtime_index(encoder, index))?;
            }
        }
        Ok(())
    })
}

pub(in crate::encoding::encode::values) fn encode_runtime_index(
    encoder: &mut Encoder,
    index: &PackageReviewRuntimeIndex,
) -> Result<(), PackageReviewEncodingError> {
    match index {
        PackageReviewRuntimeIndex::Parameter { position } => {
            encoder.tag("parameter", 0);
            encoder.u32(*position);
        }
        PackageReviewRuntimeIndex::AssignmentIndex { depth } => {
            encoder.tag("assignment_index", 1);
            encoder.u32(*depth);
        }
        PackageReviewRuntimeIndex::StructuralField(field) => {
            encoder.tag("structural_field", 2);
            encoder.u32(field.parameter_position);
            encode_structural_path(encoder, &field.path)?;
        }
        PackageReviewRuntimeIndex::Local { position } => {
            encoder.tag("local", 3);
            encoder.u32(*position);
        }
    }
    Ok(())
}

// Structural Boolean expression encoding with shared binary tags and named text fields.
// `ieee_float_comparison_row` maps each IEEE float comparison kind to its persisted tag and authored spelling so structural and scalar comparison arms read one roster.

pub(crate) fn encode_boolean_expression(
    encoder: &mut Encoder,
    expression: &PackageReviewBooleanExpression,
) -> Result<(), PackageReviewEncodingError> {
    encoder.nested(|encoder| encode_boolean_node(encoder, expression))
}

pub(in crate::encoding::encode::values) fn encode_boolean_node(
    encoder: &mut Encoder,
    expression: &PackageReviewBooleanExpression,
) -> Result<(), PackageReviewEncodingError> {
    match expression {
        PackageReviewBooleanExpression::Constant(value) => {
            encoder.tag("constant", 0);
            encoder.boolean_field("value", *value)?;
        }
        PackageReviewBooleanExpression::Parameter { position } => {
            encoder.tag("parameter", 1);
            encoder.field("position", |encoder| encoder.usize(*position))?;
        }
        PackageReviewBooleanExpression::Local { position } => {
            encoder.tag("local", 2);
            encoder.field("position", |encoder| encoder.usize(*position))?;
        }
        PackageReviewBooleanExpression::StructuralParameterField {
            parameter_position,
            path,
        } => {
            encoder.tag("structural_parameter_field", 3);
            encoder.u32_field("parameter_position", *parameter_position)?;
            encoder.field("path", |encoder| encode_structural_path(encoder, path))?;
        }
        PackageReviewBooleanExpression::Not(operand) => {
            encoder.tag("not", 4);
            encoder.field_by("operand", &**operand, encode_boolean_expression)?;
        }
        PackageReviewBooleanExpression::Equal { left, right } => {
            encoder.tag("equal", 5);
            encoder.field("left", |encoder| encode_boolean_expression(encoder, left))?;
            encoder.field("right", |encoder| encode_boolean_expression(encoder, right))?;
        }
        PackageReviewBooleanExpression::IntegerComparison { kind, left, right } => {
            encoder.tag("integer_comparison", 6);
            encoder.field("kind", |encoder| {
                let (tag, name) = integer_comparison_row(*kind);
                encoder.tag(name, tag);
                Ok(())
            })?;
            encoder.field("left", |encoder| encode_scalar_expression(encoder, left))?;
            encoder.field("right", |encoder| encode_scalar_expression(encoder, right))?;
        }
        PackageReviewBooleanExpression::IeeeFloatComparison {
            kind,
            primitive_type,
            left,
            right,
        } => {
            encoder.tag("ieee_float_comparison", 7);
            encoder.field("kind", |encoder| {
                let (tag, name) = ieee_float_comparison_row(*kind);
                encoder.tag(name, tag);
                Ok(())
            })?;
            encoder.field_by("primitive_type", *primitive_type, encode_primitive_type)?;
            encoder.field("left", |encoder| encode_structural_field(encoder, left))?;
            encoder.field("right", |encoder| encode_structural_field(encoder, right))?;
        }
        PackageReviewBooleanExpression::ByteSequenceEqual { left, right } => {
            encoder.tag("byte_sequence_equal", 8);
            encoder.field("left", |encoder| encode_structural_field(encoder, left))?;
            encoder.field("right", |encoder| encode_structural_field(encoder, right))?;
        }
        PackageReviewBooleanExpression::StructuralIdentity { left, right } => {
            encoder.tag("structural_identity", 14);
            encoder.field("left", |encoder| encode_structural_field(encoder, left))?;
            encoder.field("right", |encoder| encode_structural_field(encoder, right))?;
        }
        PackageReviewBooleanExpression::StructuralCaseMembership { subject, case } => {
            encoder.tag("structural_case_membership", 10);
            encoder.field_by("subject", subject, encode_structural_field)?;
            encoder.field("case", |encoder| encoder.string(case))?;
        }
        PackageReviewBooleanExpression::And { left, right } => {
            encoder.tag("and", 11);
            encoder.field("left", |encoder| encode_boolean_expression(encoder, left))?;
            encoder.field("right", |encoder| encode_boolean_expression(encoder, right))?;
        }
        PackageReviewBooleanExpression::ScalarIeeeFloatComparison { kind, left, right } => {
            encoder.tag("scalar_ieee_float_comparison", 13);
            encoder.field("kind", |encoder| {
                let (tag, name) = ieee_float_comparison_row(*kind);
                encoder.tag(name, tag);
                Ok(())
            })?;
            encoder.field("left", |encoder| encode_scalar_expression(encoder, left))?;
            encoder.field("right", |encoder| encode_scalar_expression(encoder, right))?;
        }
        PackageReviewBooleanExpression::Or { left, right } => {
            encoder.tag("or", 12);
            encoder.field("left", |encoder| encode_boolean_expression(encoder, left))?;
            encoder.field("right", |encoder| encode_boolean_expression(encoder, right))?;
        }
    }
    Ok(())
}

pub(in crate::encoding::encode::values) const fn ieee_float_comparison_row(
    kind: PackageReviewIeeeFloatComparisonKind,
) -> (u8, &'static str) {
    match kind {
        PackageReviewIeeeFloatComparisonKind::Equal => (0, "equal"),
        PackageReviewIeeeFloatComparisonKind::NotEqual => (1, "not_equal"),
        PackageReviewIeeeFloatComparisonKind::LessThan => (2, "less_than"),
        PackageReviewIeeeFloatComparisonKind::LessOrEqual => (3, "less_or_equal"),
        PackageReviewIeeeFloatComparisonKind::NotLessThan => (4, "not_less_than"),
        PackageReviewIeeeFloatComparisonKind::NotLessOrEqual => (5, "not_less_or_equal"),
    }
}

// Structural scalar expression encoding preserving exact numeric vocabulary.

pub(crate) fn encode_scalar_expression(
    encoder: &mut Encoder,
    expression: &PackageReviewScalarExpression,
) -> Result<(), PackageReviewEncodingError> {
    encoder.nested(|encoder| encode_scalar_node(encoder, expression))
}

pub(in crate::encoding::encode::values) fn encode_scalar_node(
    encoder: &mut Encoder,
    expression: &PackageReviewScalarExpression,
) -> Result<(), PackageReviewEncodingError> {
    match expression {
        PackageReviewScalarExpression::Float32Literal(bits) => {
            encoder.tag("float32_literal", 9);
            encoder.u32_field("bits", *bits)?;
        }
        PackageReviewScalarExpression::Float64Literal(bits) => {
            encoder.tag("float64_literal", 10);
            encoder.u64_field("bits", *bits)?;
        }
        PackageReviewScalarExpression::Parameter {
            position,
            primitive_type,
        } => {
            encoder.tag("parameter", 0);
            encoder.field("position", |encoder| encoder.usize(*position))?;
            encoder.field_by("primitive_type", *primitive_type, encode_primitive_type)?;
        }
        PackageReviewScalarExpression::Local {
            position,
            primitive_type,
        } => {
            encoder.tag("local", 1);
            encoder.field("position", |encoder| encoder.usize(*position))?;
            encoder.field_by("primitive_type", *primitive_type, encode_primitive_type)?;
        }
        PackageReviewScalarExpression::StructuralParameterField {
            parameter_position,
            path,
            primitive_type,
        } => {
            encoder.tag("structural_parameter_field", 2);
            encoder.u32_field("parameter_position", *parameter_position)?;
            encoder.field("path", |encoder| encode_structural_path(encoder, path))?;
            encoder.field_by("primitive_type", *primitive_type, encode_primitive_type)?;
        }
        PackageReviewScalarExpression::IntegerLiteral(literal) => {
            encoder.tag("integer_literal", 3);
            encoder.string_field("canonical_text", &literal.canonical_text)?;
            encoder.field("landing", |encoder| {
                encoder.option(literal.landing.as_ref(), |encoder, landing| {
                    encoder.field_by("landed_type", landing.landed_type, encode_primitive_type)?;
                    encoder.string_field(
                        "arithmetic_domain",
                        arithmetic_domain_name(landing.arithmetic_domain),
                    )
                })
            })?;
        }
        PackageReviewScalarExpression::IntegerBinary {
            kind,
            primitive_type,
            left,
            right,
        } => {
            encoder.tag("integer_binary", 4);
            encoder.field("kind", |encoder| {
                let (tag, name) = integer_binary_row(*kind);
                encoder.tag(name, tag);
                Ok(())
            })?;
            encoder.field_by("primitive_type", *primitive_type, encode_primitive_type)?;
            encoder.field("left", |encoder| encode_scalar_expression(encoder, left))?;
            encoder.field("right", |encoder| encode_scalar_expression(encoder, right))?;
        }
        PackageReviewScalarExpression::IntegerBitwiseNot {
            primitive_type,
            operand,
        } => {
            encoder.tag("integer_bitwise_not", 5);
            encoder.field_by("primitive_type", *primitive_type, encode_primitive_type)?;
            encoder.field_by("operand", &**operand, encode_scalar_expression)?;
        }
        PackageReviewScalarExpression::IntegerWiden {
            primitive_type,
            operand,
        } => {
            encoder.tag("integer_widen", 6);
            encoder.field_by("primitive_type", *primitive_type, encode_primitive_type)?;
            encoder.field_by("operand", &**operand, encode_scalar_expression)?;
        }
        PackageReviewScalarExpression::IntegerExactCast {
            primitive_type,
            operand,
            range,
        } => {
            encoder.tag("integer_exact_cast", 7);
            encoder.field_by("primitive_type", *primitive_type, encode_primitive_type)?;
            encoder.field_by("operand", &**operand, encode_scalar_expression)?;
            encoder.field("minimum", |encoder| encoder.string(&range.minimum))?;
            encoder.field("maximum", |encoder| encoder.string(&range.maximum))?;
        }
        PackageReviewScalarExpression::RepresentationView {
            primitive_type,
            operand,
        } => {
            encoder.tag("representation_view", 14);
            encoder.field_by("primitive_type", *primitive_type, encode_primitive_type)?;
            encoder.field_by("operand", &**operand, encode_scalar_expression)?;
        }
        PackageReviewScalarExpression::StructuralParameterByteLength {
            parameter_position,
            path,
        } => {
            encoder.tag("structural_parameter_byte_length", 15);
            encoder.u32_field("parameter_position", *parameter_position)?;
            encoder.field("path", |encoder| encode_structural_path(encoder, path))?;
        }
        PackageReviewScalarExpression::StructuralParameterIndexedRead {
            parameter_position,
            path,
            index,
            inner_indexes,
            element_path,
            primitive_type,
        } => {
            encoder.tag("structural_parameter_indexed_read", 16);
            encoder.u32_field("parameter_position", *parameter_position)?;
            encoder.field("path", |encoder| encode_structural_path(encoder, path))?;
            encoder.field("index", |encoder| encode_scalar_expression(encoder, index))?;
            encoder.sequence_field("inner_indexes", inner_indexes, encode_scalar_expression)?;
            encoder.field_by(
                "element_path",
                element_path.as_slice(),
                encode_structural_path,
            )?;
            encoder.field_by("primitive_type", *primitive_type, encode_primitive_type)?;
        }
        PackageReviewScalarExpression::Boolean(expression) => {
            encoder.tag("boolean", 8);
            encoder.field_by("expression", &**expression, encode_boolean_expression)?;
        }
    }
    Ok(())
}

// Structural integer types retain their sign and width, not a host-width catalog.
// Arithmetic and comparison operators remain closed tagged vocabularies.

pub(crate) fn encode_primitive_type(
    encoder: &mut Encoder,
    primitive_type: PackageReviewPrimitiveType,
) -> Result<(), PackageReviewEncodingError> {
    match primitive_type {
        PackageReviewPrimitiveType::Bool => encoder.tag("bool", 0),
        PackageReviewPrimitiveType::F32 => encoder.tag("f32", 1),
        PackageReviewPrimitiveType::F64 => encoder.tag("f64", 2),
        PackageReviewPrimitiveType::Integer(carrier) => {
            encoder.tag("integer", 3);
            encoder.boolean_field("signed", carrier.is_signed())?;
            encoder.u64_field("width", carrier.width())?;
        }
        PackageReviewPrimitiveType::Addr => encoder.tag("addr", 11),
        PackageReviewPrimitiveType::Selector => encoder.tag("selector", 12),
    }
    Ok(())
}

pub(crate) const fn integer_comparison_row(
    kind: PackageReviewIntegerComparisonKind,
) -> (u8, &'static str) {
    match kind {
        PackageReviewIntegerComparisonKind::Equal => (0, "equal"),
        PackageReviewIntegerComparisonKind::LessThan => (1, "less_than"),
        PackageReviewIntegerComparisonKind::LessOrEqual => (2, "less_or_equal"),
    }
}

pub(crate) const fn integer_binary_row(kind: PackageReviewIntegerBinaryKind) -> (u8, &'static str) {
    match kind {
        PackageReviewIntegerBinaryKind::ExactAdd => (0, "exact_add"),
        PackageReviewIntegerBinaryKind::ExactSubtract => (1, "exact_subtract"),
        PackageReviewIntegerBinaryKind::ExactMultiply => (2, "exact_multiply"),
        PackageReviewIntegerBinaryKind::ExactDivide => (3, "exact_divide"),
        PackageReviewIntegerBinaryKind::ExactRemainder => (4, "exact_remainder"),
        PackageReviewIntegerBinaryKind::WrappingDivide => (5, "wrapping_divide"),
        PackageReviewIntegerBinaryKind::WrappingRemainder => (6, "wrapping_remainder"),
        PackageReviewIntegerBinaryKind::SaturatingDivide => (7, "saturating_divide"),
        PackageReviewIntegerBinaryKind::SaturatingRemainder => (8, "saturating_remainder"),
        PackageReviewIntegerBinaryKind::WrappingAdd => (9, "wrapping_add"),
        PackageReviewIntegerBinaryKind::SaturatingAdd => (10, "saturating_add"),
        PackageReviewIntegerBinaryKind::WrappingSubtract => (11, "wrapping_subtract"),
        PackageReviewIntegerBinaryKind::SaturatingSubtract => (12, "saturating_subtract"),
        PackageReviewIntegerBinaryKind::WrappingMultiply => (13, "wrapping_multiply"),
        PackageReviewIntegerBinaryKind::SaturatingMultiply => (14, "saturating_multiply"),
        PackageReviewIntegerBinaryKind::BitwiseAnd => (15, "bitwise_and"),
        PackageReviewIntegerBinaryKind::BitwiseOr => (16, "bitwise_or"),
        PackageReviewIntegerBinaryKind::BitwiseXor => (17, "bitwise_xor"),
        PackageReviewIntegerBinaryKind::WrappingShiftLeft => (18, "wrapping_shift_left"),
        PackageReviewIntegerBinaryKind::WrappingShiftRight => (19, "wrapping_shift_right"),
        PackageReviewIntegerBinaryKind::ExactShiftLeft => (20, "exact_shift_left"),
        PackageReviewIntegerBinaryKind::ExactShiftRight => (21, "exact_shift_right"),
        PackageReviewIntegerBinaryKind::TrappingShiftLeft => (22, "trapping_shift_left"),
        PackageReviewIntegerBinaryKind::TrappingShiftRight => (23, "trapping_shift_right"),
        PackageReviewIntegerBinaryKind::TrappingAdd => (24, "trapping_add"),
        PackageReviewIntegerBinaryKind::TrappingSubtract => (25, "trapping_subtract"),
        PackageReviewIntegerBinaryKind::TrappingMultiply => (26, "trapping_multiply"),
        PackageReviewIntegerBinaryKind::TrappingDivide => (27, "trapping_divide"),
        PackageReviewIntegerBinaryKind::TrappingRemainder => (28, "trapping_remainder"),
        PackageReviewIntegerBinaryKind::SaturatingShiftLeft => (29, "saturating_shift_left"),
        PackageReviewIntegerBinaryKind::SaturatingShiftRight => (30, "saturating_shift_right"),
    }
}

pub(in crate::encoding::encode::values) const fn arithmetic_domain_name(
    domain: PackageReviewArithmeticDomain,
) -> &'static str {
    match domain {
        PackageReviewArithmeticDomain::Exact => "Exact",
        PackageReviewArithmeticDomain::Wrapping => "Wrapping",
        PackageReviewArithmeticDomain::Saturating => "Saturating",
        PackageReviewArithmeticDomain::Trapping => "Trapping",
    }
}

// Bounded physical-component bytes for boundary calling policies, never a complete boundary-policy record.

pub(crate) fn encode_physical(
    encoder: &mut Encoder,
    physical: &PackagePolicyPhysicalCallingContract,
) -> Result<(), PackageReviewEncodingError> {
    if physical
        .ordinary_clobbers()
        .windows(2)
        .any(|pair| pair[0] >= pair[1])
    {
        return Err(PackageReviewEncodingError::new(
            "physical calling policy requires canonical register sets",
        ));
    }
    encoder.field("policy", |encoder| {
        let name = match physical.policy() {
            PackageReviewBoundaryCallingPolicy::MicrosoftX64 => "microsoft_x64",
            PackageReviewBoundaryCallingPolicy::SystemVAMD64 => "system_v_amd64",
            PackageReviewBoundaryCallingPolicy::Aapcs64 => "aapcs64",
            PackageReviewBoundaryCallingPolicy::LinuxSyscallX86_64 => "linux_syscall_x86_64",
            PackageReviewBoundaryCallingPolicy::LinuxSyscallAarch64 => "linux_syscall_aarch64",
            PackageReviewBoundaryCallingPolicy::DarwinAapcs64 => "darwin_aapcs64",
        };
        encoder.tag(name, calling_policy_tag(physical.policy()));
        Ok(())
    })?;
    encoder.sequence_field("parameters", physical.parameters(), encode_value_placement)?;
    encoder.option_field("result", physical.result(), encode_value_placement)?;
    encoder.field("ordinary_clobbers", |encoder| {
        encoder.sequence(physical.ordinary_clobbers(), |encoder, register| {
            encoder.field_by("register", *register, encode_machine_register)?;
            Ok(())
        })
    })?;
    encoder.u16_field("stack_alignment", physical.stack_alignment())?;
    encoder.u16_field("shadow_bytes", physical.shadow_bytes())?;
    encoder.field("entry_control", |encoder| {
        match physical.entry_control() {
            PackagePolicyEntryControl::CallReturn => encoder.tag("call_return", 0),
            PackagePolicyEntryControl::SupervisorCall {
                number_register,
                immediate,
            } => {
                encoder.tag("supervisor_call", 1);
                encoder.field_by("number_register", number_register, encode_machine_register)?;
                encoder.u16_field("immediate", immediate)?;
            }
            PackagePolicyEntryControl::InterruptReturn => encoder.tag("interrupt_return", 2),
        };
        Ok(())
    })?;
    encoder.field("state", |encoder| encode_state(encoder, physical.state()))
}

pub(in crate::encoding::encode::values) fn encode_state(
    encoder: &mut Encoder,
    state: &PackagePolicyStatePlan,
) -> Result<(), PackageReviewEncodingError> {
    encoder.field("initial_regime", |encoder| {
        match state.initial_regime() {
            PackagePolicyMachineRegime::X86Long64 => encoder.tag("x86_long64", 0),
            PackagePolicyMachineRegime::Aarch64A64 { exception_level } => {
                encoder.tag("aarch64_a64", 1);
                encoder.byte_field("exception_level", exception_level)?;
            }
        };
        Ok(())
    })?;
    encoder.field_by(
        "interrupted_state",
        state.interrupted_state(),
        encode_state_set,
    )?;
    encoder.field_by("saved_state", state.saved_state(), encode_state_set)?;
    encoder.field_by("restored_state", state.restored_state(), encode_state_set)?;
    encoder.field_by(
        "permitted_transitive_use",
        state.permitted_transitive_use(),
        encode_state_set,
    )?;
    encoder.field("stack", |encoder| {
        match state.stack() {
            PackagePolicyEntryStack::Interrupted => encoder.tag("interrupted", 0),
            PackagePolicyEntryStack::Dedicated { class } => {
                encoder.tag("dedicated", 1);
                encoder.u16_field("class", class)?;
            }
            PackagePolicyEntryStack::ProviderSelected => encoder.tag("provider_selected", 2),
        };
        Ok(())
    })?;
    encoder.field("preemption", |encoder| {
        match state.preemption() {
            PackagePolicyPreemption::NotApplicable => encoder.tag("not_applicable", 0),
            PackagePolicyPreemption::Masked => encoder.tag("masked", 1),
            PackagePolicyPreemption::Nestable { maximum_depth } => {
                encoder.tag("nestable", 2);
                encoder.u16_field("maximum_depth", maximum_depth)?;
            }
            PackagePolicyPreemption::ProviderDefined => encoder.tag("provider_defined", 3),
        };
        Ok(())
    })?;
    Ok(())
}

pub(in crate::encoding::encode::values) fn encode_state_set(
    encoder: &mut Encoder,
    states: &PackagePolicyMachineStateSet,
) -> Result<(), PackageReviewEncodingError> {
    encoder.field("states", |encoder| {
        encoder.sequence(states.as_slice(), |encoder, state| {
            encoder.field("state", |encoder| {
                match state {
                    PackagePolicyMachineState::GeneralRegisters => {
                        encoder.tag("general_registers", 0)
                    }
                    PackagePolicyMachineState::VectorRegisters => {
                        encoder.tag("vector_registers", 1)
                    }
                    PackagePolicyMachineState::Flags => encoder.tag("flags", 2),
                    PackagePolicyMachineState::InstructionPointer => {
                        encoder.tag("instruction_pointer", 3)
                    }
                    PackagePolicyMachineState::StackPointer => encoder.tag("stack_pointer", 4),
                    PackagePolicyMachineState::SegmentState => encoder.tag("segment_state", 5),
                    PackagePolicyMachineState::ControlState => encoder.tag("control_state", 6),
                    PackagePolicyMachineState::DebugState => encoder.tag("debug_state", 7),
                    PackagePolicyMachineState::ExtendedState => encoder.tag("extended_state", 8),
                };
                Ok(())
            })?;
            Ok(())
        })
    })
}
