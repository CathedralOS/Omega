//! Evaluating one via binding and validating its shape and widths.

use crate::evaluated_via_bindings::binding_digests::{
    at, evaluation_digest, materialization_digest, producer_closure_digest,
};
use crate::evaluated_via_bindings::binding_values::{
    DecodedBindingValue, decode_binding_value, retained_usage,
};
use crate::evaluated_via_bindings::{
    BindingVocabulary, EvaluatedViaBinding, EvaluatedViaBindingRow, MATERIALIZER_SCHEMA_VERSION,
};
use build_time_evaluation::{
    BuildTimeAdmissionPlan, BuildTimeInvocationCustody, CURRENT_EVALUATION_SEMANTICS,
};
use diagnostics::Diagnostic;
use effects::provider_plan::{
    EvaluatedBindingReceipt, EvaluatedForeignImport, EvaluatedForeignSyscall,
};
use source::{SourceFile, SourceOrigin};
use std::path::Path;
use symbols::SymbolHandle;
use target::{TargetProfile, normalize_foreign_locator};
use typed_trees::TypedTrees;
use typed_trees::data::{DataDefinition, DataMember, TypeParameterKind};
use typed_trees::expression::ExpressionNode;
use typed_trees::types::{FixedArrayLength, PrimitiveType, TypeReferenceHandle, TypeReferenceNode};

pub(crate) fn evaluate_one(
    typed: &TypedTrees,
    admission: &BuildTimeAdmissionPlan,
    vocabulary: &BindingVocabulary,
    target: TargetProfile,
    machine: &typed_trees::machine::Machine,
    conformance: &typed_trees::machine::TraitConformance,
) -> Result<EvaluatedViaBindingRow, Diagnostic> {
    let expression = conformance.via_expression;
    let source_span = typed.expression_table.source_span(expression);
    if conformance.external_binding.is_some() {
        return Err(at(
            source_span,
            "external realization cannot combine legacy and ordinary `via` bindings",
        ));
    }
    let ExpressionNode::Call(call) = typed.expression_table.expression(expression) else {
        return Err(at(
            source_span,
            "ordinary external `via` must remain one exact machine call",
        ));
    };
    let producers = typed
        .machines()
        .iter()
        .filter_map(|producer| {
            typed
                .machine_states(producer)
                .iter()
                .find(|state| state.symbol == call.target_symbol)
                .map(|state| (producer, state))
        })
        .collect::<Vec<_>>();
    let [(producer, entry)] = producers.as_slice() else {
        return Err(at(
            source_span,
            "ordinary external `via` does not resolve to one exact producer entry",
        ));
    };
    if typed
        .machine_states(producer)
        .first()
        .is_none_or(|first| first.symbol != entry.symbol)
    {
        return Err(at(
            source_span,
            "ordinary external `via` target is not the producer entry state",
        ));
    }
    let widths = binding_widths(typed, vocabulary.binding, entry.return_type)
        .map_err(|message| at(source_span, message))?;
    let custody = BuildTimeInvocationCustody::Source(source_span);
    // The evaluated binding crosses the boundary as an ordinary closed const
    // value: the producer result is admitted under ConstEvaluable, with the
    // authored `via` span retained as its invocation custody.
    let measured = admission
        .evaluate_const_evaluable_machine_symbol_for_invocation_measured(
            typed,
            producer.symbol,
            Vec::new(),
            custody,
        )
        .map_err(|message| {
            at(
                source_span,
                format!("ordinary external `via` evaluation failed: {message}"),
            )
        })?;
    let closure = admission
        .admitted_machine_closure_symbols(typed, producer.symbol, custody)
        .map_err(|message| {
            at(
                source_span,
                format!("ordinary external `via` closure admission failed: {message}"),
            )
        })?;
    let (value, usage) = measured.into_parts();
    let decoded =
        decode_binding_value(&value, widths).map_err(|message| at(source_span, message))?;
    let binding_identity_digest = match &decoded {
        DecodedBindingValue::Import(candidate) => {
            normalize_foreign_locator(candidate.clone(), target)
                .map_err(|error| {
                    at(
                        source_span,
                        format!("ordinary external `via` returned an invalid locator: {error}"),
                    )
                })?
                .identity_digest()
        }
        DecodedBindingValue::Syscall { number } => {
            let number = u32::try_from(*number).map_err(|_| {
                at(
                    source_span,
                    "ordinary ForeignBinding::Syscall number does not fit u32",
                )
            })?;
            if !matches!(target, TargetProfile::LinuxArm64 | TargetProfile::LinuxX64) {
                return Err(at(
                    source_span,
                    format!(
                        "ordinary ForeignBinding::Syscall is not applicable to selected target `{}`",
                        target.target_name(),
                    ),
                ));
            }
            effects::provider_plan::evaluated_syscall_identity_digest(target, number)
        }
    };
    let producer_identity = typed
        .normalized_machine_overload_identity(producer)
        .map(|identity| identity.identity().to_owned())
        .ok_or_else(|| {
            at(
                source_span,
                "ordinary external `via` producer has no canonical callable identity",
            )
        })?;
    let closure_digest =
        producer_closure_digest(typed, &closure).map_err(|message| at(source_span, message))?;
    let evaluation_digest = evaluation_digest(target, closure_digest, usage, &value);
    let materialization_digest = materialization_digest(
        target,
        vocabulary.source_digest,
        widths,
        &value,
        binding_identity_digest.as_bytes(),
    );
    let retained_usage = retained_usage(usage).map_err(|message| at(source_span, message))?;
    let receipt = EvaluatedBindingReceipt::from_evaluation(
        typed.symbols.symbol_package_identity(producer.symbol),
        producer_identity,
        closure_digest,
        CURRENT_EVALUATION_SEMANTICS.marker(),
        retained_usage,
        evaluation_digest,
        MATERIALIZER_SCHEMA_VERSION,
        materialization_digest,
        binding_identity_digest,
    )
    .map_err(|message| at(source_span, message))?;
    let evaluated = match decoded {
        DecodedBindingValue::Import(candidate) => {
            let locator = normalize_foreign_locator(candidate, target).map_err(|error| {
                at(
                    source_span,
                    format!("ordinary external `via` returned an invalid locator: {error}"),
                )
            })?;
            EvaluatedViaBinding::Import(
                EvaluatedForeignImport::from_retained_evidence(locator, receipt)
                    .map_err(|message| at(source_span, message))?,
            )
        }
        DecodedBindingValue::Syscall { number } => EvaluatedViaBinding::Syscall(
            EvaluatedForeignSyscall::from_retained_evidence(target, number, receipt)
                .map_err(|message| at(source_span, message))?,
        ),
    };
    Ok(EvaluatedViaBindingRow {
        realization_machine: machine.symbol,
        satisfied_owner: conformance.symbol,
        requirement: conformance.requirement_symbol,
        via_expression: expression,
        producer_machine: producer.symbol,
        producer_entry_state: entry.symbol,
        via_source_span: source_span,
        evaluated,
    })
}

pub(crate) fn exact_binding_vocabulary(
    typed: &TypedTrees,
) -> Result<BindingVocabulary, Vec<Diagnostic>> {
    let exact = |name: &str| {
        typed
            .data_definitions()
            .iter()
            .filter(|definition| {
                definition.name.as_str() == name
                    && definition.generic_instance.is_none()
                    && definition.is_public
                    && definition.supply_mode == language_semantics::DataSupplyMode::CheckedShape
                    && exact_external_binding_source(typed, definition.symbol).is_some()
            })
            .collect::<Vec<_>>()
    };
    let bindings = exact("ForeignBinding");
    let imports = exact("DllImport");
    let ([binding], [dll_import]) = (bindings.as_slice(), imports.as_slice()) else {
        return Err(vec![Diagnostic::error(
            "ordinary external `via` requires the unique compiler-owned Binding and DllImport vocabulary",
        )]);
    };
    let binding_parameters = exact_width_parameters(typed, binding)?;
    let import_parameters = exact_width_parameters(typed, dll_import)?;
    validate_dll_import_shape(typed, dll_import, import_parameters)?;
    validate_binding_shape(typed, binding, binding_parameters, dll_import.symbol)?;
    let source = exact_external_binding_source(typed, binding.symbol)
        .expect("exact vocabulary predicate retained source");
    let source_digest = package_compilation::toolchain_source_identity_digest(source)?;
    Ok(BindingVocabulary {
        binding: binding.symbol,
        source_digest,
    })
}

fn exact_external_binding_source(typed: &TypedTrees, symbol: SymbolHandle) -> Option<&SourceFile> {
    let source = typed
        .symbols
        .symbol_source_span(symbol)
        .and_then(|span| typed.symbols.source_file(span))?;
    (source.origin == SourceOrigin::Toolchain
        && source
            .path
            .ends_with(Path::new("core/external_binding.omg")))
    .then_some(source)
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
        let DataMember::Variant(variant) = member else {
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
        let TypeReferenceNode::Named { symbol, .. } =
            typed.type_reference_table.type_reference(*argument)
        else {
            return Err(vec![Diagnostic::error(
                "compiler-owned Binding widths must pass through exact const binders",
            )]);
        };
        if *symbol != expected {
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
            .data_definitions()
            .iter()
            .find(|definition| definition.symbol == *symbol)
            .and_then(|definition| definition.generic_instance)
            .ok_or_else(|| {
                "ordinary external `via` producer must return a closed Binding application"
                    .to_owned()
            })?,
        TypeReferenceNode::Generic { .. } => return_type,
        _ => {
            return Err(
                "ordinary external `via` producer must return a closed Binding application"
                    .to_owned(),
            );
        }
    };
    let TypeReferenceNode::Generic {
        base_symbol,
        arguments,
        ..
    } = typed.type_reference_table.type_reference(application)
    else {
        return Err(
            "ordinary external `via` producer return lost its generic Binding origin".to_owned(),
        );
    };
    let arguments = typed
        .type_reference_table
        .type_reference_handles(*arguments);
    if *base_symbol != binding_symbol || arguments.len() != 3 {
        return Err("ordinary external `via` producer must return the exact compiler-owned ForeignBinding<ObjectLength, SymbolLength, VersionLength>".to_owned());
    }
    let mut widths = [0u64; 3];
    for (index, argument) in arguments.iter().enumerate() {
        let TypeReferenceNode::Named { symbol, name } =
            typed.type_reference_table.type_reference(*argument)
        else {
            return Err(
                "ordinary external `via` Binding widths must be closed decimal constants"
                    .to_owned(),
            );
        };
        if symbol.is_valid()
            || name.as_str().is_empty()
            || !name.as_str().bytes().all(|byte| byte.is_ascii_digit())
        {
            return Err(
                "ordinary external `via` Binding widths must be closed decimal constants"
                    .to_owned(),
            );
        }
        widths[index] = name
            .as_str()
            .parse::<u64>()
            .map_err(|_| "ordinary external `via` Binding width does not fit u64".to_owned())?;
    }
    Ok(widths)
}
