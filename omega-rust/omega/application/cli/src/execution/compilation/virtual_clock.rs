//! Project selected physical waits into the differential interpreter's virtual clock.
//!
//! Selection, evaluated foreign identity and target meaning belong to Omega. Psi
//! receives only an exact checked requirement and its operand contract. A source
//! spelling cannot select a clock operation, and an unselected import cannot
//! replace the checked adapter that the program actually selected. No locator is
//! loaded and no live host effect is granted by these virtual bindings.

use calling_conventions::{CallingPolicy, ExternalBindingKind, ExternalBindingRow};
use checked_interpreter::{
    VirtualClockBinding, VirtualClockOperand, VirtualClockOperation, VirtualClockResult,
};
use checked_trees::CheckedTrees;
use diagnostics::Diagnostic;
use effects::SelectedProviderPlanFacts;
use effects::provider_plan::{
    EvaluatedForeignImport, ProviderBinding, ProviderPlan, ProviderPlanRow,
};
use provider_planning::evaluated_via_bindings::EvaluatedViaBindingTable;
use provider_planning::{
    ProviderPlanningContext, ProviderSchemaDeclaration, SelectedProviderReviewProvenance,
};
use symbols::SymbolHandle;
use target::{ForeignLocatorCandidate, NormalizedForeignLocator, TargetProfile};
use typed_trees::signature::StateParameter;
use typed_trees::types::PrimitiveType;

pub(super) fn bindings(
    checked: &compiler::CheckedCompilation,
) -> Result<Vec<VirtualClockBinding>, Vec<Diagnostic>> {
    let execution = checked.execution();
    selected_bindings(
        checked,
        execution.selected_target_profile,
        &execution.selected_provider_plan_facts,
        &execution.selected_provider_provenance,
        &checked.providers().evaluated_via_bindings,
        &checked.providers().external_binding_rows,
    )
}

fn selected_bindings(
    checked: &CheckedTrees,
    target: Option<TargetProfile>,
    selected: &SelectedProviderPlanFacts,
    provenance: &[SelectedProviderReviewProvenance],
    evaluated_bindings: &EvaluatedViaBindingTable,
    external_rows: &[ExternalBindingRow],
) -> Result<Vec<VirtualClockBinding>, Vec<Diagnostic>> {
    let failure = |message: &str| vec![Diagnostic::error(message)];
    if selected.plans().len() != provenance.len() {
        return Err(failure(
            "virtual clock selection has incomplete provider provenance",
        ));
    }
    let context = ProviderPlanningContext::new(checked, source::DependencyScope::Product);
    let mut bindings = Vec::new();
    let mut requirements = Vec::new();
    let mut replayed_evaluation = false;
    // Provider settlement sorts both populations together; selected-dispatch's
    // intrinsic review enforces this same row-aligned equality before sealing.
    for (plan, retained) in selected.plans().iter().zip(provenance) {
        if plan != &retained.plan
            || retained.provider.row_requirements.len() != plan.rows.len()
            || retained.provider.row_realizations.len() != plan.rows.len()
        {
            return Err(failure(
                "virtual clock selection has misaligned provider provenance",
            ));
        }
        for ((row, requirement), realization) in plan
            .rows
            .iter()
            .zip(&retained.provider.row_requirements)
            .zip(&retained.provider.row_realizations)
        {
            let ProviderBinding::Import { evaluated } = &row.binding else {
                // In particular, execute checked adapters through their normal dispatch.
                continue;
            };
            let Some(wait) = PhysicalClockOperation::from_locator(evaluated.locator()) else {
                continue;
            };
            if target != Some(evaluated.locator().target())
                || evaluated_bindings.target() != target
                || target.map(TargetProfile::target_name) != Some(plan.target.as_str())
            {
                return Err(failure(
                    "virtual clock import belongs to another selected target",
                ));
            }
            if !replayed_evaluation {
                evaluated_bindings.validate_against_typed(context)?;
                replayed_evaluation = true;
            }
            let parameters = exact_requirement_parameters(
                checked,
                context,
                retained,
                *requirement,
                &row.requirement_identity,
            )
            .ok_or_else(|| failure("virtual clock import lost its exact checked requirement"))?;
            if !wait.matches_parameters(checked, parameters) {
                return Err(failure(
                    "virtual clock import has an unsupported scalar signature",
                ));
            }
            validate_physical_calling_plan(checked, plan, row, evaluated, external_rows)
                .map_err(|message| failure(&message))?;
            let machine = checked
                .machine_by_symbol(*realization)
                .filter(|machine| !machine.body_is_present && context.contains(machine.symbol))
                .ok_or_else(|| failure("virtual clock import lost its bodyless realization"))?;
            let [entry] = checked.machine_states(machine) else {
                return Err(failure(
                    "virtual clock import has no unique realization entry",
                ));
            };
            if !wait.matches_parameters(checked, checked.state_parameters(entry))
                || !wait
                    .operation()
                    .result()
                    .matches_type(checked, entry.return_type)
            {
                return Err(failure(
                    "virtual clock realization disagrees with its physical signature",
                ));
            }
            if retained.provider.provider_type
                != machine
                    .attached_data_symbol
                    .is_valid()
                    .then_some(machine.attached_data_symbol)
                || checked.symbols.symbol_package_identity(machine.symbol)
                    != plan.origin_package_identity
                || checked
                    .symbols
                    .symbol_package_identity(machine.attached_data_symbol)
                    != plan.provider_type_package_identity
            {
                return Err(failure(
                    "virtual clock import has substituted provider ownership",
                ));
            }
            let conformances = checked
                .machine_trait_conformances(machine)
                .iter()
                .filter(|clause| {
                    clause.symbol == retained.provider.schema.symbol()
                        && clause.requirement_symbol == *requirement
                        && clause.via_expression.is_valid()
                        && clause.external_binding.is_none()
                })
                .collect::<Vec<_>>();
            let [conformance] = conformances.as_slice() else {
                return Err(failure(
                    "virtual clock import has no unique selected satisfaction edge",
                ));
            };
            let evaluated_row = evaluated_bindings
                .exact(
                    machine.symbol,
                    conformance.symbol,
                    conformance.requirement_symbol,
                )
                .ok_or_else(|| {
                    failure("virtual clock import lost its evaluated binding receipt")
                })?;
            if evaluated_row.via_expression() != conformance.via_expression
                || evaluated_row.evaluated().as_import() != Some(evaluated)
                || plan
                    .schema
                    .methods
                    .iter()
                    .filter(|method| method.requirement_identity == row.requirement_identity)
                    .count()
                    != 1
            {
                return Err(failure(
                    "virtual clock import disagrees with its evaluated selection",
                ));
            }
            if requirements.contains(requirement) {
                return Err(failure(
                    "virtual clock requirement has multiple selected wait bindings",
                ));
            }
            let binding = VirtualClockBinding::from_compiler_resolved_requirement(
                checked,
                *requirement,
                wait.operands(),
                wait.operation(),
            )
            .and_then(|binding| binding.with_compiler_resolved_realization(checked, machine.symbol))
            .map_err(failure)?;
            requirements.push(*requirement);
            bindings.push(binding);
        }
    }
    Ok(bindings)
}

fn validate_physical_calling_plan(
    checked: &CheckedTrees,
    plan: &ProviderPlan,
    row: &ProviderPlanRow,
    evaluated: &EvaluatedForeignImport,
    external_rows: &[ExternalBindingRow],
) -> Result<(), String> {
    let profile = evaluated.locator().target();
    let expected =
        provider_planning::calling_policy_plans::evaluate_compatibility_boundary_entry_plan(
            checked,
            profile.native_target(),
            &plan.schema.trait_name,
            plan.schema.trait_package_identity,
            &row.method,
            &row.requirement_identity,
            CallingPolicy::native_for_target(profile.native_target()),
            0,
            &[],
        )?
        .ok_or("virtual clock import has no physical scalar calling plan")?;
    let candidates = external_rows.iter().filter(|external| {
        external.target_name == profile.target_name()
            && external.trait_name == plan.schema.trait_name
            && external.method == row.method
            && external.requirement_identity == row.requirement_identity
            && external.table_type == plan.provider_type
            && matches!(&external.binding, ExternalBindingKind::Import { locator } if locator == evaluated.locator())
    }).collect::<Vec<_>>();
    let [external] = candidates.as_slice() else {
        return Err("virtual clock import has no unique retained physical calling plan".into());
    };
    if external.boundary_entry_plan.as_ref() != Some(&expected) {
        return Err(
            "virtual clock import does not use its reviewed physical calling convention".into(),
        );
    }
    Ok(())
}

fn exact_requirement_parameters<'program>(
    checked: &'program CheckedTrees,
    context: ProviderPlanningContext<'program>,
    retained: &SelectedProviderReviewProvenance,
    requirement: SymbolHandle,
    requirement_identity: &str,
) -> Option<&'program [StateParameter]> {
    match retained.provider.schema {
        ProviderSchemaDeclaration::BoundaryRequirement(symbol) if symbol == requirement => {
            let machine = checked.machine_by_symbol(symbol)?;
            let schema = provider_planning::service_schema::from_typed_boundary_requirement(
                context, machine,
            )?;
            if schema != retained.plan.schema
                || checked
                    .normalized_machine_overload_identity(machine)?
                    .identity()
                    != requirement_identity
            {
                return None;
            }
            let [state] = checked.machine_states(machine) else {
                return None;
            };
            Some(checked.state_parameters(state))
        }
        ProviderSchemaDeclaration::BoundaryTrait(symbol) => {
            let definition = checked
                .traits()
                .iter()
                .find(|definition| definition.symbol == symbol)?;
            if provider_planning::service_schema::from_typed(context, definition).as_ref()
                != Some(&retained.plan.schema)
            {
                return None;
            }
            let signature = checked
                .trait_machine_signatures(definition)
                .iter()
                .find(|signature| signature.symbol == requirement)?;
            if checked
                .normalized_trait_requirement_overload_identity(definition, signature)
                .identity()
                != requirement_identity
            {
                return None;
            }
            Some(checked.state_signature_parameters(signature))
        }
        _ => None,
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum PhysicalClockOperation {
    DarwinPoll,
    DarwinMonotonicRead,
    WindowsSleep,
}

impl PhysicalClockOperation {
    fn from_locator(locator: &NormalizedForeignLocator) -> Option<Self> {
        match (locator.target(), locator.locator()) {
            (
                TargetProfile::MacosArm64,
                ForeignLocatorCandidate::MachODylibSymbol {
                    install_name,
                    symbol,
                },
            ) if install_name == b"/usr/lib/libSystem.B.dylib" && symbol == b"_poll" => {
                Some(Self::DarwinPoll)
            }
            (
                TargetProfile::MacosArm64,
                ForeignLocatorCandidate::MachODylibSymbol {
                    install_name,
                    symbol,
                },
            ) if install_name == b"/usr/lib/libSystem.B.dylib"
                && symbol == b"_clock_gettime_nsec_np" =>
            {
                Some(Self::DarwinMonotonicRead)
            }
            (TargetProfile::WindowsX64, ForeignLocatorCandidate::PeByName { library, export })
                if library == b"kernel32.dll" && export == b"Sleep" =>
            {
                Some(Self::WindowsSleep)
            }
            _ => None,
        }
    }

    fn matches_parameters(self, checked: &CheckedTrees, parameters: &[StateParameter]) -> bool {
        let expected: &[PrimitiveType] = match self {
            Self::DarwinPoll => &[PrimitiveType::U64, PrimitiveType::U64, PrimitiveType::I32],
            Self::WindowsSleep | Self::DarwinMonotonicRead => &[PrimitiveType::U32],
        };
        parameters.len() == expected.len()
            && parameters
                .iter()
                .zip(expected)
                .all(|(parameter, expected)| {
                    !parameter.is_self
                        && checked.primitive_type_reference(parameter.type_reference)
                            == Some(*expected)
                })
    }

    fn operation(self) -> VirtualClockOperation {
        match self {
            Self::DarwinPoll => VirtualClockOperation::Wait {
                maximum_milliseconds: i64::from(i32::MAX),
                result: VirtualClockResult::Integer(PrimitiveType::I32),
            },
            Self::WindowsSleep => VirtualClockOperation::Wait {
                // DWORD::MAX is INFINITE, not a finite duration to virtualize.
                maximum_milliseconds: i64::from(u32::MAX) - 1,
                result: VirtualClockResult::Unit,
            },
            Self::DarwinMonotonicRead => VirtualClockOperation::Read {
                primitive: PrimitiveType::U64,
                units_per_millisecond: 1_000_000,
            },
        }
    }

    fn operands(self) -> &'static [VirtualClockOperand] {
        match self {
            // With nfds = 0, poll does not dereference the descriptor pointer.
            Self::DarwinPoll => &[
                VirtualClockOperand::Ignored,
                VirtualClockOperand::ExactInteger(0),
                VirtualClockOperand::Milliseconds,
            ],
            Self::WindowsSleep => &[VirtualClockOperand::Milliseconds],
            Self::DarwinMonotonicRead => &[VirtualClockOperand::ExactInteger(6)],
        }
    }
}

#[cfg(test)]
mod tests;

#[cfg(test)]
mod read_tests;
