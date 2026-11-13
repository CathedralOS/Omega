/*
We represent symbol-resolved constant, data, domain, measure, operator, trait,
type-alias, wire, and machine-declaration records (`declarations` and
`machine_declarations`) in `SymbolResolvedTrees`.
*/

pub mod declarations {
    pub mod constant {
        use symbols::SymbolHandle;

        #[derive(Debug, Clone, Default, PartialEq, Eq, compiler_storage::Record)]
        pub struct ConstDeclaration {
            pub symbol: SymbolHandle,
            pub is_public: bool,
            pub declared_type: crate::types::TypeReference,
            pub initializer: crate::expression::ExpressionHandle,
            pub authored_initializer: crate::expression::ExpressionHandle,
            pub initializer_source_span: source::SourceSpan,
            pub pending_target_observation: bool,
            pub evaluated_value: Option<language_core::constant_value::EvaluatedConstant>,
        }
    }
    pub mod data {
        use crate::name::DiagnosticName;
        use crate::types::TypeReference;
        use arena::HandleSpan;
        use std::ops::{Deref, DerefMut};
        use symbols::SymbolHandle;

        #[derive(Debug, Clone, Default, PartialEq, Eq, compiler_storage::Record)]
        pub struct DataDefinition {
            pub symbol: SymbolHandle,
            pub name: DiagnosticName,
            pub is_public: bool,
            pub storage: DataDefinitionStorage,
        }

        #[derive(Debug, Clone, Default, PartialEq, Eq, compiler_storage::Record)]
        pub struct DataDefinitionStorage {
            pub predicate_subject_symbol: SymbolHandle,
            pub predicate_subject_type: TypeReference,
            pub supply_mode: language_semantics::DataSupplyMode,
            pub lifetime_parameters: Vec<DiagnosticName>,
            pub type_parameters: HandleSpan<TypeParameter>,
            pub generic_instance: Option<TypeReference>,
            pub conformance_bounds: Vec<crate::machine::GenericConformanceBound>,
            pub properties: DataProperties,
            pub cleanup_selection: Option<crate::expression::StaticMachineArgument>,
            pub quotient: Option<QuotientDefinition>,
            pub requires_facts: HandleSpan<crate::domain::ProofFact>,
            pub zero_gated: bool,
            pub retired_identities: Vec<u64>,
            pub members: HandleSpan<DataMember>,
        }

        #[derive(Debug, Clone, PartialEq, Eq, compiler_storage::Record)]
        pub struct QuotientDefinition {
            pub carrier: TypeReference,
            pub relation: Vec<DiagnosticName>,
            pub relation_symbol: SymbolHandle,
            pub equivalence: Option<QuotientEquivalenceSelection>,
        }

        #[derive(Debug, Clone, PartialEq, Eq, compiler_storage::Record)]
        pub struct QuotientEquivalenceSelection {
            pub relation: Vec<DiagnosticName>,
            pub relation_symbol: SymbolHandle,
            pub trait_name: DiagnosticName,
            pub trait_symbol: SymbolHandle,
            pub trait_arguments: HandleSpan<TypeReference>,
            pub conformance_name: DiagnosticName,
            pub conformance_symbol: SymbolHandle,
        }

        #[derive(Debug, Clone, Copy, Default, PartialEq, Eq, compiler_storage::Record)]
        pub struct DataProperties {
            pub carry: Option<language_semantics::CarryPolicy>,
            pub multiplicity: language_semantics::Multiplicity,
        }

        deref_to_storage!(DataDefinition => DataDefinitionStorage);

        impl DataDefinition {
            pub fn shape_kind_from_members(members: &[DataMember]) -> DataShapeKind {
                let mut has_fields = false;
                let mut has_variants = false;

                for member in members {
                    match member {
                        DataMember::Field(_) => has_fields = true,
                        DataMember::Variant(_) => has_variants = true,
                    }
                }

                match (has_fields, has_variants) {
                    (false, false) => DataShapeKind::Empty,
                    (true, false) => DataShapeKind::Record,
                    (false, true) => DataShapeKind::Enum,
                    (true, true) => DataShapeKind::Mixed,
                }
            }
        }

        #[derive(Debug, Clone, Copy, PartialEq, Eq)]
        pub enum DataShapeKind {
            Empty,
            Enum,
            Mixed,
            Record,
        }

        #[derive(Debug, Clone, PartialEq, Eq, compiler_storage::Record)]
        pub enum DataMember {
            Field(DataField),
            Variant(DataVariant),
        }

        impl Default for DataMember {
            fn default() -> Self {
                Self::Variant(DataVariant::default())
            }
        }

        #[derive(Debug, Clone, Default, PartialEq, Eq, compiler_storage::Record)]
        pub struct Binder<K> {
            pub symbol: SymbolHandle,
            pub name: DiagnosticName,
            pub kind: K,
            pub bounds: DataProperties,
        }

        pub type TypeParameter = Binder<TypeParameterKind>;

        #[derive(Debug, Clone, Default, PartialEq, Eq, compiler_storage::Record)]
        pub enum TypeParameterKind {
            #[default]
            Type,
            Implementation,
            Const {
                type_reference: TypeReference,
            },
            Value {
                type_reference: TypeReference,
            },
            Machine {
                contract: MachineParameterContract,
            },
            Proposition {
                contract: PropositionParameterSignature,
            },
        }

        #[derive(Debug, Clone, PartialEq, Eq, compiler_storage::Record)]
        pub enum MachineParameterContract {
            /// Pending declaration inference; never admitted to typed publication.
            Inferred,
            RequirementIdentity,
            Structural(crate::signature::StateSignature),
            AuthoredNominal {
                requirement: Vec<DiagnosticName>,
                trait_arguments: HandleSpan<crate::types::TypeReference>,
            },
            Nominal {
                trait_definition: SymbolHandle,
                requirement: SymbolHandle,
                authored_path: Vec<DiagnosticName>,
                trait_arguments: HandleSpan<crate::types::TypeReference>,
            },
        }

        impl Default for MachineParameterContract {
            fn default() -> Self {
                Self::Structural(crate::signature::StateSignature::default())
            }
        }

        impl MachineParameterContract {
            pub fn structural(&self) -> Option<&crate::signature::StateSignature> {
                match self {
                    Self::Structural(signature) => Some(signature),
                    Self::Inferred
                    | Self::RequirementIdentity
                    | Self::AuthoredNominal { .. }
                    | Self::Nominal { .. } => None,
                }
            }

            pub fn structural_mut(&mut self) -> Option<&mut crate::signature::StateSignature> {
                match self {
                    Self::Structural(signature) => Some(signature),
                    Self::Inferred
                    | Self::RequirementIdentity
                    | Self::AuthoredNominal { .. }
                    | Self::Nominal { .. } => None,
                }
            }
        }

        #[derive(Debug, Clone, Copy)]
        pub enum MachineParameterContractView<'program> {
            Structural(&'program crate::signature::StateSignature),
            Nominal {
                trait_definition: &'program crate::trait_definition::TraitDefinition,
                requirement: &'program crate::signature::StateSignature,
            },
        }

        impl<'program> MachineParameterContractView<'program> {
            pub fn signature(self) -> &'program crate::signature::StateSignature {
                match self {
                    Self::Structural(signature)
                    | Self::Nominal {
                        requirement: signature,
                        ..
                    } => signature,
                }
            }
        }

        #[derive(Debug, Clone, Default, PartialEq, Eq, compiler_storage::Record)]
        pub struct PropositionParameterSignature {
            pub name: DiagnosticName,
            pub parameters: HandleSpan<crate::signature::StateParameter>,
        }

        #[derive(Debug, Clone, PartialEq, Eq, Default, compiler_storage::Record)]
        pub struct DataField {
            pub identity: Option<u64>,
            pub symbol: SymbolHandle,
            pub name: DiagnosticName,
            pub relevance: language_core::BindingRelevance,
            pub type_reference: TypeReference,
        }

        #[derive(Debug, Clone, Default, PartialEq, Eq, compiler_storage::Record)]
        pub struct DataVariant {
            pub identity: Option<u64>,
            pub symbol: SymbolHandle,
            pub name: DiagnosticName,
            pub payload: HandleSpan<DataField>,
            pub requires_facts: HandleSpan<crate::domain::ProofFact>,
            pub retired_payload_identities: Vec<u64>,
        }
    }
    pub mod domain {
        use crate::name::DiagnosticName;
        use crate::types::TypeReference;
        use arena::HandleSpan;
        use symbols::SymbolHandle;

        #[derive(Debug, Clone, Default, PartialEq, Eq, compiler_storage::Record)]
        pub struct DomainDefinition {
            pub symbol: SymbolHandle,
            pub name: DiagnosticName,
            pub subject_name: DiagnosticName,
            pub subject_symbol: SymbolHandle,
            pub type_parameters: HandleSpan<crate::data::TypeParameter>,
            pub target_type: TypeReference,
            pub index_arguments: HandleSpan<TypeReference>,
            pub is_public: bool,
            pub cleanup_selection: Option<crate::expression::StaticMachineArgument>,
            pub alias: Option<DomainAliasDefinition>,
            pub authored_routes: Vec<Vec<DiagnosticName>>,
            /// The policy named after `access`. Its `symbol` is the named
            /// implementation the path selects, once selection has run.
            pub access_policy: Option<crate::expression::StaticMachineArgument>,
            pub classification: Option<language_semantics::DomainClassification>,
            pub predicate_presence: language_semantics::DomainPredicatePresence,
            pub facts: HandleSpan<ProofFact>,
            pub operators: HandleSpan<crate::operator::OperatorDefinition>,
            pub semantic_clause_token_count: usize,
            pub semantic_id: language_semantics::SemanticDomainId,
            pub semantic_roles: language_semantics::DomainSemanticRoles,
            pub establishment_routes: Vec<language_semantics::DomainEstablishmentRoute>,
        }

        #[derive(Debug, Clone, Default, PartialEq, Eq, compiler_storage::Record)]
        pub struct DomainAliasDefinition {
            pub constituents: Vec<DomainAliasConstituent>,
        }

        #[derive(Debug, Clone, Copy, PartialEq, Eq, Default, compiler_storage::Record)]
        pub struct DomainAliasConstituent {
            pub domain: HandleSpan<DiagnosticName>,
            pub domain_symbol: SymbolHandle,
        }

        #[derive(Debug, Clone, PartialEq, Eq, compiler_storage::Record)]
        pub enum ProofFact {
            Expression(crate::expression::ExpressionHandle),
            Membership(ProofMembershipFact),
        }

        impl Default for ProofFact {
            fn default() -> Self {
                Self::Expression(crate::expression::ExpressionHandle::invalid())
            }
        }

        #[derive(Debug, Clone, Copy, PartialEq, Eq, Default, compiler_storage::Record)]
        pub struct ProofMembershipFact {
            pub value: crate::expression::ExpressionHandle,
            pub domain: HandleSpan<DiagnosticName>,
            pub domain_symbol: SymbolHandle,
            pub domain_arguments: HandleSpan<TypeReference>,
            pub authored_domain_selection: Option<
                language_semantics::declaration_selection::AuthoredDeclarationSelectionOccurrenceId,
            >,
        }
    }
    pub mod measure {
        use crate::expression::ExpressionHandle;
        use crate::name::DiagnosticName;
        use crate::signature::StateParameter;
        use crate::types::TypeReference;
        use arena::HandleSpan;
        use symbols::SymbolHandle;

        #[derive(Debug, Clone, Default, PartialEq, Eq, compiler_storage::Record)]
        pub struct MeasureDefinition {
            pub symbol: SymbolHandle,
            pub name: HandleSpan<DiagnosticName>,
            pub parameter: Option<StateParameter>,
            pub return_type: Option<TypeReference>,
            pub lexicographic: bool,
            pub body: HandleSpan<ExpressionHandle>,
        }
    }
    pub mod operator {
        use crate::name::DiagnosticName;
        use crate::types::TypeReference;
        use arena::HandleSpan;
        use language_core::operator_spelling::OperatorSpelling;
        use symbols::SymbolHandle;

        #[derive(Debug, Clone, Default, PartialEq, Eq, compiler_storage::Record)]
        pub struct OperatorDefinition {
            pub is_public: bool,
            pub is_boundary: bool,
            pub symbol: SymbolHandle,
            pub name: HandleSpan<DiagnosticName>,
            pub lifetime_parameters: Vec<DiagnosticName>,
            pub type_parameters: HandleSpan<crate::data::TypeParameter>,
            pub parameters: HandleSpan<crate::signature::StateParameter>,
            pub return_type: Option<TypeReference>,
            pub contracts: HandleSpan<crate::signature::SignatureContract>,
            pub spelling: Option<OperatorSpelling>,
            pub token_count: usize,
        }
    }
    pub mod trait_definition {
        use crate::name::DiagnosticName;
        use crate::signature::StateSignature;
        use arena::HandleSpan;
        use std::ops::{Deref, DerefMut};
        use symbols::SymbolHandle;

        #[derive(Debug, Clone, Default, PartialEq, Eq, compiler_storage::Record)]
        pub struct TraitDefinition {
            pub symbol: SymbolHandle,
            pub is_boundary: bool,
            pub is_public: bool,
            pub name: DiagnosticName,
            pub storage: TraitStorage,
        }

        #[derive(Debug, Clone, Default, PartialEq, Eq, compiler_storage::Record)]
        pub struct TraitStorage {
            pub lifetime_parameters: Vec<DiagnosticName>,
            pub type_parameters: HandleSpan<crate::data::TypeParameter>,
            pub conformance_bounds: Vec<crate::machine::GenericConformanceBound>,
            pub requires: HandleSpan<TraitRequirement>,
            pub machines: HandleSpan<StateSignature>,
            pub refines: Option<TraitRequirement>,
            pub refinement_clauses: Vec<TraitRefinementClause>,
        }

        #[derive(Debug, Clone, PartialEq, Eq, compiler_storage::Record)]
        pub struct TraitRefinementClause {
            pub requirement: Option<DiagnosticName>,
            pub signature: StateSignature,
            pub service_reach_keyword_source_spans: Vec<source::SourceSpan>,
            pub service_reaches: Vec<DiagnosticName>,
        }

        #[derive(Debug, Clone, Default, PartialEq, Eq, compiler_storage::Record)]
        pub struct Conformance {
            pub symbol: SymbolHandle,
            pub is_public: bool,
            pub lifetime_parameters: Vec<DiagnosticName>,
            pub type_parameters: HandleSpan<crate::data::TypeParameter>,
            pub trait_name: DiagnosticName,
            pub trait_symbol: SymbolHandle,
            pub trait_lifetime_arguments: Vec<DiagnosticName>,
            pub arguments: HandleSpan<crate::types::TypeReference>,
            pub alias: Option<DiagnosticName>,
            pub rows: Vec<ConformanceRow>,
        }

        #[derive(Debug, Clone, PartialEq, Eq, compiler_storage::Record)]
        pub struct ConformanceRow {
            pub declaring_trait: SymbolHandle,
            pub declaring_trait_name: DiagnosticName,
            pub requirement: SymbolHandle,
            pub requirement_name: DiagnosticName,
            pub provisional_requirement_ordinal: Option<usize>,
            pub realization_machine: SymbolHandle,
            pub realization_state: SymbolHandle,
            pub realization_name: DiagnosticName,
            pub authored_realization_source_span: Option<source::SourceSpan>,
            pub provisional_realization_ordinal: Option<usize>,
            pub source: ConformanceRowSource,
        }

        #[derive(Debug, Clone, Copy, PartialEq, Eq, compiler_storage::Record)]
        pub enum ConformanceRowSource {
            Inline,
            Reference,
            TraitDefault,
            Synthesized,
        }

        impl ConformanceRowSource {
            pub fn owns_realization(self) -> bool {
                matches!(self, Self::Inline | Self::TraitDefault | Self::Synthesized)
            }
        }

        #[derive(Debug, Clone, PartialEq, Eq, Default, compiler_storage::Record)]
        pub struct TraitRequirement {
            pub symbol: SymbolHandle,
            pub name: DiagnosticName,
            pub lifetime_arguments: Vec<DiagnosticName>,
            pub arguments: HandleSpan<crate::types::TypeReference>,
        }

        deref_to_storage!(TraitDefinition => TraitStorage);
    }
    pub mod wire {
        use crate::name::DiagnosticName;
        use crate::types::TypeReference;
        use arena::HandleSpan;
        use symbols::SymbolHandle;

        #[derive(Debug, Clone, Default, PartialEq, Eq, compiler_storage::Record)]
        pub struct WireSchema {
            pub symbol: SymbolHandle,
            pub name: DiagnosticName,
            pub is_public: bool,
            pub encoding: Option<DiagnosticName>,
            pub members: HandleSpan<WireMember>,
        }

        #[derive(Debug, Clone, PartialEq, Eq, compiler_storage::Record)]
        pub enum WireMember {
            Field(WireField),
            Reserved(WireReserved),
            Version(WireVersion),
        }

        impl Default for WireMember {
            fn default() -> Self {
                Self::Reserved(WireReserved::default())
            }
        }

        #[derive(Debug, Clone, PartialEq, Eq, Default, compiler_storage::Record)]
        pub struct WireField {
            pub number: u64,
            pub name: DiagnosticName,
            pub relevance: language_core::BindingRelevance,
            pub type_reference: TypeReference,
        }

        #[derive(Debug, Clone, Default, PartialEq, Eq, compiler_storage::Record)]
        pub struct WireReserved {
            pub number: u64,
        }

        #[derive(Debug, Clone, Default, PartialEq, Eq, compiler_storage::Record)]
        pub struct WireVersion {
            pub name: DiagnosticName,
            pub members: HandleSpan<WireMember>,
        }
    }

    pub mod type_aliases {
        use crate::data::TypeParameter;
        use crate::machine::GenericConformanceBound;
        use crate::name::DiagnosticName;
        use crate::types::TypeReference;
        use arena::HandleSpan;
        use symbols::SymbolHandle;

        #[derive(Debug, Clone, Default, PartialEq, Eq, compiler_storage::Record)]
        pub struct TypeAliasDefinition {
            pub symbol: SymbolHandle,
            pub name: DiagnosticName,
            pub is_public: bool,
            pub lifetime_parameters: Vec<DiagnosticName>,
            pub type_parameters: HandleSpan<TypeParameter>,
            pub conformance_bounds: Vec<GenericConformanceBound>,
            pub expansion: TypeReference,
        }
    }
}

pub mod machine_declarations {
    use crate::machine::Machine;
    use arena::{OrderedRootArena, OwnedIndex};
    use std::ops::{Deref, DerefMut};
    use symbols::SymbolHandle;

    #[derive(Clone, Default, PartialEq, Eq, compiler_storage::Record)]
    pub struct MachineDeclarations {
        arena: OrderedRootArena<Machine>,
        positions: OwnedIndex<Vec<(SymbolHandle, usize)>, ()>,
    }

    impl From<OrderedRootArena<Machine>> for MachineDeclarations {
        fn from(arena: OrderedRootArena<Machine>) -> Self {
            Self {
                arena,
                positions: OwnedIndex::default(),
            }
        }
    }

    impl std::fmt::Debug for MachineDeclarations {
        fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            std::fmt::Debug::fmt(&self.arena, formatter)
        }
    }

    impl Deref for MachineDeclarations {
        type Target = OrderedRootArena<Machine>;

        fn deref(&self) -> &Self::Target {
            &self.arena
        }
    }

    impl DerefMut for MachineDeclarations {
        fn deref_mut(&mut self) -> &mut Self::Target {
            self.positions.clear();
            &mut self.arena
        }
    }

    impl<'arena> IntoIterator for &'arena MachineDeclarations {
        type Item = &'arena Machine;
        type IntoIter = <&'arena OrderedRootArena<Machine> as IntoIterator>::IntoIter;

        fn into_iter(self) -> Self::IntoIter {
            self.arena.iter()
        }
    }

    impl MachineDeclarations {
        pub fn by_symbol(&self, symbol: SymbolHandle) -> Option<&Machine> {
            self.symbol_position(symbol)
                .map(|(position, _)| &self.arena[position])
        }

        pub fn unique_by_symbol(&self, symbol: SymbolHandle) -> Option<&Machine> {
            self.symbol_position(symbol)
                .filter(|(_, duplicate)| !duplicate)
                .map(|(position, _)| &self.arena[position])
        }

        fn symbol_position(&self, symbol: SymbolHandle) -> Option<(usize, bool)> {
            self.positions.with(
                (),
                || {
                    let mut positions = self
                        .arena
                        .iter()
                        .enumerate()
                        .map(|(position, machine)| (machine.symbol, position))
                        .collect::<Vec<_>>();
                    positions
                        .sort_by_key(|(symbol, _)| (symbol.arena_index(), symbol.generation()));
                    positions
                },
                |positions| {
                    let key = (symbol.arena_index(), symbol.generation());
                    let first = positions.partition_point(|(candidate, _)| {
                        (candidate.arena_index(), candidate.generation()) < key
                    });
                    positions
                        .get(first)
                        .filter(|(candidate, _)| *candidate == symbol)
                        .map(|(_, position)| {
                            (
                                *position,
                                positions
                                    .get(first + 1)
                                    .is_some_and(|(candidate, _)| *candidate == symbol),
                            )
                        })
                },
            )
        }
    }

    #[cfg(test)]
    pub(crate) mod tests {
        use super::MachineDeclarations;
        use crate::machine::Machine;
        use arena::OrderedRootArena;
        use symbols::SymbolHandle;

        fn machine(symbol: SymbolHandle, is_public: bool) -> Machine {
            Machine {
                symbol,
                is_public,
                ..Machine::default()
            }
        }

        #[test]
        fn selection_keeps_first_duplicate_and_exact_symbol_generation() {
            let symbol = SymbolHandle::from_parts(3, 1);
            let next_generation = SymbolHandle::from_parts(3, 2);
            let mut declarations = MachineDeclarations::default();
            let hidden = declarations.push(machine(symbol, false));
            declarations.push(machine(next_generation, false));
            declarations.push(machine(symbol, true));
            assert!(!declarations.by_symbol(symbol).unwrap().is_public);
            assert!(declarations.unique_by_symbol(symbol).is_none());
            assert!(!declarations.by_symbol(next_generation).unwrap().is_public);
            assert!(declarations.unique_by_symbol(next_generation).is_some());
            declarations.retain(|machine| machine.symbol != symbol || machine.is_public);
            assert!(declarations.by_symbol(symbol).unwrap().is_public);
            assert!(declarations.unique_by_symbol(symbol).is_some());
            assert!(!declarations.contains(hidden));
            assert!(!declarations.get(hidden).is_public);
            declarations.retain(|machine| machine.symbol == symbol);
            assert!(declarations.by_symbol(next_generation).is_none());
        }

        #[test]
        fn mutable_storage_and_clones_cannot_share_stale_positions() {
            let original = SymbolHandle::from_parts(1, 1);
            let replacement = SymbolHandle::from_parts(2, 1);
            let mut declarations = MachineDeclarations::default();
            let handle = declarations.push(machine(original, false));
            assert!(declarations.by_symbol(original).is_some());
            let clone = declarations.clone();
            declarations.get_mut(handle).symbol = replacement;
            assert!(declarations.by_symbol(original).is_none());
            assert!(declarations.by_symbol(replacement).is_some());
            assert!(clone.by_symbol(original).is_some());
            assert!(clone.by_symbol(replacement).is_none());
            declarations[0].symbol = original;
            assert!(declarations.by_symbol(replacement).is_none());
            declarations.for_each_mut(|machine| machine.symbol = replacement);
            assert!(declarations.by_symbol(original).is_none());
            declarations.push(machine(original, true));
            assert!(declarations.by_symbol(original).unwrap().is_public);
        }

        #[test]
        fn replacing_equal_revision_arena_replaces_derived_relationships() {
            let original = SymbolHandle::from_parts(1, 1);
            let replacement = SymbolHandle::from_parts(2, 1);
            let mut declarations = MachineDeclarations::default();
            declarations.push(machine(original, false));
            assert!(declarations.by_symbol(original).is_some());
            let mut unrelated = OrderedRootArena::default();
            unrelated.push(machine(replacement, true));
            assert_eq!(declarations.revision(), unrelated.revision());
            *declarations = unrelated;
            assert!(declarations.by_symbol(original).is_none());
            assert!(declarations.by_symbol(replacement).unwrap().is_public);
            let mut unrelated = OrderedRootArena::default();
            unrelated.push(machine(original, false));
            declarations = unrelated.into();
            assert!(declarations.by_symbol(original).is_some());
            assert!(declarations.by_symbol(replacement).is_none());
        }
    }
}
