//! Form the complete type of a qualified static argument to a Product choice.
use super::*;
use crate::declaration_selection::product_operands::ProductOperandConstraint;
use crate::source_state::domain::{
    domain_accepts_carrier, has_direct_index_telescope, index_parameters,
};
use crate::source_state::types::TypeConstraintNode;
use arena::HandleSpan;

impl Lower<'_> {
    pub(super) fn qualified_type(
        &mut self,
        base: Handle<ProductOperandType>,
        constraints: HandleSpan<ProductOperandConstraint>,
    ) -> Formed<Closed> {
        let Closed::Type(base_type) = self.ty(base, true)? else {
            return Err(Stop::Rejected(
                "a qualified static argument requires a carrier type".into(),
            ));
        };
        let graph = self.graph;
        let written = graph
            .constraints
            .span(constraints)
            .filter(|items| !items.is_empty())
            .ok_or(unserved(
                "a qualified static type without its written constraints",
            ))?;
        let mut selected = Vec::with_capacity(written.len());
        for constraint in written {
            let ProductOperandConstraint::Domain { name, arguments } = constraint else {
                return Err(unserved(
                    "a static type qualification other than a declared domain",
                ));
            };
            if !graph.names.is_valid(*name) {
                return Err(unserved(
                    "a static type qualification without its authored name",
                ));
            }
            let name = graph.names.get(*name).clone();
            let symbol = self.symbol(&name, &name.text, SymbolKind::Domain)?;
            let definition = self.program.domain_definition(symbol).ok_or(unserved(
                "a static type qualification without its domain declaration",
            ))?;
            if definition.alias.is_some() {
                return Err(unserved("a domain alias in a static Product type argument"));
            }
            if !has_direct_index_telescope(self.program, definition) {
                return Err(unserved(
                    "a domain qualification with a computed index pattern",
                ));
            }
            let parameters = index_parameters(self.program, definition).to_vec();
            let supplied = graph.type_arguments.span(*arguments).ok_or(unserved(
                "a static domain qualification without its index tuple",
            ))?;
            let arguments = self.closed_arguments(&parameters, &name, supplied, Self::ty)?;
            let definition = self
                .program
                .domain_definition(symbol)
                .expect("the selected domain remains in the program");
            if !domain_accepts_carrier(self.program, definition, base_type, &arguments) {
                return Err(Stop::Rejected(format!(
                    "domain `{}` does not qualify this static argument's carrier",
                    name.text
                )));
            }
            let instance = self
                .program
                .retain_domain_application(symbol, base_type, &arguments)
                .map_err(|error| Stop::Rejected(error.message))?;
            let definition = self
                .program
                .domain_definition(symbol)
                .expect("retaining an application keeps its declaration");
            selected.push(TypeConstraintNode::Domain(definition.applied_constraint(
                Identifier::generated(name.text),
                arguments,
                instance,
            )));
        }
        let constraints = self
            .program
            .type_reference_table
            .insert_constraints(selected);
        Ok(Closed::Type(self.program.type_reference_table.insert(
            TypeReferenceNode::Constrained {
                base_type,
                constraints,
            },
        )))
    }
}
