//! Apply a domain's explicit index telescope with the shared signature copier.
//! The source declaration and its constraints remain immutable; this view owns
//! substituted facts and cleanup operands, never new establishment authority.

use super::CallableApplication;
use crate::{TypedTrees, domain, types};
use symbols::SymbolHandle;

#[cfg(test)]
mod tests;

impl CallableApplication {
    /// An instance ID is derived from its exact index tuple, not copied from
    /// the declaration being applied. Publish the new ID with the substituted
    /// operands so later membership checks cannot observe two different
    /// identities for the same applied contract.
    pub(super) fn domain_identity(
        program: &mut TypedTrees,
        declaration: SymbolHandle,
        arguments: &[types::TypeReferenceHandle],
        original: language_semantics::SemanticDomainId,
    ) -> Result<language_semantics::SemanticDomainId, &'static str> {
        if arguments.is_empty() || !declaration.is_valid() {
            return Ok(original);
        }
        let Some(domain) = program.domain_definition(declaration) else {
            // Authored, unresolved domains are still owned by normalization.
            return Ok(original);
        };
        let identity = domain::indexed_domain_instance_name(
            program,
            domain,
            domain::index_parameters(program, domain),
            arguments,
        )
        .map_err(|_| "applied domain arguments have no canonical instance identity")?;
        Ok(program.semantic_domains.intern(&identity))
    }
}

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
            // This is a logical theory view, not a fresh executable signature.
            owned_signature: SymbolHandle::invalid(),
            runtime_values: Vec::new(),
            region_scope: declaration,
            source_lifetime_owner: SymbolHandle::invalid(),
            trait_region_application: None,
            lifetime_declaration: declaration,
            lifetimes: Vec::new(),
            elided_lifetimes: Vec::new(),
            types: substitutions,
            // Type/Const substitutions belong to the value/type map. The
            // namespace map is reserved for selected evidence and code.
            static_arguments: Vec::new(),
            static_argument_owner: SymbolHandle::invalid(),
            selected_providers: Vec::new(),
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
        if let Some(selection) = theory.cleanup_selection {
            theory.cleanup_selection =
                Some(self.apply_cleanup_selection(&mut application, selection, declaration)?);
        }
        self.applied_domain_applications
            .push(domain::AppliedDomainApplication {
                declaration,
                arguments: arguments.to_vec(),
                theory,
            });
        Ok(true)
    }

    /// Retarget a declaration's cleanup selection through `application`. The
    /// copy keeps the selection's registration, target, lifetimes, binding
    /// declaration and authorization; only its static operands and subject are
    /// substituted, and its executable stays unresolved until specialization.
    pub(super) fn apply_cleanup_selection(
        &mut self,
        application: &mut CallableApplication,
        selection: domain::CleanupSelection,
        lexical_owner: SymbolHandle,
    ) -> Result<domain::CleanupSelection, &'static str> {
        let original = self
            .static_machine_application(selection.application)
            .clone();
        let mut arguments = self
            .static_machine_application_arguments(selection.application)
            .to_vec();
        application.static_arguments(self, &mut arguments)?;
        let selected = self.expression_table.retain_static_machine_application(
            original.registration_operation,
            original.target,
            &arguments,
        );
        let lifetimes = self
            .static_machine_application_lifetimes(selection.application)
            .to_vec();
        self.expression_table
            .set_static_machine_application_lifetimes(
                selected,
                original.lifetime_binding_declaration,
                &lifetimes,
            );
        let row = self
            .expression_table
            .static_machine_application_mut(selected);
        row.lexical_owner = lexical_owner;
        row.binding_declaration = original.binding_declaration;
        Ok(domain::CleanupSelection {
            application: selected,
            authorization: selection.authorization,
            subject: application.type_reference(self, selection.subject)?,
            executable: SymbolHandle::invalid(),
        })
    }
}
