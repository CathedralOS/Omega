use crate::{
    data, domain, expression, invariant, machine, measure, platform, signature, snapshot,
    trait_definition, types, wire,
};
use omega_core::arena::{Arena, HandleSpan};
use omega_core::diagnostics::PhaseSnapshot;
use omega_core::symbols::SymbolTable;
use std::ops::{Deref, DerefMut};

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TypedTrees {
    pub roots: TypedTreeRoots,
    pub tables: TypedTreeTables,
    pub symbols: SymbolTable,
    /// Validated layout plans for PLAN-LAID VALUE TYPES (`gdt: CLayout<Gdt>`
    /// in type position; programmable-layouts L4). Populated by the compiler
    /// pipeline AFTER build-time plan evaluation + validation; the native
    /// layout builder places the named data definitions at these offsets
    /// instead of running its own packing. Empty for programs with no
    /// plan-laid fields.
    pub plan_laid_layouts: Vec<PlanLaidLayout>,
}

/// One validated, FULLY-STATIC layout plan applied to a synthesized data
/// definition (the compiler-generated `Policy<Schema>` instance). Offsets are
/// per field in declaration order; the plan was validated (bounds, overlap,
/// alignment) before it was recorded here, so the layout builder may trust it.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PlanLaidLayout {
    /// Name of the synthesized data definition (e.g. `CLayout<GdtEntryish>`).
    pub data_name: String,
    /// Byte offset of each field, in declaration order.
    pub offsets: Vec<usize>,
    /// Total value size (fixed by the value-type gate).
    pub size: usize,
    pub align: usize,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TypedTreeTables {
    pub data_definitions: Arena<data::DataDefinition>,
    pub data_type_parameters: Arena<data::TypeParameter>,
    pub data_members: Arena<data::DataMember>,
    pub data_payload_fields: Arena<data::DataField>,
    pub domain_definitions: Arena<domain::DomainDefinition>,
    pub proof_facts: Arena<domain::ProofFact>,
    pub domain_path_members: Arena<crate::name::Identifier>,
    pub operator_path_members: Arena<crate::name::Identifier>,
    pub invariant_definitions: Arena<invariant::InvariantDefinition>,
    pub machines: Arena<machine::Machine>,
    pub measures: Arena<measure::MeasureDefinition>,
    pub measure_path_members: Arena<crate::name::Identifier>,
    pub operators: Arena<crate::operator::OperatorDefinition>,
    pub machine_contained_objects: Arena<machine::ContainedObject>,
    pub machine_owned_data: Arena<machine::OwnedData>,
    pub machine_trait_conformances: Arena<machine::TraitConformance>,
    pub machine_states: Arena<crate::state::State>,
    pub state_parameters: Arena<signature::StateParameter>,
    pub platforms: Arena<platform::Platform>,
    pub platform_state_signatures: Arena<signature::StateSignature>,
    pub traits: Arena<trait_definition::TraitDefinition>,
    pub data_conformances: Arena<trait_definition::DataConformance>,
    pub trait_requirements: Arena<trait_definition::TraitRequirement>,
    pub trait_machine_signatures: Arena<signature::StateSignature>,
    pub signature_effects: Arena<crate::name::Identifier>,
    pub signature_contracts: Arena<signature::SignatureContract>,
    pub expression_table: expression::ExpressionTable,
    pub statement_table: crate::statement::StatementTable,
    pub type_reference_table: types::TypeReferenceTable,
    pub wire_schemas: Arena<wire::WireSchema>,
    pub wire_members: Arena<wire::WireMember>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TypedTreeRoots {
    pub data_definitions: HandleSpan<data::DataDefinition>,
    pub domain_definitions: HandleSpan<domain::DomainDefinition>,
    pub invariant_definitions: HandleSpan<invariant::InvariantDefinition>,
    pub machines: HandleSpan<machine::Machine>,
    pub measures: HandleSpan<measure::MeasureDefinition>,
    pub operators: HandleSpan<crate::operator::OperatorDefinition>,
    pub platforms: HandleSpan<platform::Platform>,
    pub traits: HandleSpan<trait_definition::TraitDefinition>,
    pub data_conformances: HandleSpan<trait_definition::DataConformance>,
    pub wire_schemas: HandleSpan<wire::WireSchema>,
}

impl TypedTreeRoots {
    pub fn with_roots(
        data_definitions: HandleSpan<data::DataDefinition>,
        domain_definitions: HandleSpan<domain::DomainDefinition>,
        invariant_definitions: HandleSpan<invariant::InvariantDefinition>,
        machines: HandleSpan<machine::Machine>,
        operators: HandleSpan<crate::operator::OperatorDefinition>,
        platforms: HandleSpan<platform::Platform>,
        traits: HandleSpan<trait_definition::TraitDefinition>,
    ) -> Self {
        Self {
            data_definitions,
            domain_definitions,
            invariant_definitions,
            machines,
            measures: HandleSpan::default(),
            operators,
            platforms,
            traits,
            data_conformances: HandleSpan::default(),
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
            plan_laid_layouts: Vec::new(),
        }
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

    pub fn domain_path_members(
        &self,
        span: HandleSpan<crate::name::Identifier>,
    ) -> &[crate::name::Identifier] {
        self.domain_path_members.span_or_empty(span)
    }

    pub fn push_invariant_definition(
        &mut self,
        invariant_definition: invariant::InvariantDefinition,
    ) {
        self.tables
            .invariant_definitions
            .append_to_span(&mut self.roots.invariant_definitions, invariant_definition);
    }

    pub fn invariant_definitions(&self) -> &[invariant::InvariantDefinition] {
        self.tables
            .invariant_definitions
            .span_or_empty(self.roots.invariant_definitions)
    }

    pub fn push_platform(&mut self, platform: platform::Platform) {
        self.tables
            .platforms
            .append_to_span(&mut self.roots.platforms, platform);
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

    pub fn platforms(&self) -> &[platform::Platform] {
        self.tables.platforms.span_or_empty(self.roots.platforms)
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
    /// `Schema::encode_wire(&value, &mut out, &mut written)` (chapter 20,
    /// wire stage 2a): a statement call whose receiver path is exactly one
    /// member naming a wire schema and whose target is `encode_wire`.
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
    /// `Schema::decode_wire(&mut value, &buffer, &mut read, &mut ok)`
    /// (chapter 20, wire stage 2b): a statement call whose receiver path is
    /// exactly one member naming a wire schema and whose target is
    /// `decode_wire`.
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
    pub fn wire_field_nested_schema(
        &self,
        field: &wire::WireField,
    ) -> Option<&wire::WireSchema> {
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

    /// A wire field's REPEATED encoding (chapter 20): `[scalar; max]` with a
    /// stage 2 scalar element (i32/i64/u32/u64/bool) and a positive literal
    /// maximum. `None` for plain fields AND for array fields whose element is
    /// not an encodable scalar (the caller rejects those with its own
    /// diagnostic -- repeated String / repeated nested message are not
    /// supported).
    pub fn wire_field_repeated_encoding(
        &self,
        field: &wire::WireField,
    ) -> Option<wire::WireRepeatedEncoding> {
        let (element_type, max_count) = self.wire_field_fixed_array(field)?;
        if max_count == 0 {
            return None;
        }
        let element = self
            .primitive_type_reference(element_type)
            .and_then(wire::WireScalarEncoding::for_primitive)?;
        Some(wire::WireRepeatedEncoding { element, max_count })
    }

    /// The worst-case byte count of a schema's CURRENT-era body WITHOUT the
    /// era discriminator -- the sub-message framing a nested message field
    /// carries (chapter 20, decision 10: the era rides only the top-level
    /// envelope, never a nested struct). `Some` only when every current-era
    /// field is a plain stage 2 scalar: a String body is runtime-unbounded
    /// and a doubly-nested body is a deeper composition, so both make the
    /// caller reject with its own diagnostic.
    pub fn wire_schema_scalar_body_worst_case(
        &self,
        schema: &wire::WireSchema,
    ) -> Option<usize> {
        let mut worst_case_bytes = 0usize;
        for member in self.wire_members(schema.members) {
            let wire::WireMember::Field(field) = member else {
                continue;
            };
            if field.number < 0 {
                return None;
            }
            let scalar = self
                .primitive_type_reference(field.type_reference)
                .and_then(wire::WireScalarEncoding::for_primitive)?;
            worst_case_bytes += wire::wire_varint_bytes(field.number as u64).len()
                + scalar.max_varint_length();
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

    pub fn push_data_conformance(&mut self, conformance: trait_definition::DataConformance) {
        self.tables
            .data_conformances
            .append_to_span(&mut self.roots.data_conformances, conformance);
    }

    pub fn data_conformances(&self) -> &[trait_definition::DataConformance] {
        self.tables
            .data_conformances
            .span_or_empty(self.roots.data_conformances)
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

    pub fn push_trait_invariant(
        &mut self,
        trait_definition: &mut trait_definition::TraitDefinition,
        fact: domain::ProofFact,
    ) {
        self.proof_facts
            .append_to_span(&mut trait_definition.invariants, fact);
    }

    pub fn trait_invariants(
        &self,
        trait_definition: &trait_definition::TraitDefinition,
    ) -> &[domain::ProofFact] {
        self.proof_facts.span_or_empty(trait_definition.invariants)
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

    pub fn push_platform_state_signature(
        &mut self,
        platform: &mut platform::Platform,
        signature: signature::StateSignature,
    ) {
        self.platform_state_signatures
            .append_to_span(&mut platform.states, signature);
    }

    pub fn platform_state_signatures(
        &self,
        platform: &platform::Platform,
    ) -> &[signature::StateSignature] {
        self.platform_state_signatures
            .span_or_empty(platform.states)
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

    pub fn push_machine_contained_object(
        &mut self,
        machine: &mut machine::Machine,
        contained_object: machine::ContainedObject,
    ) {
        self.machine_contained_objects
            .append_to_span(&mut machine.contains, contained_object);
    }

    pub fn machine_contained_objects(
        &self,
        machine: &machine::Machine,
    ) -> &[machine::ContainedObject] {
        self.machine_contained_objects
            .span_or_empty(machine.contains)
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

    /// The DATA symbol of the UNIQUE data type that structurally satisfies the
    /// (non-boundary) trait `trait_symbol` -- i.e. for every trait method there is
    /// a machine attached to that data type with a state of that name. Returns
    /// `None` if zero or more than one data types satisfy it, or `trait_symbol` is
    /// not such a trait. Used to DEVIRTUALIZE a `dyn Trait` receiver to its single
    /// concrete implementation (Omega has no runtime vtable). The conformance
    /// registry (`Machine::satisfies`) is not populated, so satisfaction is
    /// computed structurally here.
    pub fn single_trait_impl_data_symbol(
        &self,
        trait_symbol: omega_core::symbols::SymbolHandle,
    ) -> Option<omega_core::symbols::SymbolHandle> {
        let mut implementations = self.trait_impl_data_symbols(trait_symbol).into_iter();
        let single = implementations.next()?;
        implementations.next().is_none().then_some(single)
    }

    /// ALL data types that structurally satisfy the (non-boundary) trait
    /// `trait_symbol`, in data-definition order. The CLOSED WORLD of a
    /// `dyn Trait`: whole-program compilation knows every satisfying type, so a
    /// dyn receiver call site can be monomorphized over this list. Empty when
    /// the symbol is not such a trait or nothing satisfies it.
    pub fn trait_impl_data_symbols(
        &self,
        trait_symbol: omega_core::symbols::SymbolHandle,
    ) -> Vec<omega_core::symbols::SymbolHandle> {
        if !trait_symbol.is_valid() {
            return Vec::new();
        }
        let Some(trait_definition) = self.traits().iter().find(|t| t.symbol == trait_symbol)
        else {
            return Vec::new();
        };
        if trait_definition.is_boundary {
            return Vec::new();
        }
        let signatures = self.trait_machine_signatures(trait_definition);
        if signatures.is_empty() {
            return Vec::new();
        }
        self.data_definitions()
            .iter()
            .filter(|data| {
                signatures.iter().all(|signature| {
                    self.machines().iter().any(|machine| {
                        machine.attached_data.as_ref() == Some(&data.name)
                            && self
                                .machine_states(machine)
                                .iter()
                                .any(|state| state.name == signature.name)
                    })
                })
            })
            .map(|data| data.symbol)
            .collect()
    }

    pub fn push_machine_effect(
        &mut self,
        machine: &mut machine::Machine,
        effect: crate::name::Identifier,
    ) {
        self.signature_effects
            .append_to_span(&mut machine.effects, effect);
    }

    pub fn machine_effects(&self, machine: &machine::Machine) -> &[crate::name::Identifier] {
        self.signature_effects.span_or_empty(machine.effects)
    }

    pub fn machine_decrease_order(
        &self,
        span: HandleSpan<crate::name::Identifier>,
    ) -> &[crate::name::Identifier] {
        self.signature_effects.span_or_empty(span)
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

    pub fn push_state_signature_effect(
        &mut self,
        signature: &mut signature::StateSignature,
        effect: crate::name::Identifier,
    ) {
        self.signature_effects
            .append_to_span(&mut signature.effects, effect);
    }

    pub fn state_signature_effects(
        &self,
        signature: &signature::StateSignature,
    ) -> &[crate::name::Identifier] {
        self.signature_effects.span_or_empty(signature.effects)
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

    /// True when the type is a borrowed byte slice `&[u8]` -- the honest
    /// zero-copy raw-bytes/text view (a length-prefixed buffer window), as
    /// opposed to an owned `[u8; N]` repeated field. Replaces the retired
    /// `&string` for borrowed wire text.
    pub fn is_borrowed_byte_slice(&self, type_reference: types::TypeReferenceHandle) -> bool {
        self.type_reference_table.is_borrowed_byte_slice(type_reference)
    }

    /// The arithmetic domain (`T in Wrapping/Saturating/Trapping`, decision 17)
    /// declared on a type reference; `Exact` when unconstrained.
    pub fn arithmetic_domain_for_type_reference(
        &self,
        type_reference: types::TypeReferenceHandle,
    ) -> omega_core::arithmetic::ArithmeticDomain {
        self.type_reference_table.arithmetic_domain(type_reference)
    }

    pub fn type_reference_symbol(
        &self,
        type_reference: types::TypeReferenceHandle,
    ) -> omega_core::symbols::SymbolHandle {
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
        TypedTreeRoots, TypedTreeTables, TypedTrees, data, domain, invariant, machine, operator,
        platform, trait_definition,
    };
    use omega_core::arena::HandleSpan;
    use omega_core::symbols::SymbolTable;

    #[test]
    fn typed_tree_roots_constructor_keeps_top_level_roots_explicit() {
        let data_definitions = HandleSpan::<data::DataDefinition>::default();
        let domain_definitions = HandleSpan::<domain::DomainDefinition>::default();
        let invariant_definitions = HandleSpan::<invariant::InvariantDefinition>::default();
        let machines = HandleSpan::<machine::Machine>::default();
        let operators = HandleSpan::<operator::OperatorDefinition>::default();
        let platforms = HandleSpan::<platform::Platform>::default();
        let traits = HandleSpan::<trait_definition::TraitDefinition>::default();

        let roots = TypedTreeRoots::with_roots(
            data_definitions,
            domain_definitions,
            invariant_definitions,
            machines,
            operators,
            platforms,
            traits,
        );

        assert_eq!(roots.data_definitions, data_definitions);
        assert_eq!(roots.domain_definitions, domain_definitions);
        assert_eq!(roots.invariant_definitions, invariant_definitions);
        assert_eq!(roots.machines, machines);
        assert_eq!(roots.operators, operators);
        assert_eq!(roots.platforms, platforms);
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
}
