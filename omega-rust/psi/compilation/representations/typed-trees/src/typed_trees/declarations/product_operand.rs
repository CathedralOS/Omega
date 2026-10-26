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
    /// Exact product declaration, shared by query compatibility and final
    /// settlement. Display names cannot authenticate its contract inputs.
    machine_symbol: SymbolHandle,
    /// Complete retained input headers. Only the Psi factory may construct
    /// these; realization receives their data query, never a supplied truth bit.
    machine_contracts: &'program [crate::signature::SignatureContract],
    entry_contracts: &'program [crate::signature::SignatureContract],
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
    /// Rejoin an exact retained product declaration independently of a supplied
    /// projection. A missing owner or incomplete header never becomes empty
    /// input metadata. Bodyless descriptions retain these same source headers.
    pub fn from_declared_owner(
        program: &'program TypedTrees,
        symbol: SymbolHandle,
    ) -> Option<Self> {
        let machine = program.machine(symbol)?;
        let contracts = program.machine_contracts(machine);
        if contracts.len() != machine.contracts.len()
            || program.machine_states(machine).len() != machine.states.len()
        {
            return None;
        }
        let entry_contracts = program
            .machine_states(machine)
            .first()
            .map_or(&[][..], |entry| program.state_contracts(entry));
        if program
            .machine_states(machine)
            .first()
            .is_some_and(|entry| entry_contracts.len() != entry.contracts.len())
        {
            return None;
        }
        if contracts.iter().chain(entry_contracts).any(|contract| {
            program.proof_facts.span_or_empty(contract.facts).len() != contract.facts.len()
        }) {
            return None;
        }
        Some(Self::from_machine(program, machine))
    }

    pub const fn machine_symbol(&self) -> SymbolHandle {
        self.machine_symbol
    }

    /// Exact named erased inputs declared by this retained entry interface.
    /// This is boundary input data, not evidence that any input was supplied.
    /// Normal source binding rules still require a real invocation witness.
    pub fn erased_requires_inputs(self) -> impl Iterator<Item = &'program str> + 'program {
        self.machine_contracts
            .iter()
            .chain(self.entry_contracts)
            .filter_map(|contract| {
                (contract.kind == crate::signature::SignatureContractKind::Requires)
                    .then_some(contract.binding.as_ref())
                    .flatten()
                    .map(|binding| binding.as_str())
            })
    }

    /// Project the existing complete-program producer onto the declaration
    /// boundary. A missing entry stays missing; it is never an empty body.
    pub fn from_machine(
        program: &'program TypedTrees,
        machine: &'program crate::machine::Machine,
    ) -> Self {
        Self {
            machine_symbol: machine.symbol,
            machine_contracts: program.machine_contracts(machine),
            entry_contracts: program
                .machine_states(machine)
                .first()
                .map_or(&[][..], |entry| program.state_contracts(entry)),
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
