/*
We define parsed top-level item declarations (`Item`, `DataDefinition`,
`Machine`, `TraitDefinition`, `CapabilityDefinition`, `MathematicalDefinition`,
and `PropositionDefinition`) and their paged arena storage in `SyntaxTrees`.
*/

pub mod item {
    use crate::expression::ExpressionHandle;
    use crate::identifier::Identifier;
    use crate::paged_roster::paged_roster;
    use crate::statement::StatementHandle;
    use crate::types::TypeReferenceHandle;
    use arena::{Handle, HandleSpan};
    use language_core::operator_spelling::OperatorSpelling;

    pub type ItemHandle = Handle<Item>;
    pub type StateParameterHandle = Handle<StateParameterNode>;
    pub type StateSignatureHandle = Handle<StateSignature>;
    pub type StateHandle = Handle<State>;
    pub type MachineHandle = Handle<MachineNode>;
    pub type TraitHandle = Handle<TraitNode>;
    pub type MathematicalDefinitionHandle = Handle<MathematicalDefinition>;
    pub type MathematicalParameterHandle = Handle<MathematicalParameterNode>;
    pub type MathematicalTypeHandle = Handle<MathematicalTypeNode>;

    pub(crate) mod capabilities {
        use super::*;

        #[derive(
            Debug, Clone, Default, syntax_trees_derive::SyntaxEquivalent, compiler_storage::Record,
        )]
        pub struct OperatorDefinition {
            pub is_public: bool,
            pub is_boundary: bool,
            pub name: HandleSpan<Identifier>,
            pub lifetime_parameters: Vec<Identifier>,
            pub type_parameters: HandleSpan<TypeParameter>,
            pub parameters: HandleSpan<StateParameterHandle>,
            pub return_type: crate::types::TypeReferenceHandle,
            pub contracts: HandleSpan<CapabilityContract>,
            pub spelling: Option<OperatorSpelling>,
            pub token_count: usize,
        }

        #[derive(Debug, Clone, syntax_trees_derive::SyntaxEquivalent, compiler_storage::Record)]
        pub struct CapabilityDefinition {
            pub name: Identifier,
            pub members: HandleSpan<CapabilityMember>,
        }

        #[derive(Debug, Clone, syntax_trees_derive::SyntaxEquivalent, compiler_storage::Record)]
        pub enum CapabilityMember {
            Field(CapabilityField),
            State(CapabilityState),
        }

        impl Default for CapabilityMember {
            fn default() -> Self {
                Self::Field(CapabilityField::default())
            }
        }

        #[derive(
            Debug, Clone, Default, syntax_trees_derive::SyntaxEquivalent, compiler_storage::Record,
        )]
        pub struct CapabilityField {
            pub name: Identifier,
            pub type_reference: crate::types::TypeReferenceHandle,
        }

        #[derive(Debug, Clone, syntax_trees_derive::SyntaxEquivalent, compiler_storage::Record)]
        pub struct CapabilityState {
            pub signature: StateSignature,
            pub contracts: HandleSpan<CapabilityContract>,
        }

        #[derive(
            Debug, Clone, Default, syntax_trees_derive::SyntaxEquivalent, compiler_storage::Record,
        )]
        pub struct CapabilityContract {
            pub kind: CapabilityContractKind,
            pub keyword_source_span: Option<source::SourceSpan>,
            pub binding: Option<Identifier>,
            pub facts: HandleSpan<ProofFact>,
            pub token_count: usize,
        }

        #[derive(
            Debug,
            Clone,
            PartialEq,
            Eq,
            Default,
            syntax_trees_derive::PlainSyntaxEquivalent,
            compiler_storage::Record,
        )]
        pub enum CapabilityContractKind {
            Ensures,
            EnsuresForResultCase {
                result_case: HandleSpan<Identifier>,
            },
            #[default]
            Requires,
            Crashes {
                cause: CrashCause,
            },
        }

        #[derive(Debug, Clone, Copy, PartialEq, Eq, compiler_storage::Record)]
        pub enum CrashCause {
            Trap,
            Abort,
        }
    }
    pub(crate) mod data_items {
        use super::*;

        #[derive(Debug, Clone, syntax_trees_derive::SyntaxEquivalent, compiler_storage::Record)]
        pub struct DataDefinition {
            pub name: Identifier,
            pub is_public: bool,
            pub supply_mode: language_core::DataSupplyMode,
            pub lifetime_parameters: Vec<Identifier>,
            pub type_parameters: HandleSpan<TypeParameter>,
            pub generic_instance: Option<crate::types::TypeReferenceHandle>,
            pub conformance_bounds: Vec<crate::item::GenericConformanceBound>,
            pub properties: DataProperties,
            pub quotient: Option<QuotientDefinition>,
            pub cleanup_selection: Option<crate::expression::StaticMachineArgument>,
            pub requires_facts: HandleSpan<ProofFact>,
            pub members: HandleSpan<DataMember>,
        }

        #[derive(Debug, Clone, syntax_trees_derive::SyntaxEquivalent, compiler_storage::Record)]
        pub struct QuotientDefinition {
            pub carrier: crate::types::TypeReferenceHandle,
            pub relation: HandleSpan<Identifier>,
            pub equivalence: Option<QuotientEquivalenceSelection>,
        }

        #[derive(Debug, Clone, syntax_trees_derive::SyntaxEquivalent, compiler_storage::Record)]
        pub struct QuotientEquivalenceSelection {
            pub relation: HandleSpan<Identifier>,
            pub trait_name: Identifier,
            pub trait_arguments: HandleSpan<crate::types::TypeReferenceHandle>,
            pub conformance_name: Identifier,
        }

        #[derive(
            Debug, Clone, Default, syntax_trees_derive::SyntaxEquivalent, compiler_storage::Record,
        )]
        pub struct ConformanceItem {
            pub is_public: bool,
            pub lifetime_parameters: Vec<Identifier>,
            pub type_parameters: HandleSpan<TypeParameter>,
            pub trait_name: Identifier,
            pub trait_lifetime_arguments: Vec<Identifier>,
            pub trait_arguments: HandleSpan<crate::types::TypeReferenceHandle>,
            pub alias: Option<Identifier>,
            pub members: HandleSpan<ConformanceMember>,
        }

        #[derive(Debug, Clone, syntax_trees_derive::SyntaxEquivalent, compiler_storage::Record)]
        pub enum ConformanceMember {
            Machine(Machine),
            TraitDefault {
                declaring_trait: Identifier,
                requirement_ordinal: usize,
                machine: Machine,
            },
            Synthesized(Machine),
            /*
                A selection row, `Trait::member = Existing::machine;`. The
                implementation is a record of selected operations
                (wiki/spec/language/conformances.md, "Implementations are
                operation records"), so this slot holds the named machine
                itself: no member machine is declared, and the selected
                declaration keeps its own identity, contract and laws.
            */
            Selection {
                declaring_trait: Identifier,
                requirement: Identifier,
                target: HandleSpan<Identifier>,
            },
        }

        impl Default for ConformanceMember {
            fn default() -> Self {
                Self::Machine(Machine::default())
            }
        }

        #[derive(
            Debug,
            Clone,
            Copy,
            Default,
            PartialEq,
            Eq,
            syntax_trees_derive::PlainSyntaxEquivalent,
            compiler_storage::Record,
        )]
        pub struct DataProperties {
            pub multiplicity: language_core::Multiplicity,
            pub carry: Option<language_core::CarryPolicy>,
        }

        #[derive(
            Debug, Clone, Default, syntax_trees_derive::SyntaxEquivalent, compiler_storage::Record,
        )]
        pub struct TypeParameter {
            pub name: Identifier,
            pub kind: TypeParameterKind,
            pub bounds: DataProperties,
        }

        #[derive(
            Debug, Clone, Default, syntax_trees_derive::SyntaxEquivalent, compiler_storage::Record,
        )]
        pub enum TypeParameterKind {
            #[default]
            Type,
            Implementation,
            Const {
                type_reference: crate::types::TypeReferenceHandle,
            },
            Value {
                type_reference: crate::types::TypeReferenceHandle,
            },
            Machine {
                contract: Option<MachineParameterContract>,
            },
            Proposition {
                contract: Option<PropositionParameterSignature>,
            },
        }

        impl TypeParameterKind {
            pub fn is_const_or_value(&self) -> bool {
                matches!(self, Self::Const { .. } | Self::Value { .. })
            }
        }

        #[derive(Debug, Clone, syntax_trees_derive::SyntaxEquivalent, compiler_storage::Record)]
        pub enum MachineParameterContract {
            /// An authored `const F: machine`; exact typed uses must supply its type.
            Inferred,
            RequirementIdentity,
            Structural(StateSignature),
            Nominal {
                requirement: HandleSpan<Identifier>,
                trait_arguments: HandleSpan<crate::types::TypeReferenceHandle>,
            },
        }

        #[derive(
            Debug, Clone, Default, syntax_trees_derive::SyntaxEquivalent, compiler_storage::Record,
        )]
        pub struct PropositionParameterSignature {
            pub name: Identifier,
            pub parameters: HandleSpan<StateParameterHandle>,
        }

        #[derive(Debug, Clone, syntax_trees_derive::SyntaxEquivalent, compiler_storage::Record)]
        pub enum DataMember {
            Field(DataField),
            Variant(DataVariant),
            Retired(u64),
        }

        impl Default for DataMember {
            fn default() -> Self {
                Self::Variant(DataVariant::default())
            }
        }

        #[derive(
            Debug, Clone, Default, syntax_trees_derive::SyntaxEquivalent, compiler_storage::Record,
        )]
        pub struct DataField {
            pub identity: Option<u64>,
            pub name: Identifier,
            pub relevance: language_core::BindingRelevance,
            pub type_reference: crate::types::TypeReferenceHandle,
        }

        #[derive(
            Debug, Clone, Default, syntax_trees_derive::SyntaxEquivalent, compiler_storage::Record,
        )]
        pub struct DataVariant {
            pub identity: Option<u64>,
            pub name: Identifier,
            pub payload: HandleSpan<DataField>,
            pub requires_facts: HandleSpan<ProofFact>,
            pub retired_payload_identities: Vec<u64>,
        }

        #[derive(Debug, Clone, syntax_trees_derive::SyntaxEquivalent, compiler_storage::Record)]
        pub struct DomainDefinition {
            pub name: Identifier,
            pub type_parameters: HandleSpan<TypeParameter>,
            pub target_type: crate::types::TypeReferenceHandle,
            pub index_arguments: HandleSpan<crate::types::TypeReferenceHandle>,
            pub is_public: bool,
            pub alias: Option<DomainAliasDefinition>,
            pub cleanup_selection: Option<crate::expression::StaticMachineArgument>,
            pub authored_routes: Vec<Vec<Identifier>>,
            /// The policy named after `access`: its plan states who may
            /// introduce the domain and who may read or write what it governs.
            pub access_policy: Option<crate::expression::StaticMachineArgument>,
            pub classification: Option<language_core::DomainClassification>,
            pub predicate_presence: language_core::DomainPredicatePresence,
            pub facts: HandleSpan<ProofFact>,
            pub operators: HandleSpan<OperatorDefinition>,
            pub semantic_clause_token_count: usize,
        }

        #[derive(
            Debug,
            Clone,
            Default,
            PartialEq,
            Eq,
            syntax_trees_derive::PlainSyntaxEquivalent,
            compiler_storage::Record,
        )]
        pub struct DomainAliasDefinition {
            pub constituents: Vec<HandleSpan<Identifier>>,
        }

        impl Default for DomainDefinition {
            fn default() -> Self {
                Self {
                    name: Identifier::generated(""),
                    type_parameters: HandleSpan::empty(),
                    target_type: crate::types::TypeReferenceHandle::invalid(),
                    index_arguments: HandleSpan::empty(),
                    is_public: false,
                    alias: None,
                    cleanup_selection: None,
                    authored_routes: Vec::new(),
                    access_policy: None,
                    classification: None,
                    predicate_presence: language_core::DomainPredicatePresence::Absent,
                    facts: HandleSpan::empty(),
                    operators: HandleSpan::empty(),
                    semantic_clause_token_count: 0,
                }
            }
        }
    }
    pub(crate) mod machine_items {
        use super::*;

        #[derive(
            Debug, Clone, Default, syntax_trees_derive::SyntaxEquivalent, compiler_storage::Record,
        )]
        pub struct Machine {
            pub name: Identifier,
            pub generic_data_template: Identifier,
            pub default_body_trait: Identifier,
            pub attached_data: Option<Identifier>,
            pub spelling: Option<OperatorSpelling>,
            pub is_public: bool,
            pub bodyless: bool,
            pub target_family_interface: bool,
            pub target: Option<Identifier>,
            pub authored_target: Option<Identifier>,
            pub boundary: bool,
            pub lifetime_parameters: Vec<Identifier>,
            pub type_parameters: HandleSpan<TypeParameter>,
            pub satisfies: HandleSpan<SatisfiesClause>,
            pub conformance_bounds: Vec<GenericConformanceBound>,
            pub terminates_guarantee: bool,
            pub ranking_subjects: HandleSpan<crate::expression::ExpressionHandle>,
            pub ranking_view: HandleSpan<Identifier>,
            pub ranking_view_arguments: HandleSpan<crate::expression::ExpressionHandle>,
            pub ranking_range: crate::expression::ExpressionHandle,
            pub service_reach_keyword_source_spans: Vec<source::SourceSpan>,
            pub service_reaches: HandleSpan<Identifier>,
            pub service_reach_is_installation_bound: bool,
            pub invokes: HandleSpan<Identifier>,
            pub suspends_keyword_source_spans: Vec<source::SourceSpan>,
            pub blocks_keyword_source_spans: Vec<source::SourceSpan>,
            pub suspends: bool,
            pub blocks: bool,
            pub admitted_same_stack_bytes: u64,
            /// The header wrote `crashes none`, which publishes no row in
            /// `contracts`. `wiki/spec/language/effects.md`: an explicit
            /// empty value "promises the empty value and rejects any body or
            /// selection that violates it", so on a private body it is not
            /// the omitted clause that requests inference.
            pub declares_no_crash_route: bool,
            pub contracts: HandleSpan<CapabilityContract>,
            pub states: HandleSpan<StateHandle>,
            pub nested_machines: HandleSpan<Item>,
        }

        #[derive(
            Debug, Clone, Default, syntax_trees_derive::SyntaxEquivalent, compiler_storage::Record,
        )]
        pub struct State {
            pub name: Identifier,
            pub parameters: HandleSpan<StateParameterHandle>,
            pub return_type: crate::types::TypeReferenceHandle,
            pub contracts: HandleSpan<CapabilityContract>,
            pub statements: HandleSpan<crate::statement::StatementHandle>,
        }

        #[derive(Debug, Clone, syntax_trees_derive::SyntaxEquivalent, compiler_storage::Record)]
        pub struct TraitDefinition {
            pub is_boundary: bool,
            pub is_public: bool,
            pub name: Identifier,
            pub lifetime_parameters: Vec<Identifier>,
            pub type_parameters: HandleSpan<TypeParameter>,
            pub conformance_bounds: Vec<GenericConformanceBound>,
            pub parents: HandleSpan<crate::types::TypeReferenceHandle>,
            pub requires: HandleSpan<Identifier>,
            pub machines: HandleSpan<StateSignatureHandle>,
            pub refines: Option<crate::types::TypeReferenceHandle>,
            pub refinement_clauses: Vec<TraitRefinementClause>,
        }

        #[derive(Debug, Clone, syntax_trees_derive::SyntaxEquivalent, compiler_storage::Record)]
        pub struct TraitRefinementClause {
            pub requirement: Option<Identifier>,
            pub signature: StateSignature,
        }

        #[derive(
            Debug, Clone, Default, syntax_trees_derive::SyntaxEquivalent, compiler_storage::Record,
        )]
        pub struct StateSignature {
            pub name: Identifier,
            pub spelling: Option<crate::operator_spelling::OperatorSpelling>,
            pub lifetime_parameters: Vec<Identifier>,
            pub type_parameters: HandleSpan<TypeParameter>,
            pub conformance_bounds: Vec<GenericConformanceBound>,
            pub is_default: bool,
            pub parameters: HandleSpan<StateParameterHandle>,
            pub native_callback_parameters: Vec<NativeCallbackParameterNode>,
            pub return_type: crate::types::TypeReferenceHandle,
            pub service_reach_is_installation_bound: bool,
            pub service_reach_keyword_source_spans: Vec<source::SourceSpan>,
            pub service_reaches: HandleSpan<Identifier>,
            pub invokes: HandleSpan<Identifier>,
            pub suspends_keyword_source_spans: Vec<source::SourceSpan>,
            pub blocks_keyword_source_spans: Vec<source::SourceSpan>,
            pub suspends: bool,
            pub blocks: bool,
            pub contracts: HandleSpan<CapabilityContract>,
            pub default_body: HandleSpan<crate::statement::StatementHandle>,
            pub terminates_guarantee: bool,
            /// The requirement wrote `crashes none`. That clause publishes no
            /// contract row, so this is the only record that the empty crash
            /// set was authored rather than omitted, which for a requirement
            /// defers to the selected member's routes.
            pub declares_no_crash_route: bool,
        }
    }
    pub(crate) mod proof_items {
        use super::*;

        #[derive(Debug, Clone, syntax_trees_derive::SyntaxEquivalent, compiler_storage::Record)]
        pub struct PropositionDefinition {
            pub name: Identifier,
            pub is_public: bool,
            pub type_parameters: HandleSpan<TypeParameter>,
            pub parameters: HandleSpan<StateParameterHandle>,
            pub transparent_formula_source_span: Option<source::SourceSpan>,
            pub body: PropositionBody,
        }

        #[derive(
            Debug,
            Clone,
            PartialEq,
            Eq,
            syntax_trees_derive::PlainSyntaxEquivalent,
            compiler_storage::Record,
        )]
        pub enum PropositionBody {
            Primitive,
            Witness { evidence: TypeReferenceHandle },
            Transparent { proposition: ExpressionHandle },
        }

        #[derive(
            Debug, Clone, Default, syntax_trees_derive::SyntaxEquivalent, compiler_storage::Record,
        )]
        pub struct MathematicalDefinition {
            pub name: Identifier,
            pub is_public: bool,
            pub type_parameters: HandleSpan<TypeParameter>,
            pub parameters: HandleSpan<MathematicalParameterHandle>,
            pub result: MathematicalTypeHandle,
            pub body: MathematicalDefinitionBody,
        }

        #[derive(
            Debug,
            Clone,
            PartialEq,
            Eq,
            syntax_trees_derive::PlainSyntaxEquivalent,
            compiler_storage::Record,
        )]
        pub enum MathematicalDefinitionBody {
            Definition(ExpressionHandle),
            Assumption,
        }

        impl Default for MathematicalDefinitionBody {
            fn default() -> Self {
                Self::Definition(ExpressionHandle::invalid())
            }
        }

        #[derive(
            Debug, Clone, Default, syntax_trees_derive::SyntaxEquivalent, compiler_storage::Record,
        )]
        pub struct MathematicalParameterNode {
            pub name: Identifier,
            pub relevance: language_core::BindingRelevance,
            pub ty: MathematicalTypeHandle,
        }

        #[derive(Debug, Clone, syntax_trees_derive::SyntaxEquivalent, compiler_storage::Record)]
        pub enum MathematicalTypeNode {
            Ordinary(TypeReferenceHandle),
            Arrow {
                binder: Option<Identifier>,
                domain: MathematicalTypeHandle,
                codomain: MathematicalTypeHandle,
            },
            Application {
                callee: MathematicalTypeHandle,
                arguments: HandleSpan<ExpressionHandle>,
            },
        }

        impl Default for MathematicalTypeNode {
            fn default() -> Self {
                Self::Ordinary(TypeReferenceHandle::invalid())
            }
        }

        #[derive(
            Debug,
            Clone,
            PartialEq,
            Eq,
            Default,
            syntax_trees_derive::PlainSyntaxEquivalent,
            compiler_storage::Record,
        )]
        pub struct MeasureDefinition {
            pub name: HandleSpan<Identifier>,
            pub parameter: StateParameterHandle,
            pub return_type: TypeReferenceHandle,
            pub lexicographic: bool,
            pub body: HandleSpan<ExpressionHandle>,
            pub token_count: usize,
        }

        #[derive(
            Debug,
            Clone,
            PartialEq,
            Eq,
            syntax_trees_derive::PlainSyntaxEquivalent,
            compiler_storage::Record,
        )]
        pub enum ProofFact {
            Expression(ExpressionHandle),
            Membership(ProofMembershipFact),
        }

        impl Default for ProofFact {
            fn default() -> Self {
                Self::Expression(ExpressionHandle::invalid())
            }
        }

        #[derive(Debug, Clone, Copy, PartialEq, Eq, Default, compiler_storage::Record)]
        pub struct ProofMembershipFact {
            pub value: ExpressionHandle,
            pub domain: HandleSpan<Identifier>,
            pub domain_arguments: HandleSpan<TypeReferenceHandle>,
        }

        #[derive(
            Debug, Clone, Default, syntax_trees_derive::SyntaxEquivalent, compiler_storage::Record,
        )]
        pub struct SatisfiesClause {
            pub trait_name: Identifier,
            pub lifetime_arguments: Vec<Identifier>,
            pub arguments: HandleSpan<TypeReferenceHandle>,
            pub requirement: Option<Identifier>,
            pub alias: Option<Identifier>,
            pub via: Option<ExternalBinding>,
            pub via_expression: ExpressionHandle,
            pub via_keyword_source_span: Option<source::SourceSpan>,
        }

        #[derive(
            Debug, Clone, Default, syntax_trees_derive::SyntaxEquivalent, compiler_storage::Record,
        )]
        pub struct GenericConformanceBound {
            pub binder: Identifier,
            pub trait_name: Identifier,
            pub lifetime_arguments: Vec<Identifier>,
            pub arguments: HandleSpan<TypeReferenceHandle>,
        }
    }
    pub(crate) mod type_aliases {
        use super::{GenericConformanceBound, TypeParameter};
        use crate::identifier::Identifier;
        use crate::types::TypeReferenceHandle;
        use arena::HandleSpan;

        #[derive(Debug, Clone, syntax_trees_derive::SyntaxEquivalent, compiler_storage::Record)]
        pub struct TypeAliasDefinition {
            pub name: Identifier,
            pub is_public: bool,
            pub lifetime_parameters: Vec<Identifier>,
            pub type_parameters: HandleSpan<TypeParameter>,
            pub conformance_bounds: Vec<GenericConformanceBound>,
            pub authored_expansion: TypeReferenceHandle,
            pub expansion: TypeReferenceHandle,
        }
    }
    pub use capabilities::*;
    pub use data_items::*;
    pub use machine_items::*;
    pub use proof_items::*;
    pub use type_aliases::*;

    #[derive(Debug, Clone, syntax_trees_derive::SyntaxEquivalent, compiler_storage::Record)]
    pub enum Item {
        Capability(CapabilityDefinition),
        Conformance(ConformanceItem),
        Const(ConstDefinition),
        Data(DataDefinition),
        Domain(DomainDefinition),
        Measure(MeasureDefinition),
        Module(ModuleDeclaration),
        Operator(OperatorDefinition),
        Package(PackageDeclaration),
        Proposition(PropositionDefinition),
        Use(UseItem),
        Machine(Machine),
        Trait(TraitDefinition),
        TypeAlias(TypeAliasDefinition),
    }

    impl Item {
        pub fn as_data(&self) -> Option<&DataDefinition> {
            let Item::Data(data) = self else {
                return None;
            };
            Some(data)
        }

        pub fn as_type_alias(&self) -> Option<&TypeAliasDefinition> {
            let Item::TypeAlias(alias) = self else {
                return None;
            };
            Some(alias)
        }

        pub fn as_domain(&self) -> Option<&DomainDefinition> {
            let Item::Domain(domain) = self else {
                return None;
            };
            Some(domain)
        }

        pub fn as_machine(&self) -> Option<&Machine> {
            let Item::Machine(machine) = self else {
                return None;
            };
            Some(machine)
        }
    }

    #[derive(
        Debug, Clone, Default, syntax_trees_derive::SyntaxEquivalent, compiler_storage::Record,
    )]
    pub struct ConstDefinition {
        pub scope: Identifier,
        pub name: Identifier,
        pub is_public: bool,
        pub type_reference: TypeReferenceHandle,
        pub value: ExpressionHandle,
        pub pending_target_observation: bool,
        pub normalization: Option<ConstInitializerNormalization>,
        pub qualification: Option<ConstQualification>,
        pub pending_qualification: Option<ConstDestinationObligation>,
    }

    #[derive(
        Debug,
        Clone,
        PartialEq,
        Eq,
        syntax_trees_derive::PlainSyntaxEquivalent,
        compiler_storage::Record,
    )]
    pub struct ConstQualification {
        pub destination: TypeReferenceHandle,
        pub initializer: ExpressionHandle,
    }

    #[derive(
        Debug,
        Clone,
        PartialEq,
        Eq,
        syntax_trees_derive::PlainSyntaxEquivalent,
        compiler_storage::Record,
    )]
    pub struct ConstDestinationObligation {
        pub declaration: source::SourceSpan,
        pub destination: TypeReferenceHandle,
        pub initializer: ExpressionHandle,
    }

    #[derive(Debug, Clone, Default, PartialEq, Eq, compiler_storage::Record)]
    pub struct ConstInitializerCustody {
        pub selections: Vec<crate::types::ConstArgumentOrigin>,
        pub builtin_operators: Vec<source::SourceSpan>,
        pub call_selections: Vec<(source::SourceSpan, source::SourceSpan)>,
    }

    #[derive(
        Debug,
        Clone,
        Default,
        PartialEq,
        Eq,
        syntax_trees_derive::PlainSyntaxEquivalent,
        compiler_storage::Record,
    )]
    pub struct ConstInitializerNormalization {
        pub authored_custody: ConstInitializerCustody,
        pub declaration_custody: ConstInitializerCustody,
        pub authored_expression: ExpressionHandle,
        pub result_value: language_core::constant_value::EvaluatedConstant,
        pub selections: Vec<crate::types::ConstArgumentOrigin>,
        pub builtin_operators: Vec<source::SourceSpan>,
        pub call_selections: Vec<(source::SourceSpan, source::SourceSpan)>,
    }

    #[derive(Debug, Clone, syntax_trees_derive::SyntaxEquivalent, compiler_storage::Record)]
    pub enum ExternalBinding {
        Syscall { number: i64 },
        CompilerIntrinsic,
        VtableSlot { index: i64 },
        VtableField { field: Identifier },
        TableFunction { field: Identifier },
    }

    impl Default for ExternalBinding {
        fn default() -> Self {
            Self::Syscall { number: 0 }
        }
    }

    impl Default for Item {
        fn default() -> Self {
            Self::Use(UseItem::default())
        }
    }

    #[derive(
        Debug,
        Clone,
        PartialEq,
        Eq,
        Default,
        syntax_trees_derive::PlainSyntaxEquivalent,
        compiler_storage::Record,
    )]
    pub struct UseItem {
        pub path: HandleSpan<Identifier>,
    }

    #[derive(
        Debug,
        Clone,
        PartialEq,
        Eq,
        Default,
        syntax_trees_derive::PlainSyntaxEquivalent,
        compiler_storage::Record,
    )]
    pub struct ModuleDeclaration {
        pub path: HandleSpan<Identifier>,
    }

    #[derive(
        Debug,
        Clone,
        PartialEq,
        Eq,
        Default,
        syntax_trees_derive::PlainSyntaxEquivalent,
        compiler_storage::Record,
    )]
    pub struct PackageDeclaration {
        pub path: HandleSpan<Identifier>,
    }

    paged_roster! {
        pub struct ItemTable => pub(crate) ItemTableAppendCheckpoint {
            arena items: Item,
            arena mathematical_definitions: MathematicalDefinition,
            roster state_storage: StateStorage => StateStorageAppendCheckpoint,
            roster declaration_storage: DeclarationStorage => DeclarationStorageAppendCheckpoint,
        }
    }

    paged_roster! {
        struct StateStorage => pub(crate) StateStorageAppendCheckpoint {
            arena parameters: StateParameterNode,
            arena signatures: StateSignature,
            arena states: State,
            arena parameter_handles: StateParameterHandle,
            arena state_handles: StateHandle,
            arena signature_handles: StateSignatureHandle,
            arena statement_handles: StatementHandle,
            arena machines: MachineNode,
            arena traits: TraitNode,
        }
    }

    paged_roster! {
        struct DeclarationStorage => pub(crate) DeclarationStorageAppendCheckpoint {
            arena identifier_path_members: Identifier,
            arena conformance_members: ConformanceMember,
            arena satisfies_clauses: SatisfiesClause,
            arena type_parameters: TypeParameter,
            arena capability_members: CapabilityMember,
            arena capability_contracts: CapabilityContract,
            arena data_members: DataMember,
            arena data_payload_fields: DataField,
            arena operators: OperatorDefinition,
            arena measures: MeasureDefinition,
            arena proof_facts: ProofFact,
            arena proof_fact_source_spans: Option<source::SourceSpan>,
            arena mathematical_types: MathematicalTypeNode,
            arena mathematical_parameters: MathematicalParameterNode,
            arena mathematical_parameter_handles: MathematicalParameterHandle,
        }
    }

    impl ItemTable {
        pub fn new() -> Self {
            Self::new_with_generation(1)
        }

        pub(crate) fn page_count(&self) -> usize {
            self.items.page_count()
        }

        pub fn state_parameter(&self, handle: StateParameterHandle) -> &StateParameterNode {
            self.state_storage.parameters.get(handle)
        }

        #[inline]
        pub fn item(&self, handle: ItemHandle) -> &Item {
            self.items.get(handle)
        }

        pub fn mathematical_definition(
            &self,
            handle: MathematicalDefinitionHandle,
        ) -> &MathematicalDefinition {
            self.mathematical_definitions.get(handle)
        }

        pub fn append_mathematical_definition(
            &mut self,
            definition: MathematicalDefinition,
        ) -> MathematicalDefinitionHandle {
            self.mathematical_definitions.append(definition)
        }

        pub fn mathematical_type(&self, handle: MathematicalTypeHandle) -> &MathematicalTypeNode {
            self.declaration_storage.mathematical_types.get(handle)
        }

        pub fn insert_mathematical_type(
            &mut self,
            node: MathematicalTypeNode,
        ) -> MathematicalTypeHandle {
            self.declaration_storage.mathematical_types.append(node)
        }

        pub fn mathematical_parameter(
            &self,
            handle: MathematicalParameterHandle,
        ) -> &MathematicalParameterNode {
            self.declaration_storage.mathematical_parameters.get(handle)
        }

        pub fn insert_mathematical_parameter(
            &mut self,
            parameter: MathematicalParameterNode,
        ) -> MathematicalParameterHandle {
            self.declaration_storage
                .mathematical_parameters
                .append(parameter)
        }

        pub fn mathematical_parameters(
            &self,
            span: HandleSpan<MathematicalParameterHandle>,
        ) -> &[MathematicalParameterHandle] {
            self.declaration_storage
                .mathematical_parameter_handles
                .span_or_empty(span)
        }

        pub fn append_mathematical_parameter_handle(
            &mut self,
            handle: MathematicalParameterHandle,
        ) -> Handle<MathematicalParameterHandle> {
            self.declaration_storage
                .mathematical_parameter_handles
                .append(handle)
        }

        pub fn replace_item(&mut self, handle: ItemHandle, item: Item) {
            *self.items.get_mut(handle) = item;
        }

        pub fn state_signature(&self, handle: StateSignatureHandle) -> &StateSignature {
            self.state_storage.signatures.get(handle)
        }

        pub fn state(&self, handle: StateHandle) -> &State {
            self.state_storage.states.get(handle)
        }

        pub fn state_mut(&mut self, handle: StateHandle) -> &mut State {
            self.state_storage.states.get_mut(handle)
        }

        pub fn machine(&self, handle: MachineHandle) -> &MachineNode {
            self.state_storage.machines.get(handle)
        }

        pub fn trait_definition(&self, handle: TraitHandle) -> &TraitNode {
            self.state_storage.traits.get(handle)
        }

        pub fn type_parameter_entries(&self) -> impl Iterator<Item = &TypeParameter> {
            self.declaration_storage
                .type_parameters
                .iter()
                .map(|(_, parameter)| parameter)
        }

        pub fn type_parameters(&self, span: HandleSpan<TypeParameter>) -> &[TypeParameter] {
            self.declaration_storage.type_parameters.span_or_empty(span)
        }

        pub fn type_parameters_mut(
            &mut self,
            span: HandleSpan<TypeParameter>,
        ) -> &mut [TypeParameter] {
            self.declaration_storage
                .type_parameters
                .span_mut_or_empty(span)
        }

        pub fn identifier_path_members(&self, span: HandleSpan<Identifier>) -> &[Identifier] {
            self.declaration_storage
                .identifier_path_members
                .span_or_empty(span)
        }

        pub fn conformance_members(
            &self,
            span: HandleSpan<ConformanceMember>,
        ) -> &[ConformanceMember] {
            self.declaration_storage
                .conformance_members
                .span_or_empty(span)
        }

        pub fn satisfies_clauses(&self, span: HandleSpan<SatisfiesClause>) -> &[SatisfiesClause] {
            self.declaration_storage
                .satisfies_clauses
                .span_or_empty(span)
        }

        pub fn capability_members(
            &self,
            span: HandleSpan<CapabilityMember>,
        ) -> &[CapabilityMember] {
            self.declaration_storage
                .capability_members
                .span_or_empty(span)
        }

        pub fn capability_contracts(
            &self,
            span: HandleSpan<CapabilityContract>,
        ) -> &[CapabilityContract] {
            self.declaration_storage
                .capability_contracts
                .span_or_empty(span)
        }

        pub fn requirement_facts(
            &self,
            contracts: HandleSpan<CapabilityContract>,
        ) -> Vec<ProofFact> {
            self.capability_contracts(contracts)
                .iter()
                .filter(|contract| contract.kind == CapabilityContractKind::Requires)
                .flat_map(|contract| self.proof_facts(contract.facts).iter().cloned())
                .collect()
        }

        pub fn data_members(&self, span: HandleSpan<DataMember>) -> &[DataMember] {
            self.declaration_storage.data_members.span_or_empty(span)
        }

        pub fn data_members_mut(&mut self, span: HandleSpan<DataMember>) -> &mut [DataMember] {
            self.declaration_storage
                .data_members
                .span_mut_or_empty(span)
        }

        pub fn data_payload_fields(&self, span: HandleSpan<DataField>) -> &[DataField] {
            self.declaration_storage
                .data_payload_fields
                .span_or_empty(span)
        }

        pub fn operators(&self, span: HandleSpan<OperatorDefinition>) -> &[OperatorDefinition] {
            self.declaration_storage.operators.span_or_empty(span)
        }

        pub fn measures(&self, span: HandleSpan<MeasureDefinition>) -> &[MeasureDefinition] {
            self.declaration_storage.measures.span_or_empty(span)
        }

        pub fn proof_facts(&self, span: HandleSpan<ProofFact>) -> &[ProofFact] {
            self.declaration_storage.proof_facts.span_or_empty(span)
        }

        pub fn proof_fact(&self, handle: Handle<ProofFact>) -> &ProofFact {
            self.declaration_storage.proof_facts.get(handle)
        }

        pub fn proof_fact_source_span(
            &self,
            handle: Handle<ProofFact>,
        ) -> Option<source::SourceSpan> {
            *self
                .declaration_storage
                .proof_fact_source_spans
                .get(proof_fact_source_span_handle(handle))
        }

        pub fn set_proof_fact_source_span(
            &mut self,
            handle: Handle<ProofFact>,
            source_span: source::SourceSpan,
        ) {
            *self
                .declaration_storage
                .proof_fact_source_spans
                .get_mut(proof_fact_source_span_handle(handle)) = Some(source_span);
        }

        pub fn state_parameters(
            &self,
            span: HandleSpan<StateParameterHandle>,
        ) -> &[StateParameterHandle] {
            self.state_storage.parameter_handles.span_or_empty(span)
        }

        pub fn state_signatures(
            &self,
            span: HandleSpan<StateSignatureHandle>,
        ) -> &[StateSignatureHandle] {
            self.state_storage.signature_handles.span_or_empty(span)
        }

        pub fn state_handles(&self, span: HandleSpan<StateHandle>) -> &[StateHandle] {
            self.state_storage.state_handles.span_or_empty(span)
        }

        pub fn statements(&self, span: HandleSpan<StatementHandle>) -> &[StatementHandle] {
            self.state_storage.statement_handles.span_or_empty(span)
        }

        pub fn insert_state_parameter_node(
            &mut self,
            parameter: StateParameterNode,
        ) -> StateParameterHandle {
            self.state_storage.parameters.append(parameter)
        }

        pub fn append_state_parameter_handle(
            &mut self,
            handle: StateParameterHandle,
        ) -> Handle<StateParameterHandle> {
            self.state_storage.parameter_handles.append(handle)
        }

        pub fn append_state_handle(&mut self, handle: StateHandle) -> Handle<StateHandle> {
            self.state_storage.state_handles.append(handle)
        }

        pub fn append_state_signature_handle(
            &mut self,
            handle: StateSignatureHandle,
        ) -> Handle<StateSignatureHandle> {
            self.state_storage.signature_handles.append(handle)
        }

        pub fn append_statement_handle(
            &mut self,
            handle: StatementHandle,
        ) -> Handle<StatementHandle> {
            self.state_storage.statement_handles.append(handle)
        }

        pub fn append_item(&mut self, item: Item) -> ItemHandle {
            self.items.append(item)
        }

        pub fn insert_identifier_path_members(
            &mut self,
            members: impl IntoIterator<Item = Identifier>,
        ) -> HandleSpan<Identifier> {
            self.declaration_storage
                .identifier_path_members
                .insert_many(members)
        }

        pub fn append_identifier_path_member(&mut self, member: Identifier) -> Handle<Identifier> {
            self.declaration_storage
                .identifier_path_members
                .append(member)
        }

        pub fn append_conformance_member(
            &mut self,
            member: ConformanceMember,
        ) -> Handle<ConformanceMember> {
            self.declaration_storage.conformance_members.append(member)
        }

        pub fn append_satisfies_clause(
            &mut self,
            clause: SatisfiesClause,
        ) -> Handle<SatisfiesClause> {
            self.declaration_storage.satisfies_clauses.append(clause)
        }

        pub fn append_type_parameter(
            &mut self,
            type_parameter: TypeParameter,
        ) -> Handle<TypeParameter> {
            self.declaration_storage
                .type_parameters
                .append(type_parameter)
        }

        pub fn append_capability_member(
            &mut self,
            member: CapabilityMember,
        ) -> Handle<CapabilityMember> {
            self.declaration_storage.capability_members.append(member)
        }

        pub fn append_capability_contract(
            &mut self,
            contract: CapabilityContract,
        ) -> Handle<CapabilityContract> {
            self.declaration_storage
                .capability_contracts
                .append(contract)
        }

        pub fn append_data_member(&mut self, member: DataMember) -> Handle<DataMember> {
            self.declaration_storage.data_members.append(member)
        }

        pub fn append_data_payload_field(&mut self, field: DataField) -> Handle<DataField> {
            self.declaration_storage.data_payload_fields.append(field)
        }

        pub fn append_operator(
            &mut self,
            operator: OperatorDefinition,
        ) -> Handle<OperatorDefinition> {
            self.declaration_storage.operators.append(operator)
        }

        pub fn append_proof_fact(&mut self, fact: ProofFact) -> Handle<ProofFact> {
            let handle = self.declaration_storage.proof_facts.append(fact);
            let source_span = self
                .declaration_storage
                .proof_fact_source_spans
                .append(None);
            debug_assert_eq!(source_span.arena_index(), handle.arena_index());
            debug_assert_eq!(source_span.generation(), handle.generation());
            handle
        }

        pub fn state_count(&self) -> usize {
            self.state_storage.states.len()
        }

        pub fn machine_count(&self) -> usize {
            self.state_storage.machines.len()
        }

        pub fn insert_state_signature(
            &mut self,
            signature: &StateSignature,
        ) -> StateSignatureHandle {
            self.state_storage.signatures.append(signature.clone())
        }

        pub fn insert_state(&mut self, state: &State) -> StateHandle {
            self.state_storage.states.append(state.clone())
        }

        pub fn insert_machine(&mut self, machine: &Machine) -> MachineHandle {
            self.state_storage.machines.append(MachineNode {
                name: machine.name.clone(),
                satisfies: machine.satisfies,
                service_reach_keyword_source_spans: machine
                    .service_reach_keyword_source_spans
                    .clone(),
                service_reaches: machine.service_reaches,
                invokes: machine.invokes,
                suspends_keyword_source_spans: machine.suspends_keyword_source_spans.clone(),
                blocks_keyword_source_spans: machine.blocks_keyword_source_spans.clone(),
                suspends: machine.suspends,
                blocks: machine.blocks,
                contracts: machine.contracts,
                states: machine.states,
            })
        }

        pub fn insert_trait_definition(
            &mut self,
            trait_definition: &TraitDefinition,
        ) -> TraitHandle {
            self.state_storage.traits.append(TraitNode {
                is_boundary: trait_definition.is_boundary,
                name: trait_definition.name.clone(),
                parents: trait_definition.parents,
                requires: trait_definition.requires,
                machines: trait_definition.machines,
            })
        }
    }

    impl Default for ItemTable {
        fn default() -> Self {
            Self::new()
        }
    }

    fn proof_fact_source_span_handle(
        handle: Handle<ProofFact>,
    ) -> Handle<Option<source::SourceSpan>> {
        Handle::from_parts(handle.arena_index(), handle.generation())
    }

    #[derive(
        Debug, Clone, Default, syntax_trees_derive::SyntaxEquivalent, compiler_storage::Record,
    )]
    pub struct StateParameterNode {
        pub name: Identifier,
        pub type_reference: TypeReferenceHandle,
        pub is_const: bool,
        pub is_mutable: bool,
        pub relevance: language_core::BindingRelevance,
    }

    #[derive(
        Debug, Clone, Default, syntax_trees_derive::SyntaxEquivalent, compiler_storage::Record,
    )]
    pub struct NativeCallbackParameterNode {
        pub name: Identifier,
        pub binder: Identifier,
        pub native_ordinal: u32,
    }

    #[derive(
        Debug, Clone, Default, syntax_trees_derive::SyntaxEquivalent, compiler_storage::Record,
    )]
    pub struct MachineNode {
        pub name: Identifier,
        pub satisfies: HandleSpan<SatisfiesClause>,
        pub service_reach_keyword_source_spans: Vec<source::SourceSpan>,
        pub service_reaches: HandleSpan<Identifier>,
        pub invokes: HandleSpan<Identifier>,
        pub suspends_keyword_source_spans: Vec<source::SourceSpan>,
        pub blocks_keyword_source_spans: Vec<source::SourceSpan>,
        pub suspends: bool,
        pub blocks: bool,
        pub contracts: HandleSpan<CapabilityContract>,
        pub states: HandleSpan<StateHandle>,
    }

    #[derive(
        Debug, Clone, Default, syntax_trees_derive::SyntaxEquivalent, compiler_storage::Record,
    )]
    pub struct TraitNode {
        pub is_boundary: bool,
        pub name: Identifier,
        pub parents: HandleSpan<TypeReferenceHandle>,
        pub requires: HandleSpan<Identifier>,
        pub machines: HandleSpan<StateSignatureHandle>,
    }
}
