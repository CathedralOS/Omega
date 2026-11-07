//! Substitute a selected implementation namespace and its exact requirement
//! children together. A namespace is not a const/value expression. These rows
//! transport a source selection; ordinary bound and call admission still own
//! its fit and authority.

use super::CallableApplication;
use crate::{
    TypedTrees, expression::TableCallExpression, signature, trait_definition::AppliedTraitScope,
};
use symbols::{SymbolHandle, SymbolKind};

impl CallableApplication {
    pub(super) fn namespace_symbols(
        &self,
        program: &TypedTrees,
        binder: SymbolHandle,
    ) -> Result<Vec<(SymbolHandle, SymbolHandle)>, &'static str> {
        let selected = if let Some((_, argument)) = self
            .static_arguments
            .iter()
            .rev()
            .find(|(source, _)| *source == binder)
        {
            argument.clone()
        } else if let Some((_, argument)) = self
            .types
            .iter()
            .rev()
            .find(|(source, _)| *source == binder)
        {
            program.static_operand_from_type_reference(*argument)?
        } else {
            return Ok(Vec::new());
        };
        program.selected_implementation_namespace_symbols(binder, &selected)
    }
}

impl TypedTrees {
    /// Rejoin an already selected implementation operand with the exact
    /// requirement children of its receiving binder. Materialization and
    /// read-only contract observation consume the same correspondence; this
    /// supplies no bound admission, invocation or implementation guarantee.
    pub fn selected_implementation_namespace_symbols(
        &self,
        binder: SymbolHandle,
        selected: &crate::expression::StaticMachineArgument,
    ) -> Result<Vec<(SymbolHandle, SymbolHandle)>, &'static str> {
        let program = self;
        if !selected.names_implementation(&program.symbols) {
            return Err("applied requirement namespace needs an implementation operand");
        }
        if selected.application.is_some() {
            return Err("applied requirement namespace needs its complete selected application");
        }
        let mut symbols = vec![(binder, selected.symbol)];
        let owner = program.symbols.get(binder).parent;
        for child in program.symbols.child_handles(binder).into_iter().flatten() {
            if program.symbols.get(child).kind != SymbolKind::State {
                continue;
            }
            let requirement = program
                .evidence_bound_requirement(owner, binder, child)
                .map_err(|_| "applied requirement namespace lost its authored bound")?
                .ok_or("applied requirement namespace lost its requirement")?;
            let mut candidates = Vec::new();
            if let Some(conformance) = program.conformance_by_symbol(selected.symbol) {
                candidates.extend(
                    conformance
                        .rows
                        .iter()
                        .filter(|row| {
                            row.declaring_trait == requirement.application.declaration.symbol
                                && row.requirement == requirement.signature.symbol
                        })
                        .map(|row| row.requirement),
                );
            } else {
                let selected_owner = program.symbols.get(selected.symbol).parent;
                for target in program
                    .symbols
                    .child_handles(selected.symbol)
                    .into_iter()
                    .flatten()
                {
                    if let Some(bound) = program
                        .evidence_bound_requirement(selected_owner, selected.symbol, target)
                        .map_err(|_| "selected namespace lost its authored bound")?
                        && bound.signature.symbol == requirement.signature.symbol
                        && bound.application.declaration.symbol
                            == requirement.application.declaration.symbol
                    {
                        candidates.push(target);
                    }
                }
            }
            let [target] = candidates.as_slice() else {
                return Err("selected namespace has no unique exact requirement row");
            };
            symbols.push((child, *target));
        }
        Ok(symbols)
    }
}

impl TypedTrees {
    /// An explicit named map supplies the public requirement's complete
    /// application. Keep its selected row separate from a private realizer;
    /// this query transports result vocabulary, not invocation admission.
    pub fn named_conformance_call_signature(
        &self,
        call: &TableCallExpression,
    ) -> Option<&signature::StateSignature> {
        self.resolve_named_conformance_call_signature(call)
            .ok()
            .flatten()
    }

    /// Separate an ordinary receiver from a named map whose selected public
    /// view refuses. An absent or ambiguous map must not borrow an ambient
    /// trait application after losing its own selection.
    pub(super) fn resolve_named_conformance_call_signature(
        &self,
        call: &TableCallExpression,
    ) -> Result<Option<&signature::StateSignature>, &'static str> {
        if call.static_requirement_dispatch.is_some() || call.requirement_application.is_valid() {
            return Ok(None);
        }
        let Some(receiver) = self.expression_table.name_path_node(call.receiver) else {
            return Ok(None);
        };
        let Some(selected) = self.conformance_by_symbol(receiver.symbol) else {
            return if receiver.symbol.is_valid()
                && self.symbols.get(receiver.symbol).kind == SymbolKind::Conformance
            {
                Err("named public view lost its selected implementation")
            } else {
                Ok(None)
            };
        };
        // Applied generic maps retain their independently supplied tuple in
        // the ordinary dispatch path above; a bare declaration cannot supply it.
        if !selected.type_parameters.is_empty() || !selected.lifetime_parameters.is_empty() {
            return Err("named public view has no complete selected application");
        }
        let closed = self.closed_requirement_application(call.target_symbol);
        let template = closed
            .as_ref()
            .map_or(call.target_symbol, |selected| selected.template);
        let mut rows = selected
            .rows
            .iter()
            .filter(|row| row.requirement == template);
        let row = rows
            .next()
            .ok_or("named public view has no exact requirement row")?;
        if rows.next().is_some() {
            return Err("named public view has ambiguous requirement rows");
        }
        let scope = AppliedTraitScope::from_conformance(self, selected)
            .ok_or("named public view lost its complete trait application")?
            .declaring_scope(self, row.declaring_trait)
            .ok_or("named public view lost its declaring trait scope")?;
        if closed.is_some()
            && !self.requirement_application_matches_scope(call.target_symbol, &scope)
        {
            return Err("named public view disagrees with its selected trait application");
        }
        self.applied_callable_signature(&scope, template)
            .or_else(|| {
                (scope.substitutions.is_empty() && scope.declaration.lifetime_parameters.is_empty())
                    .then(|| self.trait_machine_signature_by_symbol(scope.declaration, template))
                    .flatten()
            })
            .map(Some)
            .ok_or("named public view has no prepared applied signature")
    }
}
