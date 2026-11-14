// Retained provider plans and canonical direct-boundary data. Source declaration
// discharge objects have no producer after the old checker connection is cut.
use crate::DerivedProviderPlan;
use diagnostics::Diagnostic;
use effects::provider_plan::{ProviderPlan, ProviderPlanRow};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProviderPlanCandidate {
    source: DerivedProviderPlan,
}

impl ProviderPlanCandidate {
    pub fn plan(&self) -> &ProviderPlan {
        &self.source.plan
    }
    pub fn direct_source(
        &self,
        input: &DirectBoundaryInput,
    ) -> Result<DirectBoundarySource<'_>, Diagnostic> {
        derived_direct_source(&self.source, input.row_index)
    }
    pub fn matches_selection(&self, selected: &crate::SelectedProviderReviewProvenance) -> bool {
        self.plan() == &selected.plan && self.source.provenance == selected.provider
    }
}

fn derived_direct_source(
    source: &DerivedProviderPlan,
    row_index: usize,
) -> Result<DirectBoundarySource<'_>, Diagnostic> {
    let provenance = &source.provenance;
    let row = source
        .plan
        .rows
        .get(row_index)
        .ok_or_else(|| Diagnostic::error("direct boundary lost its original provider row"))?;
    let requirement = provenance
        .row_requirements
        .get(row_index)
        .copied()
        .ok_or_else(|| Diagnostic::error("direct boundary lost its original requirement"))?;
    let realization = provenance
        .row_realizations
        .get(row_index)
        .and_then(crate::ProviderRowRealization::authored)
        .ok_or_else(|| Diagnostic::error("direct boundary lost its authored realization"))?;
    Ok(DirectBoundarySource {
        plan: &source.plan,
        row,
        schema: provenance.schema,
        provider: provenance.provider_type,
        requirement,
        realization,
    })
}

pub struct DirectBoundarySource<'a> {
    pub plan: &'a ProviderPlan,
    pub row: &'a ProviderPlanRow,
    pub schema: crate::ProviderSchemaDeclaration,
    pub provider: Option<symbols::SymbolHandle>,
    pub requirement: symbols::SymbolHandle,
    pub realization: symbols::SymbolHandle,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DirectBoundaryMechanism {
    Intrinsic,
    Foreign(Result<calling_conventions::ExternalBindingRow, Vec<Diagnostic>>),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DirectBoundaryInput {
    row_index: usize,
    declaration_identity: String,
    callable_identity: String,
    realization_owner: String,
    realization_package: Option<semantic_vocabulary::PackageKeyIdentity>,
    pub mechanism: DirectBoundaryMechanism,
}

impl DirectBoundaryInput {
    /// Keep the exact external declaration the checker admitted. This adapter
    /// describes a native leaf; receiving execution still admits its binding.
    pub(crate) fn from_checked(
        row_index: usize,
        source: &terminal_psi::ExternalProviderConformance,
        identities: &terminal_psi::TerminalIdentityTable,
        mechanism: DirectBoundaryMechanism,
    ) -> Result<Self, Diagnostic> {
        source.validate(identities).map_err(Diagnostic::error)?;
        let text = |identity| {
            identities
                .text(identity)
                .filter(|value| !value.is_empty())
                .map(str::to_owned)
                .ok_or_else(|| {
                    Diagnostic::error("native leaf lost a complete checked declaration identity")
                })
        };
        Ok(Self {
            row_index,
            declaration_identity: text(source.realization.machine.identity())?,
            callable_identity: text(source.realization.callable)?,
            realization_owner: source
                .provider
                .as_ref()
                .map(|provider| text(provider.identity))
                .transpose()?
                .unwrap_or_default(),
            realization_package: source
                .provider
                .as_ref()
                .and_then(|provider| provider.package),
            mechanism,
        })
    }
    pub fn finish(
        &self,
        original: &ProviderPlan,
        mut execution: backend_plan::DirectBoundaryExecution,
    ) -> Result<backend_plan::DirectBoundaryLeaf, Diagnostic> {
        let row = original.rows.get(self.row_index).ok_or_else(|| {
            Diagnostic::error("direct boundary projection lost its original provider row")
        })?;
        let requirement = original.identities.text(row.declared_requirement_identity);
        let satisfied_schema = original.schema.clone();
        let mut plan = original.clone();
        plan.rows = vec![row.clone()];
        std::sync::Arc::make_mut(&mut plan.schema.declaration)
            .methods
            .retain(|method| {
                original.schema.identity_text(method.requirement_identity) == requirement
            });
        let declaration = std::sync::Arc::make_mut(&mut plan.schema.declaration);
        declaration.receiver_applications.clear();
        plan.name = format!("direct-boundary:{}:{}", plan.target, self.callable_identity);
        plan.provider_type = self.realization_owner.clone();
        plan.provider_type_package_identity = self.realization_package;
        declaration.trait_name = declaration.identities.intern(&self.callable_identity);
        declaration.trait_package_identity = self.realization_package;
        plan.rows[0].requirement_identity = plan.identities.intern(&self.callable_identity);
        plan.rows[0].declared_requirement_identity = plan.rows[0].requirement_identity;
        plan.rows[0].calling_signature = None;
        let [method] = declaration.methods.as_mut_slice() else {
            return Err(Diagnostic::error(
                "boundary leaf does not bind one exact requirement contract",
            ));
        };
        method.requirement_identity = declaration.identities.intern(&self.callable_identity);
        plan.schema.calling_applications.clear();
        if let backend_plan::DirectBoundaryExecution::Foreign(Ok(binding)) = &mut execution {
            binding.trait_name = self.callable_identity.clone();
            binding.requirement_identity = self.callable_identity.clone();
            binding.provider_type = self.realization_owner.clone();
        }
        let leaf = backend_plan::DirectBoundaryLeaf {
            declaration_identity: self.declaration_identity.clone(),
            callable_identity: self.callable_identity.clone(),
            satisfied_schema,
            plan,
            satisfied_requirement_identity: requirement.to_owned(),
            execution,
        };
        leaf.validate_calling_custody().map_err(Diagnostic::error)?;
        Ok(leaf)
    }
}
