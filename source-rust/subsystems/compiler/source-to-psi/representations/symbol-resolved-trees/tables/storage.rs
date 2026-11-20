//! Owned arenas for declarations, types, expressions and statements.

use crate::{
    data, domain, expression, mathematical, operator, proposition, signature, state, statement,
    types, wire,
};
use arena::{Arena, Handle};
#[derive(Debug, Clone, Default, PartialEq, Eq, record_storage::Record)]
pub struct SymbolResolvedDeclarationStorage {
    pub runtime_callable_contracts:
        Arena<crate::source_resolved::callable_values::RuntimeCallableContract>,
    pub data_members: Arena<data::DataMember>,
    pub data_payload_fields: Arena<data::DataField>,
    pub data_type_parameters: Arena<data::TypeParameter>,
    pub declared_type_equations: Arena<crate::source_resolved::machine::DeclaredTypeEquation>,
    pub proof_facts: Arena<domain::ProofFact>,
    pub(crate) proof_fact_source_spans: Vec<Option<source_text::SourceSpan>>,
    pub domain_path_members: Arena<crate::source_resolved::name::DiagnosticName>,
    pub operator_path_members: Arena<crate::source_resolved::name::DiagnosticName>,
    pub operator_definitions: Arena<operator::OperatorDefinition>,
    pub proposition_binders: Arena<proposition::PropositionBinder>,
    pub mathematical_types: Arena<mathematical::MathematicalType>,
    pub mathematical_parameters: Arena<mathematical::MathematicalParameter>,
    pub machine_owned_data: Arena<crate::source_resolved::machine::OwnedData>,
    pub machine_trait_conformances: Arena<crate::source_resolved::machine::TraitConformance>,
    pub machine_state_handles: Arena<Handle<state::State>>,
    pub machine_states: Arena<state::State>,
    pub trait_requirements: Arena<crate::source_resolved::trait_definition::TraitRequirement>,
    pub trait_machine_signatures: Arena<signature::StateSignature>,
    pub ranking_views: Arena<crate::source_resolved::name::DiagnosticName>,
    pub signature_invokes: Arena<crate::source_resolved::name::DiagnosticName>,
    pub signature_contracts: Arena<signature::SignatureContract>,
    pub state_parameters: Arena<signature::StateParameter>,
    pub statement_path_members: Arena<crate::source_resolved::name::DiagnosticName>,
    pub state_statements: Arena<statement::Statement>,
    pub assembly_graphs: Arena<statement::AssemblyControlFlow>,
    pub assembly_blocks: Arena<statement::AssemblyBlock>,
    pub assembly_expressions: Arena<expression::ExpressionHandle>,
    pub child_type_references: types::TypeReferenceArena,
    pub wire_members: Arena<wire::WireMember>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, record_storage::Record)]
pub struct SymbolResolvedTypeStorage {
    pub operand_lifetime_owners: Arena<types::OperandLifetimeOwner>,
    pub constraints: Arena<types::TypeConstraint>,
    pub references: types::TypeReferenceTable,
    pub generic_application_origins: Arena<types::GenericApplicationOrigin>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, record_storage::Record)]
pub struct SymbolResolvedBodyStorage {
    pub expressions: expression::ExpressionTable,
    pub statements: statement::StatementTable,
}
