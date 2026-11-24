/*
Which declarations are mathematical only. Data that contains itself by value
has no layout, so no value of it is stored and no machine over it runs: such a
machine is a definition or a law, read into the compilation's mathematical
signature (`crate::lowering::mathematics`) and never lowered to operations.

The test reads the declaration as written. A field or payload field that
names its own declaration makes the data proof-only; a machine with a formal
or a result of such a type is a proof machine, and so is a machine declared
inside one.
*/

use crate::source_state::data::{DataDefinition, DataMember};
use crate::source_state::machine::Machine;
use crate::source_state::types::{TypeReferenceHandle, TypeReferenceNode};
use symbols::SymbolHandle;

use super::Program;

impl Program<'_> {
    fn names_data(&self, reference: TypeReferenceHandle) -> Option<SymbolHandle> {
        if !reference.is_valid() {
            return None;
        }
        match self.typed.type_reference_table.type_reference(reference) {
            TypeReferenceNode::Named { symbol, .. }
                if symbol.is_valid()
                    && self.typed.symbols.builtin_type_atom(*symbol).is_none()
                    && self.data_declaration(*symbol).is_some() =>
            {
                Some(*symbol)
            }
            _ => None,
        }
    }

    /// Whether a field of this declaration, or of one of its cases, has the
    /// declaration itself as its type.
    pub(crate) fn contains_itself(&self, data: &DataDefinition) -> bool {
        let typed = self.typed;
        typed.data_members(data).iter().any(|member| match member {
            DataMember::Field(field) => self.names_data(field.type_reference) == Some(data.symbol),
            DataMember::Variant(variant) => typed
                .data_payload_fields(variant)
                .iter()
                .any(|field| self.names_data(field.type_reference) == Some(data.symbol)),
        })
    }

    fn proof_only_type(&self, reference: TypeReferenceHandle) -> bool {
        self.names_data(reference)
            .and_then(|symbol| self.typed.unique_data_definition_by_symbol(symbol))
            .is_some_and(|data| self.contains_itself(data))
    }

    /// The trait a machine satisfies a requirement of, with the data each of
    /// the trait's type arguments names: which conformance the machine is a
    /// part of. Read from the declaration as written, before the machine.
    pub(crate) fn conformance(
        &self,
        machine: &Machine,
    ) -> Option<(SymbolHandle, Vec<Option<SymbolHandle>>)> {
        let typed = self.typed;
        let mut edges = typed.machine_trait_conformances(machine).into_iter();
        let (Some(edge), None) = (edges.next(), edges.next()) else {
            return None;
        };
        let Some(crate::source_state::machine::SatisfiedDeclaration::Trait { definition, .. }) =
            crate::source_state::machine::resolve_satisfied_declaration(typed, machine, edge)
        else {
            return None;
        };
        let application =
            crate::source_state::trait_definition::AppliedTraitScope::from_machine_conformance(
                typed, machine, edge,
            )?;
        Some((
            definition.symbol,
            application
                .substitutions
                .iter()
                .map(|(_, argument)| self.names_data(*argument))
                .collect(),
        ))
    }

    /// The machine a nested machine is declared inside, if any.
    pub(crate) fn enclosing_machine(&self, symbol: SymbolHandle) -> Option<SymbolHandle> {
        let typed = self.typed;
        let mut enclosing = typed.symbols.get(symbol).parent;
        while enclosing.is_valid() && typed.symbols.symbols().nodes().is_valid(enclosing) {
            if let Some(outer) = typed.machine_by_symbol(enclosing) {
                return (outer.symbol != symbol).then_some(outer.symbol);
            }
            enclosing = typed.symbols.get(enclosing).parent;
        }
        None
    }

    /// Whether the machine takes or returns data that contains itself, or is
    /// declared inside a machine that does.
    pub(crate) fn proof_machine(&self, machine: &Machine) -> bool {
        let typed = self.typed;
        let own = typed.machine_states(machine).first().is_some_and(|entry| {
            self.proof_only_type(entry.return_type)
                || typed
                    .state_parameters
                    .span(entry.parameters)
                    .is_some_and(|formals| {
                        formals
                            .iter()
                            .any(|formal| self.proof_only_type(formal.type_reference))
                    })
        });
        if own {
            return true;
        }
        let mut enclosing = typed.symbols.get(machine.symbol).parent;
        while enclosing.is_valid() && typed.symbols.symbols().nodes().is_valid(enclosing) {
            if let Some(outer) = typed.machine_by_symbol(enclosing) {
                return outer.symbol != machine.symbol && self.proof_machine(outer);
            }
            enclosing = typed.symbols.get(enclosing).parent;
        }
        false
    }
}
