//! Exact source-to-compiler agreement for the first ordinary foreign-binding
//! value surface.

use omega_compiler::compile_to_checked;
use psi_language_core::DataSupplyMode;
use psi_source::SourceOrigin;
use psi_typed_trees::data::{DataMember, TypeParameterKind};
use psi_typed_trees::types::{FixedArrayLength, PrimitiveType, TypeReferenceNode};
use std::fs;
use std::path::{Path, PathBuf};

struct TemporaryProgram(PathBuf);

impl TemporaryProgram {
    fn new() -> Self {
        let directory = std::env::temp_dir().join(format!(
            "omega-external-binding-core-surface-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&directory);
        fs::create_dir_all(&directory).expect("create external-binding core fixture");
        fs::write(
            directory.join("main.omg"),
            r#"use omega::language::core::external_binding;

data Main {}

machine Main::main(&mut self) {
}
"#,
        )
        .expect("write external-binding core fixture");
        Self(directory)
    }

    fn main(&self) -> PathBuf {
        self.0.join("main.omg")
    }
}

impl Drop for TemporaryProgram {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

#[test]
fn imported_external_binding_vocabulary_has_the_exact_first_rung_shape() {
    let fixture = TemporaryProgram::new();
    let checked = compile_to_checked(&fixture.main(), None).unwrap_or_else(|diagnostics| {
        panic!(
            "the normative external-binding vocabulary should compile:\n{}",
            diagnostics
                .iter()
                .map(ToString::to_string)
                .collect::<Vec<_>>()
                .join("\n")
        )
    });
    let typed = &checked.typed;

    let dll_imports = typed
        .data_definitions()
        .iter()
        .filter(|definition| definition.name.as_str() == "DllImport")
        .collect::<Vec<_>>();
    let [dll_import] = dll_imports.as_slice() else {
        panic!("core import must load exactly one DllImport declaration");
    };
    assert!(dll_import.is_public);
    assert_eq!(dll_import.supply_mode, DataSupplyMode::CheckedShape);
    assert_eq!(dll_import.generic_instance, None);
    assert_toolchain_source(typed, dll_import.symbol);
    let dll_parameters = exact_width_parameters(typed, dll_import);
    let members = typed.data_members(dll_import);
    let [
        DataMember::Variant(pe_by_name),
        DataMember::Variant(pe_by_ordinal),
        DataMember::Variant(elf_versioned),
        DataMember::Variant(macho_dylib_symbol),
    ] = members
    else {
        panic!("DllImport must remain the exact four-case closed sum");
    };

    assert_eq!(pe_by_name.name.as_str(), "PeByName");
    let [library, export] = typed.data_payload_fields(pe_by_name) else {
        panic!("PeByName must carry library and export");
    };
    assert_eq!(library.name.as_str(), "library");
    assert_eq!(export.name.as_str(), "export");
    assert_fixed_bytes(typed, library.type_reference, dll_parameters[0]);
    assert_fixed_bytes(typed, export.type_reference, dll_parameters[1]);

    assert_eq!(pe_by_ordinal.name.as_str(), "PeByOrdinal");
    let [library, ordinal] = typed.data_payload_fields(pe_by_ordinal) else {
        panic!("PeByOrdinal must carry library and ordinal");
    };
    assert_eq!(library.name.as_str(), "library");
    assert_eq!(ordinal.name.as_str(), "ordinal");
    assert_fixed_bytes(typed, library.type_reference, dll_parameters[0]);
    assert_eq!(
        typed.primitive_type_reference(ordinal.type_reference),
        Some(PrimitiveType::U16)
    );

    assert_eq!(elf_versioned.name.as_str(), "ElfVersioned");
    let [object, symbol, version] = typed.data_payload_fields(elf_versioned) else {
        panic!("ElfVersioned must carry object, symbol, and version");
    };
    assert_eq!(object.name.as_str(), "object");
    assert_eq!(symbol.name.as_str(), "symbol");
    assert_eq!(version.name.as_str(), "version");
    assert_fixed_bytes(typed, object.type_reference, dll_parameters[0]);
    assert_fixed_bytes(typed, symbol.type_reference, dll_parameters[1]);
    assert_fixed_bytes(typed, version.type_reference, dll_parameters[2]);

    assert_eq!(macho_dylib_symbol.name.as_str(), "MachODylibSymbol");
    let [install_name, symbol] = typed.data_payload_fields(macho_dylib_symbol) else {
        panic!("MachODylibSymbol must carry install_name and symbol");
    };
    assert_eq!(install_name.name.as_str(), "install_name");
    assert_eq!(symbol.name.as_str(), "symbol");
    assert_fixed_bytes(typed, install_name.type_reference, dll_parameters[0]);
    assert_fixed_bytes(typed, symbol.type_reference, dll_parameters[1]);

    let bindings = typed
        .data_definitions()
        .iter()
        .filter(|definition| definition.name.as_str() == "Binding")
        .collect::<Vec<_>>();
    let [binding] = bindings.as_slice() else {
        panic!("core import must load exactly one Binding declaration");
    };
    assert!(binding.is_public);
    assert_eq!(binding.supply_mode, DataSupplyMode::CheckedShape);
    assert_eq!(binding.generic_instance, None);
    assert_toolchain_source(typed, binding.symbol);
    let binding_parameters = exact_width_parameters(typed, binding);
    let [DataMember::Variant(import)] = typed.data_members(binding) else {
        panic!("the first Binding rung must remain import-only");
    };
    assert_eq!(import.name.as_str(), "DllImport");
    let [payload] = typed.data_payload_fields(import) else {
        panic!("Binding::DllImport must carry exactly one import payload");
    };
    assert_eq!(payload.name.as_str(), "import");
    let TypeReferenceNode::Generic {
        base_symbol,
        arguments,
        ..
    } = typed
        .type_reference_table
        .type_reference(payload.type_reference)
    else {
        panic!("Binding::DllImport payload must be an exact DllImport application");
    };
    assert_eq!(*base_symbol, dll_import.symbol);
    let arguments = typed
        .type_reference_table
        .type_reference_handles(*arguments);
    assert_eq!(arguments.len(), binding_parameters.len());
    for (argument, parameter) in arguments.iter().zip(binding_parameters) {
        let TypeReferenceNode::Named { symbol, .. } =
            typed.type_reference_table.type_reference(*argument)
        else {
            panic!("Binding widths must pass through their exact const binders");
        };
        assert_eq!(*symbol, parameter);
    }
}

fn exact_width_parameters(
    typed: &psi_typed_trees::TypedTrees,
    definition: &psi_typed_trees::data::DataDefinition,
) -> [psi_symbols::SymbolHandle; 3] {
    let [object, symbol, version] = typed.data_type_parameters(definition) else {
        panic!("binding vocabulary must carry exactly three width parameters");
    };
    for (parameter, name) in
        [object, symbol, version]
            .into_iter()
            .zip(["ObjectLength", "SymbolLength", "VersionLength"])
    {
        assert_eq!(parameter.name.as_str(), name);
        let TypeParameterKind::Const { type_reference } = parameter.kind else {
            panic!("{name} must be a const parameter");
        };
        assert_eq!(
            typed.primitive_type_reference(type_reference),
            Some(PrimitiveType::U64)
        );
    }
    [object.symbol, symbol.symbol, version.symbol]
}

fn assert_fixed_bytes(
    typed: &psi_typed_trees::TypedTrees,
    type_reference: psi_typed_trees::types::TypeReferenceHandle,
    length_symbol: psi_symbols::SymbolHandle,
) {
    let TypeReferenceNode::FixedArray {
        element_type,
        length:
            FixedArrayLength::ConstParameter {
                symbol: actual_length,
                ..
            },
    } = typed.type_reference_table.type_reference(type_reference)
    else {
        panic!("locator coordinate must be a const-sized fixed array");
    };
    assert_eq!(
        typed.primitive_type_reference(*element_type),
        Some(PrimitiveType::U8)
    );
    assert_eq!(*actual_length, length_symbol);
}

fn assert_toolchain_source(typed: &psi_typed_trees::TypedTrees, symbol: psi_symbols::SymbolHandle) {
    let source = typed
        .symbols
        .symbol_source_span(symbol)
        .and_then(|span| typed.symbols.source_file(span))
        .expect("binding declaration must retain authored source custody");
    assert_eq!(source.origin, SourceOrigin::Toolchain);
    assert!(
        source
            .path
            .ends_with(Path::new("core/external_binding.omg")),
        "expected external_binding.omg source custody, got {}",
        source.path.display()
    );
}
