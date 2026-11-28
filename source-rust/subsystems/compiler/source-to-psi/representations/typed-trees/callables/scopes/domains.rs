use super::CallableApplication;
use crate::source_state::{TypedTrees, domain, types};
use symbols::SymbolHandle;

impl CallableApplication {
    pub(crate) fn domain_identity(
        &mut self,
        program: &mut TypedTrees,
        declaration: SymbolHandle,
        arguments: &[types::TypeReferenceHandle],
        original: semantic_vocabulary::SemanticDomainId,
    ) -> Result<semantic_vocabulary::SemanticDomainId, &'static str> {
        if !declaration.is_valid() {
            return Ok(original);
        }
        let domain = program
            .domain_definition(declaration)
            .ok_or("applied domain lost its declaration")?;
        if program.domain_type_parameters(domain).is_empty() {
            if !arguments.is_empty() {
                return Err("unindexed domain received index arguments");
            }
            return Ok(domain.semantic_id);
        }
        // A fixed carrier's entire telescope is already explicit in
        // this packet. Only a carrier-generic family needs the retained
        // implicit carrier coordinate from its source application.
        if !domain::has_generic_carrier(program, domain) {
            return program
                .retain_domain_arguments(declaration, arguments)
                .map_err(|_| "copied domain indices have no canonical instance identity");
        }
        let retained = program
            .domain_arguments_for_instance(declaration, original)
            .ok_or("applied domain lost its complete carrier/index coordinate")?
            .to_vec();
        let complete = retained
            .into_iter()
            .map(|argument| self.type_reference(program, argument))
            .collect::<Result<Vec<_>, _>>()?;
        if arguments.len()
            != domain::index_parameters(
                program,
                program
                    .domain_definition(declaration)
                    .ok_or("applied domain lost its declaration")?,
            )
            .len()
        {
            return Err("applied domain has an incomplete explicit index telescope");
        };
        program
            .retain_domain_arguments(declaration, &complete)
            .map_err(|_| "copied domain arguments have no canonical instance identity")
    }
}

impl TypedTrees {
    /// Bind the constraint and its semantic roles to the same applied domain on this carrier.
    pub fn rebind_domain_constraint(
        &mut self,
        carrier: types::TypeReferenceHandle,
        domain: &mut types::DomainConstraint,
    ) -> Result<(), String> {
        let identity = self
            .retain_domain_application(domain.symbol, carrier, &domain.arguments)
            .map_err(|error| error.message)?;
        domain.semantic_id = identity;
        domain.semantic_roles.denotation_dimension =
            domain.semantic_roles.denotation_dimension.map(|_| identity);
        domain.semantic_roles.arithmetic_policy =
            domain.semantic_roles.arithmetic_policy.map(|_| identity);
        Ok(())
    }

    /// Retains a domain's complete declaration application while the
    /// producer still knows the carrier. This captures meaning, not proof.
    pub fn retain_domain_application(
        &mut self,
        declaration: SymbolHandle,
        carrier: types::TypeReferenceHandle,
        arguments: &[types::TypeReferenceHandle],
    ) -> Result<semantic_vocabulary::SemanticDomainId, diagnostics::Diagnostic> {
        let definition = self.domain_definition(declaration).ok_or_else(|| {
            diagnostics::Diagnostic::error("domain application lost its declaration")
        })?;
        if arguments.len() != domain::index_parameters(self, definition).len() {
            return Err(diagnostics::Diagnostic::error(
                "domain application has an incomplete index telescope",
            ));
        }
        if !self.contains_type_reference(carrier) {
            return Err(diagnostics::Diagnostic::error(
                "domain application lost its exact carrier",
            ));
        }
        let mut complete = Vec::with_capacity(self.domain_type_parameters(definition).len());
        if domain::has_generic_carrier(self, definition) {
            complete.push(
                self.type_reference_table
                    .unconstrained_type_reference(carrier),
            );
        }
        complete.extend_from_slice(arguments);
        self.retain_domain_arguments(declaration, &complete)
    }

    pub(super) fn retain_domain_arguments(
        &mut self,
        declaration: SymbolHandle,
        arguments: &[types::TypeReferenceHandle],
    ) -> Result<semantic_vocabulary::SemanticDomainId, diagnostics::Diagnostic> {
        let definition = self.domain_definition(declaration).ok_or_else(|| {
            diagnostics::Diagnostic::error("domain application lost its declaration")
        })?;
        let parameters = self.domain_type_parameters(definition);
        let identity =
            domain::indexed_domain_instance_name(self, definition, parameters, arguments)?;
        if parameters.is_empty() {
            return Ok(definition.semantic_id);
        }
        let identity = self.semantic_domains.intern(&identity);
        if self.domain_application(declaration, identity).is_none() {
            let is_closed = arguments
                .iter()
                .all(|argument| domain::index_argument_is_closed(self, *argument));
            self.applied_domain_applications
                .insert(domain::AppliedDomainApplication {
                    declaration,
                    identity,
                    arguments: arguments.to_vec(),
                    is_closed,
                    theory: None,
                });
        }
        Ok(identity)
    }
}
#[cfg(test)]
#[path = "../tests/domain_scopes.rs"]
pub(crate) mod tests;
