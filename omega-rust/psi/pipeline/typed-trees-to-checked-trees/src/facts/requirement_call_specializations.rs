//! Checked requirement-call specialization facts.
//!
//! MP2b admits each static machine argument at a generic call edge and, in
//! doing so, derives the callee's `Type` substitutions from the selected
//! callable's shape. A nominal contract replays that judgment into a
//! `CheckedNominalMachineUse` row; a structural contract emits no row at
//! all, so the specialization was dropped at validation. These rows carry
//! it across the checked boundary so Unit call construction can substitute
//! the requirement's formals and bind the selected provider.

use validation::{ValidatedNominalMachineUseSite, ValidatedRequirementCallSpecialization};

pub(crate) fn build_requirement_call_specialization_facts(
    specializations: Vec<ValidatedRequirementCallSpecialization>,
) -> Result<checked_trees::RequirementCallSpecializationFacts, Vec<diagnostics::Diagnostic>> {
    let checked = specializations
        .into_iter()
        .map(
            |specialization| checked_trees::CheckedRequirementCallSpecialization {
                site: match specialization.site {
                    ValidatedNominalMachineUseSite::Statement(handle) => {
                        checked_trees::NominalMachineUseSite::Statement(handle)
                    }
                    ValidatedNominalMachineUseSite::Expression(handle) => {
                        checked_trees::NominalMachineUseSite::Expression(handle)
                    }
                },
                registration_operation: specialization.registration_operation,
                type_bindings: specialization
                    .type_bindings
                    .iter()
                    .map(|binding| checked_trees::CheckedRequirementCallTypeBinding {
                        parameter: binding.parameter,
                        actual: binding.actual,
                    })
                    .collect(),
                machine_selections: specialization
                    .machine_selections
                    .iter()
                    .map(
                        |selection| checked_trees::CheckedRequirementCallMachineSelection {
                            static_machine_ordinal: selection.static_machine_ordinal,
                            parameter: selection.parameter,
                            selected_machine: selection.selected_machine,
                            selected: selection.selected,
                        },
                    )
                    .collect(),
            },
        )
        .collect::<Vec<_>>();
    checked_trees::RequirementCallSpecializationFacts::try_with_specializations(checked)
        .map_err(|message| vec![diagnostics::Diagnostic::error(message)])
}
