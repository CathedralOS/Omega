//! Typing shares signatures and declaration relationships across all files in the admitted stage. Machine bodies query these tables during selection.

use crate::TypedSignature;
use crate::type_system::type_catalog::TypeCatalog;
use crate::{AssemblyTypes, LogicalHypothesis, RankingView, RequirementSlot, Site, TraitRefinement, ValueType};
use omega_resolver::DeclarationId;
use std::collections::{HashMap, HashSet};
use std::sync::Arc;

/// Source and installed schemas keep one declaration-ordered list. A repeated
/// short label has no unique lookup, while each originating field keeps its type.
#[derive(Clone, Default)]
pub(crate) struct RecordFields<'a> {
    pub ordered: Arc<Vec<crate::TypedField>>,
    by_name: HashMap<&'a str, Option<usize>>,
}
impl<'a> RecordFields<'a> {
    pub fn insert(
        &mut self,
        declaration: &'a DeclarationId,
        site: Site,
        ty: ValueType,
    ) {
        self.by_name
            .entry(declaration.path.rsplit("::").next().unwrap())
            .and_modify(|slot| *slot = None)
            .or_insert(Some(self.ordered.len()));
        Arc::make_mut(&mut self.ordered).push(crate::TypedField {
            declaration: declaration.clone(),
            site,
            ty,
        });
    }
    pub fn get(
        &self,
        name: &str,
    ) -> Option<(usize, &crate::TypedField)> {
        let index = (*self.by_name.get(name)?)?;
        Some((index, &self.ordered[index]))
    }
}

#[derive(Default)]
pub(crate) struct DeclarationCatalog<'a> {
    pub(crate) types: TypeCatalog<'a>,
    pub(crate) assembly: Option<&'a AssemblyTypes>,
    pub(crate) declarations: DeclaredItems<'a>,
    pub(crate) overloads: HashMap<(&'a str, &'a str), Vec<&'a DeclarationId>>,
    pub(crate) local_machines: Vec<&'a DeclarationId>,
}

impl<'a> std::ops::Deref for DeclarationCatalog<'a> {
    type Target = DeclaredItems<'a>;
    fn deref(&self) -> &Self::Target {
        &self.declarations
    }
}
impl std::ops::DerefMut for DeclarationCatalog<'_> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.declarations
    }
}

#[derive(Clone, Default)]
pub(crate) struct DeclaredItems<'a> {
    pub(crate) ranking_views: std::borrow::Cow<'a, [RankingView]>,
    pub(crate) access_plan_requirement: Option<&'a DeclarationId>,
    pub(crate) signatures: HashMap<&'a DeclarationId, Arc<TypedSignature>>,
    pub(crate) fields: HashMap<&'a DeclarationId, RecordFields<'a>>,
    pub(crate) cases: HashMap<&'a DeclarationId, Vec<&'a DeclarationId>>,
    pub(crate) case_owner: HashMap<&'a DeclarationId, &'a DeclarationId>,
    pub(crate) implementations: HashMap<&'a DeclarationId, RequirementSlot>,
    pub(crate) requirements: HashMap<&'a DeclarationId, Vec<&'a DeclarationId>>,
    pub(crate) refinements: HashMap<&'a DeclarationId, Vec<TraitRefinement>>,
    pub(crate) trait_parents: HashMap<&'a DeclarationId, RequirementSlot>,
    pub(crate) token_bindings: HashMap<&'a str, Vec<super::operator_declarations::TypedOperatorBinding<'a>>>,
    pub(crate) predicates: HashSet<&'a DeclarationId>,
    pub(crate) hypotheses: HashMap<Site, LogicalHypothesis>,
    pub(crate) assumptions: HashMap<DeclarationId, Vec<DeclarationId>>,

    pub(crate) constants: HashMap<&'a DeclarationId, ValueType>,
}

impl DeclarationCatalog<'_> {
    /// Retain source order, including installed declarations ahead of source machines.
    /// Consumers share the checked machine contracts and field layouts.
    pub(crate) fn publish_declarations(
        &self,
        output: &mut crate::TypedStage,
    ) {
        for &id in &self.local_machines {
            for index in 0..self.types.arity(id) {
                if let Some(ValueType::Callable(bound)) = self.types.bound(id, index) {
                    output.callable_bounds.push((id.clone(), index, *bound.clone()));
                }
            }
            output.signatures.push(self.signatures[id].clone());
        }
        output.record_fields = self
            .fields
            .iter()
            .map(|(&id, fields)| (id.clone(), fields.ordered.clone()))
            .collect();
    }

    /// Field positions start at zero in each sum case. A known case narrows the traversal.
    pub(crate) fn stored_fields<'a>(
        &'a self,
        owner: &'a DeclarationId,
        active_case: Option<&'a DeclarationId>,
    ) -> impl Iterator<Item = (usize, &'a crate::TypedField)> {
        std::iter::once(owner)
            .chain(self.cases.get(owner).into_iter().flatten().copied())
            .filter(move |shape| active_case.is_none_or(|case| *shape == case))
            .filter_map(|shape| self.fields.get(shape))
            .flat_map(|fields| fields.ordered.iter().enumerate())
    }
}
