//! Preserve the public application selected by static evidence dispatch.
//! Report strings cannot reconstruct a named map's simultaneous Type/Const
//! telescope. Prepare the same retained source frames used by requirement
//! applications; checking still owns operand and selected-row admission.

use crate::TypedTrees;
use crate::name::Identifier;
use crate::trait_definition::{AppliedTraitScope, TraitApplicationArguments};

impl TypedTrees {
    pub fn prepare_closed_conformance_scopes(&mut self) -> Result<(), &'static str> {
        let pending = self
            .machine_specializations
            .iter()
            .enumerate()
            .flat_map(|(specialization_position, specialization)| {
                specialization
                    .conformance_applications
                    .iter()
                    .enumerate()
                    .map(move |(application_position, application)| {
                        (
                            specialization_position,
                            application_position,
                            specialization.binding_application,
                            application.clone(),
                        )
                    })
            })
            .collect::<Vec<_>>();
        for (specialization_position, application_position, binding_application, application) in
            pending
        {
            if !binding_application.is_valid() {
                return Err(
                    "closed implementation application lost its supplying source application",
                );
            }
            // Specialization owns the executable body, not its caller's regions.
            // The retained binding application preserves the supplying telescope.
            let owner = self
                .expression_table
                .static_machine_application(binding_application)
                .lexical_owner;
            let arguments = application
                .arguments
                .iter()
                .map(|argument| self.static_operand_type_reference(argument))
                .collect::<Result<Vec<_>, _>>()?;
            let lifetimes = application
                .lifetime_arguments
                .iter()
                .map(Identifier::generated)
                .collect::<Vec<_>>();
            let declaration = self
                .conformance_by_symbol(application.declaration)
                .ok_or("closed implementation application lost its declaration")?;
            let scope = AppliedTraitScope::from_conformance_application(
                self,
                declaration,
                TraitApplicationArguments {
                    lexical_owner: owner,
                    enclosing_data_applications: &[],
                    arguments: &arguments,
                    lifetimes: &lifetimes,
                },
            )
            .ok_or("closed implementation application lost its complete source telescope")?;
            let scope = scope.environment().retain(self)?;
            self.machine_specializations[specialization_position].conformance_applications
                [application_position]
                .trait_scope = scope;
        }
        Ok(())
    }
}
