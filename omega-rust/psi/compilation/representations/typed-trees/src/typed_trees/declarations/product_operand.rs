//! Resolve a Build operation's static operand path to its declaration symbol
//! over the typed program.

mod input_selections;
pub use input_selections::{
    ProductArgumentStep, ProductInputSelection, ProductInputSelectionTarget,
    ProductTypeSelectionSlot,
};

mod applications;
pub use applications::{ProductApplicationDemand, ProductOperandApplication};

mod occurrences;
pub use occurrences::{ProductSelectionOccurrence, collect_product_selection_occurrences};

mod static_environment;
pub use static_environment::capture_static_environment;

use crate::expression::StaticMachineArgument;
use crate::typed_trees::TypedTrees;
use symbols::{JoinedPath, SymbolHandle};

/// Declaration roles of non-callable Product operands. A requirement member,
/// a boundary subject and a representation have different declaration roles;
/// none acquires ordinary host-call nameability through this selection.
pub use language_semantics::declaration_selection::ProductOperandRole as ProductOperandKind;

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
    if !is_declaration_operand(argument) {
        return None;
    }
    resolve_product_path(typed, &argument.path.joined_path(), occurrence, kind)
}

/// Resolve a source-issued locator at its retained lexical occurrence. Paths
/// describe declarations; role, package and source-stratum authority remain
/// with this common Product query.
pub fn resolve_product_path(
    typed: &TypedTrees,
    path: &str,
    occurrence: source::SourceSpan,
    kind: ProductOperandKind,
) -> Option<symbols::SymbolHandle> {
    let program = typed.product_declarations();
    let candidates = product_candidates(program, kind)?;
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
        path,
        occurrence,
        candidates,
        &token_bound_machines,
    )
}

/// Resolve a completed dependency Build's captured lexical package, without
/// importing its Build code or inventing a source occurrence in this Product.
/// The caller must carry the original package output/source custody.
pub fn resolve_captured_product_path(
    typed: &TypedTrees,
    scope: &language_semantics::declaration_selection::CapturedNativeDeclaration,
    kind: ProductOperandKind,
) -> Option<SymbolHandle> {
    let program = typed.product_declarations();
    let candidates = product_candidates(program, kind)?;
    program.symbols.find_captured_product_declaration(
        &scope.requirement_path,
        scope.producer.package,
        scope.resolution_stratum,
        candidates,
    )
}

fn product_candidates(
    program: &TypedTrees,
    kind: ProductOperandKind,
) -> Option<Vec<(SymbolHandle, bool)>> {
    let candidates = match kind {
        ProductOperandKind::WireCompatibilityFact => return None,
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
    Some(candidates)
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

/// Rejoin an exact authored Build owner without importing generated instances.
pub fn resolve_build_request_author(
    typed: &TypedTrees,
    author: source::SourceSpan,
) -> Result<SymbolHandle, String> {
    let author = typed.symbols.rejoin_product_occurrence(author);
    let mut matching = typed
        .machines()
        .iter()
        .filter(|machine| typed.symbols.symbol_source_span(machine.symbol) == Some(author));
    let machine = matching
        .next()
        .ok_or("executed Build request has no authored selecting machine")?;
    if matching.next().is_some() {
        return Err("executed Build request has ambiguous authored machine custody".into());
    }
    Ok(machine.symbol)
}

/// Capture exact authored provenance before source erasure. The Terminal
/// producer separately proves this machine owns the issued request.
pub fn native_binding_formation(
    typed: &TypedTrees,
    author: SymbolHandle,
    root: Option<SymbolHandle>,
    request: std::sync::Arc<language_semantics::declaration_selection::ProductDeclarationRequest>,
) -> Result<language_semantics::declaration_selection::NativeBindingFormation, String> {
    use sha2::{Digest, Sha256};
    let machine = typed
        .machine_by_symbol(author)
        .ok_or("native source author has no exact checked machine")?;
    let span = typed
        .symbols
        .symbol_provenance_source_span(author)
        .ok_or("native source author lost its source")?;
    let source = typed
        .symbols
        .source_file(span)
        .ok_or("native declaration has no retained source file")?;
    let callable_identity = typed
        .machine_callable_identity_text(machine)
        .ok_or("native declaration has no checked callable identity")?;
    let [operand] = request.operands.as_ref() else {
        return Err("native formation lost its exact requirement".into());
    };
    let relative = source
        .path
        .strip_prefix(&source.package_root)
        .map_err(|_| "native source is outside its checked package")?
        .to_string_lossy();
    let mut digest = Sha256::new();
    digest.update(b"omega.native-source-occurrence.v1\0");
    for bytes in [
        relative.as_bytes(),
        source.source.as_bytes(),
        callable_identity.as_bytes(),
        operand.path.as_bytes(),
    ] {
        digest.update((bytes.len() as u64).to_le_bytes());
        digest.update(bytes);
    }
    digest.update((request.source_span.span.start as u64).to_le_bytes());
    digest.update((request.source_span.span.end as u64).to_le_bytes());
    let root_scope = root
        .and_then(|root| typed.symbols.symbol_provenance_source_span(root))
        .is_some_and(|root| typed.symbols.same_source_package(span, root));
    Ok(
        language_semantics::declaration_selection::NativeBindingFormation {
            request,
            author: span,
            author_package: typed.symbols.symbol_package_identity(author),
            root_scope,
            declaration_path: typed.symbols.display_path(author, "::"),
            callable_identity,
            source_occurrence_digest: digest.finalize().into(),
            resolution_stratum: source.resolution_stratum,
        },
    )
}

impl TypedTrees {
    /// Exact source correspondence, followed by whole owned-graph equality.
    /// Equal display paths or numeric SourceIds cannot replace this join.
    pub fn same_product_operand_source(
        &self,
        left: &language_semantics::declaration_selection::ProductDeclarationOperand,
        right: &language_semantics::declaration_selection::ProductDeclarationOperand,
    ) -> bool {
        if left.path != right.path
            || self.symbols.rejoin_product_occurrence(left.source_span)
                != self.symbols.rejoin_product_occurrence(right.source_span)
        {
            return false;
        }
        match (&left.application, &right.application) {
            (None, None) => true,
            (Some(left), Some(right)) => {
                if std::sync::Arc::ptr_eq(left, right) {
                    return true;
                }
                let mut left = left.as_ref().clone();
                let mut right = right.as_ref().clone();
                left.remap_source_spans(|span| self.symbols.rejoin_product_occurrence(span));
                right.remap_source_spans(|span| self.symbols.rejoin_product_occurrence(span));
                left == right
            }
            _ => false,
        }
    }

    pub fn same_product_request_source(
        &self,
        left: &language_semantics::declaration_selection::ProductDeclarationRequest,
        right: &language_semantics::declaration_selection::ProductDeclarationRequest,
    ) -> bool {
        left.operation == right.operation
            && self.symbols.rejoin_product_occurrence(left.source_span)
                == self.symbols.rejoin_product_occurrence(right.source_span)
            && left.operands.len() == right.operands.len()
            && left
                .operands
                .iter()
                .zip(right.operands.iter())
                .all(|(left, right)| self.same_product_operand_source(left, right))
    }
}
