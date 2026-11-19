//! Typing shares signatures and declaration relationships across all files in the admitted stage. Machine bodies query these tables during selection.

use crate::declarations::machine_signature::MachineSignature;
use crate::type_system::type_catalog::TypeCatalog;
use crate::{AssemblyTypes, DataProperties, LogicalHypothesis, RankingView, RequirementSlot, Site, TraitRefinement, ValueType};
use omega_resolver::DeclarationId;
use std::collections::{HashMap, HashSet};

#[derive(Default)]
pub(crate) struct DeclarationCatalog<'a> {
    pub(crate) types: TypeCatalog<'a>,
    pub(crate) assembly: Option<&'a AssemblyTypes>,
    pub(crate) ranking_views: &'a [RankingView],
    pub(crate) overloads: HashMap<(&'a str, &'a str), Vec<&'a DeclarationId>>,
    pub(crate) signatures: HashMap<&'a DeclarationId, MachineSignature>,
    pub(crate) data_properties: HashMap<&'a DeclarationId, DataProperties>,
    pub(crate) fields: HashMap<&'a DeclarationId, HashMap<String, ValueType>>,
    pub(crate) field_order: HashMap<&'a DeclarationId, Vec<String>>,
    pub(crate) cases: HashMap<&'a DeclarationId, Vec<&'a DeclarationId>>,
    pub(crate) case_owner: HashMap<&'a DeclarationId, &'a DeclarationId>,
    pub(crate) implementations: HashMap<&'a DeclarationId, RequirementSlot>,
    pub(crate) requirements: HashMap<&'a DeclarationId, Vec<&'a DeclarationId>>,
    pub(crate) refinements: HashMap<&'a DeclarationId, Vec<TraitRefinement>>,
    pub(crate) trait_parents: HashMap<&'a DeclarationId, RequirementSlot>,
    pub(crate) token_bindings: HashMap<&'a str, Vec<&'a omega_resolver::OperatorDeclaration>>,
    pub(crate) predicates: HashSet<&'a DeclarationId>,
    pub(crate) hypotheses: HashMap<Site, LogicalHypothesis>,
    pub(crate) assumptions: HashMap<DeclarationId, Vec<DeclarationId>>,

    pub(crate) constants: HashMap<&'a DeclarationId, ValueType>,
}
