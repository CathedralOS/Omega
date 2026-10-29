//! Reify captured static actuals through their exact original declaration scope.
//! This changes neither the source-issued request nor its activation custody.
use super::*;

fn rejoin_owner(
    program: &TypedTrees,
    owner: source::SourceSpan,
) -> Result<SymbolHandle, Vec<Diagnostic>> {
    typed_trees::product_operand::resolve_build_request_author(program, owner)
        .map_err(|reason| vec![Diagnostic::error(reason)])
}

pub(super) fn supplier_owner(
    program: &TypedTrees,
    environment: Option<&ProductStaticEnvironment>,
    author: SymbolHandle,
) -> Result<SymbolHandle, Vec<Diagnostic>> {
    match environment {
        None => Ok(author),
        Some(environment) => {
            let template = environment.template.ok_or_else(|| {
                vec![Diagnostic::error(
                    "static helper environment lost its exact original template",
                )]
            })?;
            if program
                .symbols
                .symbol_provenance_source_span(author)
                .map(|source| program.symbols.rejoin_product_occurrence(source))
                != Some(program.symbols.rejoin_product_occurrence(template))
            {
                return Err(vec![Diagnostic::error(
                    "static helper environment does not belong to the exact selecting helper",
                )]);
            }
            rejoin_owner(
                program,
                environment.lexical_owner.ok_or_else(|| {
                    vec![Diagnostic::error(
                        "static helper environment lost its supplying source scope",
                    )]
                })?,
            )
        }
    }
}

pub(super) struct SuppliedArgument {
    pub argument: expr::StaticMachineArgument,
    pub owner: SymbolHandle,
    pub declared_type: Option<TypeReferenceHandle>,
}

impl Lower<'_> {
    pub(super) fn environment_argument(
        &mut self,
        name: Handle<ProductOperandName>,
    ) -> Result<Option<SuppliedArgument>, Vec<Diagnostic>> {
        let Some(environment) = self.static_environment else {
            return Ok(None);
        };
        let Some(parameter) = self.graph.names.get(name).declaration else {
            return Ok(None);
        };
        let parameter = self.program.symbols.rejoin_product_occurrence(parameter);
        let mut selected = environment
            .bindings
            .iter()
            .map(|(_, binding)| binding)
            .filter(|binding| {
                self.program
                    .symbols
                    .rejoin_product_occurrence(binding.parameter)
                    == parameter
            });
        let Some(binding) = selected.next().cloned() else {
            return Ok(None);
        };
        if selected.next().is_some() {
            return Err(
                self.error("static helper environment binds the same exact source parameter twice")
            );
        }
        if self.active_bindings.contains(&binding.parameter) {
            return Err(self.error("static helper environment has a cyclic substitution"));
        }
        let owner = rejoin_owner(self.program, binding.lexical_owner)?;
        let mut active_bindings = self.active_bindings.clone();
        active_bindings.push(binding.parameter);
        let mut actual = Lower::new(self.program, &environment.graph, owner, Some(environment));
        actual.active_bindings = active_bindings;
        let argument = actual.operand(binding.argument)?;
        if argument.type_reference.is_valid() {
            actual
                .program
                .type_reference_table
                .retain_operand_lifetime_owner(argument.type_reference, owner)
                .map_err(|reason| actual.error(reason))?;
        }
        let declared_type = if binding.declared_type.is_valid() {
            Some(actual.ty(binding.declared_type)?)
        } else {
            None
        };
        Ok(Some(SuppliedArgument {
            argument,
            owner,
            declared_type,
        }))
    }

    pub(super) fn lifetime_identifier(
        &self,
        name: Handle<ProductOperandName>,
    ) -> Result<Identifier, Vec<Diagnostic>> {
        let Some(environment) = self.static_environment else {
            return Ok(self.identifier(name));
        };
        // Captured actuals already belong to their original supplying scope;
        // never reinterpret a caller region as another helper's binder.
        if std::ptr::eq(self.graph, &environment.graph) {
            return Ok(self.identifier(name));
        }
        let named = self.graph.names.get(name);
        let mut matched = None;
        for (_, binding) in environment.lifetimes.iter() {
            let owner = rejoin_owner(self.program, binding.parameter_owner)?;
            let machine = self
                .program
                .machine_by_symbol(owner)
                .ok_or_else(|| self.error("static helper lifetime owner is not a machine"))?;
            let parameter = machine
                .lifetime_parameters
                .get(binding.parameter_ordinal as usize)
                .ok_or_else(|| {
                    self.error("static helper lifetime lost its original binder ordinal")
                })?;
            // This is ordinary lexical lifetime resolution within the exact
            // retained declaring machine, not matching an actual by spelling.
            let source = self
                .program
                .symbols
                .symbol_provenance_source_span(owner)
                .ok_or_else(|| self.error("static helper lifetime owner lost source custody"))?;
            let occurrence = self
                .program
                .symbols
                .rejoin_product_occurrence(named.source_span);
            if source.source_id == occurrence.source_id && parameter.as_str() == named.text {
                if matched.is_some() {
                    return Err(self.error("static helper lifetime has multiple supplying frames"));
                }
                let actual = environment.graph.names.get(binding.argument);
                matched = Some(Identifier::generated(&actual.text));
            }
        }
        Ok(matched.unwrap_or_else(|| self.identifier(name)))
    }
    pub(super) fn lifetime_identifiers(
        &self,
        names: HandleSpan<ProductOperandName>,
    ) -> Result<Vec<Identifier>, Vec<Diagnostic>> {
        names
            .handles()
            .map(|name| self.lifetime_identifier(name))
            .collect()
    }
}
