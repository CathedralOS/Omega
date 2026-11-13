/*
Run every ordinary source `via` binding producer once and hand its result to
provider planning (`evaluate_via_bindings`, `evaluate_checked_via_bindings`).

A `via` leaf names a machine whose build-time result says which foreign import
or syscall realizes a requirement. Running that machine is source-side
build-time evaluation, so it happens here, where Build evaluation already
drives Psi's evaluator. Provider planning receives only what the run produced:
the value, its measured usage, the admitted producer closure, the Binding
coordinate widths and the evaluator's semantics marker. It decodes, digests and
retains those as an `EvaluatedViaBindingTable` without evaluating anything
itself.

Evaluation discloses all authored members under their own targets; a member
without an authored target runs under the selected profile, and its absence is
reported at the `via` expression.
*/

use build_time_evaluation::{
    BuildTimeAdmissionPlan, BuildTimeInvocationCustody, BuildTimeProgram,
    BuildTimeSelectionAuthority, CURRENT_EVALUATION_SEMANTICS, InvocationTarget,
};
use diagnostics::Diagnostic;
use package_compilation::PackageCompilationInputs;
use provider_planning::ProviderPlanningContext;
use provider_planning::evaluated_via_bindings::{
    BindingVocabulary, EvaluatedViaBindingRows, EvaluatedViaBindingTable, ViaBindingEvaluation,
    ViaProducer,
};
use std::sync::Arc;
use target::TargetProfile;

pub(crate) fn evaluate_via_bindings(
    context: ProviderPlanningContext<'_>,
    selected_target: Option<TargetProfile>,
    package_inputs: Option<&PackageCompilationInputs>,
) -> Result<EvaluatedViaBindingTable, Vec<Diagnostic>> {
    evaluate_program_via_bindings(
        BuildTimeProgram::Typed(context.typed),
        context.scope,
        selected_target,
        package_inputs,
    )
}

pub fn evaluate_checked_via_bindings(
    checked: &checked_trees::CheckedTrees,
    scope: source::DependencyScope,
    selected_target: Option<TargetProfile>,
    package_inputs: Option<&PackageCompilationInputs>,
) -> Result<EvaluatedViaBindingTable, Vec<Diagnostic>> {
    evaluate_program_via_bindings(
        BuildTimeProgram::Checked(checked),
        scope,
        selected_target,
        package_inputs,
    )
}

fn evaluate_program_via_bindings(
    program: BuildTimeProgram<'_>,
    scope: source::DependencyScope,
    selected_target: Option<TargetProfile>,
    package_inputs: Option<&PackageCompilationInputs>,
) -> Result<EvaluatedViaBindingTable, Vec<Diagnostic>> {
    let typed = match program {
        BuildTimeProgram::Typed(typed) => typed,
        BuildTimeProgram::Checked(checked) => &checked.typed,
    };
    let context = ProviderPlanningContext::new(typed, scope);
    let pending = typed
        .machines()
        .iter()
        .filter(|machine| context.contains(machine.symbol))
        .flat_map(|machine| {
            typed
                .machine_trait_conformances(machine)
                .iter()
                .filter(|conformance| conformance.via_expression.is_valid())
                .map(move |conformance| (machine, conformance))
        })
        .collect::<Vec<_>>();
    if pending.is_empty() {
        return Ok(EvaluatedViaBindingTable::without_bindings(
            context.scope,
            selected_target,
        ));
    }
    let vocabulary = build_time_evaluation::native_locators::vocabulary(typed, |symbol| {
        context.contains(symbol)
    })?;
    let vocabulary = BindingVocabulary::retain(typed, vocabulary.binding)?;
    let selection_authority = package_inputs
        .cloned()
        .map(|inputs| Arc::new(inputs) as Arc<dyn BuildTimeSelectionAuthority>);
    let admission = BuildTimeAdmissionPlan::infer(typed, selection_authority)?;
    let mut rows = EvaluatedViaBindingRows::with_capacity(pending.len());
    let mut diagnostics = Vec::new();

    for (machine, conformance) in pending {
        let target = match typed.authored_machine_target(machine.symbol) {
            Ok(Some(authored)) => TargetProfile::from_canonical_target_name(authored),
            Ok(None) => selected_target.ok_or_else(|| {
                Diagnostic::error(
                    "ordinary external `via` evaluation requires one selected target profile",
                )
            }),
            Err(reason) => Err(Diagnostic::error(reason)),
        };
        let target = match target {
            Ok(target) => target,
            Err(diagnostic) => {
                diagnostics.push(
                    diagnostic.with_source_span(
                        typed
                            .expression_table
                            .source_span(conformance.via_expression),
                    ),
                );
                continue;
            }
        };
        if let Err(diagnostic) = evaluate_one(
            program,
            &admission,
            &vocabulary,
            target,
            machine,
            conformance,
            &mut rows,
        ) {
            diagnostics.push(diagnostic);
        }
    }
    rows.finish(context.scope, selected_target, diagnostics)
}

// Target-scoped producers resolve to their family head and run the member body
// authored for `target` under `ConstEvaluable`, with the `via` span retained
// as invocation custody.
fn evaluate_one(
    program: BuildTimeProgram<'_>,
    admission: &BuildTimeAdmissionPlan,
    vocabulary: &BindingVocabulary,
    target: TargetProfile,
    machine: &typed_trees::machine::Machine,
    conformance: &typed_trees::machine::TraitConformance,
    rows: &mut EvaluatedViaBindingRows,
) -> Result<(), Diagnostic> {
    let typed = match program {
        BuildTimeProgram::Typed(typed) => typed,
        BuildTimeProgram::Checked(checked) => &checked.typed,
    };
    let producer = ViaProducer::resolve(typed, target, conformance)?;
    let source_span = producer.via_source_span();
    let at = |message: String| Diagnostic::error(message).with_source_span(source_span);
    let widths = build_time_evaluation::native_locators::binding_widths(
        typed,
        vocabulary.binding(),
        producer.return_type(),
    )
    .map_err(at)?;
    let custody = BuildTimeInvocationCustody::Source(source_span);
    // The producer may retain scalar conditions whose values depend on this
    // target; the invocation discharges them from these target facts.
    let layout_target = target.layout_target_facts();
    let address_model = target::TargetSemantics::close_address_model(target).map(|model| {
        terminal_installation::ScalarAddressModel {
            target: target.target_name().to_owned(),
            carrier: model.carrier(),
            semantics_version: model.semantics_version(),
            commitment: model.commitment(),
        }
    });
    let measured = admission
        .evaluate_const_evaluable_program_machine_symbol_for_invocation_measured(
            program,
            producer.machine(),
            Vec::new(),
            custody,
            Some(InvocationTarget {
                name: target.target_name(),
                layout_target: layout_target.as_ref(),
                carrier_layout: target
                    .native_realization()
                    .map(runtime_abi::target_carrier_layout),
                address_model: address_model.as_ref(),
            }),
        )
        .map_err(|message| {
            at(format!(
                "ordinary external `via` evaluation failed: {message}"
            ))
        })?;
    let closure = admission
        .admitted_program_machine_closure_symbols(program, producer.machine(), custody)
        .map_err(|message| {
            at(format!(
                "ordinary external `via` closure admission failed: {message}"
            ))
        })?;
    let (value, usage) = measured.into_parts();
    rows.retain(
        typed,
        vocabulary,
        target,
        machine.symbol,
        producer,
        ViaBindingEvaluation {
            value: &value,
            usage,
            closure: &closure,
            widths,
            evaluation_semantics_marker: CURRENT_EVALUATION_SEMANTICS.marker(),
        },
    )
}
