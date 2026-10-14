//! Shared module declarations; function views retain this catalog, not copies.
//! Reference targets without demanded storage remain exact identity rows,
//! disjoint from concrete definitions. Every later IR retains both rosters;
//! only actual owned storage or projection asks for a concrete declaration.

use std::ops::Deref;
use std::sync::Arc;

use semantic_vocabulary::StructuralTypeId;
use terminal_psi::{
    StructuralReferentIdentity, StructuralTypeDeclaration, TerminalPlacedViewInput,
    TerminalPlacedViewPlan,
};

/// Immutable shared structural declarations at native compilation boundaries.
///
/// Cloning retains the same storage. Builders and corruption tests must request
/// copy-on-write explicitly; editing one revision cannot change another. Equality
/// remains content-based, with a shared-allocation fast path, and is not evidence
/// of semantic admission.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct StructuralTypeCatalog(
    Arc<Vec<StructuralTypeDeclaration>>,
    Arc<Vec<StructuralReferentIdentity>>,
    Arc<Vec<TerminalPlacedViewPlan>>,
    Arc<Vec<TerminalPlacedViewInput>>,
    Option<Arc<RetainedQuotients>>,
);

#[derive(Clone, Debug, PartialEq, Eq)]
struct RetainedQuotients {
    catalog: terminal_psi::TerminalQuotientCatalog,
    canonical: Vec<u8>,
}
static EMPTY_QUOTIENTS: terminal_psi::TerminalQuotientCatalog =
    terminal_psi::TerminalQuotientCatalog {
        representations: Vec::new(),
        operations: Vec::new(),
        adaptations: Vec::new(),
    };
impl StructuralTypeCatalog {
    /// Retain exact immutable metadata and its canonical encoding together.
    /// This checks serialization, not theorem admission.
    pub fn with_quotients(
        mut self,
        catalog: terminal_psi::TerminalQuotientCatalog,
    ) -> Result<Self, terminal_codec::CodecError> {
        self.4 = if catalog == EMPTY_QUOTIENTS {
            None
        } else {
            let canonical = terminal_codec::encode_quotient_catalog(&catalog)?;
            Some(Arc::new(RetainedQuotients { catalog, canonical }))
        };
        Ok(self)
    }
    pub fn quotients(&self) -> &terminal_psi::TerminalQuotientCatalog {
        self.4.as_ref().map_or(&EMPTY_QUOTIENTS, |row| &row.catalog)
    }
    pub fn quotient_identity_bytes(&self) -> &[u8] {
        self.4.as_ref().map_or(&[], |row| row.canonical.as_slice())
    }

    /// Retain storage definitions and identity-only referents in one shared
    /// namespace. Neither copying a pointer nor naming its target creates a
    /// storage definition; projections still require the concrete declaration.
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
        )
    }

    /// Retain the checked placement interpretation separately from ordinary
    /// storage declarations. Geometry and pointer transport cannot recreate it.
    pub fn with_placed_views(mut self, plans: Vec<TerminalPlacedViewPlan>) -> Self {
        self.2 = Arc::new(plans);
        self
    }

    pub fn placed_views(&self) -> &[TerminalPlacedViewPlan] {
        self.2.as_slice()
    }

    /// Exact formal demands survive independently of carrier geometry. This
    /// roster is data; only the caller or physical entry can supply its loan.
    pub fn with_placed_view_inputs(mut self, inputs: Vec<TerminalPlacedViewInput>) -> Self {
        self.3 = Arc::new(inputs);
        self
    }

    pub fn placed_view_inputs(&self) -> &[TerminalPlacedViewInput] {
        self.3.as_slice()
    }

    /// Rejoin one formal demand to its exact carrier and complete plan seal.
    /// This does not establish a backing: every call or host entry still owes
    /// an independently checked occurrence answering this demand.
    pub fn placed_parameter_demand(
        &self,
        machine: semantic_vocabulary::MachineId,
        runtime_position: usize,
        parameter: &terminal_psi::StructuralParameterDeclaration,
    ) -> Option<(&TerminalPlacedViewInput, &TerminalPlacedViewPlan)> {
        let mut inputs = self.placed_view_inputs().iter().filter(|input| {
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
            .placed_views()
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

    /// Start an independently mutable revision, copying only when shared.
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
            && self.referents().is_empty()
            && self.placed_views().is_empty()
            && self.placed_view_inputs().is_empty()
            && self.as_slice() == other.as_slice()
    }
}

impl PartialEq<StructuralTypeCatalog> for Vec<StructuralTypeDeclaration> {
    fn eq(&self, other: &StructuralTypeCatalog) -> bool {
        other.4.is_none()
            && other.referents().is_empty()
            && other.placed_views().is_empty()
            && other.placed_view_inputs().is_empty()
            && self.as_slice() == other.as_slice()
    }
}

/// Return the sole borrowed identity match, stopping at the second match.
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
