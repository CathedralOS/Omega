use crate::{ObjectPlanningInput, build_object_plan};
use omega_calling_conventions::{
    HostAbiPlan, HostBinding, HostBindingMechanism, HostOperationReference, build_host_abi_plan,
};
use omega_control_flow::{MachineFunctionIdentity, StateKey};
use omega_layout::{DataLayout, FieldLayout, LayoutPlan, MachineLayout, TypeLayout, VariantLayout};
use omega_machine_bytes::{EncodedMachineFunction, EncodedMachinePlan};
use omega_object_file::{
    FunctionSymbolPlan, SectionKind, SymbolKind, SymbolSection, object_entry_symbol_name,
    object_function_symbol, private_function_symbol_name, runtime_frame_storage_symbol_name,
};
use omega_target::NativeTarget;
use omega_target_operations::{TargetDataObject, TargetDataPlan};
use psi_arena::Arena;
use psi_symbols::SymbolHandle;
use std::sync::Arc;

#[test]
fn builds_sections_and_symbols_for_runtime_frame_import_and_data() {
    let target = NativeTarget::host();
    let machine_symbol = SymbolHandle::invalid();
    let entry_function_identity = MachineFunctionIdentity::source(valid_source_key(2));
    let mut layouts = LayoutPlan {
        data_layouts: Arena::<DataLayout>::new(),
        fields: Arena::<FieldLayout>::new(),
        bit_fields: Vec::new(),
        stored_integers: Vec::new(),
        repeated_fields: Vec::new(),
        machine_layouts: Arena::<MachineLayout>::new(),
        variants: Arena::<VariantLayout>::new(),
    };
    layouts.machine_layouts.insert(MachineLayout {
        symbol: machine_symbol,
        layout: TypeLayout {
            size: 24,
            alignment: 8,
        },
        ..MachineLayout::default()
    });

    let mut encoded_machine = EncodedMachinePlan::with_capacity(target, 1, 0, 0);
    encoded_machine.code.byte_count = 64;
    encoded_machine
        .code
        .functions
        .insert(EncodedMachineFunction {
            symbol: std::sync::Arc::from(omega_object_file::entry_symbol_name(target)),
            identity: entry_function_identity,
            byte_offset: 32,
            byte_count: 12,
            instructions: Default::default(),
        });

    let mut host_abi = HostAbiPlan {
        target,
        bindings: Arena::<HostBinding>::new(),
        host_operations: Arena::<HostOperationReference>::new(),
        platform_call_lowerings: Arena::new(),
        boundary_policies: Arena::new(),
    };
    let retained = build_host_abi_plan(target)
        .bindings
        .iter()
        .next()
        .map(|(_, binding)| binding.clone())
        .expect("hosted target has a plan-bearing binding");
    host_abi.bindings.insert(HostBinding {
        mechanism: HostBindingMechanism::Import {
            library: Arc::from("host"),
            symbol: Arc::from("host_write"),
        },
        ..retained
    });

    let mut data = TargetDataPlan::with_capacity(1, 3);
    let data_bytes = data.bytes.insert_many([1, 2, 3]);
    data.objects.insert(TargetDataObject {
        symbol: Arc::from("payload"),
        offset: 4,
        bytes: data_bytes,
        ..TargetDataObject::default()
    });

    let object = build_object_plan(ObjectPlanningInput {
        target,
        host_abi: &host_abi,
        layouts: &layouts,
        entry_machine_symbol: machine_symbol,
        entry_machine_name: "Main",
        entry_function_identity,
        encoded_machine: &encoded_machine,
        data: &data,
        runtime_frame_size: 8,
        runtime_frame_alignment: 16,
    })
    .expect("object planning should produce sections and symbols");

    assert_eq!(object.layout.sections.len(), 3);
    assert_eq!(
        object
            .layout
            .sections
            .iter()
            .find(|(_, section)| section.kind == SectionKind::Text)
            .map(|(_, section)| section.size),
        Some(64)
    );
    assert_eq!(
        object
            .layout
            .sections
            .iter()
            .find(|(_, section)| section.kind == SectionKind::Data)
            .map(|(_, section)| section.size),
        Some(3)
    );
    assert_eq!(
        object
            .layout
            .sections
            .iter()
            .find(|(_, section)| section.kind == SectionKind::Bss)
            .map(|(_, section)| (section.size, section.alignment)),
        Some((40, 16))
    );

    let entry = object.layout.symbols.get(object.layout.entry_symbol);
    assert_eq!(object_entry_symbol_name(&object), entry.name);
    assert_eq!(entry.kind, SymbolKind::Function);
    assert_eq!(entry.section, SymbolSection::Section(SectionKind::Text));
    assert_eq!((entry.offset, entry.size), (32, 12));

    assert!(
        object
            .layout
            .symbols
            .iter()
            .any(|(_, symbol)| symbol.name == "host_write" && symbol.kind == SymbolKind::Import)
    );
    assert!(
        object
            .layout
            .symbols
            .iter()
            .any(|(_, symbol)| symbol.name == "payload"
                && symbol.kind == SymbolKind::Object
                && symbol.section == SymbolSection::Section(SectionKind::Data)
                && symbol.offset == 4
                && symbol.size == 3)
    );
    assert!(object.layout.symbols.iter().any(|(_, symbol)| symbol.name
        == runtime_frame_storage_symbol_name()
        && symbol.kind == SymbolKind::Object
        && symbol.offset == 32
        && symbol.size == 8));
}

#[test]
fn reports_missing_entry_machine_layout() {
    let target = NativeTarget::host();
    let entry_function_identity = MachineFunctionIdentity::source(valid_source_key(2));
    let host_abi = empty_host_abi(target);
    let layouts = empty_layouts();
    let mut encoded_machine = EncodedMachinePlan::with_capacity(target, 1, 0, 0);
    encoded_machine
        .code
        .functions
        .insert(EncodedMachineFunction {
            symbol: std::sync::Arc::from(omega_object_file::entry_symbol_name(target)),
            identity: entry_function_identity,
            byte_offset: 0,
            byte_count: 4,
            instructions: Default::default(),
        });
    let data = TargetDataPlan::with_capacity(0, 0);

    let diagnostic = build_object_plan(ObjectPlanningInput {
        target,
        host_abi: &host_abi,
        layouts: &layouts,
        entry_machine_symbol: SymbolHandle::invalid(),
        entry_machine_name: "Main",
        entry_function_identity,
        encoded_machine: &encoded_machine,
        data: &data,
        runtime_frame_size: 0,
        runtime_frame_alignment: 1,
    })
    .expect_err("object planning should require the entry machine layout");

    assert_eq!(
        diagnostic.message,
        "missing native layout for entry machine `Main`"
    );
}

#[test]
fn reports_missing_encoded_entry_function() {
    let target = NativeTarget::host();
    let machine_symbol = SymbolHandle::invalid();
    let host_abi = empty_host_abi(target);
    let mut layouts = empty_layouts();
    layouts.machine_layouts.insert(MachineLayout {
        symbol: machine_symbol,
        layout: TypeLayout {
            size: 8,
            alignment: 8,
        },
        ..MachineLayout::default()
    });
    let encoded_machine = EncodedMachinePlan::with_capacity(target, 0, 0, 0);
    let data = TargetDataPlan::with_capacity(0, 0);

    let diagnostic = build_object_plan(ObjectPlanningInput {
        target,
        host_abi: &host_abi,
        layouts: &layouts,
        entry_machine_symbol: machine_symbol,
        entry_machine_name: "Main",
        entry_function_identity: MachineFunctionIdentity::source(valid_source_key(2)),
        encoded_machine: &encoded_machine,
        data: &data,
        runtime_frame_size: 0,
        runtime_frame_alignment: 1,
    })
    .expect_err("object planning should require an encoded entry function");

    assert!(
        diagnostic
            .message
            .starts_with("missing encoded entry function `")
    );
    assert!(diagnostic.message.contains("for identity"));
}

#[test]
fn object_entry_can_name_generated_wrapper_without_relabeling_source_continuation() {
    let target = NativeTarget::host();
    let continuation_key = valid_source_key(2);
    let source_identity = MachineFunctionIdentity::source(continuation_key);
    let wrapper_identity = MachineFunctionIdentity::program_storage_entry_wrapper(continuation_key)
        .expect("valid source continuation should admit its canonical wrapper identity");
    let machine_symbol = SymbolHandle::invalid();
    let mut host_abi = empty_host_abi(target);
    let retained = build_host_abi_plan(target)
        .bindings
        .iter()
        .next()
        .map(|(_, binding)| binding.clone())
        .expect("hosted target has a plan-bearing binding");
    host_abi.bindings.insert(HostBinding {
        mechanism: HostBindingMechanism::Import {
            library: Arc::from("libc"),
            symbol: Arc::from("_write"),
        },
        ..retained
    });
    let layouts = layouts_with_entry_machine(machine_symbol);
    let mut encoded_machine = EncodedMachinePlan::with_capacity(target, 2, 0, 0);
    encoded_machine.code.byte_count = 48;
    encoded_machine
        .code
        .functions
        .insert(EncodedMachineFunction {
            // A source display symbol may legitimately coincide with a host
            // import. Object-local linkage must come from identity, not this
            // spelling.
            symbol: Arc::from("_write"),
            identity: source_identity,
            byte_offset: 8,
            byte_count: 12,
            instructions: Default::default(),
        });
    encoded_machine
        .code
        .functions
        .insert(EncodedMachineFunction {
            symbol: Arc::from(omega_object_file::entry_symbol_name(target)),
            identity: wrapper_identity,
            byte_offset: 32,
            byte_count: 8,
            instructions: Default::default(),
        });
    let data = TargetDataPlan::with_capacity(0, 0);

    let object = build_object_plan(ObjectPlanningInput {
        target,
        host_abi: &host_abi,
        layouts: &layouts,
        entry_machine_symbol: machine_symbol,
        entry_machine_name: "Main",
        entry_function_identity: wrapper_identity,
        encoded_machine: &encoded_machine,
        data: &data,
        runtime_frame_size: 0,
        runtime_frame_alignment: 1,
    })
    .expect("object entry should select the generated wrapper by canonical identity");

    let entry = object.layout.symbols.get(object.layout.entry_symbol);
    assert_eq!((entry.offset, entry.size), (32, 8));
    let (wrapper_symbol_handle, wrapper_symbol) = object_function_symbol(&object, wrapper_identity)
        .expect("wrapper identity should own one exact object text symbol");
    assert_eq!(wrapper_symbol_handle, object.layout.entry_symbol);
    assert_eq!(
        wrapper_symbol.name,
        omega_object_file::entry_symbol_name(target)
    );
    let (_, source_symbol) = object_function_symbol(&object, source_identity)
        .expect("source continuation should own a separately targetable text symbol");
    assert_eq!(
        source_symbol.name,
        private_function_symbol_name(source_identity).expect("canonical private source symbol")
    );
    assert_ne!(source_symbol.name, "_write");
    assert_eq!((source_symbol.offset, source_symbol.size), (8, 12));
    assert!(
        object
            .layout
            .symbols
            .iter()
            .any(|(_, symbol)| { symbol.name == "_write" && symbol.kind == SymbolKind::Import })
    );
    let source = encoded_machine
        .code
        .functions
        .iter()
        .find(|(_, function)| function.identity == source_identity)
        .map(|(_, function)| function)
        .expect("source continuation identity must remain retained separately");
    assert_eq!(source.symbol.as_ref(), "_write");
    assert_eq!((source.byte_offset, source.byte_count), (8, 12));
    assert_eq!(source.identity.source_key(), Some(continuation_key));

    let mut duplicate_binding = object.clone();
    duplicate_binding
        .layout
        .function_symbols
        .insert(FunctionSymbolPlan {
            identity: source_identity,
            symbol: wrapper_symbol_handle,
        });
    assert!(
        object_function_symbol(&duplicate_binding, source_identity).is_none(),
        "duplicate identity bindings must fail closed instead of selecting one"
    );
    let mut invalid_symbol_binding = object.clone();
    let source_binding_handle = invalid_symbol_binding
        .layout
        .function_symbols
        .iter()
        .find(|(_, binding)| binding.identity == source_identity)
        .map(|(handle, _)| handle)
        .expect("source function binding");
    invalid_symbol_binding
        .layout
        .function_symbols
        .get_mut(source_binding_handle)
        .symbol = psi_arena::Handle::invalid();
    assert!(
        object_function_symbol(&invalid_symbol_binding, source_identity).is_none(),
        "a detached function-symbol binding must fail closed"
    );
}

#[test]
fn object_planning_rejects_duplicate_private_function_identity() {
    let target = NativeTarget::host();
    let continuation_key = valid_source_key(2);
    let source_identity = MachineFunctionIdentity::source(continuation_key);
    let wrapper_identity = MachineFunctionIdentity::program_storage_entry_wrapper(continuation_key)
        .expect("valid continuation should admit wrapper identity");
    let machine_symbol = SymbolHandle::invalid();
    let host_abi = empty_host_abi(target);
    let layouts = layouts_with_entry_machine(machine_symbol);
    let mut encoded_machine = EncodedMachinePlan::with_capacity(target, 3, 0, 0);
    encoded_machine.code.byte_count = 48;
    encoded_machine
        .code
        .functions
        .insert(EncodedMachineFunction {
            symbol: Arc::from("source_a"),
            identity: source_identity,
            byte_offset: 0,
            byte_count: 8,
            instructions: Default::default(),
        });
    let source_b = encoded_machine
        .code
        .functions
        .insert(EncodedMachineFunction {
            symbol: Arc::from("source_b"),
            identity: source_identity,
            byte_offset: 8,
            byte_count: 8,
            instructions: Default::default(),
        });
    encoded_machine
        .code
        .functions
        .insert(EncodedMachineFunction {
            symbol: Arc::from(omega_object_file::entry_symbol_name(target)),
            identity: wrapper_identity,
            byte_offset: 32,
            byte_count: 8,
            instructions: Default::default(),
        });
    let data = TargetDataPlan::with_capacity(0, 0);

    let build = |encoded_machine: &EncodedMachinePlan| {
        build_object_plan(ObjectPlanningInput {
            target,
            host_abi: &host_abi,
            layouts: &layouts,
            entry_machine_symbol: machine_symbol,
            entry_machine_name: "Main",
            entry_function_identity: wrapper_identity,
            encoded_machine,
            data: &data,
            runtime_frame_size: 0,
            runtime_frame_alignment: 1,
        })
    };
    let diagnostic = build(&encoded_machine)
        .expect_err("one function identity must not select between private text functions");

    assert!(
        diagnostic
            .message
            .contains("names more than one text function")
    );

    encoded_machine.code.functions.get_mut(source_b).identity =
        MachineFunctionIdentity::source(valid_source_key(3));
    encoded_machine.code.functions.get_mut(source_b).byte_offset = 4;
    let diagnostic = build(&encoded_machine)
        .expect_err("separate function identities must not overlap their text intervals");
    assert!(diagnostic.message.contains("overlapping text intervals"));

    encoded_machine.code.functions.get_mut(source_b).byte_offset = 8;
    encoded_machine.code.byte_count = 39;
    let diagnostic = build(&encoded_machine)
        .expect_err("a function link target must remain within the encoded program");
    assert!(diagnostic.message.contains("exceeds the encoded program"));
}

#[test]
fn callback_thunk_preserves_placement_bound_private_symbol_and_identity() {
    let target = NativeTarget::host();
    let continuation_key = valid_source_key(2);
    let entry_identity = MachineFunctionIdentity::source(continuation_key);
    let callback_identity = MachineFunctionIdentity::callback_thunk(continuation_key, 7)
        .expect("valid callback continuation");
    let callback_symbol = "__omega_callback_e00000009g00000001_a00000007_exact";
    let machine_symbol = SymbolHandle::invalid();
    let host_abi = empty_host_abi(target);
    let layouts = layouts_with_entry_machine(machine_symbol);
    let mut encoded_machine = EncodedMachinePlan::with_capacity(target, 2, 0, 0);
    encoded_machine.code.byte_count = 24;
    encoded_machine
        .code
        .functions
        .insert(EncodedMachineFunction {
            symbol: Arc::from(omega_object_file::entry_symbol_name(target)),
            identity: entry_identity,
            byte_offset: 0,
            byte_count: 8,
            instructions: Default::default(),
        });
    encoded_machine
        .code
        .functions
        .insert(EncodedMachineFunction {
            symbol: Arc::from(callback_symbol),
            identity: callback_identity,
            byte_offset: 8,
            byte_count: 16,
            instructions: Default::default(),
        });
    let data = TargetDataPlan::with_capacity(0, 0);

    let object = build_object_plan(ObjectPlanningInput {
        target,
        host_abi: &host_abi,
        layouts: &layouts,
        entry_machine_symbol: machine_symbol,
        entry_machine_name: "Main",
        entry_function_identity: entry_identity,
        encoded_machine: &encoded_machine,
        data: &data,
        runtime_frame_size: 0,
        runtime_frame_alignment: 1,
    })
    .expect("callback identity should retain its placement-bound symbol");

    let (_, symbol) = object_function_symbol(&object, callback_identity)
        .expect("callback identity should own one exact object text symbol");
    assert_eq!(symbol.name, callback_symbol);
    assert_eq!((symbol.offset, symbol.size), (8, 16));
    assert!(private_function_symbol_name(callback_identity).is_none());
}

#[test]
fn object_entry_rejects_wrapper_identity_for_another_continuation() {
    let target = NativeTarget::host();
    let selected_identity =
        MachineFunctionIdentity::program_storage_entry_wrapper(valid_source_key(2))
            .expect("valid continuation should admit wrapper identity");
    let encoded_identity =
        MachineFunctionIdentity::program_storage_entry_wrapper(valid_source_key(3))
            .expect("valid continuation should admit wrapper identity");
    let machine_symbol = SymbolHandle::invalid();
    let host_abi = empty_host_abi(target);
    let layouts = layouts_with_entry_machine(machine_symbol);
    let mut encoded_machine = EncodedMachinePlan::with_capacity(target, 1, 0, 0);
    encoded_machine
        .code
        .functions
        .insert(EncodedMachineFunction {
            symbol: Arc::from(omega_object_file::entry_symbol_name(target)),
            identity: encoded_identity,
            byte_offset: 0,
            byte_count: 8,
            instructions: Default::default(),
        });
    let data = TargetDataPlan::with_capacity(0, 0);

    let diagnostic = build_object_plan(ObjectPlanningInput {
        target,
        host_abi: &host_abi,
        layouts: &layouts,
        entry_machine_symbol: machine_symbol,
        entry_machine_name: "Main",
        entry_function_identity: selected_identity,
        encoded_machine: &encoded_machine,
        data: &data,
        runtime_frame_size: 0,
        runtime_frame_alignment: 1,
    })
    .expect_err("object entry must not accept a wrapper for another continuation");

    assert!(diagnostic.message.contains("has identity"));
    assert!(diagnostic.message.contains("not selected identity"));
}

#[test]
fn object_entry_rejects_duplicate_generated_wrapper_identity() {
    let target = NativeTarget::host();
    let wrapper_identity =
        MachineFunctionIdentity::program_storage_entry_wrapper(valid_source_key(2))
            .expect("valid continuation should admit wrapper identity");
    let machine_symbol = SymbolHandle::invalid();
    let host_abi = empty_host_abi(target);
    let layouts = layouts_with_entry_machine(machine_symbol);
    let mut encoded_machine = EncodedMachinePlan::with_capacity(target, 2, 0, 0);
    for byte_offset in [0, 8] {
        encoded_machine
            .code
            .functions
            .insert(EncodedMachineFunction {
                symbol: Arc::from(omega_object_file::entry_symbol_name(target)),
                identity: wrapper_identity,
                byte_offset,
                byte_count: 8,
                instructions: Default::default(),
            });
    }
    let data = TargetDataPlan::with_capacity(0, 0);

    let diagnostic = build_object_plan(ObjectPlanningInput {
        target,
        host_abi: &host_abi,
        layouts: &layouts,
        entry_machine_symbol: machine_symbol,
        entry_machine_name: "Main",
        entry_function_identity: wrapper_identity,
        encoded_machine: &encoded_machine,
        data: &data,
        runtime_frame_size: 0,
        runtime_frame_alignment: 1,
    })
    .expect_err("object entry must reject duplicate identity/symbol claims");

    assert!(diagnostic.message.contains("is ambiguous for identity"));
}

fn empty_host_abi(target: NativeTarget) -> HostAbiPlan {
    HostAbiPlan {
        target,
        bindings: Arena::<HostBinding>::new(),
        host_operations: Arena::<HostOperationReference>::new(),
        platform_call_lowerings: Arena::new(),
        boundary_policies: Arena::new(),
    }
}

fn valid_source_key(state: u32) -> StateKey {
    StateKey {
        machine: SymbolHandle::from_arena_index(1),
        state: SymbolHandle::from_arena_index(state),
        segment_index: 0,
    }
}

fn empty_layouts() -> LayoutPlan {
    LayoutPlan {
        data_layouts: Arena::<DataLayout>::new(),
        fields: Arena::<FieldLayout>::new(),
        bit_fields: Vec::new(),
        stored_integers: Vec::new(),
        repeated_fields: Vec::new(),
        machine_layouts: Arena::<MachineLayout>::new(),
        variants: Arena::<VariantLayout>::new(),
    }
}

fn layouts_with_entry_machine(machine_symbol: SymbolHandle) -> LayoutPlan {
    let mut layouts = empty_layouts();
    layouts.machine_layouts.insert(MachineLayout {
        symbol: machine_symbol,
        layout: TypeLayout {
            size: 8,
            alignment: 8,
        },
        ..MachineLayout::default()
    });
    layouts
}
