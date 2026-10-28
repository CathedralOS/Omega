//! Exact compiler-owned ForeignBinding vocabulary and closed const application.
//! This query authenticates source/type custody only; neither locator payload
//! validation nor this closed schema grants provider execution authority.
use diagnostics::Diagnostic;
use source::SourceOrigin;
use symbols::SymbolHandle;
use typed_trees::TypedTrees;
use typed_trees::data::{DataDefinition, DataMember, TypeParameterKind};
use typed_trees::types::{FixedArrayLength, PrimitiveType, TypeReferenceHandle, TypeReferenceNode};

pub fn native_binding_widths(
    typed: &TypedTrees,
    reference: TypeReferenceHandle,
) -> Result<[u64; 3], Vec<Diagnostic>> {
    let exact = |name: &str| {
        typed
            .data_definitions()
            .iter()
            .filter(|definition| {
                definition.name.as_str() == name
                    && definition.generic_instance.is_none()
                    && definition.is_public
                    && definition.supply_mode == language_semantics::DataSupplyMode::CheckedShape
                    && typed
                        .symbols
                        .symbol_source_span(definition.symbol)
                        .and_then(|span| typed.symbols.source_file(span))
                        .is_some_and(|file| {
                            file.origin == SourceOrigin::Toolchain
                                && file
                                    .path
                                    .ends_with(std::path::Path::new("core/external_binding.omg"))
                        })
            })
            .collect::<Vec<_>>()
    };
    let bindings = exact("ForeignBinding");
    let imports = exact("DllImport");
    let ([binding], [import]) = (bindings.as_slice(), imports.as_slice()) else {
        return Err(vec![Diagnostic::error(
            "native binding requires the unique compiler-owned ForeignBinding and DllImport declarations",
        )]);
    };
    let parameters = exact_width_parameters(typed, binding)?;
    let import_parameters = exact_width_parameters(typed, import)?;
    validate_dll_import_shape(typed, import, import_parameters)?;
    validate_binding_shape(typed, binding, parameters, import.symbol)?;
    binding_widths(typed, binding.symbol, reference)
        .map_err(|message| vec![Diagnostic::error(message)])
}

fn exact_width_parameters(
    typed: &TypedTrees,
    definition: &DataDefinition,
) -> Result<[SymbolHandle; 3], Vec<Diagnostic>> {
    let parameters = typed.data_type_parameters(definition);
    if parameters.len() != 3 {
        return Err(vec![Diagnostic::error(format!(
            "compiler-owned `{}` must retain exactly three width parameters",
            definition.name
        ))]);
    }
    let names = ["ObjectLength", "SymbolLength", "VersionLength"];
    let mut symbols = [SymbolHandle::invalid(); 3];
    for (index, (parameter, expected_name)) in parameters.iter().zip(names).enumerate() {
        let TypeParameterKind::Const { type_reference } = parameter.kind else {
            return Err(vec![Diagnostic::error(format!(
                "compiler-owned `{}` parameter `{expected_name}` must remain const u64",
                definition.name
            ))]);
        };
        if parameter.name.as_str() != expected_name
            || typed.primitive_type_reference(type_reference) != Some(PrimitiveType::U64)
        {
            return Err(vec![Diagnostic::error(format!(
                "compiler-owned `{}` parameter `{expected_name}` drifted from const u64",
                definition.name
            ))]);
        }
        symbols[index] = parameter.symbol;
    }
    Ok(symbols)
}

fn validate_dll_import_shape(
    typed: &TypedTrees,
    definition: &DataDefinition,
    widths: [SymbolHandle; 3],
) -> Result<(), Vec<Diagnostic>> {
    let members = typed.data_members(definition);
    let expected = [
        ("PeByName", &["library", "export"][..]),
        ("PeByOrdinal", &["library", "ordinal"][..]),
        ("ElfVersioned", &["object", "symbol", "version"][..]),
        ("MachODylibSymbol", &["install_name", "symbol"][..]),
    ];
    if members.len() != expected.len() {
        return Err(vec![Diagnostic::error(
            "compiler-owned DllImport case set drifted",
        )]);
    }
    for (index, (member, (variant_name, field_names))) in members.iter().zip(expected).enumerate() {
        let Some(variant) = member.as_variant() else {
            return Err(vec![Diagnostic::error(
                "compiler-owned DllImport must remain a closed case sum",
            )]);
        };
        let fields = typed.data_payload_fields(variant);
        if variant.name.as_str() != variant_name
            || fields.len() != field_names.len()
            || fields
                .iter()
                .zip(field_names)
                .any(|(field, name)| field.name.as_str() != *name)
        {
            return Err(vec![Diagnostic::error(format!(
                "compiler-owned DllImport::{variant_name} shape drifted"
            ))]);
        }
        match index {
            0 => {
                require_fixed_bytes(typed, fields[0].type_reference, widths[0])?;
                require_fixed_bytes(typed, fields[1].type_reference, widths[1])?;
            }
            1 => {
                require_fixed_bytes(typed, fields[0].type_reference, widths[0])?;
                if typed.primitive_type_reference(fields[1].type_reference)
                    != Some(PrimitiveType::U16)
                {
                    return Err(vec![Diagnostic::error(
                        "compiler-owned PeByOrdinal ordinal must remain u16",
                    )]);
                }
            }
            2 => {
                require_fixed_bytes(typed, fields[0].type_reference, widths[0])?;
                require_fixed_bytes(typed, fields[1].type_reference, widths[1])?;
                require_fixed_bytes(typed, fields[2].type_reference, widths[2])?;
            }
            3 => {
                require_fixed_bytes(typed, fields[0].type_reference, widths[0])?;
                require_fixed_bytes(typed, fields[1].type_reference, widths[1])?;
            }
            _ => unreachable!(),
        }
    }
    Ok(())
}

fn validate_binding_shape(
    typed: &TypedTrees,
    definition: &DataDefinition,
    widths: [SymbolHandle; 3],
    dll_import: SymbolHandle,
) -> Result<(), Vec<Diagnostic>> {
    let [
        DataMember::Variant(import_variant),
        DataMember::Variant(syscall_variant),
    ] = typed.data_members(definition)
    else {
        return Err(vec![Diagnostic::error(
            "compiler-owned Binding must retain the exact DllImport and Syscall cases",
        )]);
    };
    let [field] = typed.data_payload_fields(import_variant) else {
        return Err(vec![Diagnostic::error(
            "compiler-owned ForeignBinding::DllImport payload drifted",
        )]);
    };
    if import_variant.name.as_str() != "DllImport" || field.name.as_str() != "import" {
        return Err(vec![Diagnostic::error(
            "compiler-owned ForeignBinding::DllImport names drifted",
        )]);
    }
    let TypeReferenceNode::Generic {
        base_symbol,
        arguments,
        ..
    } = typed
        .type_reference_table
        .type_reference(field.type_reference)
    else {
        return Err(vec![Diagnostic::error(
            "compiler-owned Binding payload must remain an exact DllImport application",
        )]);
    };
    let arguments = typed
        .type_reference_table
        .type_reference_handles(*arguments);
    if *base_symbol != dll_import || arguments.len() != 3 {
        return Err(vec![Diagnostic::error(
            "compiler-owned Binding payload generic identity drifted",
        )]);
    }
    for (argument, expected) in arguments.iter().zip(widths) {
        let Some(symbol) = typed
            .type_reference_table
            .type_reference(*argument)
            .as_named_symbol()
        else {
            return Err(vec![Diagnostic::error(
                "compiler-owned Binding widths must pass through exact const binders",
            )]);
        };
        if symbol != expected {
            return Err(vec![Diagnostic::error(
                "compiler-owned Binding width binder identity drifted",
            )]);
        }
    }
    let [number] = typed.data_payload_fields(syscall_variant) else {
        return Err(vec![Diagnostic::error(
            "compiler-owned ForeignBinding::Syscall payload drifted",
        )]);
    };
    if syscall_variant.name.as_str() != "Syscall"
        || number.name.as_str() != "number"
        || typed.primitive_type_reference(number.type_reference) != Some(PrimitiveType::U64)
    {
        return Err(vec![Diagnostic::error(
            "compiler-owned ForeignBinding::Syscall must retain one u64 `number` field",
        )]);
    }
    Ok(())
}

fn require_fixed_bytes(
    typed: &TypedTrees,
    reference: TypeReferenceHandle,
    width: SymbolHandle,
) -> Result<(), Vec<Diagnostic>> {
    let TypeReferenceNode::FixedArray {
        element_type,
        length: FixedArrayLength::ConstParameter { symbol, .. },
    } = typed.type_reference_table.type_reference(reference)
    else {
        return Err(vec![Diagnostic::error(
            "compiler-owned foreign coordinate must remain a const-sized byte array",
        )]);
    };
    if *symbol != width || typed.primitive_type_reference(*element_type) != Some(PrimitiveType::U8)
    {
        return Err(vec![Diagnostic::error(
            "compiler-owned foreign coordinate width or element type drifted",
        )]);
    }
    Ok(())
}

fn binding_widths(
    typed: &TypedTrees,
    binding_symbol: SymbolHandle,
    return_type: TypeReferenceHandle,
) -> Result<[u64; 3], String> {
    let application = match typed.type_reference_table.type_reference(return_type) {
        TypeReferenceNode::Named { symbol, .. } => typed
            .data_definition_by_symbol(*symbol)
            .and_then(|definition| definition.generic_instance)
            .ok_or_else(|| "native locator must return a closed Binding application".to_owned())?,
        TypeReferenceNode::Generic { .. } => return_type,
        _ => {
            return Err("native locator must return a closed Binding application".to_owned());
        }
    };
    let TypeReferenceNode::Generic {
        base_symbol,
        arguments,
        ..
    } = typed.type_reference_table.type_reference(application)
    else {
        return Err("native locator return lost its generic Binding origin".to_owned());
    };
    let arguments = typed
        .type_reference_table
        .type_reference_handles(*arguments);
    if *base_symbol != binding_symbol || arguments.len() != 3 {
        return Err("native locator must return the exact compiler-owned ForeignBinding<ObjectLength, SymbolLength, VersionLength>".to_owned());
    }
    let mut widths = [0u64; 3];
    for (index, argument) in arguments.iter().enumerate() {
        let TypeReferenceNode::Named { symbol, name } =
            typed.type_reference_table.type_reference(*argument)
        else {
            return Err(
                "native locator Binding widths must be closed decimal constants".to_owned(),
            );
        };
        if symbol.is_valid()
            || name.as_str().is_empty()
            || !name.as_str().bytes().all(|byte| byte.is_ascii_digit())
        {
            return Err(
                "native locator Binding widths must be closed decimal constants".to_owned(),
            );
        }
        widths[index] = name
            .as_str()
            .parse::<u64>()
            .map_err(|_| "native locator Binding width does not fit u64".to_owned())?;
    }
    Ok(widths)
}
