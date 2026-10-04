use symbols::{SymbolHandle, SymbolKind};

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
            | SymbolKind::Measure
            | SymbolKind::Proposition
            | SymbolKind::State
            | SymbolKind::Trait
            | SymbolKind::Const
            | SymbolKind::WireSchema
    )
}

/// Whether the machine that owns `symbol` (the nearest machine ancestor, if
/// any) still has an executable declaration root. A symbol under a pruned
/// machine, such as a field slot of a target sibling checked for another target,
/// belongs to no declaration of this realization.
pub fn owning_machine_is_retained(program: &TypedTrees, symbol: SymbolHandle) -> bool {
    let mut current = symbol;
    while current.is_valid() {
        let entry = program.symbols.get(current);
        if entry.kind == SymbolKind::Machine {
            return program
                .machines()
                .iter()
                .any(|declaration| declaration.symbol == current);
        }
        current = entry.parent;
    }
    true
}

/// Resolve declaration visibility independently of executable root membership.
/// Authored selections retain exact symbols across private projections, whose
/// backing arenas still own their declarations. Reading that visibility grants
/// no callable body or invocation authority. Nested members inherit their exact
/// owner's visibility; domain-homed operators own visibility independently.
pub fn declaration_visibility(
    program: &TypedTrees,
    symbol: SymbolHandle,
) -> Option<DeclarationVisibility> {
    let kind = program.symbols.get(symbol).kind;
    macro_rules! retained_visibility {
        ($table:ident, $kind:literal) => {
            consistent_visibility(
                $kind,
                program
                    .tables
                    .$table
                    .iter()
                    .filter(|(_, declaration)| declaration.symbol == symbol)
                    .map(|(_, declaration)| declaration.is_public),
            )
        };
    }
    let direct = match kind {
        SymbolKind::Data => retained_visibility!(data_definitions, "data"),
        SymbolKind::Conformance => retained_visibility!(conformances, "conformance"),
        SymbolKind::Domain => retained_visibility!(domain_definitions, "domain"),
        SymbolKind::Machine => retained_visibility!(machines, "machine"),
        SymbolKind::Operator => retained_visibility!(operators, "operator"),
        SymbolKind::Measure => program
            .tables
            .measures
            .iter()
            .any(|(_, declaration)| declaration.symbol == symbol)
            .then_some(DeclarationVisibility {
                kind: "measure",
                is_public: false,
            }),
        SymbolKind::Proposition => retained_visibility!(propositions, "proposition"),
        SymbolKind::Trait => retained_visibility!(traits, "trait"),
        SymbolKind::Const => retained_visibility!(const_declarations, "const"),
        SymbolKind::WireSchema => retained_visibility!(wire_schemas, "wire schema"),
        SymbolKind::Field | SymbolKind::Variant | SymbolKind::State => None,
        _ => return None,
    };
    if direct.is_some() {
        return direct;
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

/// Root projection can append another row for the same declaration. Visibility
/// is an immutable declaration property, so every retained copy must agree;
/// conflicting copies are missing evidence, never a choice of the public row.
fn consistent_visibility(
    kind: &'static str,
    mut copies: impl Iterator<Item = bool>,
) -> Option<DeclarationVisibility> {
    let is_public = copies.next()?;
    copies
        .all(|copy| copy == is_public)
        .then_some(DeclarationVisibility { kind, is_public })
}

#[cfg(test)]
mod tests {
    use super::{consistent_visibility, declaration_visibility, owning_machine_is_retained};
    use crate::{TypedTrees, machine::Machine};
    use symbols::{SymbolKind, SymbolNameRef, SymbolTableBuilder};

    #[test]
    fn projected_machine_visibility_does_not_restore_execution_membership() {
        for is_public in [false, true] {
            let mut builder = SymbolTableBuilder::default();
            let machine = builder.insert_root(SymbolKind::Machine, SymbolNameRef::Static("owner"));
            let state = builder
                .insert_children(
                    machine,
                    [(SymbolKind::State, SymbolNameRef::Static("entry"))],
                )
                .start();
            let mut program = TypedTrees::default();
            program.symbols = builder.finish();
            let declaration = Machine {
                symbol: machine,
                is_public,
                ..Machine::default()
            };
            program.tables.machines.insert(declaration.clone());
            program.tables.machines.insert(declaration);
            let visibility = declaration_visibility(&program, state)
                .expect("the exact backing owner retains visibility");
            assert_eq!(visibility.kind(), "machine");
            assert_eq!(visibility.is_public(), is_public);
            assert!(!owning_machine_is_retained(&program, state));
        }
    }

    #[test]
    fn conflicting_retained_visibility_never_selects_a_public_copy() {
        assert!(consistent_visibility("machine", [].into_iter()).is_none());
        assert!(consistent_visibility("machine", [false, true].into_iter()).is_none());
        assert!(consistent_visibility("machine", [true, false].into_iter()).is_none());
    }
}
