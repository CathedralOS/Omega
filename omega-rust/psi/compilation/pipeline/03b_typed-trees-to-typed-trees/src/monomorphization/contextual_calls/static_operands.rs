//! Executable closure follows complete typed payloads, not only carrier leaves.
//! Array extents, const expressions and domain/range arguments can still belong
//! to a caller telescope. The representation's reference visitor owns that
//! graph; pending folds and runtime subjects are not closed cache operands.

use super::super::{SymbolHandle, TypeReferenceHandle, TypedTrees};
use symbols::SymbolKind;
use typed_trees::expression::ExpressionHandle;
use typed_trees::references::{TypedReferences, visit_type_reference};
use typed_trees::types::{FixedArrayLength, TypeReferenceNode};

#[derive(Default)]
struct OperandReferences {
    symbols: Vec<SymbolHandle>,
    types: Vec<TypeReferenceHandle>,
    expressions: Vec<ExpressionHandle>,
}

impl Extend<SymbolHandle> for OperandReferences {
    fn extend<References: IntoIterator<Item = SymbolHandle>>(&mut self, symbols: References) {
        self.symbols.extend(symbols);
    }
}

impl TypedReferences for OperandReferences {
    fn type_reference(&mut self, reference: TypeReferenceHandle) -> bool {
        if self.types.contains(&reference) {
            return false;
        }
        self.types.push(reference);
        true
    }

    fn expression(&mut self, expression: ExpressionHandle) -> bool {
        if self.expressions.contains(&expression) {
            return false;
        }
        self.expressions.push(expression);
        true
    }
}

fn collect(program: &TypedTrees, reference: TypeReferenceHandle) -> OperandReferences {
    let mut references = OperandReferences::default();
    visit_type_reference(program, &mut references, reference);
    // Normalization may name a data instance instead of retaining Generic at
    // the root. Its exact application still owns the operand's open binders.
    let mut symbol_ordinal = 0usize;
    while symbol_ordinal < references.symbols.len() {
        let symbol = references.symbols[symbol_ordinal];
        symbol_ordinal += 1;
        if let Some(application) = program
            .data_definition_by_symbol(symbol)
            .and_then(|definition| definition.generic_instance)
        {
            visit_type_reference(program, &mut references, application);
        }
    }
    references
}

pub(crate) fn names_any(
    program: &TypedTrees,
    reference: TypeReferenceHandle,
    symbols: &[SymbolHandle],
) -> bool {
    collect(program, reference)
        .symbols
        .iter()
        .any(|symbol| symbols.contains(symbol))
}

pub(in crate::monomorphization) fn is_open(
    program: &TypedTrees,
    reference: TypeReferenceHandle,
) -> bool {
    let references = collect(program, reference);
    references.symbols.iter().any(|symbol| {
        symbol.is_valid()
            && matches!(
                program.symbols.get(*symbol).kind,
                SymbolKind::TypeParameter
                    | SymbolKind::MachineParameter
                    | SymbolKind::ConformanceParameter
                    | SymbolKind::Parameter
                    | SymbolKind::Local
            )
    }) || references
        .expressions
        .iter()
        .any(|expression| program.pending_const_range_endpoints.contains(expression))
        || references.types.iter().any(|reference| {
            matches!(
                program.type_reference_table.type_reference(*reference),
                TypeReferenceNode::FixedArray {
                    length: FixedArrayLength::ConstCall { .. },
                    ..
                }
            )
        })
}
