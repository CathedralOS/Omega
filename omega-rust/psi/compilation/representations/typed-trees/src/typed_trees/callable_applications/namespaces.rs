//! Substitute a selected implementation namespace and its exact requirement
//! children together. A namespace is not a const/value expression. These rows
//! transport a source selection; ordinary bound and call admission still own
//! its fit and authority.

use super::CallableApplication;
use crate::TypedTrees;
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
                        .map(|row| row.realization_state),
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
