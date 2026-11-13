// Retention of a single evaluated `via` binding. `ViaProducer::resolve` finds
// the one machine a `via` leaf names so the caller knows what to run:
// target-scoped producers resolve to their family head and then to the member
// body authored for the selected target. `retain_one` takes what that run
// produced, validates the value's shape and widths, and seals the receipt.
// Nothing here evaluates; the value arrives already computed.

use crate::evaluated_via_bindings::binding_digests::{
    at, evaluation_digest, materialization_digest, producer_closure_digest,
};
use crate::evaluated_via_bindings::{
    BindingVocabulary, DecodedBindingValue, EvaluatedNativeBinding, EvaluatedViaBindingRow,
    MATERIALIZER_SCHEMA_VERSION, ViaBindingEvaluation, ViaProducer, decode_binding_value,
    retained_usage,
};
use crate::provider_planning::entry_state;
use diagnostics::Diagnostic;
use effects::provider_plan::{
    EvaluatedBindingReceipt, EvaluatedForeignImport, EvaluatedForeignSyscall,
};
use symbols::SymbolHandle;
use target::{TargetProfile, normalize_foreign_locator};
use typed_trees::TypedTrees;

impl<'program> ViaProducer<'program> {
    pub fn resolve(
        typed: &'program TypedTrees,
        target: TargetProfile,
        conformance: &typed_trees::machine::TraitConformance,
    ) -> Result<Self, Diagnostic> {
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
        if entry_state(typed, producer).is_none_or(|first| first.symbol != entry.symbol) {
            return Err(at(
                source_span,
                "ordinary external `via` target is not the producer entry state",
            ));
        }
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
        let entry = entry_state(typed, producer).ok_or_else(|| {
            at(
                source_span,
                "ordinary external `via` producer body has no entry state",
            )
        })?;
        Ok(Self {
            producer,
            entry,
            satisfied_owner: conformance.symbol,
            requirement: conformance.requirement_symbol,
            via_expression: expression,
            via_source_span: source_span,
        })
    }
}

pub(super) fn retain_one(
    typed: &TypedTrees,
    vocabulary: &BindingVocabulary,
    target: TargetProfile,
    realization_machine: SymbolHandle,
    producer: ViaProducer<'_>,
    evaluation: ViaBindingEvaluation<'_>,
    identities: &mut terminal_psi::TerminalIdentityTableBuilder,
) -> Result<EvaluatedViaBindingRow, Diagnostic> {
    let ViaProducer {
        producer,
        entry,
        satisfied_owner,
        requirement,
        via_expression,
        via_source_span: source_span,
    } = producer;
    let ViaBindingEvaluation {
        value,
        usage,
        closure,
        widths,
        evaluation_semantics_marker,
    } = evaluation;
    let decoded =
        decode_binding_value(value, widths).map_err(|message| at(source_span, message))?;
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
        producer_closure_digest(typed, closure).map_err(|message| at(source_span, message))?;
    let evaluation_digest = evaluation_digest(
        target,
        closure_digest,
        evaluation_semantics_marker,
        usage,
        value,
    );
    let materialization_digest = materialization_digest(
        target,
        vocabulary.source_digest,
        widths,
        value,
        binding_identity_digest.as_bytes(),
    );
    let retained_usage = retained_usage(usage).map_err(|message| at(source_span, message))?;
    let receipt = EvaluatedBindingReceipt::from_evaluation(
        typed.symbols.symbol_package_identity(producer.symbol),
        &producer_identity,
        identities,
        closure_digest,
        evaluation_semantics_marker,
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
        realization_machine,
        satisfied_owner,
        requirement,
        via_expression,
        producer_machine: producer.symbol,
        producer_entry_state: entry.symbol,
        via_source_span: source_span,
        evaluated,
    })
}

impl BindingVocabulary {
    /// Retains the native `Binding` vocabulary the caller resolved, together
    /// with the digest of the toolchain source that declares it.
    pub fn retain(typed: &TypedTrees, binding: SymbolHandle) -> Result<Self, Vec<Diagnostic>> {
        let source = typed
            .symbols
            .symbol_source_span(binding)
            .and_then(|span| typed.symbols.source_file(span))
            .ok_or_else(|| {
                vec![Diagnostic::error(
                    "native vocabulary lost its checked source",
                )]
            })?;
        let source_digest = package_compilation::toolchain_source_identity_digest(source)?;
        Ok(Self {
            binding,
            source_digest,
        })
    }

    pub const fn binding(&self) -> SymbolHandle {
        self.binding
    }
}
