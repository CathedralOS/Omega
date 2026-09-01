//! Exact evaluation and retention of ordinary source `via` bindings.
//!
//! The table is the sole bridge between a typed source expression and provider
//! derivation. It retains arena coordinates only for exact replay inside the
//! same compilation; the installed import owns stable locator and receipt
//! identity and contains no arena handles.

use omega_effects::provider_plan::{
    EvaluatedBindingEvaluationDigest, EvaluatedBindingMaterializationDigest,
    EvaluatedBindingProducerClosureDigest, EvaluatedBindingReceipt, EvaluatedBindingUsage,
    EvaluatedForeignImport,
};
use omega_package_compilation::PackageCompilationInputs;
use omega_target::{ForeignLocatorCandidate, TargetProfile, normalize_foreign_locator};
use psi_build_time_evaluation::{
    BuildTimeAdmissionPlan, BuildTimeInvocationCustody, BuildTimeValue,
    CURRENT_EVALUATION_SEMANTICS, EvaluationUsage,
};
use psi_diagnostics::Diagnostic;
use psi_source::{SourceFile, SourceOrigin, SourceSpan};
use psi_symbols::SymbolHandle;
use psi_typed_trees::TypedTrees;
use psi_typed_trees::data::{DataDefinition, DataMember, TypeParameterKind};
use psi_typed_trees::expression::ExpressionNode;
use psi_typed_trees::types::{
    FixedArrayLength, PrimitiveType, TypeReferenceHandle, TypeReferenceNode,
};
use sha2::{Digest, Sha256};
use std::path::Path;
use std::sync::Arc;

const MATERIALIZER_SCHEMA_VERSION: u32 = 1;

/// Exact typed-program join for one ordinary `via` expression.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EvaluatedViaBindingRow {
    realization_machine: SymbolHandle,
    satisfied_owner: SymbolHandle,
    requirement: SymbolHandle,
    via_expression: psi_typed_trees::expression::ExpressionHandle,
    producer_machine: SymbolHandle,
    producer_entry_state: SymbolHandle,
    via_source_span: SourceSpan,
    evaluated: EvaluatedForeignImport,
}

impl EvaluatedViaBindingRow {
    pub const fn realization_machine(&self) -> SymbolHandle {
        self.realization_machine
    }
    pub const fn satisfied_owner(&self) -> SymbolHandle {
        self.satisfied_owner
    }
    pub const fn requirement(&self) -> SymbolHandle {
        self.requirement
    }
    pub const fn via_expression(&self) -> psi_typed_trees::expression::ExpressionHandle {
        self.via_expression
    }
    pub const fn producer_machine(&self) -> SymbolHandle {
        self.producer_machine
    }
    pub const fn producer_entry_state(&self) -> SymbolHandle {
        self.producer_entry_state
    }
    pub const fn via_source_span(&self) -> SourceSpan {
        self.via_source_span
    }
    pub const fn evaluated(&self) -> &EvaluatedForeignImport {
        &self.evaluated
    }
}

/// Complete evaluated ordinary-`via` population for one typed compilation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EvaluatedViaBindingTable {
    target: Option<TargetProfile>,
    rows: Vec<EvaluatedViaBindingRow>,
}

impl EvaluatedViaBindingTable {
    pub const fn target(&self) -> Option<TargetProfile> {
        self.target
    }
    pub fn rows(&self) -> &[EvaluatedViaBindingRow] {
        &self.rows
    }

    pub fn exact(
        &self,
        realization_machine: SymbolHandle,
        satisfied_owner: SymbolHandle,
        requirement: SymbolHandle,
    ) -> Option<&EvaluatedViaBindingRow> {
        self.rows.iter().find(|row| {
            row.realization_machine == realization_machine
                && row.satisfied_owner == satisfied_owner
                && row.requirement == requirement
        })
    }

    /// Replay every arena-local join before a later trust boundary consumes
    /// this table. Stable receipt data remains immutable; this catches a
    /// substituted conformance, expression, producer entry, or producer
    /// identity in a retained/mutated typed program.
    pub fn validate_against_typed(&self, typed: &TypedTrees) -> Result<(), Vec<Diagnostic>> {
        let expected = typed
            .machines()
            .iter()
            .flat_map(|machine| {
                typed
                    .machine_trait_conformances(machine)
                    .iter()
                    .filter(|conformance| conformance.via_expression.is_valid())
                    .map(move |conformance| (machine, conformance))
            })
            .collect::<Vec<_>>();
        let mut diagnostics = Vec::new();
        if expected.len() != self.rows.len() {
            diagnostics.push(Diagnostic::error(format!(
                "evaluated `via` binding table retains {} rows for {} exact typed expressions",
                self.rows.len(),
                expected.len(),
            )));
        }
        for (machine, conformance) in expected {
            let source_span = typed
                .expression_table
                .source_span(conformance.via_expression);
            if conformance.external_binding.is_some() {
                diagnostics.push(at(
                    source_span,
                    "ordinary external `via` replay found a legacy binding on the same conformance",
                ));
                continue;
            }
            if machine.body_is_present
                || !matches!(
                    machine.supply_mode,
                    psi_language_semantics::MachineSupplyMode::ExternalRealization {
                        binding: None,
                        mechanism: None,
                    }
                )
                || conformance.external_binding_source_span.is_none()
            {
                diagnostics.push(at(
                    source_span,
                    "ordinary external `via` replay found a mixed, body-bearing, or source-uncustodied supply carrier",
                ));
                continue;
            }
            let matches = self
                .rows
                .iter()
                .filter(|row| {
                    row.realization_machine == machine.symbol
                        && row.satisfied_owner == conformance.symbol
                        && row.requirement == conformance.requirement_symbol
                })
                .collect::<Vec<_>>();
            let [row] = matches.as_slice() else {
                diagnostics.push(at(
                    source_span,
                    format!(
                        "ordinary external `via` replay found {} evaluated rows for one exact conformance",
                        matches.len(),
                    ),
                ));
                continue;
            };
            let call = match typed
                .expression_table
                .expression(conformance.via_expression)
            {
                ExpressionNode::Call(call) => Some(call),
                _ => None,
            };
            let producers = call
                .into_iter()
                .flat_map(|call| {
                    typed.machines().iter().filter_map(move |producer| {
                        typed
                            .machine_states(producer)
                            .iter()
                            .find(|state| state.symbol == call.target_symbol)
                            .map(|state| (producer, state))
                    })
                })
                .collect::<Vec<_>>();
            let producer = match producers.as_slice() {
                [(producer, entry)]
                    if producer.symbol == row.producer_machine
                        && entry.symbol == row.producer_entry_state
                        && typed
                            .machine_states(producer)
                            .first()
                            .is_some_and(|first| first.symbol == entry.symbol) =>
                {
                    Some(*producer)
                }
                _ => None,
            };
            let producer_matches_receipt = producer.is_some_and(|producer| {
                typed
                    .normalized_machine_overload_identity(producer)
                    .is_some_and(|identity| {
                        identity.identity() == row.evaluated.receipt().producer_callable_identity()
                    })
                    && typed.symbols.symbol_package_identity(producer.symbol)
                        == row.evaluated.receipt().producer_package()
            });
            if row.via_expression != conformance.via_expression
                || row.via_source_span != source_span
                || !producer_matches_receipt
                || self.target != Some(row.evaluated.locator().target())
                || row.evaluated.receipt().locator_identity_digest()
                    != row.evaluated.locator().identity_digest()
            {
                diagnostics.push(at(
                    source_span,
                    "ordinary external `via` replay disagrees with its retained expression, producer, target, or receipt",
                ));
            }
        }
        if diagnostics.is_empty() {
            Ok(())
        } else {
            Err(diagnostics)
        }
    }
}

struct BindingVocabulary {
    binding: SymbolHandle,
    source_digest: [u8; 32],
}

/// Evaluate every ordinary `via` leaf exactly once and retain the resulting
/// atomic normalized import plus its durable evaluation receipt.
pub fn evaluate_via_bindings(
    typed: &TypedTrees,
    selected_target: Option<TargetProfile>,
    package_inputs: Option<&PackageCompilationInputs>,
) -> Result<EvaluatedViaBindingTable, Vec<Diagnostic>> {
    let pending = typed
        .machines()
        .iter()
        .flat_map(|machine| {
            typed
                .machine_trait_conformances(machine)
                .iter()
                .filter(|conformance| conformance.via_expression.is_valid())
                .map(move |conformance| (machine, conformance))
        })
        .collect::<Vec<_>>();
    if pending.is_empty() {
        return Ok(EvaluatedViaBindingTable {
            target: selected_target,
            rows: Vec::new(),
        });
    }
    let Some(target) = selected_target else {
        return Err(vec![Diagnostic::error(
            "ordinary external `via` evaluation requires one selected target profile",
        )]);
    };
    let vocabulary = exact_binding_vocabulary(typed)?;
    let selection_authority = package_inputs.cloned().map(|inputs| {
        Arc::new(inputs) as Arc<dyn psi_build_time_evaluation::BuildTimeSelectionAuthority>
    });
    let admission =
        BuildTimeAdmissionPlan::infer_with_selection_authority(typed, selection_authority);
    let mut rows = Vec::with_capacity(pending.len());
    let mut diagnostics = Vec::new();

    for (machine, conformance) in pending {
        match evaluate_one(typed, &admission, &vocabulary, target, machine, conformance) {
            Ok(row) => rows.push(row),
            Err(diagnostic) => diagnostics.push(diagnostic),
        }
    }
    rows.sort_by_key(|row| {
        (
            row.realization_machine.arena_index(),
            row.satisfied_owner.arena_index(),
            row.requirement.arena_index(),
            row.producer_entry_state.arena_index(),
        )
    });
    for pair in rows.windows(2) {
        if pair[0].realization_machine == pair[1].realization_machine
            && pair[0].satisfied_owner == pair[1].satisfied_owner
            && pair[0].requirement == pair[1].requirement
        {
            diagnostics.push(
                Diagnostic::error("ordinary external `via` has duplicate evaluated binding rows")
                    .with_source_span(pair[1].via_source_span),
            );
        }
    }
    if diagnostics.is_empty() {
        Ok(EvaluatedViaBindingTable {
            target: Some(target),
            rows,
        })
    } else {
        Err(diagnostics)
    }
}

fn evaluate_one(
    typed: &TypedTrees,
    admission: &BuildTimeAdmissionPlan,
    vocabulary: &BindingVocabulary,
    target: TargetProfile,
    machine: &psi_typed_trees::machine::Machine,
    conformance: &psi_typed_trees::machine::TraitConformance,
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
    let measured = admission
        .evaluate_machine_symbol_for_invocation_measured(
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
    let candidate =
        decode_binding_value(&value, widths).map_err(|message| at(source_span, message))?;
    let locator = normalize_foreign_locator(candidate, target).map_err(|error| {
        at(
            source_span,
            format!("ordinary external `via` returned an invalid locator: {error}"),
        )
    })?;
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
        locator.identity_digest().as_bytes(),
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
        locator.identity_digest(),
    )
    .map_err(|message| at(source_span, message))?;
    let evaluated = EvaluatedForeignImport::from_retained_evidence(locator, receipt)
        .map_err(|message| at(source_span, message))?;
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

fn exact_binding_vocabulary(typed: &TypedTrees) -> Result<BindingVocabulary, Vec<Diagnostic>> {
    let exact = |name: &str| {
        typed
            .data_definitions()
            .iter()
            .filter(|definition| {
                definition.name.as_str() == name
                    && definition.generic_instance.is_none()
                    && definition.is_public
                    && definition.supply_mode
                        == psi_language_semantics::DataSupplyMode::CheckedShape
                    && exact_external_binding_source(typed, definition.symbol).is_some()
            })
            .collect::<Vec<_>>()
    };
    let bindings = exact("Binding");
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
    let source_digest = omega_package_compilation::toolchain_source_identity_digest(source)?;
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
    let [DataMember::Variant(variant)] = typed.data_members(definition) else {
        return Err(vec![Diagnostic::error(
            "compiler-owned Binding must remain import-only",
        )]);
    };
    let [field] = typed.data_payload_fields(variant) else {
        return Err(vec![Diagnostic::error(
            "compiler-owned Binding::DllImport payload drifted",
        )]);
    };
    if variant.name.as_str() != "DllImport" || field.name.as_str() != "import" {
        return Err(vec![Diagnostic::error(
            "compiler-owned Binding::DllImport names drifted",
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
        return Err("ordinary external `via` producer must return the exact compiler-owned Binding<ObjectLength, SymbolLength, VersionLength>".to_owned());
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

fn decode_binding_value(
    value: &BuildTimeValue,
    widths: [u64; 3],
) -> Result<ForeignLocatorCandidate, String> {
    let BuildTimeValue::Case { variant, payload } = value else {
        return Err("ordinary external `via` must evaluate to Binding::DllImport".to_owned());
    };
    if variant != "DllImport" || payload.len() != 1 || payload[0].0 != "import" {
        return Err(
            "ordinary external `via` must evaluate to the exact Binding::DllImport payload"
                .to_owned(),
        );
    }
    let BuildTimeValue::Case { variant, payload } = &payload[0].1 else {
        return Err("Binding::DllImport must contain one DllImport case".to_owned());
    };
    match variant.as_str() {
        "PeByName" => {
            require_unused_width(widths[2], "PeByName VersionLength")?;
            let [library, export] = exact_fields(payload, ["library", "export"])?;
            Ok(ForeignLocatorCandidate::PeByName {
                library: exact_bytes(library, widths[0], "PeByName library")?,
                export: exact_bytes(export, widths[1], "PeByName export")?,
            })
        }
        "PeByOrdinal" => {
            require_unused_width(widths[1], "PeByOrdinal SymbolLength")?;
            require_unused_width(widths[2], "PeByOrdinal VersionLength")?;
            let [library, ordinal] = exact_fields(payload, ["library", "ordinal"])?;
            let BuildTimeValue::Int(ordinal) = ordinal else {
                return Err("PeByOrdinal ordinal must evaluate as u16-compatible Int".to_owned());
            };
            let ordinal = u16::try_from(*ordinal)
                .map_err(|_| "PeByOrdinal ordinal must fit nonzero u16".to_owned())?;
            if ordinal == 0 {
                return Err("PeByOrdinal ordinal must be nonzero".to_owned());
            }
            Ok(ForeignLocatorCandidate::PeByOrdinal {
                library: exact_bytes(library, widths[0], "PeByOrdinal library")?,
                ordinal,
            })
        }
        "ElfVersioned" => {
            let [object, symbol, version] = exact_fields(payload, ["object", "symbol", "version"])?;
            Ok(ForeignLocatorCandidate::ElfVersioned {
                object: exact_bytes(object, widths[0], "ElfVersioned object")?,
                symbol: exact_bytes(symbol, widths[1], "ElfVersioned symbol")?,
                version: exact_bytes(version, widths[2], "ElfVersioned version")?,
            })
        }
        "MachODylibSymbol" => {
            require_unused_width(widths[2], "MachODylibSymbol VersionLength")?;
            let [install_name, symbol] = exact_fields(payload, ["install_name", "symbol"])?;
            Ok(ForeignLocatorCandidate::MachODylibSymbol {
                install_name: exact_bytes(
                    install_name,
                    widths[0],
                    "MachODylibSymbol install_name",
                )?,
                symbol: exact_bytes(symbol, widths[1], "MachODylibSymbol symbol")?,
            })
        }
        _ => Err(format!("unknown compiler-owned DllImport case `{variant}`")),
    }
}

fn exact_fields<'a, const N: usize>(
    payload: &'a [(String, BuildTimeValue)],
    names: [&str; N],
) -> Result<[&'a BuildTimeValue; N], String> {
    if payload.len() != N
        || payload
            .iter()
            .zip(names)
            .any(|((actual, _), expected)| actual != expected)
    {
        return Err(
            "evaluated foreign locator payload fields drifted from the compiler-owned declaration"
                .to_owned(),
        );
    }
    Ok(std::array::from_fn(|index| &payload[index].1))
}

fn exact_bytes(value: &BuildTimeValue, width: u64, label: &str) -> Result<Vec<u8>, String> {
    let BuildTimeValue::Array(elements) = value else {
        return Err(format!("{label} must evaluate as a fixed byte array"));
    };
    let expected = usize::try_from(width)
        .map_err(|_| format!("{label} width does not fit the compiler host"))?;
    if elements.len() != expected {
        return Err(format!(
            "{label} evaluated length does not match its const width"
        ));
    }
    elements
        .iter()
        .map(|element| match element {
            BuildTimeValue::Int(byte) => {
                u8::try_from(*byte).map_err(|_| format!("{label} contains a value outside u8"))
            }
            _ => Err(format!("{label} contains a non-integer byte")),
        })
        .collect()
}

fn require_unused_width(width: u64, label: &str) -> Result<(), String> {
    (width == 0)
        .then_some(())
        .ok_or_else(|| format!("{label} must be zero for this locator case"))
}

fn retained_usage(usage: EvaluationUsage) -> Result<EvaluatedBindingUsage, String> {
    EvaluatedBindingUsage::from_evaluator(
        usage.schema().schema_version(),
        usage.schedule().marker(),
        usage.fuel_units(),
        usage.fuel_ceiling(),
        usage.build_log_bytes(),
        usage.filesystem_operation_attempts(),
        usage.peak_live_cells(),
        usage.peak_live_text_bytes(),
        usage.result_cells(),
        usage.result_text_bytes(),
    )
}

fn producer_closure_digest(
    typed: &TypedTrees,
    closure: &[SymbolHandle],
) -> Result<EvaluatedBindingProducerClosureDigest, String> {
    let mut entries = closure
        .iter()
        .map(|symbol| {
            let machine = typed
                .machines()
                .iter()
                .find(|machine| machine.symbol == *symbol)
                .ok_or_else(|| "evaluated binding closure contains a missing machine".to_owned())?;
            let identity = typed
                .normalized_machine_overload_identity(machine)
                .map(|identity| identity.identity().to_owned())
                .ok_or_else(|| {
                    "evaluated binding closure machine has no canonical callable identity"
                        .to_owned()
                })?;
            let span = typed
                .symbols
                .symbol_provenance_source_span(*symbol)
                .ok_or_else(|| {
                    format!(
                        "evaluated binding closure machine `{}` has no source custody",
                        machine.name
                    )
                })?;
            let source = typed.symbols.source_file(span).ok_or_else(|| {
                format!(
                    "evaluated binding closure machine `{}` has no source file",
                    machine.name
                )
            })?;
            let source_digest = stable_source_digest(source)?;
            Ok((
                identity,
                typed.symbols.symbol_package_identity(*symbol),
                source_digest,
            ))
        })
        .collect::<Result<Vec<_>, String>>()?;
    entries.sort_by(|left, right| left.0.cmp(&right.0).then_with(|| left.2.cmp(&right.2)));
    let mut hash = Sha256::new();
    hash.update(b"omega.evaluated-binding-producer-closure.sha256.v1\0");
    hash_u64(&mut hash, entries.len() as u64);
    for (identity, package, source) in entries {
        hash_field(&mut hash, identity.as_bytes());
        match package {
            Some(package) => {
                hash.update([1]);
                hash_field(&mut hash, &package.digest());
            }
            None => hash.update([0]),
        }
        hash_field(&mut hash, &source);
    }
    EvaluatedBindingProducerClosureDigest::from_bytes(hash.finalize().into())
}

fn stable_source_digest(source: &SourceFile) -> Result<[u8; 32], String> {
    if source.origin == SourceOrigin::Toolchain {
        return omega_package_compilation::toolchain_source_identity_digest(source).map_err(
            |diagnostics| {
                diagnostics
                    .into_iter()
                    .map(|diagnostic| diagnostic.to_string())
                    .collect::<Vec<_>>()
                    .join("; ")
            },
        );
    }
    let relative = match source.path.strip_prefix(&source.package_root) {
        Ok(relative) => relative,
        Err(_) if source.package_identity.is_none() => {
            Path::new(source.path.file_name().unwrap_or_default())
        }
        Err(_) => {
            return Err(format!(
                "source `{}` is outside its retained package root",
                source.path.display()
            ));
        }
    };
    let mut hash = Sha256::new();
    hash.update(b"omega.evaluated-binding-user-source.sha256.v1\0");
    match source.package_identity {
        Some(package) => {
            hash.update([1]);
            hash_field(&mut hash, &package.digest());
        }
        None => hash.update([0]),
    }
    hash_field(&mut hash, relative.to_string_lossy().as_bytes());
    hash_field(&mut hash, source.source.as_bytes());
    Ok(hash.finalize().into())
}

fn evaluation_digest(
    target: TargetProfile,
    closure: EvaluatedBindingProducerClosureDigest,
    usage: EvaluationUsage,
    value: &BuildTimeValue,
) -> EvaluatedBindingEvaluationDigest {
    let mut hash = Sha256::new();
    hash.update(b"omega.evaluated-binding-evaluation.sha256.v1\0");
    hash_field(&mut hash, target.identity().as_str().as_bytes());
    hash_field(&mut hash, &closure.as_bytes());
    hash.update(CURRENT_EVALUATION_SEMANTICS.marker().to_le_bytes());
    encode_usage(&mut hash, usage);
    encode_value(&mut hash, value);
    EvaluatedBindingEvaluationDigest::from_bytes(hash.finalize().into())
        .expect("domain-separated SHA-256 is nonzero")
}

fn materialization_digest(
    target: TargetProfile,
    vocabulary: [u8; 32],
    widths: [u64; 3],
    value: &BuildTimeValue,
    locator: [u8; 32],
) -> EvaluatedBindingMaterializationDigest {
    let mut hash = Sha256::new();
    hash.update(b"omega.evaluated-binding-materialization.sha256.v1\0");
    hash.update(MATERIALIZER_SCHEMA_VERSION.to_le_bytes());
    hash_field(&mut hash, target.identity().as_str().as_bytes());
    hash_field(&mut hash, &vocabulary);
    for width in widths {
        hash_u64(&mut hash, width);
    }
    encode_value(&mut hash, value);
    hash_field(&mut hash, &locator);
    EvaluatedBindingMaterializationDigest::from_bytes(hash.finalize().into())
        .expect("domain-separated SHA-256 is nonzero")
}

fn encode_usage(hash: &mut Sha256, usage: EvaluationUsage) {
    for value in [
        u64::from(usage.schema().schema_version()),
        u64::from(usage.schedule().marker()),
        usage.fuel_units(),
        usage.fuel_ceiling(),
        usage.build_log_bytes(),
        usage.filesystem_operation_attempts(),
        usage.peak_live_cells(),
        usage.peak_live_text_bytes(),
        usage.result_cells(),
        usage.result_text_bytes(),
    ] {
        hash_u64(hash, value);
    }
}

fn encode_value(hash: &mut Sha256, value: &BuildTimeValue) {
    match value {
        BuildTimeValue::Unit => hash.update([0]),
        BuildTimeValue::Int(value) => {
            hash.update([1]);
            hash.update(value.to_le_bytes());
        }
        BuildTimeValue::Bool(value) => hash.update([2, u8::from(*value)]),
        BuildTimeValue::Float(value) => {
            hash.update([3]);
            hash.update(value.to_bits().to_le_bytes());
        }
        BuildTimeValue::Text(bytes) => {
            hash.update([4]);
            hash_field(hash, bytes);
        }
        BuildTimeValue::Struct { type_name, fields } => {
            hash.update([5]);
            hash_field(hash, type_name.as_bytes());
            encode_fields(hash, fields);
        }
        BuildTimeValue::Case { variant, payload } => {
            hash.update([6]);
            hash_field(hash, variant.as_bytes());
            encode_fields(hash, payload);
        }
        BuildTimeValue::Array(elements) => {
            hash.update([7]);
            hash_u64(hash, elements.len() as u64);
            for element in elements {
                encode_value(hash, element);
            }
        }
    }
}

fn encode_fields(hash: &mut Sha256, fields: &[(String, BuildTimeValue)]) {
    hash_u64(hash, fields.len() as u64);
    for (name, value) in fields {
        hash_field(hash, name.as_bytes());
        encode_value(hash, value);
    }
}

fn hash_field(hash: &mut Sha256, bytes: &[u8]) {
    hash_u64(hash, bytes.len() as u64);
    hash.update(bytes);
}
fn hash_u64(hash: &mut Sha256, value: u64) {
    hash.update(value.to_le_bytes());
}

fn at(source_span: SourceSpan, message: impl Into<String>) -> Diagnostic {
    Diagnostic::error(message.into()).with_source_span(source_span)
}

#[cfg(test)]
mod tests {
    use super::decode_binding_value;
    use omega_target::ForeignLocatorCandidate;
    use psi_build_time_evaluation::BuildTimeValue;

    fn bytes(value: &[u8]) -> BuildTimeValue {
        BuildTimeValue::Array(
            value
                .iter()
                .map(|byte| BuildTimeValue::Int(i64::from(*byte)))
                .collect(),
        )
    }

    fn binding(inner: BuildTimeValue) -> BuildTimeValue {
        BuildTimeValue::Case {
            variant: "DllImport".to_owned(),
            payload: vec![("import".to_owned(), inner)],
        }
    }

    #[test]
    fn decodes_exact_atomic_pe_name() {
        let value = binding(BuildTimeValue::Case {
            variant: "PeByName".to_owned(),
            payload: vec![
                ("library".to_owned(), bytes(b"kernel32.dll")),
                ("export".to_owned(), bytes(b"ExitProcess")),
            ],
        });
        assert_eq!(
            decode_binding_value(&value, [12, 11, 0]).unwrap(),
            ForeignLocatorCandidate::PeByName {
                library: b"kernel32.dll".to_vec(),
                export: b"ExitProcess".to_vec()
            }
        );
    }

    #[test]
    fn rejects_width_and_field_drift() {
        let wrong_width = binding(BuildTimeValue::Case {
            variant: "PeByName".to_owned(),
            payload: vec![
                ("library".to_owned(), bytes(b"a")),
                ("export".to_owned(), bytes(b"b")),
            ],
        });
        assert!(decode_binding_value(&wrong_width, [2, 1, 0]).is_err());
        let wrong_field = binding(BuildTimeValue::Case {
            variant: "PeByOrdinal".to_owned(),
            payload: vec![
                ("ordinal".to_owned(), BuildTimeValue::Int(1)),
                ("library".to_owned(), bytes(b"a")),
            ],
        });
        assert!(decode_binding_value(&wrong_field, [1, 0, 0]).is_err());
    }
}
