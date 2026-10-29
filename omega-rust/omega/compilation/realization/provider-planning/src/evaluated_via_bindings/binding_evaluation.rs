//! Evaluating one via binding and validating its shape and widths.

use crate::evaluated_via_bindings::binding_digests::{
    at, evaluation_digest, materialization_digest, producer_closure_digest,
};
use crate::evaluated_via_bindings::binding_values::{
    DecodedBindingValue, decode_binding_value, retained_usage,
};
use crate::evaluated_via_bindings::{
    BindingVocabulary, EvaluatedNativeBinding, EvaluatedViaBindingRow, MATERIALIZER_SCHEMA_VERSION,
};
use build_time_evaluation::{
    BuildTimeAdmissionPlan, BuildTimeInvocationCustody, BuildTimeProgram,
    CURRENT_EVALUATION_SEMANTICS,
};
use checked_trees::typed_trees::TypedTrees;
use diagnostics::Diagnostic;
use effects::provider_plan::{
    EvaluatedBindingReceipt, EvaluatedForeignImport, EvaluatedForeignSyscall,
};
use symbols::SymbolHandle;
use target::{TargetProfile, normalize_foreign_locator};

use checked_trees::typed_trees::types::TypeReferenceHandle;

pub(crate) fn evaluate_one(
    program: BuildTimeProgram<'_>,
    admission: &BuildTimeAdmissionPlan,
    vocabulary: &BindingVocabulary,
    target: TargetProfile,
    machine: &checked_trees::typed_trees::machine::Machine,
    conformance: &checked_trees::typed_trees::machine::TraitConformance,
    identities: &mut terminal_psi::TerminalIdentityTableBuilder,
) -> Result<EvaluatedViaBindingRow, Diagnostic> {
    let typed = match program {
        BuildTimeProgram::Typed(typed) => typed,
        BuildTimeProgram::Checked(checked) => &checked.typed,
    };
    let expression = conformance.via_expression;
    let source_span = typed.expression_table.source_span(expression);
    if conformance.external_binding.is_some() {
        return Err(at(
            source_span,
            "external realization cannot combine legacy and ordinary `via` bindings",
        ));
    }
    let Some(call) = typed.expression_table.call_node(expression) else {
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
                .state_in_machine(producer, call.target_symbol)
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
    // A target-scoped producer resolves to its family head, which has no
    // body: the evaluated binding is the member body authored for the
    // selected target, as every other realization of a family resolves it.
    let producer = if producer.target_family_interface {
        typed
            .target_family_member(producer.symbol, target.target_name())
            .ok_or_else(|| {
                at(
                    source_span,
                    format!(
                        "ordinary external `via` producer `{}` has no body for target `{}`",
                        producer.name,
                        target.target_name()
                    ),
                )
            })?
    } else {
        *producer
    };
    let entry = typed.machine_states(producer).first().ok_or_else(|| {
        at(
            source_span,
            "ordinary external `via` producer body has no entry state",
        )
    })?;
    let widths = binding_widths(typed, vocabulary.binding, entry.return_type)
        .map_err(|message| at(source_span, message))?;
    let custody = BuildTimeInvocationCustody::Source(source_span);
    // The evaluated binding crosses the boundary as an ordinary closed const
    // value: the producer result is admitted under ConstEvaluable, with the
    // authored `via` span retained as its invocation custody.
    let measured = admission
        .evaluate_const_evaluable_program_machine_symbol_for_invocation_measured(
            program,
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
        .admitted_program_machine_closure_symbols(program, producer.symbol, custody)
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
        .machine_callable_identity_text(producer)
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
        &producer_identity,
        identities,
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
            EvaluatedNativeBinding::Import(
                EvaluatedForeignImport::from_retained_evidence(locator, receipt)
                    .map_err(|message| at(source_span, message))?,
            )
        }
        DecodedBindingValue::Syscall { number } => EvaluatedNativeBinding::Syscall(
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
    context: crate::ProviderPlanningContext<'_>,
) -> Result<BindingVocabulary, Vec<Diagnostic>> {
    let vocabulary = build_time_evaluation::native_locators::vocabulary(context.typed, |symbol| {
        context.contains(symbol)
    })?;
    let source = context
        .typed
        .symbols
        .symbol_source_span(vocabulary.binding)
        .and_then(|span| context.typed.symbols.source_file(span))
        .ok_or_else(|| {
            vec![Diagnostic::error(
                "native vocabulary lost its checked source",
            )]
        })?;
    let source_digest = package_compilation::toolchain_source_identity_digest(source)?;
    Ok(BindingVocabulary {
        binding: vocabulary.binding,
        source_digest,
    })
}

fn binding_widths(
    typed: &TypedTrees,
    binding_symbol: SymbolHandle,
    return_type: TypeReferenceHandle,
) -> Result<[u64; 3], String> {
    build_time_evaluation::native_locators::binding_widths(typed, binding_symbol, return_type)
}
