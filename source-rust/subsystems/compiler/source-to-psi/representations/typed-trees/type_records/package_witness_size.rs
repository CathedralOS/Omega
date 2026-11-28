//! Whether a concrete witness requires unsized inline storage, independent of
//! its target's word or pointer size.

use crate::TypedTrees;
use crate::data::{DataDefinition, DataField};
use crate::types::{TypeReferenceHandle, TypeReferenceNode};
use std::collections::{HashMap, HashSet};
use symbols::SymbolHandle;

impl TypedTrees {
    /// A bare slice has no fixed size; a reference or callable value has its
    /// own size. Generic parameters in a checking form defer to substitution.
    /// The caller has already established this is a live type reference.
    pub fn package_witness_is_unsized(&self, witness: TypeReferenceHandle) -> bool {
        StorageCheck::default().unsized_type(self, witness, &[])
    }

    /// Runtime fields contributed by the record or any one of its cases.
    pub fn stored_data_fields<'a>(
        &'a self,
        data: &DataDefinition,
    ) -> impl Iterator<Item = &'a DataField> {
        self.data_members(data)
            .into_iter()
            .flat_map(|member| {
                let payload = member
                    .as_variant()
                    .into_iter()
                    .flat_map(|case| self.data_payload_fields(case));
                member.as_field().into_iter().chain(payload)
            })
            .filter(|field| !field.relevance.is_erased())
    }
}

#[derive(Default)]
struct StorageCheck {
    active: HashSet<SymbolHandle>,
    complete: HashMap<(SymbolHandle, Vec<bool>), bool>,
}

impl StorageCheck {
    fn unsized_type(
        &mut self,
        program: &TypedTrees,
        ty: TypeReferenceHandle,
        bindings: &[(SymbolHandle, bool)],
    ) -> bool {
        let types = &program.type_reference_table;
        match types.type_reference(ty) {
            TypeReferenceNode::Slice { .. } => true,
            TypeReferenceNode::Reference { .. }
            | TypeReferenceNode::Callable { .. }
            | TypeReferenceNode::Unit
            | TypeReferenceNode::ConstExpression(_) => false,
            TypeReferenceNode::Constrained { base_type, .. } => {
                self.unsized_type(program, *base_type, bindings)
            }
            TypeReferenceNode::FixedArray { element_type, .. } => {
                self.unsized_type(program, *element_type, bindings)
            }
            TypeReferenceNode::Named { symbol, .. } => {
                if let Some((_, unsized_storage)) =
                    bindings.iter().find(|(binder, _)| binder == symbol)
                {
                    return *unsized_storage;
                }
                self.nominal(program, *symbol, Vec::new())
            }
            TypeReferenceNode::Generic {
                base_symbol,
                arguments,
                ..
            } => {
                let arguments = types
                    .type_reference_handles(*arguments)
                    .iter()
                    .map(|arg| self.unsized_type(program, *arg, bindings))
                    .collect();
                self.nominal(program, *base_symbol, arguments)
            }
        }
    }

    fn nominal(
        &mut self,
        program: &TypedTrees,
        symbol: SymbolHandle,
        arguments: Vec<bool>,
    ) -> bool {
        let Some(data) = program.unique_data_definition_by_symbol(symbol) else {
            // Builtins, an open type parameter and opaque nominal operands have
            // no authored storage fields to inspect here.
            return false;
        };
        let key = (symbol, arguments);
        if let Some(unsized_storage) = self.complete.get(&key) {
            return *unsized_storage;
        }
        if !self.active.insert(symbol) {
            // A cycle along inline field edges has no finite representation.
            // A borrowed/referenced edge never recurses into its target.
            return true;
        }
        let bindings: Vec<_> = program
            .data_type_parameters(data)
            .iter()
            .zip(&key.1)
            .map(|(binder, value)| (binder.symbol, *value))
            .collect();
        let unsized_storage = program
            .stored_data_fields(data)
            .any(|field| self.unsized_type(program, field.type_reference, &bindings));
        self.active.remove(&symbol);
        self.complete.insert(key, unsized_storage);
        unsized_storage
    }
}
