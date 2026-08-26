use psi_symbols::{SymbolHandle, SymbolKind};

use crate::TypedTrees;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DeclarationVisibility {
    kind: &'static str,
    is_public: bool,
}

impl DeclarationVisibility {
    pub const fn kind(self) -> &'static str {
        self.kind
    }

    pub const fn is_public(self) -> bool {
        self.is_public
    }
}

/// Whether this symbol family participates in ordinary package visibility.
pub const fn requires_declaration_visibility(kind: SymbolKind) -> bool {
    matches!(
        kind,
        SymbolKind::Data
            | SymbolKind::Conformance
            | SymbolKind::Domain
            | SymbolKind::Field
            | SymbolKind::Variant
            | SymbolKind::Machine
            | SymbolKind::Operator
            | SymbolKind::Proposition
            | SymbolKind::State
            | SymbolKind::Trait
            | SymbolKind::Const
            | SymbolKind::WireSchema
    )
}

/// Resolve visibility for an independently nameable declaration or for a
/// genuine nested member which inherits its exact semantic owner's visibility.
/// Operators are checked before parent traversal because domain-homed operators
/// own visibility independently of their carrier domain.
pub fn declaration_visibility(
    program: &TypedTrees,
    symbol: SymbolHandle,
) -> Option<DeclarationVisibility> {
    let kind = program.symbols.get(symbol).kind;
    let direct = match kind {
        SymbolKind::Data => program
            .data_definitions()
            .iter()
            .find(|declaration| declaration.symbol == symbol)
            .map(|declaration| ("data", declaration.is_public)),
        SymbolKind::Conformance => program
            .conformances()
            .iter()
            .find(|declaration| declaration.symbol == symbol)
            .map(|declaration| ("conformance", declaration.is_public)),
        SymbolKind::Domain => program
            .domain_definitions()
            .iter()
            .find(|declaration| declaration.symbol == symbol)
            .map(|declaration| ("domain", declaration.is_public)),
        SymbolKind::Machine => program
            .machines()
            .iter()
            .find(|declaration| declaration.symbol == symbol)
            .map(|declaration| ("machine", declaration.is_public)),
        SymbolKind::Operator => crate::operator::declaration_by_symbol(program, symbol)
            .map(|declaration| ("operator", declaration.is_public)),
        SymbolKind::Proposition => program
            .propositions()
            .iter()
            .find(|declaration| declaration.symbol == symbol)
            .map(|declaration| ("proposition", declaration.is_public)),
        SymbolKind::Trait => program
            .traits()
            .iter()
            .find(|declaration| declaration.symbol == symbol)
            .map(|declaration| ("trait", declaration.is_public)),
        SymbolKind::Const => program
            .const_declarations()
            .iter()
            .find(|declaration| declaration.symbol == symbol)
            .map(|declaration| ("const", declaration.is_public)),
        SymbolKind::WireSchema => program
            .wire_schemas()
            .iter()
            .find(|declaration| declaration.symbol == symbol)
            .map(|declaration| ("wire schema", declaration.is_public)),
        SymbolKind::Field | SymbolKind::Variant | SymbolKind::State => None,
        _ => return None,
    };
    if let Some((kind, is_public)) = direct {
        return Some(DeclarationVisibility { kind, is_public });
    }

    if matches!(
        kind,
        SymbolKind::Field | SymbolKind::Variant | SymbolKind::State
    ) {
        let mut parent = program.symbols.get(symbol).parent;
        while parent.is_valid()
            && matches!(
                program.symbols.get(parent).kind,
                SymbolKind::ConformanceParameter
                    | SymbolKind::MachineParameter
                    | SymbolKind::PropositionMachineParameter
            )
        {
            parent = program.symbols.get(parent).parent;
        }
        if parent.is_valid() {
            return declaration_visibility(program, parent);
        }
    }
    None
}
