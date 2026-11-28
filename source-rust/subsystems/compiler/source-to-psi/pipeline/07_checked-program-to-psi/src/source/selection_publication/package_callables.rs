use super::{CallableScope, SymbolRows};
use crate::source_state::TypedTrees;
use crate::source_state::expression::ExpressionNode;
use crate::source_state::references::type_reference_inputs;
use crate::source_state::types::{TypeReferenceHandle, TypeReferenceNode};
use diagnostics::Diagnostic;
use symbols::SymbolHandle;

/// A field signature retains its declaration's lexical scope. A package projection
/// or construction can additionally supply exact abstract types to that signature.
pub(super) fn retain_bindings(
    typed: &TypedTrees,
    contracts: &mut SymbolRows<CallableScope>,
) -> Result<(), Diagnostic> {
    if typed.package_openings.is_empty() {
        return Ok(());
    }
    let mut states = SymbolRows::<Vec<SymbolHandle>>::new(typed.symbols.symbols().nodes().len());
    let mut retain = |field: TypeReferenceHandle, state, witness| -> Result<(), Diagnostic> {
        let TypeReferenceNode::Callable { contract } =
            typed.type_reference_table.type_reference(field)
        else {
            return Ok(());
        };
        if typed.type_reference_mentions_any(field, &[witness]) {
            let callable = typed.runtime_callable_contract(*contract).signature.symbol;
            let row = contracts.get_or_insert_with(callable, CallableScope::default)?;
            let binding = (state, witness);
            if !row.package_bindings.contains(&binding) {
                row.package_bindings.push(binding);
            }
        }
        Ok(())
    };
    for (_, opening) in typed.package_openings.iter() {
        for &(_, witness) in &opening.witnesses {
            let TypeReferenceNode::Named { symbol, .. } =
                typed.type_reference_table.type_reference(witness)
            else {
                continue;
            };
            let owners = states.get_or_insert_with(*symbol, Vec::new)?;
            if !owners.contains(&opening.state) {
                owners.push(opening.state);
            }
        }
    }
    for field in &typed.package_projections {
        let opening = typed.package_opening(field.opening);
        for &(_, witness) in &opening.witnesses {
            if let TypeReferenceNode::Named { symbol, .. } =
                typed.type_reference_table.type_reference(witness)
            {
                retain(field.result_type, opening.state, *symbol)?;
            }
        }
    }
    // A repack applies a fresh receiving signature, distinct from the projected
    // source signature. Its own checked substitutions identify the captured types.
    for (_, expression) in typed.expression_table.expression_entries() {
        let ExpressionNode::StructLiteral(literal) = expression else {
            continue;
        };
        let Some(package) = &literal.package else {
            continue;
        };
        for &substitution in &package.witnesses {
            for symbol in type_reference_inputs(typed, substitution).symbols {
                if let Some(owners) = states.get(&symbol) {
                    for &state in owners {
                        for field in &package.fields {
                            retain(field.ty, state, symbol)?;
                        }
                    }
                }
            }
        }
    }
    Ok(())
}
