//! Evaluating a call plan from a policy and signature: the Microsoft x64,
//! System V AMD64, AAPCS64 and Darwin variadic conventions, the Linux
//! syscall forms, and the placement arithmetic they share.

use crate::plans::{
    CallPlan, CallSignature, CallingPolicy, ConcreteVariadicCallSignature, EntryControl,
    IndirectPointerLocation, MachineRegister, PlanDiagnostic, RegisterSet, SystemVEightbyteClass,
    ValueClass, ValueLocation, ValuePlacement, ValueShape,
};
use target::Architecture;

pub fn evaluate_call_plan(
    policy: CallingPolicy,
    signature: &CallSignature,
) -> Result<CallPlan, PlanDiagnostic> {
    validate_signature_shapes(policy, signature)?;
    let runtime_signature = CallSignature {
        parameters: signature
            .parameters
            .iter()
            .copied()
            .filter(|shape| shape.byte_size != 0 || shape.class == ValueClass::BorrowedReference)
            .collect(),
        result: signature.result.filter(|shape| shape.byte_size != 0),
    };
    let mut plan = match policy {
        CallingPolicy::MicrosoftX64 => evaluate_microsoft_x64(&runtime_signature)?,
        CallingPolicy::SystemVAMD64 => evaluate_system_v_amd64(&runtime_signature)?,
        CallingPolicy::Aapcs64 => evaluate_aapcs64(&runtime_signature)?,
        CallingPolicy::LinuxSyscallX86_64 => evaluate_linux_syscall_x86_64(&runtime_signature)?,
        CallingPolicy::LinuxSyscallAarch64 => evaluate_linux_syscall_aarch64(&runtime_signature)?,
    };
    let mut runtime_parameters = plan.parameters.into_iter();
    plan.parameters = signature
        .parameters
        .iter()
        .copied()
        .map(|shape| {
            if shape.byte_size == 0 && shape.class != ValueClass::BorrowedReference {
                ValuePlacement {
                    shape,
                    locations: Vec::new(),
                }
            } else {
                runtime_parameters
                    .next()
                    .expect("runtime call plan covers every nonempty parameter")
            }
        })
        .collect();
    debug_assert!(runtime_parameters.next().is_none());
    if signature.result.is_some_and(|shape| shape.byte_size == 0) {
        plan.result = Some(ValuePlacement {
            shape: signature.result.expect("zero-sized result exists"),
            locations: Vec::new(),
        });
    }
    plan.policy = policy;
    validate_call_plan(&plan, signature)?;
    Ok(plan)
}

fn validate_signature_shapes(
    policy: CallingPolicy,
    signature: &CallSignature,
) -> Result<(), PlanDiagnostic> {
    if signature
        .result
        .is_some_and(|shape| matches!(shape.class, ValueClass::BorrowedReference))
    {
        return Err(PlanDiagnostic(
            "borrowed references are parameter-only call values".into(),
        ));
    }
    for shape in signature.parameters.iter().chain(signature.result.iter()) {
        if shape.alignment == 0 || !shape.alignment.is_power_of_two() {
            return Err(PlanDiagnostic(
                "call-signature values need power-of-two alignment".into(),
            ));
        }
        // A borrowed shape measures its referent, not its runtime pointer.
        // Only by-value empties are erased from the physical signature.
        if shape.byte_size == 0
            && shape.class != ValueClass::BorrowedReference
            && (shape.class != ValueClass::Integer || shape.alignment != 1)
        {
            return Err(PlanDiagnostic(
                "zero-sized call values must use the canonical integer-class shape".into(),
            ));
        }
        match shape.class {
            ValueClass::Integer
                if shape.byte_size > 8
                    && policy != CallingPolicy::Aapcs64
                    && policy != CallingPolicy::SystemVAMD64
                    && policy != CallingPolicy::MicrosoftX64 =>
            {
                return Err(PlanDiagnostic(
                    "aggregate integer classification is not normalized for this calling policy"
                        .into(),
                ));
            }
            ValueClass::Float if !matches!(shape.byte_size, 4 | 8) => {
                return Err(PlanDiagnostic(
                    "scalar floating-point call values must be f32 or f64 sized".into(),
                ));
            }
            ValueClass::HomogeneousFloatAggregate { members }
                if !matches!(policy, CallingPolicy::Aapcs64 | CallingPolicy::SystemVAMD64)
                    || !(1..=4).contains(&members)
                    || shape.byte_size % u16::from(members.max(1)) != 0 =>
            {
                return Err(PlanDiagnostic(
                    "homogeneous float aggregates require a supported native policy and equal members"
                        .into(),
                ));
            }
            ValueClass::HomogeneousFloatAggregate { members }
                if policy == CallingPolicy::SystemVAMD64
                    && !(matches!(members, 2..=4)
                        && shape.byte_size <= 16
                        && matches!(shape.alignment, 4 | 8)
                        && shape.byte_size == u16::from(members) * shape.alignment) =>
            {
                return Err(PlanDiagnostic(
                    "SysV AMD64 homogeneous-float normalization requires two to four f32/f64 members totaling at most two eightbytes"
                        .into(),
                ));
            }
            ValueClass::SystemVAggregate { first, second }
                if policy != CallingPolicy::SystemVAMD64
                    || !(9..=16).contains(&shape.byte_size)
                    || shape.alignment > 8
                    || matches!(
                        (first, second),
                        (
                            SystemVEightbyteClass::Integer,
                            SystemVEightbyteClass::Integer
                        )
                    ) =>
            {
                return Err(PlanDiagnostic(
                    "classified SysV aggregates require at least one SSE eightbyte, a 9-16 byte shape, and at most eight-byte alignment"
                        .into(),
                ));
            }
            _ => {}
        }
    }
    Ok(())
}

pub fn validate_call_plan(
    plan: &CallPlan,
    signature: &CallSignature,
) -> Result<(), PlanDiagnostic> {
    validate_call_plan_structure(plan, signature)?;
    if !plan.callback_materializations.is_empty() {
        return Err(PlanDiagnostic(
            "callback materializations require their nominal binder and native-place context"
                .into(),
        ));
    }
    Ok(())
}

pub(crate) fn validate_call_plan_structure(
    plan: &CallPlan,
    signature: &CallSignature,
) -> Result<(), PlanDiagnostic> {
    if plan.parameters.len() != signature.parameters.len() {
        return Err(PlanDiagnostic(format!(
            "call plan places {} parameters but the signature declares {}",
            plan.parameters.len(),
            signature.parameters.len()
        )));
    }
    if plan.result.as_ref().map(|value| value.shape) != signature.result {
        return Err(PlanDiagnostic(
            "call-plan result placement does not match the signature".into(),
        ));
    }
    if plan.stack_alignment == 0 || !plan.stack_alignment.is_power_of_two() {
        return Err(PlanDiagnostic(
            "call-plan stack alignment must be a nonzero power of two".into(),
        ));
    }
    let architecture = plan.policy.architecture();
    let mut occupied_registers = RegisterSet::default();
    let mut occupied_stack_ranges = Vec::new();
    for (index, (placement, shape)) in plan
        .parameters
        .iter()
        .zip(signature.parameters.iter())
        .enumerate()
    {
        if placement.shape != *shape {
            return Err(PlanDiagnostic(format!(
                "parameter {index} placement shape does not match the signature"
            )));
        }
        validate_value_placement(placement, architecture, index)?;
        for location in &placement.locations {
            match *location {
                ValueLocation::Register { register, .. } => {
                    if occupied_registers.contains(register) {
                        return Err(PlanDiagnostic(format!(
                            "parameter {index} reuses a register occupied by another parameter"
                        )));
                    }
                    occupied_registers = RegisterSet::new(
                        occupied_registers
                            .as_slice()
                            .iter()
                            .copied()
                            .chain([register]),
                    );
                }
                ValueLocation::Stack {
                    stack_byte_offset,
                    byte_size,
                    ..
                } => {
                    let end = stack_byte_offset + u32::from(byte_size);
                    if occupied_stack_ranges
                        .iter()
                        .any(|(start, prior_end)| stack_byte_offset < *prior_end && *start < end)
                    {
                        return Err(PlanDiagnostic(format!(
                            "parameter {index} overlaps another parameter's stack placement"
                        )));
                    }
                    occupied_stack_ranges.push((stack_byte_offset, end));
                }
                ValueLocation::Indirect {
                    pointer,
                    copy_stack_byte_offset,
                    byte_size,
                    ..
                } => {
                    match pointer {
                        IndirectPointerLocation::Register(register) => {
                            if occupied_registers.contains(register) {
                                return Err(PlanDiagnostic(format!(
                                    "parameter {index} reuses a register occupied by another parameter"
                                )));
                            }
                            occupied_registers = RegisterSet::new(
                                occupied_registers
                                    .as_slice()
                                    .iter()
                                    .copied()
                                    .chain([register]),
                            );
                        }
                        IndirectPointerLocation::Stack {
                            stack_byte_offset, ..
                        } => {
                            let end = stack_byte_offset + 8;
                            if occupied_stack_ranges.iter().any(|(start, prior_end)| {
                                stack_byte_offset < *prior_end && *start < end
                            }) {
                                return Err(PlanDiagnostic(format!(
                                    "parameter {index} indirect pointer overlaps another stack placement"
                                )));
                            }
                            occupied_stack_ranges.push((stack_byte_offset, end));
                        }
                    }
                    if let Some(copy_stack_byte_offset) = copy_stack_byte_offset {
                        let end = copy_stack_byte_offset + u32::from(byte_size);
                        if occupied_stack_ranges.iter().any(|(start, prior_end)| {
                            copy_stack_byte_offset < *prior_end && *start < end
                        }) {
                            return Err(PlanDiagnostic(format!(
                                "parameter {index} indirect copy overlaps another stack placement"
                            )));
                        }
                        occupied_stack_ranges.push((copy_stack_byte_offset, end));
                    }
                }
            }
        }
    }
    if let Some(result) = &plan.result {
        validate_value_placement(result, architecture, signature.parameters.len())?;
    }
    for register in plan.ordinary_clobbers.as_slice() {
        if register.architecture() != architecture {
            return Err(PlanDiagnostic(
                "ordinary clobber set contains a register from the wrong architecture".into(),
            ));
        }
    }
    if let EntryControl::SupervisorCall {
        number_register, ..
    } = plan.entry_control
        && number_register.architecture() != architecture
    {
        return Err(PlanDiagnostic(
            "entry-control register belongs to the wrong architecture".into(),
        ));
    }
    Ok(())
}

fn validate_value_placement(
    placement: &ValuePlacement,
    architecture: Architecture,
    value_index: usize,
) -> Result<(), PlanDiagnostic> {
    if matches!(placement.shape.class, ValueClass::BorrowedReference)
        && !matches!(
            placement.locations.as_slice(),
            [ValueLocation::Indirect {
                copy_stack_byte_offset: None,
                ..
            }]
        )
    {
        return Err(PlanDiagnostic(format!(
            "borrowed-reference value {value_index} must retain one direct pointer to caller storage"
        )));
    }
    let mut covered = vec![false; usize::from(placement.shape.byte_size)];
    for location in &placement.locations {
        let (value_byte_offset, byte_size) = match *location {
            ValueLocation::Register {
                register,
                value_byte_offset,
                byte_size,
            } => {
                if register.architecture() != architecture {
                    return Err(PlanDiagnostic(format!(
                        "value {value_index} uses a register from the wrong architecture"
                    )));
                }
                (value_byte_offset, byte_size)
            }
            ValueLocation::Stack {
                value_byte_offset,
                byte_size,
                alignment,
                stack_byte_offset,
            } => {
                if alignment == 0
                    || !alignment.is_power_of_two()
                    || stack_byte_offset % u32::from(alignment) != 0
                {
                    return Err(PlanDiagnostic(format!(
                        "value {value_index} has a misaligned stack placement"
                    )));
                }
                (value_byte_offset, byte_size)
            }
            ValueLocation::Indirect {
                pointer,
                copy_stack_byte_offset,
                byte_size,
                alignment,
            } => {
                if byte_size != placement.shape.byte_size
                    || alignment != placement.shape.alignment
                    || alignment == 0
                    || !alignment.is_power_of_two()
                {
                    return Err(PlanDiagnostic(format!(
                        "value {value_index} has an invalid indirect shape"
                    )));
                }
                match pointer {
                    IndirectPointerLocation::Register(register) => {
                        if register.architecture() != architecture {
                            return Err(PlanDiagnostic(format!(
                                "value {value_index} uses an indirect pointer register from the wrong architecture"
                            )));
                        }
                    }
                    IndirectPointerLocation::Stack {
                        stack_byte_offset,
                        alignment,
                    } => {
                        if alignment == 0
                            || !alignment.is_power_of_two()
                            || stack_byte_offset % u32::from(alignment) != 0
                        {
                            return Err(PlanDiagnostic(format!(
                                "value {value_index} has a misaligned indirect pointer"
                            )));
                        }
                    }
                }
                if let Some(copy_stack_byte_offset) = copy_stack_byte_offset
                    && copy_stack_byte_offset % u32::from(alignment.clamp(8, 16)) != 0
                {
                    return Err(PlanDiagnostic(format!(
                        "value {value_index} has a misaligned indirect copy"
                    )));
                }
                (0, byte_size)
            }
        };
        let end = usize::from(value_byte_offset) + usize::from(byte_size);
        if (byte_size == 0 && placement.shape.class != ValueClass::BorrowedReference)
            || end > covered.len()
        {
            return Err(PlanDiagnostic(format!(
                "value {value_index} placement exceeds its declared shape"
            )));
        }
        for byte in &mut covered[usize::from(value_byte_offset)..end] {
            if *byte {
                return Err(PlanDiagnostic(format!(
                    "value {value_index} placement writes one source byte more than once"
                )));
            }
            *byte = true;
        }
    }
    if covered.iter().any(|covered| !covered) {
        return Err(PlanDiagnostic(format!(
            "value {value_index} placement does not cover every source byte"
        )));
    }
    Ok(())
}

fn evaluate_microsoft_x64(signature: &CallSignature) -> Result<CallPlan, PlanDiagnostic> {
    let integer = [
        MachineRegister::X86Rcx,
        MachineRegister::X86Rdx,
        MachineRegister::X86R8,
        MachineRegister::X86R9,
    ];
    let indirect_result = signature.result.is_some_and(|shape| {
        matches!(shape.class, ValueClass::Integer) && !matches!(shape.byte_size, 1 | 2 | 4 | 8)
    });
    let parameter_slot_base = usize::from(indirect_result);
    let mut parameters = Vec::with_capacity(signature.parameters.len());
    for (index, shape) in signature.parameters.iter().copied().enumerate() {
        if matches!(
            shape.class,
            ValueClass::HomogeneousFloatAggregate { .. } | ValueClass::SystemVAggregate { .. }
        ) {
            return Err(PlanDiagnostic(
                "Microsoft x64 aggregate classification is not normalized yet".into(),
            ));
        }
        let slot = parameter_slot_base + index;
        let indirect = matches!(shape.class, ValueClass::BorrowedReference)
            || (matches!(shape.class, ValueClass::Integer)
                && !matches!(shape.byte_size, 1 | 2 | 4 | 8));
        let location = if indirect {
            let pointer = if slot < 4 {
                IndirectPointerLocation::Register(integer[slot])
            } else {
                IndirectPointerLocation::Stack {
                    stack_byte_offset: 32 + ((slot - 4) * 8) as u32,
                    alignment: 8,
                }
            };
            ValueLocation::Indirect {
                pointer,
                copy_stack_byte_offset: None,
                byte_size: shape.byte_size,
                alignment: shape.alignment,
            }
        } else if slot < 4 {
            let register = if matches!(shape.class, ValueClass::Float) {
                MachineRegister::X86Xmm(slot as u8)
            } else {
                integer[slot]
            };
            register_location(register, shape)
        } else {
            stack_location(32 + ((slot - 4) * 8) as u32, shape)
        };
        parameters.push(ValuePlacement {
            shape,
            locations: vec![location],
        });
    }
    let stack_parameter_slots = (parameter_slot_base + parameters.len()).saturating_sub(4);
    let mut copy_stack_offset = 32 + (stack_parameter_slots * 8) as u32;
    for placement in &mut parameters {
        if let [
            ValueLocation::Indirect {
                copy_stack_byte_offset,
                byte_size,
                ..
            },
        ] = placement.locations.as_mut_slice()
        {
            // Microsoft requires caller-owned temporaries for indirectly
            // passed aggregates to be 16-byte aligned, even when the source
            // type itself has a smaller natural alignment.
            copy_stack_offset = align_up(copy_stack_offset, 16);
            if !matches!(placement.shape.class, ValueClass::BorrowedReference) {
                *copy_stack_byte_offset = Some(copy_stack_offset);
                copy_stack_offset += u32::from(*byte_size).next_multiple_of(8);
            }
        }
    }
    Ok(CallPlan {
        policy: CallingPolicy::MicrosoftX64,
        parameters,
        result: if indirect_result {
            let shape = signature.result.expect("indirect result shape was present");
            Some(ValuePlacement {
                shape,
                locations: vec![ValueLocation::Indirect {
                    pointer: IndirectPointerLocation::Register(MachineRegister::X86Rcx),
                    copy_stack_byte_offset: None,
                    byte_size: shape.byte_size,
                    alignment: shape.alignment,
                }],
            })
        } else {
            result_placement(signature.result, &[MachineRegister::X86Rax], |index| {
                MachineRegister::X86Xmm(index)
            })?
        },
        callback_materializations: Vec::new(),
        ordinary_clobbers: RegisterSet::new(
            [
                MachineRegister::X86Rax,
                MachineRegister::X86Rcx,
                MachineRegister::X86Rdx,
                MachineRegister::X86R8,
                MachineRegister::X86R9,
                MachineRegister::X86R10,
                MachineRegister::X86R11,
            ]
            .into_iter()
            .chain((0..=5).map(MachineRegister::X86Xmm)),
        ),
        stack_alignment: 16,
        shadow_bytes: 32,
        entry_control: EntryControl::CallReturn,
    })
}

fn evaluate_system_v_amd64(signature: &CallSignature) -> Result<CallPlan, PlanDiagnostic> {
    let integer = [
        MachineRegister::X86Rdi,
        MachineRegister::X86Rsi,
        MachineRegister::X86Rdx,
        MachineRegister::X86Rcx,
        MachineRegister::X86R8,
        MachineRegister::X86R9,
    ];
    let mut plan = evaluate_split_bank_call(
        CallingPolicy::SystemVAMD64,
        signature,
        &integer,
        8,
        MachineRegister::X86Xmm,
        &[MachineRegister::X86Rax, MachineRegister::X86Rdx],
        16,
        RegisterSet::new(
            [
                MachineRegister::X86Rax,
                MachineRegister::X86Rcx,
                MachineRegister::X86Rdx,
                MachineRegister::X86Rsi,
                MachineRegister::X86Rdi,
                MachineRegister::X86R8,
                MachineRegister::X86R9,
                MachineRegister::X86R10,
                MachineRegister::X86R11,
            ]
            .into_iter()
            .chain((0..=15).map(MachineRegister::X86Xmm)),
        ),
    )?;
    if let Some(result) = plan.result.as_mut()
        && matches!(result.shape.class, ValueClass::Integer)
        && result.shape.byte_size > 8
    {
        result.locations = if result.shape.byte_size > 16 {
            vec![ValueLocation::Indirect {
                pointer: IndirectPointerLocation::Register(MachineRegister::X86Rdi),
                copy_stack_byte_offset: None,
                byte_size: result.shape.byte_size,
                alignment: result.shape.alignment,
            }]
        } else {
            integer_register_fragment_locations(
                result.shape,
                &[MachineRegister::X86Rax, MachineRegister::X86Rdx],
                0,
            )
        };
    }
    if let Some(result) = plan.result.as_mut()
        && matches!(
            result.shape.class,
            ValueClass::HomogeneousFloatAggregate { .. }
        )
    {
        result.locations = sysv_sse_fragment_locations(result.shape, 0);
    }
    Ok(plan)
}

fn evaluate_aapcs64(signature: &CallSignature) -> Result<CallPlan, PlanDiagnostic> {
    let integer = (0..8).map(MachineRegister::Aarch64X).collect::<Vec<_>>();
    let mut plan = evaluate_split_bank_call(
        CallingPolicy::Aapcs64,
        signature,
        &integer,
        8,
        MachineRegister::Aarch64V,
        &integer[..2],
        16,
        RegisterSet::new(
            (0..=17)
                .map(MachineRegister::Aarch64X)
                .chain((0..=7).map(MachineRegister::Aarch64V))
                .chain((16..=31).map(MachineRegister::Aarch64V)),
        ),
    )?;
    if let Some(result) = plan.result.as_mut()
        && matches!(result.shape.class, ValueClass::Integer)
    {
        if result.shape.byte_size > 16 {
            result.locations = vec![ValueLocation::Indirect {
                pointer: IndirectPointerLocation::Register(MachineRegister::Aarch64X(8)),
                copy_stack_byte_offset: None,
                byte_size: result.shape.byte_size,
                alignment: result.shape.alignment,
            }];
        } else if result.shape.byte_size > 8 {
            result.locations = integer_register_fragment_locations(result.shape, &integer, 0);
        }
    }
    Ok(plan)
}

/// Evaluate Apple's arm64 C variadic rule for one fully shaped call. Fixed
/// parameters use ordinary AAPCS64 placement; promoted anonymous scalar
/// parameters use the outgoing stack area even while argument registers remain.
///
/// The first consumer is Darwin `open(path, flags, mode)`. Aggregate variadic
/// values remain fail-closed until an actual native binding needs their Apple
/// ABI classification.
pub fn evaluate_darwin_aapcs64_variadic_call_plan(
    signature: &ConcreteVariadicCallSignature,
) -> Result<CallPlan, PlanDiagnostic> {
    if signature.variadic_parameters.is_empty() {
        return Err(PlanDiagnostic(
            "a concrete variadic call must supply at least one anonymous parameter".into(),
        ));
    }
    let fixed_signature = CallSignature {
        parameters: signature.fixed_parameters.clone(),
        result: signature.result,
    };
    validate_signature_shapes(CallingPolicy::Aapcs64, &signature.flattened())?;
    let mut plan = evaluate_aapcs64(&fixed_signature)?;
    let mut stack_offset = call_plan_stack_extent(&plan);
    for (index, shape) in signature.variadic_parameters.iter().copied().enumerate() {
        if !matches!(shape.class, ValueClass::Integer | ValueClass::Float) || shape.byte_size > 8 {
            return Err(PlanDiagnostic(format!(
                "Darwin AAPCS64 anonymous parameter {index} is not a promoted scalar"
            )));
        }
        let alignment = shape.alignment.clamp(8, 16);
        stack_offset = align_up(stack_offset, u32::from(alignment));
        plan.parameters.push(ValuePlacement {
            shape,
            locations: vec![ValueLocation::Stack {
                stack_byte_offset: stack_offset,
                value_byte_offset: 0,
                byte_size: shape.byte_size,
                alignment,
            }],
        });
        stack_offset += u32::from(shape.byte_size.max(8));
    }
    validate_call_plan(&plan, &signature.flattened())?;
    Ok(plan)
}

fn call_plan_stack_extent(plan: &CallPlan) -> u32 {
    plan.parameters
        .iter()
        .flat_map(|placement| &placement.locations)
        .fold(0, |extent, location| {
            let end = match *location {
                ValueLocation::Register { .. } => 0,
                ValueLocation::Stack {
                    stack_byte_offset,
                    byte_size,
                    ..
                } => stack_byte_offset + u32::from(byte_size.max(8)),
                ValueLocation::Indirect {
                    pointer,
                    copy_stack_byte_offset,
                    byte_size,
                    ..
                } => {
                    let pointer_end = match pointer {
                        IndirectPointerLocation::Register(_) => 0,
                        IndirectPointerLocation::Stack {
                            stack_byte_offset, ..
                        } => stack_byte_offset + 8,
                    };
                    pointer_end.max(copy_stack_byte_offset.map_or(0, |offset| {
                        offset + u32::from(byte_size).next_multiple_of(8)
                    }))
                }
            };
            extent.max(end)
        })
}

fn evaluate_split_bank_call(
    policy: CallingPolicy,
    signature: &CallSignature,
    integer_registers: &[MachineRegister],
    float_register_count: u8,
    float_register: impl Fn(u8) -> MachineRegister + Copy,
    integer_results: &[MachineRegister],
    stack_alignment: u16,
    ordinary_clobbers: RegisterSet,
) -> Result<CallPlan, PlanDiagnostic> {
    // SysV MEMORY-class results use the hidden first integer argument (`rdi`)
    // as their caller-owned destination, shifting declared integer arguments.
    let mut integer_index = usize::from(
        policy == CallingPolicy::SystemVAMD64
            && signature.result.is_some_and(|shape| {
                matches!(shape.class, ValueClass::Integer) && shape.byte_size > 16
            }),
    );
    let mut float_index = 0u8;
    let mut stack_offset = 0u32;
    let mut parameters = Vec::with_capacity(signature.parameters.len());
    for shape in signature.parameters.iter().copied() {
        let mut locations = Vec::new();
        if matches!(shape.class, ValueClass::BorrowedReference) {
            let pointer = if integer_index < integer_registers.len() {
                let register = integer_registers[integer_index];
                integer_index += 1;
                IndirectPointerLocation::Register(register)
            } else {
                stack_offset = align_up(stack_offset, 8);
                let pointer = IndirectPointerLocation::Stack {
                    stack_byte_offset: stack_offset,
                    alignment: 8,
                };
                stack_offset += 8;
                pointer
            };
            locations.push(ValueLocation::Indirect {
                pointer,
                copy_stack_byte_offset: None,
                byte_size: shape.byte_size,
                alignment: shape.alignment,
            });
            parameters.push(ValuePlacement { shape, locations });
            continue;
        }
        if let ValueClass::SystemVAggregate { first, second } = shape.class {
            debug_assert_eq!(policy, CallingPolicy::SystemVAMD64);
            let classes = [first, second];
            let integer_registers_needed = classes
                .iter()
                .filter(|class| matches!(class, SystemVEightbyteClass::Integer))
                .count();
            let float_registers_needed = classes
                .iter()
                .filter(|class| matches!(class, SystemVEightbyteClass::Sse))
                .count() as u8;
            if integer_index + integer_registers_needed <= integer_registers.len()
                && float_index.saturating_add(float_registers_needed) <= float_register_count
            {
                let mut aggregate_integer_index = integer_index;
                let mut aggregate_float_index = float_index;
                for (fragment, class) in classes.into_iter().enumerate() {
                    let register = match class {
                        SystemVEightbyteClass::Integer => {
                            let register = integer_registers[aggregate_integer_index];
                            aggregate_integer_index += 1;
                            register
                        }
                        SystemVEightbyteClass::Sse => {
                            let register = float_register(aggregate_float_index);
                            aggregate_float_index += 1;
                            register
                        }
                    };
                    let value_byte_offset = fragment as u16 * 8;
                    locations.push(ValueLocation::Register {
                        register,
                        value_byte_offset,
                        byte_size: (shape.byte_size - value_byte_offset).min(8),
                    });
                }
                integer_index = aggregate_integer_index;
                float_index = aggregate_float_index;
            } else {
                stack_offset = align_up(stack_offset, u32::from(shape.alignment.clamp(8, 16)));
                locations.extend(integer_stack_fragment_locations(shape, stack_offset));
                stack_offset += u32::from(shape.byte_size).next_multiple_of(8);
            }
            parameters.push(ValuePlacement { shape, locations });
            continue;
        }
        let float_members = match shape.class {
            ValueClass::Float => Some(1),
            ValueClass::HomogeneousFloatAggregate { members } => Some(members),
            ValueClass::Integer => None,
            ValueClass::BorrowedReference => unreachable!("handled above"),
            ValueClass::SystemVAggregate { .. } => unreachable!("handled above"),
        };
        let float_registers_needed = float_members.map(|members| {
            if policy == CallingPolicy::SystemVAMD64
                && matches!(shape.class, ValueClass::HomogeneousFloatAggregate { .. })
            {
                shape.byte_size.div_ceil(8) as u8
            } else {
                members
            }
        });
        if let Some(registers_needed) = float_registers_needed
            && float_index.saturating_add(registers_needed) <= float_register_count
        {
            if policy == CallingPolicy::SystemVAMD64
                && matches!(shape.class, ValueClass::HomogeneousFloatAggregate { .. })
            {
                locations.extend(sysv_sse_fragment_locations(shape, float_index));
            } else {
                let members = float_members.expect("float register count came from members");
                let member_size = shape.byte_size / u16::from(members);
                for member in 0..members {
                    locations.push(ValueLocation::Register {
                        register: float_register(float_index + member),
                        value_byte_offset: u16::from(member) * member_size,
                        byte_size: member_size,
                    });
                }
            }
            float_index += registers_needed;
        } else if float_members.is_none() && shape.byte_size > 16 {
            if policy == CallingPolicy::Aapcs64 {
                let pointer = if integer_index < integer_registers.len() {
                    let register = integer_registers[integer_index];
                    integer_index += 1;
                    IndirectPointerLocation::Register(register)
                } else {
                    stack_offset = align_up(stack_offset, 8);
                    let pointer = IndirectPointerLocation::Stack {
                        stack_byte_offset: stack_offset,
                        alignment: 8,
                    };
                    stack_offset += 8;
                    pointer
                };
                locations.push(ValueLocation::Indirect {
                    pointer,
                    copy_stack_byte_offset: None,
                    byte_size: shape.byte_size,
                    alignment: shape.alignment,
                });
            } else {
                debug_assert_eq!(policy, CallingPolicy::SystemVAMD64);
                stack_offset = align_up(stack_offset, u32::from(shape.alignment.clamp(8, 16)));
                locations.extend(integer_stack_fragment_locations(shape, stack_offset));
                stack_offset += u32::from(shape.byte_size).next_multiple_of(8);
            }
        } else if float_members.is_none() && shape.byte_size > 8 {
            // Integer-class aggregates up to two eightbytes stay whole: use
            // consecutive integer registers only when the complete value
            // fits, otherwise place every fragment on the stack.
            let register_count = usize::from(shape.byte_size.div_ceil(8));
            if policy == CallingPolicy::Aapcs64 && shape.alignment >= 16 {
                integer_index = integer_index.next_multiple_of(2);
            }
            if integer_index + register_count <= integer_registers.len() {
                locations.extend(integer_register_fragment_locations(
                    shape,
                    integer_registers,
                    integer_index,
                ));
                integer_index += register_count;
            } else {
                // AAPCS64 advances NGRN to eight after a register-exhausted
                // aggregate. SysV rolls back the tentative assignment, so a
                // later scalar may still consume the remaining register.
                if policy == CallingPolicy::Aapcs64 {
                    integer_index = integer_registers.len();
                }
                stack_offset = align_up(stack_offset, u32::from(shape.alignment.clamp(8, 16)));
                locations.extend(integer_stack_fragment_locations(shape, stack_offset));
                stack_offset += u32::from(shape.byte_size).next_multiple_of(8);
            }
        } else if float_members.is_some() && policy == CallingPolicy::SystemVAMD64 {
            stack_offset = align_up(stack_offset, u32::from(shape.alignment.clamp(8, 16)));
            locations.extend(integer_stack_fragment_locations(shape, stack_offset));
            stack_offset += u32::from(shape.byte_size).next_multiple_of(8);
        } else if float_members.is_none() && integer_index < integer_registers.len() {
            locations.push(register_location(integer_registers[integer_index], shape));
            integer_index += 1;
        } else {
            stack_offset = align_up(stack_offset, u32::from(shape.alignment));
            locations.push(stack_location(stack_offset, shape));
            stack_offset += u32::from(shape.byte_size.max(8));
        }
        parameters.push(ValuePlacement { shape, locations });
    }
    for placement in &mut parameters {
        if let [
            ValueLocation::Indirect {
                copy_stack_byte_offset,
                alignment,
                byte_size,
                ..
            },
        ] = placement.locations.as_mut_slice()
        {
            stack_offset = align_up(stack_offset, u32::from((*alignment).clamp(8, 16)));
            if !matches!(placement.shape.class, ValueClass::BorrowedReference) {
                *copy_stack_byte_offset = Some(stack_offset);
                stack_offset += u32::from(*byte_size).next_multiple_of(8);
            }
        }
    }
    Ok(CallPlan {
        policy,
        parameters,
        result: result_placement(signature.result, integer_results, float_register)?,
        callback_materializations: Vec::new(),
        ordinary_clobbers,
        stack_alignment,
        shadow_bytes: 0,
        entry_control: EntryControl::CallReturn,
    })
}

fn integer_register_fragment_locations(
    shape: ValueShape,
    registers: &[MachineRegister],
    first_register: usize,
) -> Vec<ValueLocation> {
    (0..usize::from(shape.byte_size.div_ceil(8)))
        .map(|fragment| {
            let value_byte_offset = fragment * 8;
            ValueLocation::Register {
                register: registers[first_register + fragment],
                value_byte_offset: value_byte_offset as u16,
                byte_size: (usize::from(shape.byte_size) - value_byte_offset).min(8) as u16,
            }
        })
        .collect()
}

fn integer_stack_fragment_locations(
    shape: ValueShape,
    first_stack_byte_offset: u32,
) -> Vec<ValueLocation> {
    (0..usize::from(shape.byte_size.div_ceil(8)))
        .map(|fragment| {
            let value_byte_offset = fragment * 8;
            ValueLocation::Stack {
                stack_byte_offset: first_stack_byte_offset + value_byte_offset as u32,
                value_byte_offset: value_byte_offset as u16,
                byte_size: (usize::from(shape.byte_size) - value_byte_offset).min(8) as u16,
                alignment: 8,
            }
        })
        .collect()
}

fn sysv_sse_fragment_locations(shape: ValueShape, first_register: u8) -> Vec<ValueLocation> {
    (0..shape.byte_size.div_ceil(8))
        .map(|fragment| {
            let value_byte_offset = fragment * 8;
            ValueLocation::Register {
                register: MachineRegister::X86Xmm(first_register + fragment as u8),
                value_byte_offset,
                byte_size: (shape.byte_size - value_byte_offset).min(8),
            }
        })
        .collect()
}

fn evaluate_linux_syscall_x86_64(signature: &CallSignature) -> Result<CallPlan, PlanDiagnostic> {
    let registers = [
        MachineRegister::X86Rdi,
        MachineRegister::X86Rsi,
        MachineRegister::X86Rdx,
        MachineRegister::X86R10,
        MachineRegister::X86R8,
        MachineRegister::X86R9,
    ];
    evaluate_syscall(
        CallingPolicy::LinuxSyscallX86_64,
        signature,
        &registers,
        MachineRegister::X86Rax,
        MachineRegister::X86Rax,
        RegisterSet::new([
            MachineRegister::X86Rax,
            MachineRegister::X86Rcx,
            MachineRegister::X86R11,
        ]),
    )
}

fn evaluate_linux_syscall_aarch64(signature: &CallSignature) -> Result<CallPlan, PlanDiagnostic> {
    let registers = (0..6).map(MachineRegister::Aarch64X).collect::<Vec<_>>();
    evaluate_syscall(
        CallingPolicy::LinuxSyscallAarch64,
        signature,
        &registers,
        MachineRegister::Aarch64X(8),
        MachineRegister::Aarch64X(0),
        RegisterSet::new(
            (0..=5)
                .map(MachineRegister::Aarch64X)
                .chain([MachineRegister::Aarch64X(8)]),
        ),
    )
}

fn evaluate_syscall(
    policy: CallingPolicy,
    signature: &CallSignature,
    registers: &[MachineRegister],
    number_register: MachineRegister,
    result_register: MachineRegister,
    ordinary_clobbers: RegisterSet,
) -> Result<CallPlan, PlanDiagnostic> {
    if signature.parameters.len() > registers.len() {
        return Err(PlanDiagnostic(format!(
            "{policy:?} admits at most {} parameters",
            registers.len()
        )));
    }
    if signature
        .parameters
        .iter()
        .any(|shape| !matches!(shape.class, ValueClass::Integer) || shape.byte_size > 8)
    {
        return Err(PlanDiagnostic(
            "the normalized Linux syscall plans currently admit only integer/pointer values up to 8 bytes"
                .into(),
        ));
    }
    let parameters = signature
        .parameters
        .iter()
        .copied()
        .zip(registers.iter().copied())
        .map(|(shape, register)| ValuePlacement {
            shape,
            locations: vec![register_location(register, shape)],
        })
        .collect();
    let result = match signature.result {
        Some(shape) if matches!(shape.class, ValueClass::Integer) && shape.byte_size <= 8 => {
            Some(ValuePlacement {
                shape,
                locations: vec![register_location(result_register, shape)],
            })
        }
        Some(_) => {
            return Err(PlanDiagnostic(
                "the normalized Linux syscall result must be an integer/pointer value up to 8 bytes"
                    .into(),
            ));
        }
        None => None,
    };
    Ok(CallPlan {
        policy,
        parameters,
        result,
        callback_materializations: Vec::new(),
        ordinary_clobbers,
        stack_alignment: 16,
        shadow_bytes: 0,
        entry_control: EntryControl::SupervisorCall {
            number_register,
            immediate: 0,
        },
    })
}

fn result_placement(
    result: Option<ValueShape>,
    integer_registers: &[MachineRegister],
    float_register: impl Fn(u8) -> MachineRegister,
) -> Result<Option<ValuePlacement>, PlanDiagnostic> {
    let Some(shape) = result else {
        return Ok(None);
    };
    let locations = match shape.class {
        ValueClass::Integer => vec![register_location(
            *integer_registers.first().ok_or_else(|| {
                PlanDiagnostic("calling policy has no integer result register".into())
            })?,
            shape,
        )],
        ValueClass::Float => vec![register_location(float_register(0), shape)],
        ValueClass::BorrowedReference => {
            return Err(PlanDiagnostic(
                "borrowed references cannot be call results".into(),
            ));
        }
        ValueClass::HomogeneousFloatAggregate { members } => {
            let member_size = shape.byte_size / u16::from(members);
            (0..members)
                .map(|member| ValueLocation::Register {
                    register: float_register(member),
                    value_byte_offset: u16::from(member) * member_size,
                    byte_size: member_size,
                })
                .collect()
        }
        ValueClass::SystemVAggregate { first, second } => {
            let classes = [first, second];
            let mut integer_index = 0usize;
            let mut float_index = 0u8;
            classes
                .into_iter()
                .enumerate()
                .map(|(fragment, class)| {
                    let register = match class {
                        SystemVEightbyteClass::Integer => {
                            let register =
                                *integer_registers.get(integer_index).ok_or_else(|| {
                                    PlanDiagnostic(
                                    "calling policy has too few integer aggregate result registers"
                                        .into(),
                                )
                                })?;
                            integer_index += 1;
                            register
                        }
                        SystemVEightbyteClass::Sse => {
                            let register = float_register(float_index);
                            float_index += 1;
                            register
                        }
                    };
                    let value_byte_offset = fragment as u16 * 8;
                    Ok(ValueLocation::Register {
                        register,
                        value_byte_offset,
                        byte_size: (shape.byte_size - value_byte_offset).min(8),
                    })
                })
                .collect::<Result<Vec<_>, PlanDiagnostic>>()?
        }
    };
    Ok(Some(ValuePlacement { shape, locations }))
}

fn register_location(register: MachineRegister, shape: ValueShape) -> ValueLocation {
    ValueLocation::Register {
        register,
        value_byte_offset: 0,
        byte_size: shape.byte_size,
    }
}

fn stack_location(stack_byte_offset: u32, shape: ValueShape) -> ValueLocation {
    ValueLocation::Stack {
        stack_byte_offset,
        value_byte_offset: 0,
        byte_size: shape.byte_size,
        alignment: shape.alignment.min(16),
    }
}

fn align_up(value: u32, alignment: u32) -> u32 {
    let mask = alignment - 1;
    (value + mask) & !mask
}
