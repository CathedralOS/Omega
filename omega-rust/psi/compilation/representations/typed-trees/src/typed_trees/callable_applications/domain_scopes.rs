//! Apply a domain's explicit index telescope with the shared signature copier.
//! The source declaration and its constraints remain immutable; this view owns
//! substituted facts and cleanup operands, never new establishment authority.

use super::CallableApplication;
use crate::{TypedTrees, domain, types};
use symbols::SymbolHandle;

impl TypedTrees {
    pub fn prepare_closed_domain_application(
        &mut self,
        declaration: SymbolHandle,
        arguments: &[types::TypeReferenceHandle],
        closed_arguments: &[types::TypeReferenceHandle],
        identity: language_semantics::SemanticDomainId,
    ) -> Result<bool, &'static str> {
        if self.applied_domain_applications.iter().any(|application| {
            application.declaration == declaration && application.theory.semantic_id == identity
        }) {
            return Ok(false);
        }
        let mut theory = self
            .domain_definition(declaration)
            .ok_or("applied domain lost its declaration")?
            .clone();
        // An implicit carrier needs a separate supplying type coordinate.
        // This operation accepts only a complete explicit telescope.
        if domain::has_generic_carrier(self, &theory) {
            return Err("applied domain requires its exact generic carrier coordinate");
        }
        let parameters = self.domain_type_parameters(&theory).to_vec();
        if parameters.len() != arguments.len() || parameters.len() != closed_arguments.len() {
            return Err("applied domain has an incomplete explicit index telescope");
        }
        let substitutions = parameters
            .iter()
            .zip(closed_arguments)
            .map(|(parameter, argument)| (parameter.symbol, *argument))
            .collect::<Vec<_>>();
        let source_subject = theory.subject_symbol;
        let subject = if source_subject.is_valid() {
            self.symbols.insert_generated_root_from(
                source_subject,
                self.symbols.get(source_subject).kind,
                theory.subject_name.as_str(),
            )
        } else {
            source_subject
        };
        let mut application = CallableApplication {
            region_scope: declaration,
            trait_region_application: None,
            lifetime_declaration: declaration,
            lifetimes: Vec::new(),
            elided_lifetimes: Vec::new(),
            types: substitutions,
            // Type/Const substitutions belong to the value/type map. The
            // namespace map is reserved for selected evidence and code.
            static_arguments: Vec::new(),
            symbols: source_subject
                .is_valid()
                .then_some((source_subject, subject))
                .into_iter()
                .collect(),
            active_requirements: Vec::new(),
            source_binders: parameters
                .iter()
                .map(|parameter| (parameter.name.clone(), parameter.symbol))
                .collect(),
        };
        theory.target_type = application.type_reference(self, theory.target_type)?;
        theory.subject_symbol = subject;
        theory.facts = application.facts(self, theory.facts)?;
        theory.type_parameters = Default::default();
        theory.index_arguments.clear();
        theory.semantic_id = identity;
        theory.semantic_roles.denotation_dimension =
            theory.semantic_roles.denotation_dimension.map(|_| identity);
        theory.semantic_roles.arithmetic_policy =
            theory.semantic_roles.arithmetic_policy.map(|_| identity);
        if let Some(selection) = &mut theory.cleanup_selection {
            let original = self
                .expression_table
                .static_machine_application(selection.application)
                .clone();
            let mut arguments = self
                .expression_table
                .static_machine_application_arguments(selection.application)
                .to_vec();
            application.static_arguments(self, &mut arguments)?;
            let selected = self.expression_table.retain_static_machine_application(
                original.registration_operation,
                original.target,
                &arguments,
            );
            let lifetimes = self
                .expression_table
                .static_machine_application_lifetimes(selection.application)
                .to_vec();
            self.expression_table
                .set_static_machine_application_lifetimes(selected, &lifetimes);
            let row = self
                .expression_table
                .static_machine_application_mut(selected);
            row.lexical_owner = declaration;
            row.binding_declaration = original.binding_declaration;
            selection.application = selected;
            selection.subject = application.type_reference(self, selection.subject)?;
            selection.executable = SymbolHandle::invalid();
        }
        self.applied_domain_applications
            .push(domain::AppliedDomainApplication {
                declaration,
                arguments: arguments.to_vec(),
                theory,
            });
        Ok(true)
    }
}
