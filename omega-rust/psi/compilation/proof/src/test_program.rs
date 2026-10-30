//! Shared test program: a minimal `TypedTrees` carrying one machine, two
//! data symbols and an `Int` builtin so obligation keys resolve canonically.

use crate::obligations::{
    BoundedValueObligation, ProofConstraint, ProofObligation, ProofObligationOwner,
};
use symbols::{SymbolHandle, SymbolKind, SymbolNameRef, SymbolTableBuilder};
use typed_trees::TypedTrees;
use typed_trees::name::Identifier;
use typed_trees::types::{TypeReferenceHandle, TypeReferenceNode};

pub(crate) struct Program {
    pub(crate) typed_trees: TypedTrees,
    pub(crate) machine: SymbolHandle,
    pub(crate) data: [SymbolHandle; 2],
    pub(crate) int_type: SymbolHandle,
}

pub(crate) fn program() -> Program {
    let mut builder = SymbolTableBuilder::new();
    let root = builder.insert_root(SymbolKind::Root, SymbolNameRef::Static("root"));
    let machine = SymbolTableBuilder::child_handles(
        builder.insert_children(root, [(SymbolKind::Machine, SymbolNameRef::Static("Main"))]),
    )
    .next()
    .expect("machine");
    let members = SymbolTableBuilder::child_handles(builder.insert_children(
        machine,
        [
            (SymbolKind::Data, SymbolNameRef::Static("count")),
            (SymbolKind::Data, SymbolNameRef::Static("total")),
            (SymbolKind::BuiltinType, SymbolNameRef::Static("Int")),
        ],
    ))
    .collect::<Vec<_>>();
    Program {
        typed_trees: TypedTrees {
            symbols: builder.finish(),
            ..TypedTrees::default()
        },
        machine,
        data: [members[0], members[1]],
        int_type: members[2],
    }
}

impl Program {
    /// The `bounded` `count` obligation over the fixture's first data symbol,
    /// with the `Int` carrier it names -- the row every derivation-consultation
    /// test opens with.
    pub(crate) fn count_value(&mut self) -> (TypeReferenceHandle, ProofObligation) {
        let base = self.int_reference();
        let obligation = main_owned_value(self.machine, self.data[0], "count", base);
        (base, obligation)
    }

    /// A `Main`-owned bounded-value obligation over one of the fixture's data
    /// symbols, carrying the `Int` reference the caller already resolved.
    pub(crate) fn data_value(
        &self,
        data: usize,
        name: &str,
        base: TypeReferenceHandle,
    ) -> ProofObligation {
        main_owned_value(self.machine, self.data[data], name, base)
    }

    pub(crate) fn int_reference(&mut self) -> TypeReferenceHandle {
        self.typed_trees
            .type_reference_table
            .insert(TypeReferenceNode::Named {
                symbol: self.int_type,
                name: Identifier::generated("Int"),
            })
    }
}

/// A bounded-value obligation owned by the fixture's `Main` machine: the only
/// place the empty constraint span and the machine's display name are spelled.
pub(crate) fn main_owned_value(
    machine: SymbolHandle,
    data_symbol: SymbolHandle,
    data_name: &str,
    base: TypeReferenceHandle,
) -> ProofObligation {
    bounded_value(
        machine,
        "Main",
        data_symbol,
        data_name,
        base,
        arena::HandleSpan::empty(),
    )
}

pub(crate) fn bounded_value(
    machine: SymbolHandle,
    machine_name: &str,
    data_symbol: SymbolHandle,
    data_name: &str,
    base: TypeReferenceHandle,
    constraints: arena::HandleSpan<ProofConstraint>,
) -> ProofObligation {
    ProofObligation::BoundedValue(BoundedValueObligation {
        owner: ProofObligationOwner::MachineOwnedData {
            machine_symbol: machine,
            machine: Identifier::generated(machine_name),
            data_symbol,
            data: Identifier::generated(data_name),
        },
        base_type: base,
        constraints,
    })
}
