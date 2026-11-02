//! Apply a generic data declaration's telescope to its default-domain cleanup.
//! Inference and specialization can close `Keeper<T>` without an authored
//! application that pre-resolution synthesis could copy. The declaration keeps
//! its open selection; this view owns the substituted machine application and
//! subject for one exact closed carrier identity, never new authority.

use super::CallableApplication;
use crate::{TypedTrees, data, domain, types};
use symbols::SymbolHandle;

impl TypedTrees {
    /// Retain the closed cleanup application for `reference`, a closed
    /// application of a generic data declaration that selects `dropped by`.
    /// Returns whether a new view was retained.
    pub fn prepare_closed_data_cleanup_application(
        &mut self,
        reference: types::TypeReferenceHandle,
    ) -> Result<bool, &'static str> {
        let types::TypeReferenceNode::Generic {
            base_symbol,
            arguments,
            ..
        } = self.type_reference_node(reference).clone()
        else {
            return Ok(false);
        };
        let Some(template) = self.unique_data_definition_by_symbol(base_symbol) else {
            return Ok(false);
        };
        let Some(selection) = template.cleanup_selection else {
            return Ok(false);
        };
        let declaration = template.symbol;
        let parameters = self.data_type_parameters(template).to_vec();
        let arguments = self
            .type_reference_table
            .type_reference_handles(arguments)
            .to_vec();
        if parameters.is_empty() || parameters.len() != arguments.len() {
            return Ok(false);
        }
        if parameters
            .iter()
            .zip(&arguments)
            .any(|(parameter, argument)| {
                !matches!(parameter.kind, data::TypeParameterKind::Value { .. })
                    && !domain::index_argument_is_closed(self, *argument)
            })
        {
            return Ok(false);
        }
        if self
            .applied_data_cleanup_selection(declaration, reference)
            .is_some()
        {
            return Ok(false);
        }
        let substitutions = parameters
            .iter()
            .zip(&arguments)
            .map(|(parameter, argument)| (parameter.symbol, *argument))
            .collect::<Vec<_>>();
        let mut application = CallableApplication {
            // This is a logical cleanup view, not a fresh executable signature.
            owned_signature: SymbolHandle::invalid(),
            region_scope: declaration,
            trait_region_application: None,
            lifetime_declaration: declaration,
            lifetimes: Vec::new(),
            elided_lifetimes: Vec::new(),
            types: substitutions,
            static_arguments: Vec::new(),
            symbols: Vec::new(),
            active_requirements: Vec::new(),
            source_binders: parameters
                .iter()
                .map(|parameter| (parameter.name.clone(), parameter.symbol))
                .collect(),
        };
        let original = self
            .static_machine_application(selection.application)
            .clone();
        let mut static_arguments = self
            .static_machine_application_arguments(selection.application)
            .to_vec();
        application.static_arguments(self, &mut static_arguments)?;
        let selected = self.expression_table.retain_static_machine_application(
            original.registration_operation,
            original.target,
            &static_arguments,
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
        row.lexical_owner = declaration;
        row.binding_declaration = original.binding_declaration;
        let subject = application.type_reference(self, selection.subject)?;
        self.applied_data_cleanup_selections
            .push(domain::AppliedDataCleanupSelection {
                declaration,
                selection: domain::CleanupSelection {
                    application: selected,
                    authorization: selection.authorization,
                    subject,
                    executable: SymbolHandle::invalid(),
                },
            });
        Ok(true)
    }
}
