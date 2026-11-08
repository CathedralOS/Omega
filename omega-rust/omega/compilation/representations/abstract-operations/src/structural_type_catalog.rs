// We hold the immutable shared structural declarations, referent identities,
// access view plans, quotient catalogs, and placement lifecycles across native
// compilation stages. Cloning shares the underlying `Arc` allocations; callers
// request copy-on-write explicitly via `make_mut`.

use std::ops::Deref;
use std::sync::Arc;

use semantic_vocabulary::StructuralTypeId;
use terminal_codec::CodecError;
use terminal_psi::{
    StructuralReferentIdentity, StructuralTypeDeclaration, TerminalAccessViewInput,
    TerminalAccessViewPlan, TerminalQuotientCatalog,
};

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct StructuralTypeCatalog(
    Arc<Vec<StructuralTypeDeclaration>>,
    Arc<Vec<StructuralReferentIdentity>>,
    Arc<Vec<TerminalAccessViewPlan>>,
    Arc<Vec<TerminalAccessViewInput>>,
    Option<Arc<RetainedQuotients>>,
    Option<Arc<RetainedAccessLifecycles>>,
);

#[derive(Clone, Debug, PartialEq, Eq)]
struct RetainedAccessLifecycles {
    plans: Vec<terminal_psi::TerminalAccessLifecyclePlan>,
    canonical: Vec<u8>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct RetainedQuotients {
    catalog: TerminalQuotientCatalog,
    canonical: Vec<u8>,
}

static EMPTY_QUOTIENTS: TerminalQuotientCatalog = TerminalQuotientCatalog {
    representations: Vec::new(),
    operations: Vec::new(),
    adaptations: Vec::new(),
};

impl StructuralTypeCatalog {
    pub fn ptr_eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.0, &other.0)
            && Arc::ptr_eq(&self.1, &other.1)
            && Arc::ptr_eq(&self.2, &other.2)
            && Arc::ptr_eq(&self.3, &other.3)
            && match (&self.4, &other.4) {
                (None, None) => true,
                (Some(left), Some(right)) => Arc::ptr_eq(left, right),
                _ => false,
            }
            && match (&self.5, &other.5) {
                (None, None) => true,
                (Some(left), Some(right)) => Arc::ptr_eq(left, right),
                _ => false,
            }
    }

    pub fn with_access_lifecycles(
        mut self,
        plans: Vec<terminal_psi::TerminalAccessLifecyclePlan>,
    ) -> Result<Self, CodecError> {
        self.5 = if plans.is_empty() {
            None
        } else {
            let canonical = terminal_codec::encode_access_lifecycle_catalog(&plans)?;
            Some(Arc::new(RetainedAccessLifecycles { plans, canonical }))
        };
        Ok(self)
    }

    pub fn access_lifecycles(&self) -> &[terminal_psi::TerminalAccessLifecyclePlan] {
        self.5
            .as_ref()
            .map_or(&[], |catalog| catalog.plans.as_slice())
    }

    pub fn access_lifecycle_identity_bytes(&self) -> &[u8] {
        self.5
            .as_ref()
            .map_or(&[], |catalog| catalog.canonical.as_slice())
    }

    pub fn with_quotients(mut self, catalog: TerminalQuotientCatalog) -> Result<Self, CodecError> {
        self.4 = if catalog == EMPTY_QUOTIENTS {
            None
        } else {
            let canonical = terminal_codec::encode_quotient_catalog(&catalog)?;
            Some(Arc::new(RetainedQuotients { catalog, canonical }))
        };
        Ok(self)
    }

    pub fn quotients(&self) -> &TerminalQuotientCatalog {
        self.4.as_ref().map_or(&EMPTY_QUOTIENTS, |row| &row.catalog)
    }

    pub fn quotient_identity_bytes(&self) -> &[u8] {
        self.4.as_ref().map_or(&[], |row| row.canonical.as_slice())
    }

    pub fn new(
        declarations: Vec<StructuralTypeDeclaration>,
        referents: Vec<StructuralReferentIdentity>,
    ) -> Self {
        Self(
            Arc::new(declarations),
            Arc::new(referents),
            Arc::default(),
            Arc::default(),
            None,
            None,
        )
    }

    pub fn with_access_views(mut self, plans: Vec<TerminalAccessViewPlan>) -> Self {
        self.2 = Arc::new(plans);
        self
    }

    pub fn access_views(&self) -> &[TerminalAccessViewPlan] {
        self.2.as_slice()
    }

    pub fn with_access_view_inputs(mut self, inputs: Vec<TerminalAccessViewInput>) -> Self {
        self.3 = Arc::new(inputs);
        self
    }

    pub fn access_view_inputs(&self) -> &[TerminalAccessViewInput] {
        self.3.as_slice()
    }

    pub fn access_parameter_demand(
        &self,
        machine: semantic_vocabulary::MachineId,
        runtime_position: usize,
        parameter: &terminal_psi::StructuralParameterDeclaration,
    ) -> Option<(&TerminalAccessViewInput, &TerminalAccessViewPlan)> {
        let mut inputs = self.access_view_inputs().iter().filter(|input| {
            input.machine == machine
                && usize::try_from(input.runtime_position).ok() == Some(runtime_position)
        });
        let (Some(input), None) = (inputs.next(), inputs.next()) else {
            return None;
        };
        let mut carriers = self
            .iter()
            .filter(|carrier| carrier.id == parameter.structural_type);
        let (Some(carrier), None) = (carriers.next(), carriers.next()) else {
            return None;
        };
        if carrier.identity != input.carrier_identity
            || parameter.access != input.access
            || parameter.access == terminal_psi::StructuralAccess::Owned
        {
            return None;
        }
        let mut plans = self
            .access_views()
            .iter()
            .filter(|plan| plan.carrier == carrier.id);
        let (Some(plan), None) = (plans.next(), plans.next()) else {
            return None;
        };
        if plan.view_identity != input.view_identity
            || plan.policy_identity != input.policy_identity
            || plan.policy_plan_machine_identity != input.policy_plan_machine_identity
            || plan.schema_identity != input.schema_identity
            || plan.placement.content_interpretation().commitment() != input.placement_commitment
        {
            return None;
        }
        Some((input, plan))
    }

    pub fn referents(&self) -> &[StructuralReferentIdentity] {
        self.1.as_slice()
    }

    pub fn as_slice(&self) -> &[StructuralTypeDeclaration] {
        self.0.as_slice()
    }

    pub fn make_mut(&mut self) -> &mut Vec<StructuralTypeDeclaration> {
        Arc::make_mut(&mut self.0)
    }
}

impl From<Vec<StructuralTypeDeclaration>> for StructuralTypeCatalog {
    fn from(declarations: Vec<StructuralTypeDeclaration>) -> Self {
        Self::new(declarations, Vec::new())
    }
}

impl FromIterator<StructuralTypeDeclaration> for StructuralTypeCatalog {
    fn from_iter<T: IntoIterator<Item = StructuralTypeDeclaration>>(declarations: T) -> Self {
        Self::from(declarations.into_iter().collect::<Vec<_>>())
    }
}

impl Deref for StructuralTypeCatalog {
    type Target = [StructuralTypeDeclaration];

    fn deref(&self) -> &Self::Target {
        self.as_slice()
    }
}

impl<'a> IntoIterator for &'a StructuralTypeCatalog {
    type Item = &'a StructuralTypeDeclaration;
    type IntoIter = std::slice::Iter<'a, StructuralTypeDeclaration>;

    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}

impl PartialEq<Vec<StructuralTypeDeclaration>> for StructuralTypeCatalog {
    fn eq(&self, other: &Vec<StructuralTypeDeclaration>) -> bool {
        self.4.is_none()
            && self.5.is_none()
            && self.referents().is_empty()
            && self.access_views().is_empty()
            && self.access_view_inputs().is_empty()
            && self.as_slice() == other.as_slice()
    }
}

impl PartialEq<StructuralTypeCatalog> for Vec<StructuralTypeDeclaration> {
    fn eq(&self, other: &StructuralTypeCatalog) -> bool {
        other.4.is_none()
            && other.5.is_none()
            && other.referents().is_empty()
            && other.access_views().is_empty()
            && other.access_view_inputs().is_empty()
            && self.as_slice() == other.as_slice()
    }
}

pub(super) fn unique_structural_type<'a>(
    types: impl Iterator<Item = &'a StructuralTypeDeclaration>,
    identity: StructuralTypeId,
) -> Option<&'a StructuralTypeDeclaration> {
    let mut declarations = types.filter(|declaration| declaration.id == identity);
    match (declarations.next(), declarations.next()) {
        (Some(declaration), None) => Some(declaration),
        _ => None,
    }
}
