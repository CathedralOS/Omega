//! Ordinary native-input vocabulary. Field and variant coverage is exhaustive.

use super::record_fields;

record_fields!(calling_conventions::BoundaryEntryPlan { call, state });

record_fields!(calling_conventions::BoundaryNativeParameterGeometry {
    identity,
    native_ordinal,
    shape,
    origin
});

record_variants!(calling_conventions::BoundaryNativeParameterOrigin {
    0 => SemanticFormal { formal_ordinal },
    1 => PrivateCallback { binder, requirement },
});

record_variants!(calling_conventions::BoundaryNativeParameterShape {
    0 => Semantic(value0),
    1 => TargetFunctionPointer { byte_size, alignment },
});

record_fields!(calling_conventions::BoundaryOpaqueApplication {
    shape_root,
    application_report_fingerprint,
    selected_application_commitment,
    conformance_application_commitment,
    representation_schema_version,
    checked_semantic_copy
});

record_fields!(calling_conventions::BoundarySignatureGeometry {
    shapes,
    fields,
    parameters,
    result,
    native_parameters,
    native_parameter_rows,
    callback_layouts,
    callback_layout_rows
});

record_variants!(calling_conventions::BoundaryValueClass {
    0 => Integer,
    1 => Float,
    2 => Reference,
    3 => FixedArray { element, length },
    4 => Record { first_field, field_count },
});

record_fields!(calling_conventions::BoundaryValueField { shape, byte_offset });

record_fields!(calling_conventions::BoundaryValueShape {
    class,
    byte_size,
    alignment
});

record_fields!(calling_conventions::CallPlan {
    policy,
    parameters,
    result,
    callback_materializations,
    ordinary_clobbers,
    stack_alignment,
    shadow_bytes,
    entry_control
});

record_fields!(calling_conventions::CallSignature { parameters, result });

record_fields!(calling_conventions::CallbackBinderRequirement {
    binder,
    requirement
});

record_fields!(calling_conventions::CallbackDataGeometry {
    identity,
    policy,
    layout,
    size,
    alignment
});

record_fields!(calling_conventions::CallbackInlineFieldGeometry {
    identity,
    offset,
    child
});

record_fields!(calling_conventions::CallbackMaterialization {
    binder,
    destination
});

record_fields!(calling_conventions::CallbackMaterializationContext { binders, demands });

record_fields!(calling_conventions::CallbackSlotGeometry {
    identity,
    policy,
    requirement_identity,
    layout,
    slot,
    requirement,
    conformance_commitment,
    offset,
    size,
    alignment
});

record_variants!(calling_conventions::CallingPolicy {
    0 => MicrosoftX64,
    1 => SystemVAMD64,
    2 => Aapcs64,
    3 => LinuxSyscallX86_64,
    4 => LinuxSyscallAarch64,
});

record_variants!(calling_conventions::EntryControl {
    0 => CallReturn,
    1 => SupervisorCall { number_register, immediate },
    2 => InterruptReturn,
});

record_variants!(calling_conventions::EntryStack {
    0 => Interrupted,
    1 => Dedicated { class },
    2 => ProviderSelected,
});

record_variants!(calling_conventions::ExternalBindingKind {
    0 => Syscall { number },
    1 => Import { locator },
    2 => CompilerIntrinsic { machine },
    3 => VtableSlot { index },
    4 => VtableField { field, byte_offset },
    5 => TableFunction { field, byte_offset },
});

record_fields!(calling_conventions::ExternalBindingRow {
    target_name,
    trait_name,
    method,
    requirement_identity,
    table_type,
    calling_application,
    binding
});

record_variants!(calling_conventions::IndirectPointerLocation {
    0 => Register(value0),
    1 => Stack { stack_byte_offset, alignment },
});

record_variants!(calling_conventions::MachineRegime {
    0 => X86Long64,
    1 => Aarch64A64 { exception_level },
});

record_variants!(calling_conventions::MachineRegister {
    0 => X86Rax,
    1 => X86Rcx,
    2 => X86Rdx,
    3 => X86Rbx,
    4 => X86Rsp,
    5 => X86Rbp,
    6 => X86Rsi,
    7 => X86Rdi,
    8 => X86R8,
    9 => X86R9,
    10 => X86R10,
    11 => X86R11,
    12 => X86R12,
    13 => X86R13,
    14 => X86R14,
    15 => X86R15,
    16 => X86Xmm(value0),
    17 => Aarch64X(value0),
    18 => Aarch64V(value0),
});

record_variants!(calling_conventions::MachineState {
    0 => GeneralRegisters,
    1 => VectorRegisters,
    2 => Flags,
    3 => InstructionPointer,
    4 => StackPointer,
    5 => SegmentState,
    6 => ControlState,
    7 => DebugState,
    8 => ExtendedState,
});

record_fields!(calling_conventions::NativeCallbackDemand {
    destination,
    requirement
});

record_fields!(calling_conventions::NativeParameterApplication {
    parameter,
    native_ordinal,
    shape,
    placement
});

record_variants!(calling_conventions::NativePlace {
    0 => Parameter(value0),
    1 => Field { parameter, layout, field_path },
});

record_variants!(calling_conventions::Preemption {
    0 => NotApplicable,
    1 => Masked,
    2 => Nestable { maximum_depth },
    3 => ProviderDefined,
});

record_fields!(calling_conventions::StatePlan {
    initial_regime,
    interrupted_state,
    saved_state,
    restored_state,
    permitted_transitive_use,
    stack,
    preemption
});

record_variants!(calling_conventions::SystemVEightbyteClass {
    0 => Integer,
    1 => Sse,
});

record_variants!(calling_conventions::ValueClass {
    0 => Integer,
    1 => Float,
    2 => BorrowedReference,
    3 => BorrowedScalarReference,
    4 => HomogeneousFloatAggregate { members },
    5 => SystemVAggregate { first, second },
});

record_variants!(calling_conventions::ValueLocation {
    0 => Register { register, value_byte_offset, byte_size },
    1 => Stack { stack_byte_offset, value_byte_offset, byte_size, alignment },
    2 => Indirect { pointer, copy_stack_byte_offset, byte_size, alignment },
});

record_fields!(calling_conventions::ValuePlacement { shape, locations });

record_fields!(calling_conventions::ValueShape {
    class,
    byte_size,
    alignment
});
