//! Product declaration lookups and the static operands retained for Build choices.
pub mod formation;
pub(crate) mod input_selections;
pub use input_selections::{
    ProductArgumentStep, ProductInputSelection, ProductInputSelectionTarget,
    ProductTypeSelectionSlot,
};

pub(crate) mod applications;
mod captures;
pub use applications::{ProductApplicationDemand, ProductOperandApplication};

pub(crate) mod occurrences;
pub use occurrences::{
    ProductSelectionOccurrence, collect_product_selection_occurrences,
    visit_build_selection_occurrences,
};

pub(crate) mod static_environment;
pub use static_environment::{capture_closed_operand, capture_static_environment};

use crate::source_state::TypedTrees;
use crate::source_state::expression::StaticMachineArgument;
use crate::source_symbols::JoinedPath;
use symbols::SymbolHandle;

pub use crate::declaration_selection::ProductOperandRole as ProductOperandKind;

/*
    A Build selection operand names a declaration by a bare path, such as
    `NativeConsole`. It is never an applied type, a conformance application,
    a value or an evidence projection. An authored type operand has no path,
    so it never qualifies.

    A type reference beside a path is not authored syntax. It is the record
    `retain_static_argument_contexts` keeps of an implementation operand's
    source scope. Reading that record as an authored type refused
    `select_provider<Console, NativeConsole>` and every program that selects
    a named provider.

    The record may accompany a bare path only when it names that same
    declaration with no arguments. Any other type reference still fails, so
    a record that drifted from its path cannot pass as bare.
*/
pub fn is_declaration_operand(typed: &TypedTrees, argument: &StaticMachineArgument) -> bool {
    argument.application.is_none()
        && argument.const_literal.is_none()
        && argument.evidence_projection.is_none()
        && !argument.path.is_empty()
        && (!argument.type_reference.is_valid()
            || retained_source_names_same_declaration(typed, argument))
}

fn retained_source_names_same_declaration(
    typed: &TypedTrees,
    argument: &StaticMachineArgument,
) -> bool {
    argument.symbol.is_valid()
        && typed
            .type_reference_table
            .contains_type_reference(argument.type_reference)
        && matches!(
            typed.type_reference_table.type_reference(argument.type_reference),
            crate::source_state::types::TypeReferenceNode::Named { symbol, .. } if *symbol == argument.symbol
        )
}

pub fn resolve_product_operand(
    typed: &TypedTrees,
    argument: &StaticMachineArgument,
    occurrence: source_text::SourceSpan,
    kind: ProductOperandKind,
) -> Option<symbols::SymbolHandle> {
    if !is_declaration_operand(typed, argument) {
        return None;
    }
    resolve_product_path(typed, &argument.path.joined_path(), occurrence, kind)
}

pub fn resolve_product_path(
    typed: &TypedTrees,
    path: &str,
    occurrence: source_text::SourceSpan,
    kind: ProductOperandKind,
) -> Option<symbols::SymbolHandle> {
    ProductDeclarationLookup::new(typed, &[kind]).resolve(path, occurrence, kind)
}

/// Prepares one immutable declaration frontier for a batch of Build operands.
/// Access checks still use each operand's exact source occurrence.
pub struct ProductDeclarationLookup<'program> {
    program: &'program TypedTrees,
    declarations: ProductDeclarationIndex,
}

impl<'program> ProductDeclarationLookup<'program> {
    pub fn new(program: &'program TypedTrees, roles: &[ProductOperandKind]) -> Self {
        Self {
            program,
            declarations: program.build_product_declaration_index(roles),
        }
    }

    pub fn resolve(
        &self,
        path: &str,
        occurrence: source_text::SourceSpan,
        kind: ProductOperandKind,
    ) -> Option<SymbolHandle> {
        self.declarations
            .resolve(&self.program.symbols, path, occurrence, kind)
    }
}

impl ProductDeclarationIndex {
    pub(crate) fn resolve(
        &self,
        symbols: &crate::source_symbols::SymbolTable,
        path: &str,
        occurrence: source_text::SourceSpan,
        kind: ProductOperandKind,
    ) -> Option<SymbolHandle> {
        let token_bound_machines = if matches!(
            kind,
            ProductOperandKind::BoundarySubject | ProductOperandKind::RootGrantSubject
        ) {
            self.token_bound_machines.as_slice()
        } else {
            &[]
        };
        self.resolve_candidates(
            symbols,
            path,
            occurrence,
            &self.roles[product_role_position(kind)?],
            token_bound_machines,
        )
    }

    fn resolve_candidates(
        &self,
        symbols: &crate::source_symbols::SymbolTable,
        path: &str,
        occurrence: source_text::SourceSpan,
        rows: &[ProductDeclarationName],
        token_bound_machines: &[SymbolHandle],
    ) -> Option<SymbolHandle> {
        let occurrence = symbols.rejoin_product_occurrence(occurrence);
        // The final name narrows the candidates. The symbol table checks the
        // complete path, source file and visibility at the authored occurrence.
        let name = product_member_name(path);
        let first = rows.partition_point(|row| row.name.as_ref() < name);
        let last = rows.partition_point(|row| row.name.as_ref() <= name);
        symbols.find_build_subject_declaration_from_source(
            path,
            occurrence,
            rows[first..last]
                .iter()
                .map(|row| (row.symbol, row.is_public)),
            token_bound_machines,
        )
    }
}

const PRODUCT_ROLES: [ProductOperandKind; 11] = [
    ProductOperandKind::BoundarySubject,
    ProductOperandKind::RootGrantSubject,
    ProductOperandKind::Provider,
    ProductOperandKind::Requirement,
    ProductOperandKind::TestRequirement,
    ProductOperandKind::Data,
    ProductOperandKind::Representation,
    ProductOperandKind::Formal,
    ProductOperandKind::Field,
    ProductOperandKind::Domain,
    ProductOperandKind::CheckedMachine,
];

fn product_role_position(role: ProductOperandKind) -> Option<usize> {
    PRODUCT_ROLES
        .iter()
        .position(|candidate| *candidate == role)
}

fn product_member_name(path: &str) -> &str {
    path.rsplit_once("::").map_or(path, |(_, name)| name)
}

struct ProductDeclarationName {
    name: Box<str>,
    symbol: SymbolHandle,
    is_public: bool,
}

#[derive(Default)]
pub(crate) struct ProductDeclarationIndex {
    roles: [Vec<ProductDeclarationName>; PRODUCT_ROLES.len()],
    token_bound_machines: Vec<SymbolHandle>,
}

fn source_span_key(span: source_text::SourceSpan) -> (usize, usize, usize) {
    (span.source_id.0, span.span.start, span.span.end)
}

impl TypedTrees {
    fn product_declaration_names(
        &self,
        candidates: impl IntoIterator<Item = (SymbolHandle, bool)>,
    ) -> Vec<ProductDeclarationName> {
        let mut rows = Vec::new();
        for (symbol, is_public) in candidates {
            if self.symbols.symbols().nodes().is_valid(symbol) {
                rows.push(ProductDeclarationName {
                    name: product_member_name(self.symbols.name(symbol)).into(),
                    symbol,
                    is_public,
                });
            }
        }
        // Stable sorting retains the original representative of an operator family.
        rows.sort_by(|left, right| left.name.cmp(&right.name));
        rows
    }

    fn build_product_declaration_index(
        &self,
        roles: &[ProductOperandKind],
    ) -> ProductDeclarationIndex {
        let mut index = ProductDeclarationIndex::default();
        for (role_position, role) in PRODUCT_ROLES
            .into_iter()
            .enumerate()
            .filter(|(_, role)| roles.contains(role))
        {
            index.roles[role_position] =
                self.product_declaration_names(product_candidates(self, role));
        }
        if roles.contains(&ProductOperandKind::BoundarySubject)
            || roles.contains(&ProductOperandKind::RootGrantSubject)
        {
            index.token_bound_machines = self
                .machine_token_bindings()
                .iter()
                .filter(|binding| binding.is_boundary)
                .map(|binding| binding.symbol)
                .collect();
        }
        index
    }
}

pub fn product_candidates(
    program: &TypedTrees,
    kind: ProductOperandKind,
) -> Vec<(SymbolHandle, bool)> {
    match kind {
        ProductOperandKind::Formal => program
            .traits()
            .iter()
            .filter(|owner| owner.is_boundary)
            .flat_map(|owner| {
                program
                    .trait_machine_signatures(owner)
                    .iter()
                    .flat_map(move |signature| {
                        program
                            .state_signature_parameters(signature)
                            .iter()
                            .map(move |formal| (formal.symbol, owner.is_public))
                    })
            })
            .collect(),
        ProductOperandKind::Field => {
            let mut fields = Vec::new();
            for data in program.data_definitions() {
                for member in program.data_members(data) {
                    match member {
                        crate::source_state::data::DataMember::Field(field) => {
                            fields.push((field.symbol, data.is_public))
                        }
                        crate::source_state::data::DataMember::Variant(case) => fields.extend(
                            program
                                .data_payload_fields(case)
                                .iter()
                                .map(|field| (field.symbol, data.is_public)),
                        ),
                    }
                }
            }
            fields
        }
        ProductOperandKind::WireCompatibilityFact => Vec::new(),
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
                            == source_declarations::MachineSupplyMode::TopLevelRequirement
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
        ProductOperandKind::RootGrantSubject => program
            .conformances()
            .iter()
            .map(|definition| (definition.symbol, definition.is_public))
            .chain(
                program
                    .machines()
                    .iter()
                    .filter(|machine| {
                        matches!(
                            machine.supply_mode,
                            source_declarations::MachineSupplyMode::AdmissionClaim
                                | source_declarations::MachineSupplyMode::TopLevelRequirement
                        )
                    })
                    .map(|machine| (machine.symbol, machine.is_public)),
            )
            .chain(product_candidates(program, ProductOperandKind::Requirement))
            .collect(),
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
        ProductOperandKind::CheckedMachine => program
            .machines()
            .iter()
            .filter(|machine| {
                machine.body_is_present
                    && machine.supply_mode == source_declarations::MachineSupplyMode::CheckedBody
            })
            .map(|machine| (machine.symbol, machine.is_public))
            .collect(),
        ProductOperandKind::TestRequirement => program
            .traits()
            .iter()
            .filter(|definition| !definition.is_boundary)
            .flat_map(|definition| {
                program
                    .trait_machine_signatures(definition)
                    .iter()
                    .map(move |requirement| (requirement.symbol, definition.is_public))
            })
            .collect(),
        ProductOperandKind::Domain => program
            .domain_definitions()
            .iter()
            .map(|definition| (definition.symbol, definition.is_public))
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
    }
}

/// Resolve the source-owned semantic arrival signature without reading a
/// body or constructing display/type-identity copies. A matching spelling
/// outside the product's exact core vocabulary is never this declaration.
pub fn core_arrival_requirement<'program>(
    program: &'program TypedTrees,
    requirement: &str,
) -> Result<
    (
        &'program crate::source_state::trait_definition::TraitDefinition,
        &'program crate::source_state::signature::StateSignature,
    ),
    String,
> {
    let (owner, method) = requirement
        .split_once("::")
        .ok_or_else(|| "target semantic arrival has no requirement owner".to_owned())?;
    let mut declarations = program.traits().iter().filter(|definition| {
        definition.name.as_str() == owner
            && definition.is_boundary
            && program.symbols.core_vocabulary(definition.symbol)
                == Some(language_core::core_vocabulary::CoreVocabulary::ProgramStorageEntry)
            && program
                .symbols
                .symbol_source_span(definition.symbol)
                .and_then(|span| program.symbols.source_file(span))
                .is_some_and(|file| {
                    file.dependency_scope
                        == source_provenance::DependencyScope::Product
                })
    });
    let definition = declarations.next().ok_or_else(|| {
        format!("target semantic arrival `{requirement}` has no exact core declaration")
    })?;
    if declarations.next().is_some() {
        return Err(format!(
            "target semantic arrival `{requirement}` has multiple core declaration instances"
        ));
    }
    let mut signatures = program
        .trait_machine_signatures(definition)
        .iter()
        .filter(|signature| signature.name.as_str() == method);
    let signature = signatures.next().ok_or_else(|| {
        format!("target semantic arrival `{requirement}` has no exact machine declaration")
    })?;
    if signatures.next().is_some() {
        return Err(format!(
            "target semantic arrival `{requirement}` has multiple machine declarations"
        ));
    }
    Ok((definition, signature))
}

#[derive(Clone, Copy)]
pub struct ProductEntryDeclaration<'program> {
    machine_symbol: SymbolHandle,
    pub name: &'program str,
    pub attached_data_symbol: SymbolHandle,
    pub type_parameters: arena::RootRows<'program, crate::source_state::data::TypeParameter>,
    pub entry: Option<ProductEntrySignature<'program>>,
}

#[derive(Clone, Copy)]
pub struct ProductEntrySignature<'program> {
    pub parameters: arena::RootRows<'program, crate::source_state::signature::StateParameter>,
    pub return_type: crate::source_state::types::TypeReferenceHandle,
}

impl<'program> ProductEntryDeclaration<'program> {
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
            .map_or_else(Default::default, |entry| program.state_contracts(entry));
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

    pub fn from_machine(
        program: &'program TypedTrees,
        machine: &'program crate::source_state::machine::Machine,
    ) -> Self {
        Self {
            machine_symbol: machine.symbol,
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

pub fn resolve_build_request_author(
    typed: &TypedTrees,
    author: source_text::SourceSpan,
) -> Result<SymbolHandle, String> {
    ProductRequestAuthors::new(typed).resolve(author)
}

/// Retains exact authored-machine spans for one immutable source batch.
pub struct ProductRequestAuthors<'program> {
    program: &'program TypedTrees,
    authors: Vec<(source_text::SourceSpan, SymbolHandle)>,
}

/// Extracted declaration and author lookups for forming applications in a
/// mutable consuming program without cloning `TypedTrees`.
pub struct ProductSourceLookups {
    declarations: ProductDeclarationIndex,
    authors: Vec<(source_text::SourceSpan, SymbolHandle)>,
    types: Vec<ProductDeclarationName>,
}

impl ProductSourceLookups {
    pub fn new(program: &TypedTrees) -> Self {
        Self {
            declarations: program.build_product_declaration_index(&[
                ProductOperandKind::BoundarySubject,
                ProductOperandKind::RootGrantSubject,
                ProductOperandKind::Provider,
                ProductOperandKind::Requirement,
                ProductOperandKind::TestRequirement,
                ProductOperandKind::Domain,
            ]),
            types: program.product_declaration_names(
                program
                    .data_definitions()
                    .iter()
                    .map(|row| (row.symbol, row.is_public))
                    .chain(
                        program
                            .type_aliases()
                            .iter()
                            .map(|row| (row.symbol, row.is_public)),
                    ),
            ),
            authors: collect_request_authors(program),
        }
    }

    pub fn resolve_declaration(
        &self,
        symbols: &crate::source_symbols::SymbolTable,
        path: &str,
        occurrence: source_text::SourceSpan,
        kind: ProductOperandKind,
    ) -> Option<SymbolHandle> {
        self.declarations.resolve(symbols, path, occurrence, kind)
    }

    pub fn resolve_type(
        &self,
        symbols: &crate::source_symbols::SymbolTable,
        path: &str,
        occurrence: source_text::SourceSpan,
    ) -> Option<SymbolHandle> {
        self.declarations
            .resolve_candidates(symbols, path, occurrence, &self.types, &[])
    }

    pub fn resolve_author(
        &self,
        symbols: &crate::source_symbols::SymbolTable,
        author: source_text::SourceSpan,
    ) -> Result<SymbolHandle, String> {
        resolve_request_author(symbols, &self.authors, author)
    }
}

fn collect_request_authors(program: &TypedTrees) -> Vec<(source_text::SourceSpan, SymbolHandle)> {
    let mut authors = program
        .machines()
        .iter()
        .filter_map(|machine| {
            program
                .symbols
                .symbol_source_span(machine.symbol)
                .map(|span| (span, machine.symbol))
        })
        .collect::<Vec<_>>();
    authors.sort_by_key(|(span, _)| source_span_key(*span));
    authors
}

fn resolve_request_author(
    symbols: &crate::source_symbols::SymbolTable,
    authors: &[(source_text::SourceSpan, SymbolHandle)],
    author: source_text::SourceSpan,
) -> Result<SymbolHandle, String> {
    let author = symbols.rejoin_product_occurrence(author);
    let key = source_span_key(author);
    let first = authors.partition_point(|(span, _)| source_span_key(*span) < key);
    let last = authors.partition_point(|(span, _)| source_span_key(*span) <= key);
    match &authors[first..last] {
        [(_, symbol)] => Ok(*symbol),
        [] => Err("executed Build request has no authored selecting machine".into()),
        _ => Err("executed Build request has ambiguous authored machine custody".into()),
    }
}

impl<'program> ProductRequestAuthors<'program> {
    pub fn new(program: &'program TypedTrees) -> Self {
        Self {
            program,
            authors: collect_request_authors(program),
        }
    }

    pub fn resolve(&self, author: source_text::SourceSpan) -> Result<SymbolHandle, String> {
        resolve_request_author(&self.program.symbols, &self.authors, author)
    }
}

pub fn native_binding_formation(
    typed: &TypedTrees,
    author: SymbolHandle,
    root: Option<SymbolHandle>,
    request: std::sync::Arc<crate::declaration_selection::ProductDeclarationRequest>,
) -> Result<crate::declaration_selection::NativeBindingFormation, String> {
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
    Ok(crate::declaration_selection::NativeBindingFormation {
        request,
        author: span,
        author_package: typed.symbols.symbol_package_identity(author),
        root_scope,
        declaration_path: typed.symbols.display_path(author, "::"),
        callable_identity,
        source_occurrence_digest: digest.finalize().into(),
        resolution_stratum: source.resolution_stratum,
    })
}

impl TypedTrees {
    pub fn same_product_operand_source(
        &self,
        left: &crate::declaration_selection::ProductDeclarationOperand,
        right: &crate::declaration_selection::ProductDeclarationOperand,
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
        left: &crate::declaration_selection::ProductDeclarationRequest,
        right: &crate::declaration_selection::ProductDeclarationRequest,
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
#[cfg(test)]
mod declaration_lookup_tests;
