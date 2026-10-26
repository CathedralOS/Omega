//! Resolve a Build operation's static operand path to its declaration symbol
//! over the typed program.

use crate::expression::StaticMachineArgument;
use crate::typed_trees::TypedTrees;
use symbols::{JoinedPath, SymbolHandle};

/// Declaration roles of non-callable Product operands. A requirement member,
/// a boundary subject and a representation have different declaration roles;
/// none acquires ordinary host-call nameability through this selection.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProductOperandKind {
    BoundarySubject,
    Provider,
    TrustSubject,
    Requirement,
    Data,
    Representation,
}

/// A product description names a declaration, not an applied type, value or
/// evidence projection. Checking and later path resolution share this shape.
pub fn is_declaration_operand(argument: &StaticMachineArgument) -> bool {
    !argument.type_reference.is_valid()
        && argument.application.is_none()
        && argument.const_literal.is_none()
        && argument.evidence_projection.is_none()
        && !argument.path.is_empty()
}

/// Resolve one designated Product declaration with its actual role and source
/// scope. These descriptions name declarations, not applied host values;
/// conformance formation remains with the conformance application owner.
pub fn resolve_product_operand(
    typed: &TypedTrees,
    argument: &StaticMachineArgument,
    occurrence: source::SourceSpan,
    kind: ProductOperandKind,
) -> Option<symbols::SymbolHandle> {
    let program = typed.product_declarations();
    if !is_declaration_operand(argument) {
        return None;
    }
    let path = argument.path.joined_path();
    let candidates = match kind {
        ProductOperandKind::BoundarySubject => program
            .traits()
            .iter()
            .filter(|definition| definition.is_boundary)
            .map(|definition| (definition.symbol, definition.is_public))
            .chain(
                program
                    .machines()
                    .iter()
                    .filter(|machine| {
                        machine.supply_mode
                            == language_semantics::MachineSupplyMode::TopLevelRequirement
                    })
                    .map(|machine| (machine.symbol, machine.is_public)),
            )
            .chain(
                program
                    .all_operators()
                    .filter(|operator| operator.is_boundary)
                    .map(|operator| (operator.symbol, operator.is_public)),
            )
            .collect::<Vec<_>>(),
        ProductOperandKind::Provider => program
            .data_definitions()
            .iter()
            .map(|definition| (definition.symbol, definition.is_public))
            .chain(
                program
                    .conformances()
                    .iter()
                    .filter(|definition| {
                        definition.type_parameters.is_empty()
                            && definition.lifetime_parameters.is_empty()
                    })
                    .map(|definition| (definition.symbol, definition.is_public)),
            )
            .collect(),
        ProductOperandKind::TrustSubject => program
            .conformances()
            .iter()
            .filter(|definition| {
                definition.type_parameters.is_empty() && definition.lifetime_parameters.is_empty()
            })
            .map(|definition| (definition.symbol, definition.is_public))
            .chain(
                program
                    .machines()
                    .iter()
                    .filter(|machine| {
                        machine.supply_mode == language_semantics::MachineSupplyMode::AdmissionClaim
                    })
                    .map(|machine| (machine.symbol, machine.is_public)),
            )
            .collect(),
        ProductOperandKind::Requirement => program
            .traits()
            .iter()
            .filter(|definition| definition.is_boundary)
            .flat_map(|definition| {
                program
                    .trait_machine_signatures(definition)
                    .iter()
                    .map(move |requirement| (requirement.symbol, definition.is_public))
            })
            .collect(),
        ProductOperandKind::Data => program
            .data_definitions()
            .iter()
            .map(|definition| (definition.symbol, definition.is_public))
            .collect(),
        ProductOperandKind::Representation => program
            .conformances()
            .iter()
            .map(|definition| (definition.symbol, definition.is_public))
            .collect(),
    };
    let token_bound_machines = if kind == ProductOperandKind::BoundarySubject {
        program
            .machine_token_bindings()
            .iter()
            .filter(|binding| binding.is_boundary)
            .map(|binding| binding.symbol)
            .collect::<Vec<_>>()
    } else {
        Vec::new()
    };
    program.symbols.find_build_subject_declaration_from_source(
        &path,
        occurrence,
        candidates,
        &token_bound_machines,
    )
}

/// Source-signature compatibility for the narrowly designated product-entry
/// query. The build owner supplies target meaning; Psi retains lexical lookup
/// and exact-symbol selection without depending on target or native crates.
/// This check must not execute the entry or require the build's unfinished
/// provider, receiver-provision, or layout settlement. Final admission rechecks
/// the selected declaration after the build completes.
/// Every declaration handle and parameter must come from the supplied program;
/// sharing a Rust lifetime alone does not establish arena identity.
pub trait ProductEntryCompatibility: Sync {
    fn validate_entry(
        &self,
        program: &TypedTrees,
        declaration: ProductEntryDeclaration<'_>,
        slot: &str,
    ) -> Result<(), String>;
}

/// Declaration data needed by the Build owner's entry-slot judgment. This view
/// carries no executable state or statement span. The query issuer retains the
/// exact selected symbol and its lexical authority separately; this is neither
/// an issued description nor evidence that the product has been admitted.
///
/// The backing program still supplies type identities and any computations
/// required by the exact source signature. It supplies no argument backing.
/// Removing the entry body does not authorize dropping those dependencies.
#[derive(Clone, Copy)]
pub struct ProductEntryDeclaration<'program> {
    pub name: &'program str,
    pub attached_data_symbol: SymbolHandle,
    pub type_parameters: &'program [crate::data::TypeParameter],
    pub entry: Option<ProductEntrySignature<'program>>,
}

#[derive(Clone, Copy)]
pub struct ProductEntrySignature<'program> {
    pub parameters: &'program [crate::signature::StateParameter],
    pub return_type: crate::types::TypeReferenceHandle,
}

impl<'program> ProductEntryDeclaration<'program> {
    /// Project the existing complete-program producer onto the declaration
    /// boundary. A missing entry stays missing; it is never an empty body.
    pub fn from_machine(
        program: &'program TypedTrees,
        machine: &'program crate::machine::Machine,
    ) -> Self {
        Self {
            name: machine.name.as_str(),
            attached_data_symbol: machine.attached_data_symbol,
            type_parameters: program.machine_type_parameters(machine),
            entry: program
                .machine_states(machine)
                .first()
                .map(|entry| ProductEntrySignature {
                    parameters: program.state_parameters(entry),
                    return_type: entry.return_type,
                }),
        }
    }
}
