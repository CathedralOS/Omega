use crate::{ObjectPlanningInput, build_object_plan};
use omega_calling_conventions::{
    HostAbiPlan, HostBinding, HostBindingMechanism, HostOperationReference,
};
use omega_core::arena::Arena;
use omega_core::symbols::SymbolHandle;
use omega_layout::{DataLayout, FieldLayout, LayoutPlan, MachineLayout, TypeLayout, VariantLayout};
use omega_machine_bytes::{EncodedMachineFunction, EncodedMachinePlan};
use omega_object_file::{
    SectionKind, SymbolKind, SymbolSection, object_entry_symbol_name,
    runtime_frame_storage_symbol_name,
};
use omega_target::NativeTarget;
use omega_target_operations::{TargetDataObject, TargetDataPlan};
use std::sync::Arc;

#[test]
fn builds_sections_and_symbols_for_runtime_frame_import_and_data() {
    let target = NativeTarget::host();
    let machine_symbol = SymbolHandle::invalid();
    let mut layouts = LayoutPlan {
        data_layouts: Arena::<DataLayout>::new(),
        fields: Arena::<FieldLayout>::new(),
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
            source_key: Default::default(),
            byte_offset: 32,
            byte_count: 12,
        });

    let mut host_abi = HostAbiPlan {
        target,
        bindings: Arena::<HostBinding>::new(),
        host_operations: Arena::<HostOperationReference>::new(),
        platform_call_lowerings: Arena::new(),
    };
    host_abi.bindings.insert(HostBinding {
        mechanism: HostBindingMechanism::Import {
            library: Arc::from("host"),
            symbol: Arc::from("host_write"),
        },
        ..HostBinding::default()
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
        entry_state_key: Default::default(),
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
    let host_abi = empty_host_abi(target);
    let layouts = empty_layouts();
    let mut encoded_machine = EncodedMachinePlan::with_capacity(target, 1, 0, 0);
    encoded_machine
        .code
        .functions
        .insert(EncodedMachineFunction {
            source_key: Default::default(),
            byte_offset: 0,
            byte_count: 4,
        });
    let data = TargetDataPlan::with_capacity(0, 0);

    let diagnostic = build_object_plan(ObjectPlanningInput {
        target,
        host_abi: &host_abi,
        layouts: &layouts,
        entry_machine_symbol: SymbolHandle::invalid(),
        entry_machine_name: "Main",
        entry_state_key: Default::default(),
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
        entry_state_key: Default::default(),
        encoded_machine: &encoded_machine,
        data: &data,
        runtime_frame_size: 0,
        runtime_frame_alignment: 1,
    })
    .expect_err("object planning should require an encoded entry function");

    assert!(
        diagnostic
            .message
            .starts_with("missing encoded entry function for state key")
    );
}

fn empty_host_abi(target: NativeTarget) -> HostAbiPlan {
    HostAbiPlan {
        target,
        bindings: Arena::<HostBinding>::new(),
        host_operations: Arena::<HostOperationReference>::new(),
        platform_call_lowerings: Arena::new(),
    }
}

fn empty_layouts() -> LayoutPlan {
    LayoutPlan {
        data_layouts: Arena::<DataLayout>::new(),
        fields: Arena::<FieldLayout>::new(),
        machine_layouts: Arena::<MachineLayout>::new(),
        variants: Arena::<VariantLayout>::new(),
    }
}
