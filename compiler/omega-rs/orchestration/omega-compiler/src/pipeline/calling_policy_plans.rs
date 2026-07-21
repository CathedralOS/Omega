//! Build-time evaluation of source-authored calling policies.
//!
//! The source vocabulary lives in `omega/language/std/calling.omg`; this
//! module is the compiler-owned decoder that keeps that open policy surface
//! behind the closed normalized-plan validator.

use omega_calling_conventions::{
    BoundaryEntryPlan, BoundaryPlanResult, CallPlan, CallSignature, CallingPolicy,
    CallingPolicyRejection, EntryControl, EntryStack, IndirectPointerLocation, MachineRegime,
    MachineRegister, MachineState, MachineStateSet, Preemption, RegisterSet, StatePlan,
    SystemVEightbyteClass, ValidatedBoundaryEntryPlan, ValueClass, ValueLocation, ValuePlacement,
    ValueShape, validate_boundary_plan_result,
};
use omega_interpreter::BuildTimeValue;
use omega_typed_trees::TypedTrees;

const PARAMETER_CAPACITY: usize = 32;
const LOCATION_CAPACITY: usize = 16;
const REGISTER_CAPACITY: usize = 64;

pub fn evaluate_calling_policy_plan(
    typed: &TypedTrees,
    policy_machine: &str,
    signature: &CallSignature,
) -> Result<ValidatedBoundaryEntryPlan, String> {
    if signature.parameters.len() > PARAMETER_CAPACITY {
        return Err(format!(
            "boundary signature has {} parameters; calling policies currently support at most {PARAMETER_CAPACITY}",
            signature.parameters.len()
        ));
    }

    let effect_plan = omega_effects::infer_effects(typed);
    let machine = typed
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == policy_machine)
        .ok_or_else(|| format!("no calling-policy machine named `{policy_machine}` exists"))?;
    let transitive = effect_plan
        .machines()
        .iter()
        .find(|entry| entry.symbol == machine.symbol)
        .map(|entry| entry.transitive)
        .unwrap_or_else(omega_effects::EffectSet::empty);
    if !transitive.is_empty() {
        return Err(format!(
            "calling-policy machine `{policy_machine}` is not effect-free: it reaches effects `{}`; only effect-free machines run at build time",
            transitive.names().collect::<Vec<_>>().join(", ")
        ));
    }

    let value = omega_interpreter::evaluate_build_time_machine(
        typed,
        policy_machine,
        vec![build_boundary_signature(signature)],
    )
    .map_err(|reason| {
        format!("build-time evaluation of calling policy `{policy_machine}` failed: {reason}")
    })?;
    let result = decode_boundary_plan_result(&value).map_err(|reason| {
        format!("calling policy `{policy_machine}` returned an invalid result: {reason}")
    })?;

    validate_boundary_plan_result(result, signature).map_err(|diagnostic| diagnostic.to_string())
}

fn build_boundary_signature(signature: &CallSignature) -> BuildTimeValue {
    let mut parameters = Vec::with_capacity(PARAMETER_CAPACITY);
    for index in 0..PARAMETER_CAPACITY {
        parameters.push(build_value_shape(
            signature
                .parameters
                .get(index)
                .copied()
                .unwrap_or_else(|| ValueShape::integer(0, 0)),
        ));
    }
    BuildTimeValue::Struct {
        type_name: "BoundarySignature".to_owned(),
        fields: vec![
            ("parameters".to_owned(), BuildTimeValue::Array(parameters)),
            (
                "parameter_count".to_owned(),
                BuildTimeValue::Int(signature.parameters.len() as i64),
            ),
            (
                "has_result".to_owned(),
                BuildTimeValue::Bool(signature.result.is_some()),
            ),
            (
                "result".to_owned(),
                build_value_shape(
                    signature
                        .result
                        .unwrap_or_else(|| ValueShape::integer(0, 0)),
                ),
            ),
        ],
    }
}

fn build_value_shape(shape: ValueShape) -> BuildTimeValue {
    let class = match shape.class {
        ValueClass::Integer => case("Integer", vec![]),
        ValueClass::Float => case("Float", vec![]),
        ValueClass::HomogeneousFloatAggregate { members } => case(
            "HomogeneousFloatAggregate",
            vec![(
                "members".to_owned(),
                BuildTimeValue::Int(i64::from(members)),
            )],
        ),
        ValueClass::SystemVAggregate { first, second } => case(
            "SystemVAggregate",
            vec![
                ("first".to_owned(), build_eightbyte_class(first)),
                ("second".to_owned(), build_eightbyte_class(second)),
            ],
        ),
    };
    BuildTimeValue::Struct {
        type_name: "ValueShape".to_owned(),
        fields: vec![
            ("class".to_owned(), class),
            (
                "byte_size".to_owned(),
                BuildTimeValue::Int(i64::from(shape.byte_size)),
            ),
            (
                "alignment".to_owned(),
                BuildTimeValue::Int(i64::from(shape.alignment)),
            ),
        ],
    }
}

fn build_eightbyte_class(class: SystemVEightbyteClass) -> BuildTimeValue {
    case(
        match class {
            SystemVEightbyteClass::Integer => "Integer",
            SystemVEightbyteClass::Sse => "Sse",
        },
        vec![],
    )
}

fn case(variant: &str, payload: Vec<(String, BuildTimeValue)>) -> BuildTimeValue {
    BuildTimeValue::Case {
        variant: variant.to_owned(),
        payload,
    }
}

fn decode_boundary_plan_result(value: &BuildTimeValue) -> Result<BoundaryPlanResult, String> {
    let (variant, payload) = case_parts(value, "BoundaryPlanResult")?;
    match variant {
        "Accepted" => Ok(BoundaryPlanResult::Accepted(decode_boundary_entry_plan(
            field(payload, "plan", "Accepted")?,
        )?)),
        "Rejected" => {
            let reason = struct_parts(field(payload, "reason", "Rejected")?, "rejection")?;
            let bytes = text(field(reason, "reason", "CallingPolicyRejection")?, "reason")?;
            let reason = String::from_utf8(bytes.to_vec())
                .map_err(|_| "CallingPolicyRejection.reason is not valid UTF-8".to_owned())?;
            Ok(BoundaryPlanResult::Rejected(CallingPolicyRejection::new(
                reason,
            )))
        }
        other => Err(format!(
            "BoundaryPlanResult case `{other}` is outside the compiler-owned vocabulary"
        )),
    }
}

fn decode_boundary_entry_plan(value: &BuildTimeValue) -> Result<BoundaryEntryPlan, String> {
    let fields = struct_parts(value, "BoundaryEntryPlan")?;
    Ok(BoundaryEntryPlan {
        call: decode_call_plan(field(fields, "call", "BoundaryEntryPlan")?)?,
        state: decode_state_plan(field(fields, "state", "BoundaryEntryPlan")?)?,
    })
}

fn decode_call_plan(value: &BuildTimeValue) -> Result<CallPlan, String> {
    let fields = struct_parts(value, "CallPlan")?;
    let parameters = decode_counted_array(
        field(fields, "parameters", "CallPlan")?,
        int(
            field(fields, "parameter_count", "CallPlan")?,
            "parameter_count",
        )?,
        PARAMETER_CAPACITY,
        "CallPlan.parameters",
        decode_value_placement,
    )?;
    let has_result = bool_value(field(fields, "has_result", "CallPlan")?, "has_result")?;
    Ok(CallPlan {
        policy: decode_calling_convention(field(fields, "convention", "CallPlan")?)?,
        parameters,
        result: has_result
            .then(|| decode_value_placement(field(fields, "result", "CallPlan")?))
            .transpose()?,
        ordinary_clobbers: decode_register_set(field(fields, "ordinary_clobbers", "CallPlan")?)?,
        stack_alignment: u16_value(
            field(fields, "stack_alignment", "CallPlan")?,
            "stack_alignment",
        )?,
        shadow_bytes: u16_value(field(fields, "shadow_bytes", "CallPlan")?, "shadow_bytes")?,
        entry_control: decode_entry_control(field(fields, "entry_control", "CallPlan")?)?,
    })
}

fn decode_calling_convention(value: &BuildTimeValue) -> Result<CallingPolicy, String> {
    let (variant, _) = case_parts(value, "CallingConvention")?;
    match variant {
        "MicrosoftX64" => Ok(CallingPolicy::MicrosoftX64),
        "SystemVAMD64" => Ok(CallingPolicy::SystemVAMD64),
        "Aapcs64" => Ok(CallingPolicy::Aapcs64),
        "LinuxSyscallX86_64" => Ok(CallingPolicy::LinuxSyscallX86_64),
        "LinuxSyscallAarch64" => Ok(CallingPolicy::LinuxSyscallAarch64),
        other => Err(format!(
            "CallingConvention case `{other}` is outside the compiler-owned vocabulary"
        )),
    }
}

fn decode_value_placement(value: &BuildTimeValue) -> Result<ValuePlacement, String> {
    let fields = struct_parts(value, "ValuePlacement")?;
    Ok(ValuePlacement {
        shape: decode_value_shape(field(fields, "shape", "ValuePlacement")?)?,
        locations: decode_counted_array(
            field(fields, "locations", "ValuePlacement")?,
            int(
                field(fields, "location_count", "ValuePlacement")?,
                "location_count",
            )?,
            LOCATION_CAPACITY,
            "ValuePlacement.locations",
            decode_value_location,
        )?,
    })
}

fn decode_value_shape(value: &BuildTimeValue) -> Result<ValueShape, String> {
    let fields = struct_parts(value, "ValueShape")?;
    let (variant, payload) = case_parts(field(fields, "class", "ValueShape")?, "ValueClass")?;
    let class = match variant {
        "Integer" => ValueClass::Integer,
        "Float" => ValueClass::Float,
        "HomogeneousFloatAggregate" => ValueClass::HomogeneousFloatAggregate {
            members: u8_value(
                field(payload, "members", "HomogeneousFloatAggregate")?,
                "members",
            )?,
        },
        "SystemVAggregate" => ValueClass::SystemVAggregate {
            first: decode_eightbyte_class(field(payload, "first", "SystemVAggregate")?)?,
            second: decode_eightbyte_class(field(payload, "second", "SystemVAggregate")?)?,
        },
        other => {
            return Err(format!(
                "ValueClass case `{other}` is outside the compiler-owned vocabulary"
            ));
        }
    };
    Ok(ValueShape {
        class,
        byte_size: u16_value(field(fields, "byte_size", "ValueShape")?, "byte_size")?,
        alignment: u16_value(field(fields, "alignment", "ValueShape")?, "alignment")?,
    })
}

fn decode_eightbyte_class(value: &BuildTimeValue) -> Result<SystemVEightbyteClass, String> {
    match case_parts(value, "SystemVEightbyteClass")?.0 {
        "Integer" => Ok(SystemVEightbyteClass::Integer),
        "Sse" => Ok(SystemVEightbyteClass::Sse),
        other => Err(format!(
            "SystemVEightbyteClass case `{other}` is outside the compiler-owned vocabulary"
        )),
    }
}

fn decode_value_location(value: &BuildTimeValue) -> Result<ValueLocation, String> {
    let (variant, payload) = case_parts(value, "ValueLocation")?;
    match variant {
        "Register" => Ok(ValueLocation::Register {
            register: decode_register(field(payload, "register", "Register location")?)?,
            value_byte_offset: u16_value(
                field(payload, "value_byte_offset", "Register location")?,
                "value_byte_offset",
            )?,
            byte_size: u16_value(
                field(payload, "byte_size", "Register location")?,
                "byte_size",
            )?,
        }),
        "Stack" => Ok(ValueLocation::Stack {
            stack_byte_offset: u32_value(
                field(payload, "stack_byte_offset", "Stack location")?,
                "stack_byte_offset",
            )?,
            value_byte_offset: u16_value(
                field(payload, "value_byte_offset", "Stack location")?,
                "value_byte_offset",
            )?,
            byte_size: u16_value(field(payload, "byte_size", "Stack location")?, "byte_size")?,
            alignment: u16_value(field(payload, "alignment", "Stack location")?, "alignment")?,
        }),
        "Indirect" => {
            let has_copy =
                bool_value(field(payload, "has_copy", "Indirect location")?, "has_copy")?;
            Ok(ValueLocation::Indirect {
                pointer: decode_indirect_pointer(field(payload, "pointer", "Indirect location")?)?,
                copy_stack_byte_offset: has_copy
                    .then(|| {
                        u32_value(
                            field(payload, "copy_stack_byte_offset", "Indirect location")?,
                            "copy_stack_byte_offset",
                        )
                    })
                    .transpose()?,
                byte_size: u16_value(
                    field(payload, "byte_size", "Indirect location")?,
                    "byte_size",
                )?,
                alignment: u16_value(
                    field(payload, "alignment", "Indirect location")?,
                    "alignment",
                )?,
            })
        }
        other => Err(format!(
            "ValueLocation case `{other}` is outside the compiler-owned vocabulary"
        )),
    }
}

fn decode_indirect_pointer(value: &BuildTimeValue) -> Result<IndirectPointerLocation, String> {
    let (variant, payload) = case_parts(value, "IndirectPointerLocation")?;
    match variant {
        "Register" => Ok(IndirectPointerLocation::Register(decode_register(field(
            payload,
            "register",
            "indirect register pointer",
        )?)?)),
        "Stack" => Ok(IndirectPointerLocation::Stack {
            stack_byte_offset: u32_value(
                field(payload, "stack_byte_offset", "indirect stack pointer")?,
                "stack_byte_offset",
            )?,
            alignment: u16_value(
                field(payload, "alignment", "indirect stack pointer")?,
                "alignment",
            )?,
        }),
        other => Err(format!(
            "IndirectPointerLocation case `{other}` is outside the compiler-owned vocabulary"
        )),
    }
}

fn decode_register_set(value: &BuildTimeValue) -> Result<RegisterSet, String> {
    let fields = struct_parts(value, "RegisterSet")?;
    Ok(RegisterSet::new(decode_counted_array(
        field(fields, "registers", "RegisterSet")?,
        int(
            field(fields, "register_count", "RegisterSet")?,
            "register_count",
        )?,
        REGISTER_CAPACITY,
        "RegisterSet.registers",
        decode_register,
    )?))
}

fn decode_register(value: &BuildTimeValue) -> Result<MachineRegister, String> {
    let (variant, payload) = case_parts(value, "MachineRegister")?;
    let indexed = |name: &str| u8_value(field(payload, "index", name)?, "register index");
    match variant {
        "X86Rax" => Ok(MachineRegister::X86Rax),
        "X86Rcx" => Ok(MachineRegister::X86Rcx),
        "X86Rdx" => Ok(MachineRegister::X86Rdx),
        "X86Rbx" => Ok(MachineRegister::X86Rbx),
        "X86Rsp" => Ok(MachineRegister::X86Rsp),
        "X86Rbp" => Ok(MachineRegister::X86Rbp),
        "X86Rsi" => Ok(MachineRegister::X86Rsi),
        "X86Rdi" => Ok(MachineRegister::X86Rdi),
        "X86R8" => Ok(MachineRegister::X86R8),
        "X86R9" => Ok(MachineRegister::X86R9),
        "X86R10" => Ok(MachineRegister::X86R10),
        "X86R11" => Ok(MachineRegister::X86R11),
        "X86R12" => Ok(MachineRegister::X86R12),
        "X86R13" => Ok(MachineRegister::X86R13),
        "X86R14" => Ok(MachineRegister::X86R14),
        "X86R15" => Ok(MachineRegister::X86R15),
        "X86Xmm" => Ok(MachineRegister::X86Xmm(indexed("X86Xmm")?)),
        "Aarch64X" => Ok(MachineRegister::Aarch64X(indexed("Aarch64X")?)),
        "Aarch64V" => Ok(MachineRegister::Aarch64V(indexed("Aarch64V")?)),
        other => Err(format!(
            "MachineRegister case `{other}` is outside the compiler-owned vocabulary"
        )),
    }
}

fn decode_entry_control(value: &BuildTimeValue) -> Result<EntryControl, String> {
    let (variant, payload) = case_parts(value, "EntryControl")?;
    match variant {
        "CallReturn" => Ok(EntryControl::CallReturn),
        "SupervisorCall" => Ok(EntryControl::SupervisorCall {
            number_register: decode_register(field(payload, "number_register", "SupervisorCall")?)?,
            immediate: u16_value(field(payload, "immediate", "SupervisorCall")?, "immediate")?,
        }),
        "InterruptReturn" => Ok(EntryControl::InterruptReturn),
        other => Err(format!(
            "EntryControl case `{other}` is outside the compiler-owned vocabulary"
        )),
    }
}

fn decode_state_plan(value: &BuildTimeValue) -> Result<StatePlan, String> {
    let fields = struct_parts(value, "StatePlan")?;
    Ok(StatePlan {
        initial_regime: decode_machine_regime(field(fields, "initial_regime", "StatePlan")?)?,
        interrupted_state: decode_machine_state_set(field(
            fields,
            "interrupted_state",
            "StatePlan",
        )?)?,
        saved_state: decode_machine_state_set(field(fields, "saved_state", "StatePlan")?)?,
        restored_state: decode_machine_state_set(field(fields, "restored_state", "StatePlan")?)?,
        permitted_transitive_use: decode_machine_state_set(field(
            fields,
            "permitted_transitive_use",
            "StatePlan",
        )?)?,
        stack: decode_entry_stack(field(fields, "stack", "StatePlan")?)?,
        preemption: decode_preemption(field(fields, "preemption", "StatePlan")?)?,
    })
}

fn decode_machine_regime(value: &BuildTimeValue) -> Result<MachineRegime, String> {
    let (variant, payload) = case_parts(value, "MachineRegime")?;
    match variant {
        "X86Long64" => Ok(MachineRegime::X86Long64),
        "Aarch64A64" => Ok(MachineRegime::Aarch64A64 {
            exception_level: u8_value(
                field(payload, "exception_level", "Aarch64A64")?,
                "exception_level",
            )?,
        }),
        other => Err(format!(
            "MachineRegime case `{other}` is outside the compiler-owned vocabulary"
        )),
    }
}

fn decode_machine_state_set(value: &BuildTimeValue) -> Result<MachineStateSet, String> {
    let fields = struct_parts(value, "MachineStateSet")?;
    let candidates = [
        ("general_registers", MachineState::GeneralRegisters),
        ("vector_registers", MachineState::VectorRegisters),
        ("flags", MachineState::Flags),
        ("instruction_pointer", MachineState::InstructionPointer),
        ("stack_pointer", MachineState::StackPointer),
        ("segment_state", MachineState::SegmentState),
        ("control_state", MachineState::ControlState),
        ("debug_state", MachineState::DebugState),
        ("extended_state", MachineState::ExtendedState),
    ];
    let mut states = Vec::new();
    for (name, state) in candidates {
        if bool_value(field(fields, name, "MachineStateSet")?, name)? {
            states.push(state);
        }
    }
    Ok(MachineStateSet::new(states))
}

fn decode_entry_stack(value: &BuildTimeValue) -> Result<EntryStack, String> {
    let (variant, payload) = case_parts(value, "EntryStack")?;
    match variant {
        "Interrupted" => Ok(EntryStack::Interrupted),
        "Dedicated" => Ok(EntryStack::Dedicated {
            class: u16_value(field(payload, "class", "Dedicated")?, "class")?,
        }),
        "ProviderSelected" => Ok(EntryStack::ProviderSelected),
        other => Err(format!(
            "EntryStack case `{other}` is outside the compiler-owned vocabulary"
        )),
    }
}

fn decode_preemption(value: &BuildTimeValue) -> Result<Preemption, String> {
    let (variant, payload) = case_parts(value, "Preemption")?;
    match variant {
        "NotApplicable" => Ok(Preemption::NotApplicable),
        "Masked" => Ok(Preemption::Masked),
        "Nestable" => Ok(Preemption::Nestable {
            maximum_depth: u16_value(
                field(payload, "maximum_depth", "Nestable")?,
                "maximum_depth",
            )?,
        }),
        "ProviderDefined" => Ok(Preemption::ProviderDefined),
        other => Err(format!(
            "Preemption case `{other}` is outside the compiler-owned vocabulary"
        )),
    }
}

fn decode_counted_array<T>(
    value: &BuildTimeValue,
    count: i64,
    capacity: usize,
    context: &str,
    decode: impl Fn(&BuildTimeValue) -> Result<T, String>,
) -> Result<Vec<T>, String> {
    if count < 0 || count as usize > capacity {
        return Err(format!("{context} count {count} is outside 0..={capacity}"));
    }
    let BuildTimeValue::Array(values) = value else {
        return Err(format!("{context} is not an array"));
    };
    if count as usize > values.len() {
        return Err(format!(
            "{context} count is {count}, but the value carries only {} cells",
            values.len()
        ));
    }
    values[..count as usize].iter().map(decode).collect()
}

fn struct_parts<'a>(
    value: &'a BuildTimeValue,
    context: &str,
) -> Result<&'a [(String, BuildTimeValue)], String> {
    match value {
        BuildTimeValue::Struct { fields, .. } => Ok(fields),
        other => Err(format!("{context} is not a struct: {other:?}")),
    }
}

fn case_parts<'a>(
    value: &'a BuildTimeValue,
    context: &str,
) -> Result<(&'a str, &'a [(String, BuildTimeValue)]), String> {
    match value {
        BuildTimeValue::Case { variant, payload } => Ok((
            variant.rsplit("::").next().unwrap_or(variant),
            payload.as_slice(),
        )),
        other => Err(format!("{context} is not a case value: {other:?}")),
    }
}

fn field<'a>(
    fields: &'a [(String, BuildTimeValue)],
    name: &str,
    context: &str,
) -> Result<&'a BuildTimeValue, String> {
    fields
        .iter()
        .find(|(candidate, _)| candidate == name)
        .map(|(_, value)| value)
        .ok_or_else(|| format!("{context} carries no `{name}` field"))
}

fn int(value: &BuildTimeValue, context: &str) -> Result<i64, String> {
    match value {
        BuildTimeValue::Int(value) => Ok(*value),
        other => Err(format!("{context} is not an integer: {other:?}")),
    }
}

fn bool_value(value: &BuildTimeValue, context: &str) -> Result<bool, String> {
    match value {
        BuildTimeValue::Bool(value) => Ok(*value),
        other => Err(format!("{context} is not a bool: {other:?}")),
    }
}

fn text<'a>(value: &'a BuildTimeValue, context: &str) -> Result<&'a [u8], String> {
    match value {
        BuildTimeValue::Text(value) => Ok(value),
        other => Err(format!("{context} is not text: {other:?}")),
    }
}

fn u8_value(value: &BuildTimeValue, context: &str) -> Result<u8, String> {
    u8::try_from(int(value, context)?).map_err(|_| format!("{context} is outside u8 range"))
}

fn u16_value(value: &BuildTimeValue, context: &str) -> Result<u16, String> {
    u16::try_from(int(value, context)?).map_err(|_| format!("{context} is outside u16 range"))
}

fn u32_value(value: &BuildTimeValue, context: &str) -> Result<u32, String> {
    u32::try_from(int(value, context)?).map_err(|_| format!("{context} is outside u32 range"))
}
