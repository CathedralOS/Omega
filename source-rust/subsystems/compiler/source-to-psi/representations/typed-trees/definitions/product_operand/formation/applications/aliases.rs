//! A type argument selects either nominal data or a transparent alias. The
//! alias keeps the author's selection and arguments on a fresh occurrence of
//! its expansion, just as a type reference in an ordinary signature does.

use super::{Closed, Formed, Lower, ProductOperandName, Stop, SymbolHandle, SymbolKind};
use super::{TypeParameterKind, TypeReferenceHandle, unserved};
use crate::source_state::CallableApplication;
use crate::source_state::type_aliases::TypeAliasApplication;
use crate::source_state::types::TypeReferenceNode;
use crate::static_operand_binding::{self, type_argument_meets_bounds};

impl Lower<'_> {
    pub(super) fn type_application(
        &mut self,
        symbol: SymbolHandle,
        name: &ProductOperandName,
        arguments: Vec<TypeReferenceHandle>,
    ) -> Formed<Closed> {
        let raw = if arguments.is_empty() {
            self.named(symbol, name)
        } else {
            self.applied(symbol, name, arguments)
        };
        if self.program.symbols.get(symbol).kind != SymbolKind::TypeAlias {
            return Ok(Closed::Type(raw));
        }

        let program = &mut *self.program;
        let alias = program
            .type_aliases()
            .iter()
            .find(|alias| alias.symbol == symbol)
            .cloned()
            .ok_or(unserved("a type alias without its checked declaration"))?;
        let declaration_source = program
            .symbols
            .symbol_provenance_source_span(symbol)
            .ok_or(unserved("a type alias without its declaration source"))?;
        let arguments = match *program.type_reference_table.type_reference(raw) {
            TypeReferenceNode::Generic { arguments, .. } => arguments,
            TypeReferenceNode::Named { .. } => arena::HandleSpan::empty(),
            _ => return Err(unserved("a type alias application without its head")),
        };
        let formals = program.declaration_type_parameters(symbol);
        let actuals = program.type_reference_handles(arguments);
        if formals.len() != actuals.len() {
            return Err(Stop::Rejected(format!(
                "static type `{}` needs {} arguments, received {}",
                name.text,
                formals.len(),
                actuals.len()
            )));
        }
        if !alias.lifetime_parameters.is_empty()
            || !alias.conformance_bounds.is_empty()
            || formals
                .iter()
                .any(|formal| formal.kind != TypeParameterKind::Type)
        {
            return Err(unserved(
                "a Product type alias with unserved binders or conformance bounds",
            ));
        }
        for (formal, actual) in formals.iter().zip(actuals) {
            type_argument_meets_bounds(program, formal, *actual).map_err(|stop| match stop {
                static_operand_binding::Stop::Rejected(reason) => Stop::Rejected(reason),
                static_operand_binding::Stop::Unserved(reason) => Stop::Unserved(reason),
                static_operand_binding::Stop::Defect(reason) => unserved(reason),
            })?;
        }
        let mut substitution = CallableApplication::for_type_parameters(
            symbol,
            formals.iter().zip(actuals.iter().copied()),
            None,
        );
        let expanded = substitution
            .type_reference(program, alias.expansion)
            .map_err(unserved)?;
        let instance = program.type_reference_table.fresh_occurrence(expanded);
        let supplied_operand_owner = program.type_reference_table.operand_lifetime_owner(raw);
        program
            .type_reference_table
            .retain_type_alias_application(TypeAliasApplication {
                instance,
                application: raw,
                declaration: symbol,
                declaration_source,
                reference: name.source_span,
                arguments,
                lifetime_arguments: Vec::new(),
                supplied_operand_owner,
            });
        Ok(Closed::Type(instance))
    }
}
