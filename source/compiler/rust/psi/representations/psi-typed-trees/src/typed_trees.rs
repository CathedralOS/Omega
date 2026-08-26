use crate::{
    data, domain, expression, machine, measure, name, proposition, signature, snapshot,
    trait_definition, types, wire,
};
use psi_arena::{Arena, HandleSpan};
use psi_diagnostics::PhaseSnapshot;
use psi_symbols::SymbolTable;
use std::ops::{Deref, DerefMut};

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TypedTrees {
    pub roots: TypedTreeRoots,
    pub tables: TypedTreeTables,
    pub symbols: SymbolTable,
    /// EFX: normalized boundary-service declarations and rows, copied from
    /// symbol-resolved trees. All durable service consumers migrate here.
    pub service_reaches: psi_language_semantics::ServiceReachTable,
    pub service_reach_rows: psi_language_semantics::ServiceReachRowTable,
    /// STR4 checked plans, slice 1: the semantic-domain interner, copied
    /// verbatim from the resolved trees.
    pub semantic_domains: psi_language_semantics::SemanticDomainTable,
    /// Exact structured identities for external realization bindings. Typed
    /// conformance and supply rows retain ids into this table, so downstream
    /// consumers never need to rescan source syntax to recover the binding.
    pub external_bindings: psi_language_semantics::ExternalBindingTable,
    /// Validated layout plans for PLAN-LAID VALUE TYPES (`gdt: CLayout<Gdt>`
    /// in type position; programmable-layouts L4). Populated by the compiler
    /// pipeline AFTER build-time plan evaluation + validation; the native
    /// layout builder places the named data definitions at these offsets
    /// instead of running its own packing. Empty for programs with no
    /// plan-laid fields.
    pub plan_laid_layouts: Vec<PlanLaidLayout>,
    /// Canonical source `Placed<P, T>` derivations. Each record binds the
    /// synthetic view/accessor types to the validated placement plan that
    /// selected them, so checking and lowering never reconstruct permissions
    /// from generated names.
    pub placed_view_plans: Vec<PlacedViewPlan>,
    /// Derived wire placements (mint arc rung 2a): one arena for every
    /// schema's placements, referenced by span from `wire_schema_plans` --
    /// arena-backed storage, HandleSpan ownership.
    pub wire_placements: Arena<wire::WirePlacement>,
    pub wire_encode_obligations: Arena<wire::WireEncodeObligation>,
    pub wire_schema_plans: Vec<wire::WireSchemaPlan>,
    /// MP4: deterministic records of generic-machine specializations applied
    /// before checked lowering. The template keeps its declaration symbol;
    /// this record is the cache/audit identity of the concrete argument tuple.
    pub machine_specializations: Vec<MachineSpecialization>,
    /// Canonical calling-policy identities evaluated for concrete boundary
    /// requirements. The key is semantic (boundary trait + requirement
    /// machine), while the policy type/source body is deliberately absent:
    /// only the validated plan fingerprint is public contract material.
    pub boundary_calling_plans: Vec<BoundaryCallingPlanIdentity>,
    /// PDI3 exact operation/algebra selections for proof-static open index
    /// expressions. The expression tree remains the canonical structural
    /// input; these records bind each operator node to the public operation
    /// contract and proved algebra instance that license normalization.
    pub open_index_normalizations: Vec<OpenIndexNormalization>,
    /// Exact owner identity and authored names for erased evidence forwarding;
    /// checked lowering binds both names to checked evidence-term handles.
    pub evidence_forwardings: Vec<EvidenceForwarding>,
    /// Calls whose proof-output lane is bound immediately. The
    /// group itself is proof metadata; a contextual scalar `value` separately
    /// names its corresponding ordinary runtime local/call statement.
    pub proof_output_calls: Vec<ProofOutputCall>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EvidenceForwarding {
    pub machine_symbol: psi_symbols::SymbolHandle,
    pub state_symbol: psi_symbols::SymbolHandle,
    pub statement_index: usize,
    /// Original pre-erasure source coordinate for lexical evidence scope.
    pub source_statement_index: usize,
    pub target: crate::name::Identifier,
    pub source: crate::name::Identifier,
    pub source_conformance: Option<psi_symbols::SymbolHandle>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProofOutputCall {
    pub machine_symbol: psi_symbols::SymbolHandle,
    pub state_symbol: psi_symbols::SymbolHandle,
    pub statement_index: usize,
    /// Pre-erasure coordinate used only to normalize other erased metadata.
    pub source_statement_index: usize,
    /// Exact runtime local statement synthesized for a contextual `value`
    /// result. Proof-only calls have no runtime statement.
    pub runtime_call_statement_index: Option<usize>,
    pub bindings: Box<[ProofOutputSelector]>,
    pub call: crate::expression::ExpressionHandle,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProofOutputSelector {
    pub output_field: crate::name::Identifier,
    pub binding: crate::name::Identifier,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OpenIndexNormalization {
    pub expression: crate::expression::ExpressionHandle,
    pub index_type: crate::types::TypeReferenceHandle,
    pub operations: Vec<OpenIndexOperationSelection>,
    /// Artifact provenance only. It never enters semantic type identity.
    pub normalizer_version: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OpenIndexOperationSelection {
    pub expression: crate::expression::ExpressionHandle,
    pub spelling: psi_language_core::operator_spelling::OperatorSpelling,
    pub operator: psi_symbols::SymbolHandle,
    pub operation_contract_identity: String,
    pub provider: psi_symbols::SymbolHandle,
    pub algebra_trait: psi_symbols::SymbolHandle,
    pub algebra_requirement: String,
    pub algebra_alias: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BoundaryCallingPlanIdentity {
    pub boundary_trait: psi_symbols::SymbolHandle,
    /// Concrete boundary trait argument tuple. These handles are internal
    /// lookup identity only and never enter the published contract hash.
    /// Empty for a non-generic boundary declaration.
    pub boundary_arguments: Vec<crate::types::TypeReferenceHandle>,
    pub requirement_machine: psi_symbols::SymbolHandle,
    pub fingerprint: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlacedViewPlan {
    pub data_name: String,
    /// Exact synthesized placed-view data identity. `data_name` is diagnostic
    /// presentation and must not be used as the semantic join key.
    pub data_symbol: psi_symbols::SymbolHandle,
    pub policy_name: String,
    /// Exact nominal placement-policy data identity.
    pub policy_symbol: psi_symbols::SymbolHandle,
    /// Exact build-time `Policy::plan` machine that produced `placement`.
    pub policy_plan_machine_symbol: psi_symbols::SymbolHandle,
    pub schema_name: String,
    /// Exact source schema identity whose fields the plan interprets.
    pub schema_symbol: psi_symbols::SymbolHandle,
    pub placement: psi_access_plans::ValidatedPlacementPlan,
    pub fields: Vec<PlacedFieldPlan>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlacedFieldPlan {
    pub field_name: String,
    /// Stable numbered identity when the source schema numbers this field.
    /// The spelling remains diagnostic presentation only in that case.
    pub member_identity: Option<u64>,
    pub field_symbol: psi_symbols::SymbolHandle,
    pub accessor_name: String,
    /// Exact synthesized accessor type reference. Shell-aware typed lookup
    /// rejoins through this handle rather than `accessor_name`.
    pub accessor_type: crate::types::TypeReferenceHandle,
    /// Exact generated accessor data definition. `accessor_name` is retained
    /// for diagnostics and source-oriented artifact presentation only.
    pub accessor_data_symbol: psi_symbols::SymbolHandle,
    /// Exact generated operation targets for non-atomic placed accessors.
    /// Atomic operations retain their separate typed carrier and therefore
    /// have no cloned `PlacedField` operation target rows here.
    pub accessor_targets: Vec<PlacedAccessorTarget>,
    pub value_type: crate::types::TypeReferenceHandle,
    pub access: psi_access_plans::FieldAccess,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlacedAccessorTarget {
    pub operation: String,
    pub machine_symbol: psi_symbols::SymbolHandle,
    pub state_symbol: psi_symbols::SymbolHandle,
}

fn named_type_reference_through_shells(
    table: &crate::types::TypeReferenceTable,
    handle: crate::types::TypeReferenceHandle,
) -> Option<crate::types::TypeReferenceHandle> {
    match table.type_reference(handle) {
        crate::types::TypeReferenceNode::Named { .. } => Some(handle),
        crate::types::TypeReferenceNode::Reference { referee, .. } => {
            named_type_reference_through_shells(table, *referee)
        }
        crate::types::TypeReferenceNode::Constrained { base_type, .. } => {
            named_type_reference_through_shells(table, *base_type)
        }
        _ => None,
    }
}

/// One compile-time machine specialization. Const arguments are canonical
/// proof-static identities and static machine arguments are symbols; neither
/// becomes a runtime value. `fingerprint` is normalized from declaration,
/// type, const-value, and machine-path identity rather than arena addresses.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct MachineSpecialization {
    pub template: psi_symbols::SymbolHandle,
    /// Concrete machine instance produced for this substitution. The first
    /// specialization may reuse `template`; later specializations clone it
    /// under a fresh symbol. Consumers must key executable/elaborated work by
    /// this symbol rather than guessing from specialization order.
    pub instance: psi_symbols::SymbolHandle,
    /// Readable source-oriented spellings retained for diagnostics only.
    pub type_arguments: Vec<String>,
    pub const_arguments: Vec<String>,
    /// Ordered canonical static identities used to construct the
    /// specialization fingerprint. These are retained independently from the
    /// readable spellings above and are never reconstructed from them.
    pub type_argument_identities: Vec<String>,
    pub const_argument_identities: Vec<String>,
    pub machine_arguments: Vec<psi_symbols::SymbolHandle>,
    /// Exact package-scoped conformances selected for explicit proof-static
    /// evidence binders. Separate from callable machine arguments.
    pub conformance_arguments: Vec<psi_symbols::SymbolHandle>,
    /// Exact package-scoped conformances selected by unique generic-bound
    /// inference rather than named proof-static evidence at the call site.
    pub inferred_conformance_arguments: Vec<psi_symbols::SymbolHandle>,
    /// Closed, argument-sensitive identities for the selected conformance
    /// family members. Two applications of the same declaration with
    /// different telescopes are distinct even though `conformance_arguments`
    /// contains the same package-scoped declaration symbol.
    pub conformance_applications: Vec<ClosedConformanceApplication>,
    /// The normalized authored template identity captured before in-place
    /// substitution consumes its generic parameter declarations.
    pub template_contract_fingerprint: u64,
    /// The one accepted-fact commitment this instance relies upon. Every
    /// instance points at the same template commitment; none spends a new
    /// grant. `None` for checked templates.
    pub accepted_template_commitment: Option<String>,
    /// Checked contract identities of the selected static machine arguments.
    /// Populated after contract-plan construction and folded into
    /// `fingerprint`, so a selected contract change invalidates the instance.
    pub machine_argument_contract_fingerprints: Vec<u64>,
    /// Semantic identities of the selected closed conformance maps. These
    /// commit to the exact requirement-to-realization rows, not arena handles.
    pub conformance_argument_fingerprints: Vec<u64>,
    pub fingerprint: u64,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ClosedConformanceApplication {
    pub declaration: psi_symbols::SymbolHandle,
    pub lifetime_arguments: Vec<String>,
    pub type_arguments: Vec<String>,
    pub const_arguments: Vec<String>,
    pub machine_arguments: Vec<psi_symbols::SymbolHandle>,
    pub subject_identity: Option<String>,
    pub trait_definition: psi_symbols::SymbolHandle,
    pub trait_arguments: Vec<String>,
    pub rows: Vec<ClosedConformanceRowIdentity>,
    pub fingerprint: u64,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ClosedConformanceRowIdentity {
    pub declaring_trait: psi_symbols::SymbolHandle,
    pub requirement: psi_symbols::SymbolHandle,
    pub realization_machine: psi_symbols::SymbolHandle,
    pub realization_state: psi_symbols::SymbolHandle,
}

/// One validated, FULLY-STATIC layout plan applied to a synthesized data
/// definition (the compiler-generated `Policy<Schema>` instance). Offsets are
/// per field in declaration order; the plan was validated (bounds, overlap,
/// alignment) before it was recorded here, so the layout builder may trust it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlanLaidLayout {
    /// Name of the synthesized data definition (e.g. `CLayout<GdtEntryish>`).
    /// Diagnostic/source-oriented presentation only.
    pub data_name: String,
    /// Exact synthesized data definition whose physical layout this plan owns.
    pub data_symbol: psi_symbols::SymbolHandle,
    /// Exact runtime field identities in the same order as `offsets`.
    pub field_symbols: Vec<psi_symbols::SymbolHandle>,
    /// Exact source schema and runtime field identities reflected into the
    /// synthesized value type.
    pub schema_symbol: psi_symbols::SymbolHandle,
    pub schema_field_symbols: Vec<psi_symbols::SymbolHandle>,
    /// Exact nominal layout policy and build-time plan machine that produced
    /// this geometry. These identities do not grant runtime authority.
    pub policy_symbol: psi_symbols::SymbolHandle,
    pub policy_plan_machine_symbol: psi_symbols::SymbolHandle,
    /// Exact validated target-neutral geometry from which the host-sized
    /// consumer projections below were derived.
    pub validated_layout: psi_layout_plans::LayoutPlanReport,
    /// Source-authored semantic-field-free callback destinations. These rows
    /// intentionally stop at exact declaration identity plus offset; the
    /// selected target calling plan supplies pointer extent and completes
    /// bounds/non-overlap validation before materialization.
    pub private_callback_demands: Vec<psi_layout_plans::PrivateCallbackLayoutDemandReport>,
    /// Byte offset of each field, in declaration order.
    pub offsets: Vec<usize>,
    /// Fragmented scalar fields keyed by declaration-order field index.
    /// Empty for ordinary byte-aligned plans. The compiler's layout
    /// validator has already proved complete source tiling, non-overlapping
    /// destinations, and in-bounds containers.
    pub bit_fields: Vec<PlanLaidBitField>,
    /// Fixed-width integer fields whose physical encoding is narrower than
    /// their semantic carrier. The validated plan has already proved that
    /// every stored bit pattern decodes into the carrier.
    pub integer_fields: Vec<PlanLaidIntegerField>,
    /// Outer fixed-array fields whose validated plan places one complete
    /// compiler-sized element at each destination in one constant-stride
    /// sequence. The ordinary field offset remains the first destination;
    /// consumers use this row only while indexing that outer array.
    pub repeated_fields: Vec<PlanLaidRepeatedField>,
    /// Total value size (fixed by the value-type gate).
    pub size: usize,
    pub align: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PlanLaidRepeatedField {
    pub field_index: usize,
    pub element_stride: usize,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PlanLaidBitField {
    pub field_index: usize,
    pub fragments: Vec<PlanLaidBitFragment>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PlanLaidIntegerField {
    pub field_index: usize,
    pub stored_width_bits: u16,
    pub interpretation: psi_layout_plans::IntegerInterpretation,
    /// Every value admitted by the semantic field type has an encoding at the
    /// stored width. Mutation may truncate only when this validated fact holds.
    pub write_is_total: bool,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct PlanLaidBitFragment {
    pub container_byte_offset: usize,
    pub container_width_bits: u16,
    pub destination_lsb: u16,
    pub source_lsb: u16,
    pub width: u16,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TypedTreeTables {
    /// Package-agnostic authored-selection custody carried verbatim from
    /// symbol resolution. Checked facts join late selections through the
    /// opaque occurrence identities in this ledger, never source rendering.
    authored_declaration_selections:
        psi_language_semantics::declaration_selection::AuthoredDeclarationSelections,
    pub const_declarations: Arena<crate::constant::ConstDeclaration>,
    pub data_definitions: Arena<data::DataDefinition>,
    pub data_type_parameters: Arena<data::TypeParameter>,
    pub data_members: Arena<data::DataMember>,
    pub data_payload_fields: Arena<data::DataField>,
    pub domain_definitions: Arena<domain::DomainDefinition>,
    pub proof_facts: Arena<domain::ProofFact>,
    pub propositions: Arena<proposition::PropositionDefinition>,
    pub proposition_binders: Arena<proposition::PropositionBinder>,
    pub domain_path_members: Arena<crate::name::Identifier>,
    pub operator_path_members: Arena<crate::name::Identifier>,
    pub machines: Arena<machine::Machine>,
    pub measures: Arena<measure::MeasureDefinition>,
    pub measure_path_members: Arena<crate::name::Identifier>,
    pub operators: Arena<crate::operator::OperatorDefinition>,
    pub machine_owned_data: Arena<machine::OwnedData>,
    pub machine_trait_conformances: Arena<machine::TraitConformance>,
    pub machine_states: Arena<crate::state::State>,
    pub state_parameters: Arena<signature::StateParameter>,
    pub traits: Arena<trait_definition::TraitDefinition>,
    pub conformances: Arena<trait_definition::Conformance>,
    pub trait_requirements: Arena<trait_definition::TraitRequirement>,
    pub trait_machine_signatures: Arena<signature::StateSignature>,
    pub signature_invokes: Arena<crate::name::Identifier>,
    pub signature_contracts: Arena<signature::SignatureContract>,
    pub expression_table: expression::ExpressionTable,
    pub statement_table: crate::statement::StatementTable,
    pub type_reference_table: types::TypeReferenceTable,
    pub wire_schemas: Arena<wire::WireSchema>,
    pub wire_members: Arena<wire::WireMember>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TypedTreeRoots {
    pub const_declarations: HandleSpan<crate::constant::ConstDeclaration>,
    pub data_definitions: HandleSpan<data::DataDefinition>,
    pub domain_definitions: HandleSpan<domain::DomainDefinition>,
    pub machines: HandleSpan<machine::Machine>,
    pub measures: HandleSpan<measure::MeasureDefinition>,
    pub operators: HandleSpan<crate::operator::OperatorDefinition>,
    pub propositions: HandleSpan<proposition::PropositionDefinition>,
    pub traits: HandleSpan<trait_definition::TraitDefinition>,
    pub conformances: HandleSpan<trait_definition::Conformance>,
    pub wire_schemas: HandleSpan<wire::WireSchema>,
}

impl TypedTreeRoots {
    pub fn with_roots(
        data_definitions: HandleSpan<data::DataDefinition>,
        domain_definitions: HandleSpan<domain::DomainDefinition>,
        machines: HandleSpan<machine::Machine>,
        operators: HandleSpan<crate::operator::OperatorDefinition>,
        traits: HandleSpan<trait_definition::TraitDefinition>,
    ) -> Self {
        Self {
            const_declarations: HandleSpan::default(),
            data_definitions,
            domain_definitions,
            machines,
            measures: HandleSpan::default(),
            operators,
            propositions: HandleSpan::default(),
            traits,
            conformances: HandleSpan::default(),
            wire_schemas: HandleSpan::default(),
        }
    }
}

impl TypedTrees {
    pub fn with_roots(
        roots: TypedTreeRoots,
        tables: TypedTreeTables,
        symbols: SymbolTable,
    ) -> Self {
        Self {
            roots,
            tables,
            symbols,
            service_reaches: psi_language_semantics::ServiceReachTable::default(),
            service_reach_rows: psi_language_semantics::ServiceReachRowTable::default(),
            semantic_domains: psi_language_semantics::SemanticDomainTable::default(),
            external_bindings: psi_language_semantics::ExternalBindingTable::default(),
            plan_laid_layouts: Vec::new(),
            placed_view_plans: Vec::new(),
            wire_placements: Arena::new(),
            wire_encode_obligations: Arena::new(),
            wire_schema_plans: Vec::new(),
            machine_specializations: Vec::new(),
            boundary_calling_plans: Vec::new(),
            open_index_normalizations: Vec::new(),
            evidence_forwardings: Vec::new(),
            proof_output_calls: Vec::new(),
        }
    }

    pub fn retain_authored_declaration_selections(
        &mut self,
        selections: psi_language_semantics::declaration_selection::AuthoredDeclarationSelections,
    ) {
        self.tables.authored_declaration_selections = selections;
    }

    pub fn authored_declaration_selections(
        &self,
    ) -> &psi_language_semantics::declaration_selection::AuthoredDeclarationSelections {
        &self.tables.authored_declaration_selections
    }

    pub fn push_const_declaration(&mut self, declaration: crate::constant::ConstDeclaration) {
        self.tables
            .const_declarations
            .append_to_span(&mut self.roots.const_declarations, declaration);
    }

    pub fn const_declarations(&self) -> &[crate::constant::ConstDeclaration] {
        self.tables
            .const_declarations
            .span_or_empty(self.roots.const_declarations)
    }

    pub fn record_resolved_authored_declaration_selection_once(
        &mut self,
        source_span: psi_source::SourceSpan,
        exposure: psi_language_semantics::declaration_selection::AuthoredDeclarationSelectionExposure,
        kind: psi_language_semantics::declaration_selection::AuthoredDeclarationSelectionKind,
        selected_symbol: psi_symbols::SymbolHandle,
    ) -> Result<
        psi_language_semantics::declaration_selection::AuthoredDeclarationSelectionOccurrenceId,
        psi_language_semantics::declaration_selection::AuthoredDeclarationSelectionRecordError,
    > {
        if let Some(existing) = self
            .tables
            .authored_declaration_selections
            .iter()
            .find(|selection| {
                selection.source_span() == source_span
                    && selection.exposure() == exposure
                    && selection.kind() == kind
                    && matches!(
                        selection.target(),
                        psi_language_semantics::declaration_selection::AuthoredDeclarationSelectionTarget::Resolved(target)
                            if target.selected_symbol() == selected_symbol
                    )
            })
        {
            return Ok(existing.occurrence_id());
        }
        self.tables.authored_declaration_selections.record_resolved(
            source_span,
            exposure,
            kind,
            selected_symbol,
        )
    }

    /// Record a schema's derived wire plan: placements land contiguously in
    /// the placement arena; the plan holds their span.
    pub fn record_wire_schema_plan(
        &mut self,
        schema: psi_symbols::SymbolHandle,
        placements: impl IntoIterator<Item = wire::WirePlacement>,
        encode_obligations: impl IntoIterator<Item = wire::WireEncodeObligation>,
    ) {
        let span = self.wire_placements.insert_many(placements);
        let obligations = self.wire_encode_obligations.insert_many(encode_obligations);
        self.wire_schema_plans.push(wire::WireSchemaPlan {
            schema,
            placements: span,
            encode_obligations: obligations,
        });
    }

    /// The derived wire plan for a schema, when one was computed -- the
    /// placements in tag order. `None` for schemas the plan pass skipped.
    pub fn wire_schema_plan(
        &self,
        schema: psi_symbols::SymbolHandle,
    ) -> Option<&[wire::WirePlacement]> {
        self.wire_schema_plans
            .iter()
            .find(|plan| plan.schema == schema)
            .and_then(|plan| self.wire_placements.span(plan.placements))
    }

    /// Dynamic encode obligations retained beside one schema's placements.
    pub fn wire_schema_encode_obligations(
        &self,
        schema: psi_symbols::SymbolHandle,
    ) -> Option<&[wire::WireEncodeObligation]> {
        self.wire_schema_plans
            .iter()
            .find(|plan| plan.schema == schema)
            .and_then(|plan| self.wire_encode_obligations.span(plan.encode_obligations))
    }

    pub fn push_data_definition(&mut self, data_definition: data::DataDefinition) {
        self.tables
            .data_definitions
            .append_to_span(&mut self.roots.data_definitions, data_definition);
    }

    pub fn data_definitions(&self) -> &[data::DataDefinition] {
        self.tables
            .data_definitions
            .span_or_empty(self.roots.data_definitions)
    }

    pub fn push_data_type_parameter(
        &mut self,
        data_definition: &mut data::DataDefinition,
        type_parameter: data::TypeParameter,
    ) {
        self.data_type_parameters
            .append_to_span(&mut data_definition.type_parameters, type_parameter);
    }

    pub fn data_type_parameters(
        &self,
        data_definition: &data::DataDefinition,
    ) -> &[data::TypeParameter] {
        self.data_type_parameters
            .span_or_empty(data_definition.type_parameters)
    }

    pub fn push_data_member(
        &mut self,
        data_definition: &mut data::DataDefinition,
        member: data::DataMember,
    ) {
        self.data_members
            .append_to_span(&mut data_definition.members, member);
    }

    pub fn data_members(&self, data_definition: &data::DataDefinition) -> &[data::DataMember] {
        self.data_members.span_or_empty(data_definition.members)
    }

    pub fn push_data_payload_field(
        &mut self,
        variant: &mut data::DataVariant,
        field: data::DataField,
    ) {
        self.data_payload_fields
            .append_to_span(&mut variant.payload, field);
    }

    pub fn data_payload_fields(&self, variant: &data::DataVariant) -> &[data::DataField] {
        self.data_payload_fields.span_or_empty(variant.payload)
    }

    pub fn push_domain_definition(&mut self, domain_definition: domain::DomainDefinition) {
        self.tables
            .domain_definitions
            .append_to_span(&mut self.roots.domain_definitions, domain_definition);
    }

    pub fn push_domain_type_parameter(
        &mut self,
        domain: &mut domain::DomainDefinition,
        parameter: data::TypeParameter,
    ) {
        self.data_type_parameters
            .append_to_span(&mut domain.type_parameters, parameter);
    }

    pub fn domain_type_parameters(
        &self,
        domain: &domain::DomainDefinition,
    ) -> &[data::TypeParameter] {
        self.data_type_parameters
            .span_or_empty(domain.type_parameters)
    }

    pub fn push_domain_operator(
        &mut self,
        domain: &mut domain::DomainDefinition,
        operator: crate::operator::OperatorDefinition,
    ) {
        self.operators
            .append_to_span(&mut domain.operators, operator);
    }

    pub fn domain_definitions(&self) -> &[domain::DomainDefinition] {
        self.tables
            .domain_definitions
            .span_or_empty(self.roots.domain_definitions)
    }

    pub fn domain_operators(
        &self,
        domain: &domain::DomainDefinition,
    ) -> &[crate::operator::OperatorDefinition] {
        self.operators.span_or_empty(domain.operators)
    }

    pub fn proof_facts(&self, domain: &domain::DomainDefinition) -> &[domain::ProofFact] {
        self.proof_facts.span_or_empty(domain.facts)
    }

    pub fn push_proposition(&mut self, proposition: proposition::PropositionDefinition) {
        self.tables
            .propositions
            .append_to_span(&mut self.roots.propositions, proposition);
    }

    pub fn propositions(&self) -> &[proposition::PropositionDefinition] {
        self.tables
            .propositions
            .span_or_empty(self.roots.propositions)
    }

    pub fn proposition_binders(
        &self,
        proposition: &proposition::PropositionDefinition,
    ) -> &[proposition::PropositionBinder] {
        self.tables
            .proposition_binders
            .span_or_empty(proposition.binders)
    }

    pub fn push_proposition_binder(
        &mut self,
        proposition: &mut proposition::PropositionDefinition,
        binder: proposition::PropositionBinder,
    ) {
        self.tables
            .proposition_binders
            .append_to_span(&mut proposition.binders, binder);
    }

    pub fn proposition_parameters(
        &self,
        proposition: &proposition::PropositionDefinition,
    ) -> &[signature::StateParameter] {
        self.tables
            .state_parameters
            .span_or_empty(proposition.parameters)
    }

    pub fn push_proposition_parameter(
        &mut self,
        proposition: &mut proposition::PropositionDefinition,
        parameter: signature::StateParameter,
    ) {
        self.tables
            .state_parameters
            .append_to_span(&mut proposition.parameters, parameter);
    }

    pub fn domain_path_members(
        &self,
        span: HandleSpan<crate::name::Identifier>,
    ) -> &[crate::name::Identifier] {
        self.domain_path_members.span_or_empty(span)
    }

    pub fn push_measure(&mut self, measure: measure::MeasureDefinition) {
        self.tables
            .measures
            .append_to_span(&mut self.roots.measures, measure);
    }

    pub fn measures(&self) -> &[measure::MeasureDefinition] {
        self.tables.measures.span_or_empty(self.roots.measures)
    }

    pub fn push_measure_path_member(
        &mut self,
        measure: &mut measure::MeasureDefinition,
        member: crate::name::Identifier,
    ) {
        self.measure_path_members
            .append_to_span(&mut measure.name, member);
    }

    pub fn measure_path_members(
        &self,
        span: HandleSpan<crate::name::Identifier>,
    ) -> &[crate::name::Identifier] {
        self.measure_path_members.span_or_empty(span)
    }

    pub fn push_operator(&mut self, operator: crate::operator::OperatorDefinition) {
        self.tables
            .operators
            .append_to_span(&mut self.roots.operators, operator);
    }

    pub fn operators(&self) -> &[crate::operator::OperatorDefinition] {
        self.tables.operators.span_or_empty(self.roots.operators)
    }

    pub fn push_operator_path_member(
        &mut self,
        operator: &mut crate::operator::OperatorDefinition,
        member: crate::name::Identifier,
    ) {
        self.operator_path_members
            .append_to_span(&mut operator.name, member);
    }

    pub fn push_operator_type_parameter(
        &mut self,
        operator: &mut crate::operator::OperatorDefinition,
        parameter: data::TypeParameter,
    ) {
        self.data_type_parameters
            .append_to_span(&mut operator.type_parameters, parameter);
    }

    pub fn operator_type_parameters(
        &self,
        operator: &crate::operator::OperatorDefinition,
    ) -> &[data::TypeParameter] {
        self.data_type_parameters
            .span_or_empty(operator.type_parameters)
    }

    pub fn push_operator_parameter(
        &mut self,
        operator: &mut crate::operator::OperatorDefinition,
        parameter: signature::StateParameter,
    ) {
        self.state_parameters
            .append_to_span(&mut operator.parameters, parameter);
    }

    pub fn operator_parameters(
        &self,
        operator: &crate::operator::OperatorDefinition,
    ) -> &[signature::StateParameter] {
        self.state_parameters.span_or_empty(operator.parameters)
    }

    pub fn push_operator_contract(
        &mut self,
        operator: &mut crate::operator::OperatorDefinition,
        contract: signature::SignatureContract,
    ) {
        self.signature_contracts
            .append_to_span(&mut operator.contracts, contract);
    }

    pub fn operator_contracts(
        &self,
        operator: &crate::operator::OperatorDefinition,
    ) -> &[signature::SignatureContract] {
        self.signature_contracts.span_or_empty(operator.contracts)
    }

    pub fn operator_path_members(
        &self,
        span: HandleSpan<crate::name::Identifier>,
    ) -> &[crate::name::Identifier] {
        self.operator_path_members.span_or_empty(span)
    }

    pub fn push_wire_schema(&mut self, wire_schema: wire::WireSchema) {
        self.tables
            .wire_schemas
            .append_to_span(&mut self.roots.wire_schemas, wire_schema);
    }

    pub fn wire_schemas(&self) -> &[wire::WireSchema] {
        self.tables
            .wire_schemas
            .span_or_empty(self.roots.wire_schemas)
    }

    pub fn append_wire_members(
        &mut self,
        members: Vec<wire::WireMember>,
    ) -> HandleSpan<wire::WireMember> {
        self.tables.wire_members.insert_many(members)
    }

    pub fn wire_members(&self, span: HandleSpan<wire::WireMember>) -> &[wire::WireMember] {
        self.tables.wire_members.span_or_empty(span)
    }

    /// Recognize the compiler-synthesized wire encoder call shape
    /// `Schema::encode(&value, &mut out, &mut written)` (chapter 20,
    /// wire stage 2a): a statement call whose receiver path is exactly one
    /// member naming a wire schema and whose target is `encode`.
    pub fn wire_encode_call_schema(
        &self,
        call: &crate::statement::TableCall,
    ) -> Option<&wire::WireSchema> {
        if call.target.as_str() != wire::WIRE_ENCODE_MACHINE_NAME {
            return None;
        }
        let [schema_name] = self.statement_table.name_path_members(call.receiver) else {
            return None;
        };
        self.wire_schemas()
            .iter()
            .find(|schema| schema.name.as_str() == schema_name.as_str())
    }

    /// Recognize the compiler-synthesized wire decoder call shape
    /// `Schema::decode(&mut value, &buffer, &mut read, &mut ok)`
    /// (chapter 20, wire stage 2b): a statement call whose receiver path is
    /// exactly one member naming a wire schema and whose target is
    /// `decode`.
    pub fn wire_decode_call_schema(
        &self,
        call: &crate::statement::TableCall,
    ) -> Option<&wire::WireSchema> {
        if call.target.as_str() != wire::WIRE_DECODE_MACHINE_NAME {
            return None;
        }
        let [schema_name] = self.statement_table.name_path_members(call.receiver) else {
            return None;
        };
        self.wire_schemas()
            .iter()
            .find(|schema| schema.name.as_str() == schema_name.as_str())
    }

    /// The era discriminator a schema's CURRENT body encodes (frozen decision
    /// 10): era 0 is the pre-versioning body, so a schema with no version
    /// blocks encodes era 0; declared version blocks snapshot earlier bodies
    /// in declaration order (the first block is era 0, the next era 1, ...),
    /// which leaves the current body at the era one past the newest block.
    pub fn wire_schema_current_era(&self, schema: &wire::WireSchema) -> u64 {
        self.wire_members(schema.members)
            .iter()
            .filter(|member| matches!(member, wire::WireMember::Version(_)))
            .count() as u64
    }

    /// The era discriminator a declared version block's payloads carry: its
    /// zero-based position in the declaration-ordered version chain.
    pub fn wire_schema_version_era(
        &self,
        schema: &wire::WireSchema,
        version_name: &str,
    ) -> Option<u64> {
        self.wire_members(schema.members)
            .iter()
            .filter_map(|member| match member {
                wire::WireMember::Version(version) => Some(version),
                _ => None,
            })
            .position(|version| version.name.as_str() == version_name)
            .map(|position| position as u64)
    }

    /// The sibling wire schema a wire field's type references (a NESTED
    /// MESSAGE field, chapter 20), unwrapped through reference and constraint
    /// shells. `None` for primitives and ordinary program types.
    pub fn wire_field_nested_schema(&self, field: &wire::WireField) -> Option<&wire::WireSchema> {
        let name = self.named_type_reference_name(field.type_reference)?;
        self.wire_schemas()
            .iter()
            .find(|schema| schema.name.as_str() == name)
    }

    /// The `Named` name underneath reference and constraint shells, if the
    /// type reference bottoms out in one.
    fn named_type_reference_name(
        &self,
        type_reference: types::TypeReferenceHandle,
    ) -> Option<&str> {
        if !type_reference.is_valid() {
            return None;
        }
        match self.type_reference_table.type_reference(type_reference) {
            types::TypeReferenceNode::Reference { referee, .. } => {
                self.named_type_reference_name(*referee)
            }
            types::TypeReferenceNode::Constrained { base_type, .. } => {
                self.named_type_reference_name(*base_type)
            }
            types::TypeReferenceNode::Named { name, .. } => Some(name.as_str()),
            _ => None,
        }
    }

    /// A wire field's FIXED-ARRAY shape (`[element; max]`), unwrapped through
    /// reference and constraint shells: the element type reference and the
    /// literal maximum. `None` for non-array fields and for const-parameter
    /// lengths (a wire schema is a standalone contract -- its maximum must be
    /// a literal).
    pub fn wire_field_fixed_array(
        &self,
        field: &wire::WireField,
    ) -> Option<(types::TypeReferenceHandle, usize)> {
        let mut handle = field.type_reference;
        loop {
            if !handle.is_valid() {
                return None;
            }
            match self.type_reference_table.type_reference(handle) {
                types::TypeReferenceNode::Reference { referee, .. } => handle = *referee,
                types::TypeReferenceNode::Constrained { base_type, .. } => handle = *base_type,
                types::TypeReferenceNode::FixedArray {
                    element_type,
                    length: types::FixedArrayLength::Literal(length),
                } => return Some((*element_type, *length)),
                _ => return None,
            }
        }
    }

    /// `true` when a wire field's type bottoms out in a SLICE (`[element]`)
    /// -- a repeated spelling with no declared maximum, which can never have
    /// a finite worst-case encoding and is rejected at the declaration.
    pub fn wire_field_is_unbounded_slice(&self, field: &wire::WireField) -> bool {
        let mut handle = field.type_reference;
        loop {
            if !handle.is_valid() {
                return false;
            }
            match self.type_reference_table.type_reference(handle) {
                types::TypeReferenceNode::Reference { referee, .. } => handle = *referee,
                types::TypeReferenceNode::Constrained { base_type, .. } => handle = *base_type,
                types::TypeReferenceNode::Slice { .. } => return true,
                _ => return false,
            }
        }
    }

    /// A borrowed/unbounded slice's element type, looking through reference
    /// and constraint shells.
    pub fn wire_field_slice_element(
        &self,
        field: &wire::WireField,
    ) -> Option<types::TypeReferenceHandle> {
        let mut handle = field.type_reference;
        loop {
            if !handle.is_valid() {
                return None;
            }
            match self.type_reference_table.type_reference(handle) {
                types::TypeReferenceNode::Reference { referee, .. } => handle = *referee,
                types::TypeReferenceNode::Constrained { base_type, .. } => handle = *base_type,
                types::TypeReferenceNode::Slice { element_type } => return Some(*element_type),
                _ => return None,
            }
        }
    }

    /// A general borrowed scalar slice supported by compact_binary's
    /// allocation-free encode path. `&[u8]` remains the distinct raw-byte
    /// path because `u8` is not in the packed scalar vocabulary.
    pub fn wire_field_borrowed_scalar_slice_encoding(
        &self,
        field: &wire::WireField,
    ) -> Option<wire::WireBorrowedScalarSliceEncoding> {
        let element = self.wire_field_slice_element(field)?;
        let element = self
            .primitive_type_reference(element)
            .and_then(wire::WireScalarEncoding::for_primitive)?;
        Some(wire::WireBorrowedScalarSliceEncoding { element })
    }

    /// A wire field's bounded repeated carrier. Fixed arrays carry exactly
    /// their static extent; `FixedVec<T, N>` carries a runtime length bounded
    /// by its inline array capacity.
    pub fn wire_field_repeated_carrier(
        &self,
        field: &wire::WireField,
    ) -> Option<(wire::WireRepeatedCarrier, types::TypeReferenceHandle, usize)> {
        if let Some((element, count)) = self.wire_field_fixed_array(field) {
            return Some((wire::WireRepeatedCarrier::FixedArray, element, count));
        }

        let mut handle = field.type_reference;
        loop {
            match self.type_reference_table.type_reference(handle) {
                types::TypeReferenceNode::Reference { referee, .. } => handle = *referee,
                types::TypeReferenceNode::Constrained { base_type, .. } => handle = *base_type,
                types::TypeReferenceNode::Generic {
                    base_name,
                    arguments,
                    ..
                } if base_name.as_str() == "FixedVec" => {
                    let arguments = self.type_reference_table.type_reference_handles(*arguments);
                    let [element, count] = arguments else {
                        return None;
                    };
                    let count = match self.type_reference_table.type_reference(*count) {
                        types::TypeReferenceNode::Named { name, .. } => {
                            name.as_str().parse::<usize>().ok()?
                        }
                        _ => return None,
                    };
                    return Some((wire::WireRepeatedCarrier::FixedVec, *element, count));
                }
                _ => break,
            }
        }

        let name = self.named_type_reference_name(handle)?;
        if !name.starts_with("FixedVec<") {
            return None;
        }
        let data = self
            .data_definitions()
            .iter()
            .find(|data| data.name.as_str() == name)?;
        let mut items = None;
        let mut has_u64_length = false;
        for member in self.data_members(data) {
            let crate::data::DataMember::Field(member) = member else {
                continue;
            };
            match member.name.as_str() {
                "items" => {
                    let mut synthetic = field.clone();
                    synthetic.type_reference = member.type_reference;
                    items = self.wire_field_fixed_array(&synthetic);
                }
                "length" => {
                    has_u64_length = matches!(
                        self.primitive_type_reference(member.type_reference),
                        Some(types::PrimitiveType::U64)
                    );
                }
                _ => {}
            }
        }
        let (element, count) = items?;
        has_u64_length.then_some((wire::WireRepeatedCarrier::FixedVec, element, count))
    }

    /// A wire field's bounded REPEATED encoding with a stage-2 scalar element
    /// (`i32`, `i64`, `u32`, `u64`, or `bool`).
    pub fn wire_field_repeated_encoding(
        &self,
        field: &wire::WireField,
    ) -> Option<wire::WireRepeatedEncoding> {
        let (carrier, element_type, max_count) = self.wire_field_repeated_carrier(field)?;
        let element = self
            .primitive_type_reference(element_type)
            .and_then(wire::WireScalarEncoding::for_primitive)?;
        Some(wire::WireRepeatedEncoding {
            carrier,
            element,
            max_count,
        })
    }

    /// The worst-case byte count of a schema's CURRENT-era body WITHOUT the
    /// era discriminator -- the sub-message framing a nested message field
    /// carries (chapter 20, decision 10: the era rides only the top-level
    /// envelope, never a nested struct). `Some` only when every current-era
    /// field is a plain stage 2 scalar: a String body is runtime-unbounded
    /// and a doubly-nested body is a deeper composition, so both make the
    /// caller reject with its own diagnostic. Erased fields retain schema
    /// identity but do not contribute a tag, value, or boundedness condition.
    pub fn wire_schema_scalar_body_worst_case(&self, schema: &wire::WireSchema) -> Option<usize> {
        let mut worst_case_bytes = 0usize;
        for member in self.wire_members(schema.members) {
            let wire::WireMember::Field(field) = member else {
                continue;
            };
            if field.relevance.is_erased() {
                continue;
            }
            let scalar = self
                .primitive_type_reference(field.type_reference)
                .and_then(wire::WireScalarEncoding::for_primitive)?;
            worst_case_bytes +=
                wire::wire_varint_bytes(field.number).len() + scalar.max_varint_length();
        }
        Some(worst_case_bytes)
    }

    /// The worst-case byte budget a nested message field adds to its parent's
    /// encoding: the sub-message's scalar body worst case plus the LENGTH
    /// varint that prefixes it (the actual length never exceeds the worst
    /// case, so its varint never grows past the worst case's varint). The
    /// field's TAG varint is the caller's to add.
    pub fn wire_nested_field_worst_case(&self, child: &wire::WireSchema) -> Option<usize> {
        let body = self.wire_schema_scalar_body_worst_case(child)?;
        Some(wire::wire_varint_bytes(body as u64).len() + body)
    }

    pub fn push_trait_definition(&mut self, trait_definition: trait_definition::TraitDefinition) {
        self.tables
            .traits
            .append_to_span(&mut self.roots.traits, trait_definition);
    }

    pub fn traits(&self) -> &[trait_definition::TraitDefinition] {
        self.tables.traits.span_or_empty(self.roots.traits)
    }

    pub fn push_conformance(&mut self, conformance: trait_definition::Conformance) {
        self.tables
            .conformances
            .append_to_span(&mut self.roots.conformances, conformance);
    }

    pub fn conformances(&self) -> &[trait_definition::Conformance] {
        self.tables
            .conformances
            .span_or_empty(self.roots.conformances)
    }

    pub fn push_conformance_type_parameter(
        &mut self,
        conformance: &mut trait_definition::Conformance,
        type_parameter: data::TypeParameter,
    ) {
        self.data_type_parameters
            .append_to_span(&mut conformance.type_parameters, type_parameter);
    }

    pub fn conformance_type_parameters(
        &self,
        conformance: &trait_definition::Conformance,
    ) -> &[data::TypeParameter] {
        self.data_type_parameters
            .span_or_empty(conformance.type_parameters)
    }

    pub fn closed_conformance_rows<'conformance>(
        &self,
        conformance: &'conformance trait_definition::Conformance,
    ) -> Option<&'conformance [trait_definition::ConformanceRow]> {
        match &conformance.implementation {
            trait_definition::ConformanceImplementation::AttachedRequirementMachines => None,
            trait_definition::ConformanceImplementation::Closed { rows } => Some(rows),
        }
    }

    pub fn push_trait_type_parameter(
        &mut self,
        trait_definition: &mut trait_definition::TraitDefinition,
        type_parameter: data::TypeParameter,
    ) {
        self.data_type_parameters
            .append_to_span(&mut trait_definition.type_parameters, type_parameter);
    }

    pub fn trait_type_parameters(
        &self,
        trait_definition: &trait_definition::TraitDefinition,
    ) -> &[data::TypeParameter] {
        self.data_type_parameters
            .span_or_empty(trait_definition.type_parameters)
    }

    pub fn push_trait_requirement(
        &mut self,
        trait_definition: &mut trait_definition::TraitDefinition,
        requirement: trait_definition::TraitRequirement,
    ) {
        self.trait_requirements
            .append_to_span(&mut trait_definition.requires, requirement);
    }

    pub fn trait_requirements(
        &self,
        trait_definition: &trait_definition::TraitDefinition,
    ) -> &[trait_definition::TraitRequirement] {
        self.trait_requirements
            .span_or_empty(trait_definition.requires)
    }

    pub fn trait_composition_kind(
        &self,
        requirement: &trait_definition::TraitRequirement,
    ) -> Option<trait_definition::TraitCompositionKind> {
        self.traits()
            .iter()
            .find(|candidate| candidate.symbol == requirement.symbol)
            .map(|candidate| {
                if candidate.is_boundary {
                    trait_definition::TraitCompositionKind::ServiceReach
                } else {
                    trait_definition::TraitCompositionKind::Policy
                }
            })
    }

    pub fn record_boundary_calling_plan(&mut self, identity: BoundaryCallingPlanIdentity) {
        if let Some(existing) = self.boundary_calling_plans.iter_mut().find(|candidate| {
            candidate.boundary_trait == identity.boundary_trait
                && candidate.boundary_arguments == identity.boundary_arguments
                && candidate.requirement_machine == identity.requirement_machine
        }) {
            *existing = identity;
        } else {
            self.boundary_calling_plans.push(identity);
        }
    }

    pub fn boundary_calling_plan_fingerprint(
        &self,
        boundary_trait: psi_symbols::SymbolHandle,
        requirement_machine: psi_symbols::SymbolHandle,
    ) -> Option<u64> {
        self.boundary_calling_plan_fingerprint_for_arguments(
            boundary_trait,
            &[],
            requirement_machine,
        )
    }

    pub fn boundary_calling_plan_fingerprint_for_arguments(
        &self,
        boundary_trait: psi_symbols::SymbolHandle,
        boundary_arguments: &[crate::types::TypeReferenceHandle],
        requirement_machine: psi_symbols::SymbolHandle,
    ) -> Option<u64> {
        self.boundary_calling_plans
            .iter()
            .find(|identity| {
                identity.boundary_trait == boundary_trait
                    && identity.boundary_arguments == boundary_arguments
                    && identity.requirement_machine == requirement_machine
            })
            .map(|identity| identity.fingerprint)
    }

    pub fn push_trait_machine_signature(
        &mut self,
        trait_definition: &mut trait_definition::TraitDefinition,
        signature: signature::StateSignature,
    ) {
        self.trait_machine_signatures
            .append_to_span(&mut trait_definition.machines, signature);
    }

    pub fn trait_machine_signatures(
        &self,
        trait_definition: &trait_definition::TraitDefinition,
    ) -> &[signature::StateSignature] {
        self.trait_machine_signatures
            .span_or_empty(trait_definition.machines)
    }

    pub fn machine_parameter_contract_view<'program>(
        &'program self,
        contract: &'program data::MachineParameterContract,
    ) -> Option<data::MachineParameterContractView<'program>> {
        match contract {
            data::MachineParameterContract::RequirementIdentity => None,
            data::MachineParameterContract::Structural(signature) => {
                Some(data::MachineParameterContractView::Structural(signature))
            }
            data::MachineParameterContract::Nominal {
                trait_definition,
                requirement,
            } => {
                let trait_definition = self
                    .traits()
                    .iter()
                    .find(|candidate| candidate.symbol == *trait_definition)?;
                let requirement = self
                    .trait_machine_signatures(trait_definition)
                    .iter()
                    .find(|candidate| candidate.symbol == *requirement)?;
                Some(data::MachineParameterContractView::Nominal {
                    trait_definition,
                    requirement,
                })
            }
        }
    }

    pub fn push_machine(&mut self, machine: machine::Machine) {
        self.tables
            .machines
            .append_to_span(&mut self.roots.machines, machine);
    }

    pub fn machines(&self) -> &[machine::Machine] {
        self.tables.machines.span_or_empty(self.roots.machines)
    }

    pub fn machines_mut(&mut self) -> &mut [machine::Machine] {
        self.tables.machines.span_mut_or_empty(self.roots.machines)
    }

    pub fn push_machine_type_parameter(
        &mut self,
        machine: &mut machine::Machine,
        type_parameter: data::TypeParameter,
    ) {
        self.data_type_parameters
            .append_to_span(&mut machine.type_parameters, type_parameter);
    }

    pub fn machine_type_parameters(&self, machine: &machine::Machine) -> &[data::TypeParameter] {
        self.data_type_parameters
            .span_or_empty(machine.type_parameters)
    }

    /// The authored callable contract of a compile-time machine parameter in
    /// `machine`. The parameter symbol is also the contract's call-target
    /// identity until specialization replaces it with a concrete state.
    pub fn machine_parameter_signature_in(
        &self,
        machine: &machine::Machine,
        symbol: psi_symbols::SymbolHandle,
    ) -> Option<&signature::StateSignature> {
        self.machine_type_parameters(machine)
            .iter()
            .find_map(|parameter| match &parameter.kind {
                data::TypeParameterKind::Machine { contract } if parameter.symbol == symbol => self
                    .machine_parameter_contract_view(contract)
                    .map(data::MachineParameterContractView::signature),
                _ => None,
            })
    }

    /// Find a machine-parameter contract and its declaring machine by its
    /// normalized symbol. Used by service-reach/proof consumers that see only a call
    /// target, not the lexical generic scope.
    pub fn machine_parameter_signature(
        &self,
        symbol: psi_symbols::SymbolHandle,
    ) -> Option<(&machine::Machine, &signature::StateSignature)> {
        self.machines().iter().find_map(|machine| {
            self.machine_parameter_signature_in(machine, symbol)
                .map(|signature| (machine, signature))
        })
    }

    pub fn push_machine_owned_data(
        &mut self,
        machine: &mut machine::Machine,
        owned_data: machine::OwnedData,
    ) {
        self.machine_owned_data
            .append_to_span(&mut machine.owned_data, owned_data);
    }

    pub fn machine_owned_data(&self, machine: &machine::Machine) -> &[machine::OwnedData] {
        self.machine_owned_data.span_or_empty(machine.owned_data)
    }

    pub fn push_machine_trait_conformance(
        &mut self,
        machine: &mut machine::Machine,
        conformance: machine::TraitConformance,
    ) {
        self.machine_trait_conformances
            .append_to_span(&mut machine.satisfies, conformance);
    }

    pub fn machine_trait_conformances(
        &self,
        machine: &machine::Machine,
    ) -> &[machine::TraitConformance] {
        self.machine_trait_conformances
            .span_or_empty(machine.satisfies)
    }

    pub fn push_machine_invoke(
        &mut self,
        machine: &mut machine::Machine,
        binding: crate::name::Identifier,
    ) {
        self.signature_invokes
            .append_to_span(&mut machine.invokes, binding);
    }

    pub fn machine_invokes(&self, machine: &machine::Machine) -> &[crate::name::Identifier] {
        self.signature_invokes.span_or_empty(machine.invokes)
    }

    pub fn push_machine_contract(
        &mut self,
        machine: &mut machine::Machine,
        contract: signature::SignatureContract,
    ) {
        self.signature_contracts
            .append_to_span(&mut machine.contracts, contract);
    }

    pub fn machine_contracts(&self, machine: &machine::Machine) -> &[signature::SignatureContract] {
        self.signature_contracts.span_or_empty(machine.contracts)
    }

    pub fn push_machine_state(
        &mut self,
        machine: &mut machine::Machine,
        state: crate::state::State,
    ) {
        self.machine_states
            .append_to_span(&mut machine.states, state);
    }

    pub fn machine_states(&self, machine: &machine::Machine) -> &[crate::state::State] {
        self.machine_states.span_or_empty(machine.states)
    }

    pub fn machine_states_mut(&mut self, machine: &machine::Machine) -> &mut [crate::state::State] {
        self.machine_states.span_mut_or_empty(machine.states)
    }

    pub fn push_state_parameter(
        &mut self,
        state: &mut crate::state::State,
        parameter: signature::StateParameter,
    ) {
        self.state_parameters
            .append_to_span(&mut state.parameters, parameter);
    }

    pub fn state_parameters(&self, state: &crate::state::State) -> &[signature::StateParameter] {
        self.state_parameters.span_or_empty(state.parameters)
    }

    pub fn push_state_contract(
        &mut self,
        state: &mut crate::state::State,
        contract: signature::SignatureContract,
    ) {
        self.signature_contracts
            .append_to_span(&mut state.contracts, contract);
    }

    pub fn state_contracts(&self, state: &crate::state::State) -> &[signature::SignatureContract] {
        self.signature_contracts.span_or_empty(state.contracts)
    }

    pub fn push_state_signature_parameter(
        &mut self,
        signature: &mut signature::StateSignature,
        parameter: signature::StateParameter,
    ) {
        self.state_parameters
            .append_to_span(&mut signature.parameters, parameter);
    }

    pub fn state_signature_parameters(
        &self,
        signature: &signature::StateSignature,
    ) -> &[signature::StateParameter] {
        self.state_parameters.span_or_empty(signature.parameters)
    }

    pub fn state_signature_type_parameters(
        &self,
        signature: &signature::StateSignature,
    ) -> &[data::TypeParameter] {
        self.data_type_parameters
            .span_or_empty(signature.type_parameters)
    }

    pub fn push_state_signature_invoke(
        &mut self,
        signature: &mut signature::StateSignature,
        binding: crate::name::Identifier,
    ) {
        self.signature_invokes
            .append_to_span(&mut signature.invokes, binding);
    }

    pub fn state_signature_invokes(
        &self,
        signature: &signature::StateSignature,
    ) -> &[crate::name::Identifier] {
        self.signature_invokes.span_or_empty(signature.invokes)
    }

    pub fn push_state_signature_contract(
        &mut self,
        signature: &mut signature::StateSignature,
        contract: signature::SignatureContract,
    ) {
        self.signature_contracts
            .append_to_span(&mut signature.contracts, contract);
    }

    pub fn state_signature_contracts(
        &self,
        signature: &signature::StateSignature,
    ) -> &[signature::SignatureContract] {
        self.signature_contracts.span_or_empty(signature.contracts)
    }

    pub fn display_type_reference(&self, type_reference: types::TypeReferenceHandle) -> String {
        self.type_reference_table.display_name(type_reference)
    }

    pub fn display_type_reference_with_constraints(
        &self,
        type_reference: types::TypeReferenceHandle,
    ) -> String {
        self.type_reference_table
            .display_name_with_constraints(type_reference, &self.expression_table)
    }

    pub fn primitive_type_reference(
        &self,
        type_reference: types::TypeReferenceHandle,
    ) -> Option<types::PrimitiveType> {
        self.type_reference_table.primitive_type(type_reference)
    }

    pub fn placed_field_plan_for_type_reference(
        &self,
        type_reference: types::TypeReferenceHandle,
    ) -> Option<&PlacedFieldPlan> {
        self.placed_view_field_plan_for_type_reference(type_reference)
            .map(|(_, field)| field)
    }

    pub fn placed_view_plan_for_type_reference(
        &self,
        type_reference: types::TypeReferenceHandle,
    ) -> Option<&PlacedViewPlan> {
        let view_symbol = self.type_reference_table.type_symbol(type_reference);
        if !view_symbol.is_valid() {
            return None;
        }
        self.placed_view_plans
            .iter()
            .find(|view| view.data_symbol == view_symbol)
    }

    pub fn placed_view_field_plan_for_type_reference(
        &self,
        type_reference: types::TypeReferenceHandle,
    ) -> Option<(&PlacedViewPlan, &PlacedFieldPlan)> {
        let accessor_type =
            named_type_reference_through_shells(&self.type_reference_table, type_reference)?;
        self.placed_view_plans.iter().find_map(|view| {
            view.fields
                .iter()
                .find(|field| field.accessor_type == accessor_type)
                .map(|field| (view, field))
        })
    }

    pub fn named_type_reference(
        &self,
        type_reference: types::TypeReferenceHandle,
    ) -> Option<&name::Identifier> {
        self.type_reference_table.named_type(type_reference)
    }

    /// True when the type is a borrowed byte slice `&[u8]` -- the honest
    /// zero-copy raw-bytes/text view (a length-prefixed buffer window), as
    /// opposed to an owned `[u8; N]` repeated field. Replaces the retired
    /// `&string` for borrowed wire text.
    pub fn is_borrowed_byte_slice(&self, type_reference: types::TypeReferenceHandle) -> bool {
        self.type_reference_table
            .is_borrowed_byte_slice(type_reference)
    }

    /// The arithmetic domain (`T in Wrapping/Saturating/Trapping`, decision 17)
    /// declared on a type reference; `Exact` when unconstrained.
    pub fn arithmetic_domain_for_type_reference(
        &self,
        type_reference: types::TypeReferenceHandle,
    ) -> psi_numerics::arithmetic::ArithmeticDomain {
        self.type_reference_table.arithmetic_domain(type_reference)
    }

    /// The normalized usage multiplicity of a typed value. This belongs to the
    /// typed representation rather than an individual checker: provider
    /// schemas, ownership checking, and later admission all need the same
    /// answer for constrained, generic, and aggregate carriers.
    pub fn type_multiplicity(
        &self,
        type_reference: types::TypeReferenceHandle,
    ) -> psi_language_semantics::Multiplicity {
        use psi_language_semantics::Multiplicity;
        use types::TypeReferenceNode;

        if !type_reference.is_valid() {
            return Multiplicity::Affine;
        }
        match self.type_reference_table.type_reference(type_reference) {
            TypeReferenceNode::Reference { .. }
            | TypeReferenceNode::ConstExpression(_)
            | TypeReferenceNode::Unit => Multiplicity::Unrestricted,
            TypeReferenceNode::Constrained { base_type, .. } => self.type_multiplicity(*base_type),
            TypeReferenceNode::FixedArray { element_type, .. } => {
                self.type_multiplicity(*element_type)
            }
            TypeReferenceNode::Named { symbol, name } => {
                if let Some(parameter) = self
                    .data_type_parameters
                    .iter()
                    .find_map(|(_, parameter)| (parameter.symbol == *symbol).then_some(parameter))
                {
                    return parameter.bounds.multiplicity;
                }
                if types::PrimitiveType::from_name(name.as_str()).is_some() {
                    return Multiplicity::Unrestricted;
                }
                self.data_definitions()
                    .iter()
                    .find(|definition| definition.name.as_str() == name.as_str())
                    .map(|definition| definition.properties.multiplicity)
                    .unwrap_or(Multiplicity::Affine)
            }
            TypeReferenceNode::Generic { base_name, .. } => self
                .data_definitions()
                .iter()
                .find(|definition| definition.name.as_str() == base_name.as_str())
                .map(|definition| definition.properties.multiplicity)
                .unwrap_or(Multiplicity::Affine),
            TypeReferenceNode::DynamicTrait { .. } | TypeReferenceNode::Slice { .. } => {
                Multiplicity::Affine
            }
        }
    }

    pub fn type_reference_symbol(
        &self,
        type_reference: types::TypeReferenceHandle,
    ) -> psi_symbols::SymbolHandle {
        self.type_reference_table.type_symbol(type_reference)
    }

    pub fn snapshot(&self) -> snapshot::TypedTreesSnapshot {
        snapshot::TypedTreesSnapshot::from_typed_trees(self)
    }

    pub fn snapshot_json(&self) -> Result<String, serde_json::Error> {
        self.snapshot().to_json()
    }

    pub fn snapshot_json_pretty(&self) -> Result<String, serde_json::Error> {
        self.snapshot().to_json_pretty()
    }
}

impl PhaseSnapshot for TypedTrees {
    type Snapshot = snapshot::TypedTreesSnapshot;

    fn snapshot(&self) -> Self::Snapshot {
        TypedTrees::snapshot(self)
    }
}

impl Deref for TypedTrees {
    type Target = TypedTreeTables;

    fn deref(&self) -> &Self::Target {
        &self.tables
    }
}

impl DerefMut for TypedTrees {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.tables
    }
}

#[cfg(test)]
mod tests {
    use crate::{
        TypedTreeRoots, TypedTreeTables, TypedTrees, data, domain, machine, name::Identifier,
        operator, trait_definition, types, wire,
    };
    use psi_arena::HandleSpan;
    use psi_language_core::BindingRelevance;
    use psi_symbols::SymbolTable;

    #[test]
    fn typed_tree_roots_constructor_keeps_top_level_roots_explicit() {
        let data_definitions = HandleSpan::<data::DataDefinition>::default();
        let domain_definitions = HandleSpan::<domain::DomainDefinition>::default();
        let machines = HandleSpan::<machine::Machine>::default();
        let operators = HandleSpan::<operator::OperatorDefinition>::default();
        let traits = HandleSpan::<trait_definition::TraitDefinition>::default();

        let roots = TypedTreeRoots::with_roots(
            data_definitions,
            domain_definitions,
            machines,
            operators,
            traits,
        );

        assert_eq!(roots.data_definitions, data_definitions);
        assert_eq!(roots.domain_definitions, domain_definitions);
        assert_eq!(roots.machines, machines);
        assert_eq!(roots.operators, operators);
        assert_eq!(roots.traits, traits);
    }

    #[test]
    fn typed_trees_constructor_keeps_roots_tables_and_symbols_explicit() {
        let roots = TypedTreeRoots::default();
        let tables = TypedTreeTables::default();
        let symbols = SymbolTable::default();

        let trees = TypedTrees::with_roots(roots.clone(), tables.clone(), symbols.clone());

        assert_eq!(trees.roots, roots);
        assert_eq!(trees.tables, tables);
        assert_eq!(trees.symbols, symbols);
    }

    #[test]
    fn signature_invokes_use_their_dedicated_arena() {
        let mut trees = TypedTrees::default();
        let mut signature_invokes = HandleSpan::empty();

        trees.signature_invokes.append_to_span(
            &mut signature_invokes,
            Identifier::generated("Console.write"),
        );

        assert_eq!(
            trees.signature_invokes.span_or_empty(signature_invokes)[0].as_str(),
            "Console.write"
        );
    }

    #[test]
    fn erased_fields_do_not_contribute_to_wire_scalar_body_size() {
        let mut trees = TypedTrees::default();
        let unsupported = trees
            .type_reference_table
            .insert(types::TypeReferenceNode::Named {
                symbol: Default::default(),
                name: Identifier::generated("Unsupported"),
            });
        let members = trees.append_wire_members(vec![wire::WireMember::Field(wire::WireField {
            number: 127,
            name: Identifier::generated("proof"),
            relevance: BindingRelevance::Erased,
            type_reference: unsupported,
        })]);
        let schema = wire::WireSchema {
            name: Identifier::generated("Message"),
            members,
            ..wire::WireSchema::default()
        };

        assert_eq!(trees.wire_schema_scalar_body_worst_case(&schema), Some(0));
    }
}
