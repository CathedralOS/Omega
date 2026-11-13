// We capture source declaration details needed to generate direct boundary calls
// and placed memory accesses (`CapturedProviderRows`) while keeping each
// operation paired with its original candidate (`ProviderPlanCandidate`).
// Direct-call identity errors are reported when attaching Calling details
// (`attach_physical_rows`), while intrinsic and exit-process declaration errors
// wait until after `ProgramEntry` checks (`finish_row_declarations`).

use crate::DerivedProviderPlan;
use diagnostics::Diagnostic;
use effects::provider_plan::{ProviderPlan, ProviderPlanRow};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeferredProviderRows {
    direct: Vec<Option<DirectBoundaryInput>>,
    declarations: Result<RowDeclarations, Diagnostic>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct RowDeclarations {
    placed: Result<Option<crate::SealedAccessor>, Diagnostic>,
    rows: Vec<Option<ProviderRowDeclarations>>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PreparedProviderRows {
    placed: Result<Option<crate::SealedAccessor>, Diagnostic>,
    rows: Vec<ProviderRow>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ProviderRow {
    declarations: Option<ProviderRowDeclarations>,
    direct: Option<DirectBoundaryInput>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProviderRowDeclarations {
    pub intrinsic: Option<crate::intrinsic_declarations::IntrinsicDeclaration>,
    pub exit_process: crate::intrinsic_declarations::ExitProcessDeclaration,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProviderPlanCandidate<Rows = PreparedProviderRows> {
    source: DerivedProviderPlan,
    rows: Rows,
}

impl<Rows> ProviderPlanCandidate<Rows> {
    pub fn plan(&self) -> &ProviderPlan {
        &self.source.plan
    }

    pub fn direct_source(
        &self,
        input: &DirectBoundaryInput,
    ) -> Result<DirectBoundarySource<'_>, Diagnostic> {
        derived_direct_source(&self.source, input.row_index)
    }
}

impl ProviderPlanCandidate {
    pub fn direct_boundaries(&self) -> impl Iterator<Item = &DirectBoundaryInput> {
        self.rows.rows.iter().filter_map(|row| row.direct.as_ref())
    }

    pub fn row_declarations(&self, row: usize) -> Option<&ProviderRowDeclarations> {
        self.rows.rows.get(row)?.declarations.as_ref()
    }

    pub fn direct_declarations(
        &self,
        input: &DirectBoundaryInput,
    ) -> Result<&ProviderRowDeclarations, Diagnostic> {
        self.row_declarations(input.row_index).ok_or_else(|| {
            Diagnostic::error("direct boundary has no prepared original-row declaration")
        })
    }

    pub fn matches_selection(&self, selected: &crate::SelectedProviderReviewProvenance) -> bool {
        self.plan() == &selected.plan && self.source.provenance == selected.provider
    }

    pub fn accessor(&self) -> Result<Option<&crate::SealedAccessor>, Diagnostic> {
        self.rows
            .placed
            .as_ref()
            .map(Option::as_ref)
            .map_err(Clone::clone)
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

impl ProviderPlanCandidate<DeferredProviderRows> {
    pub(crate) fn finish_row_declarations(self) -> Result<ProviderPlanCandidate, Diagnostic> {
        let Self { source, rows } = self;
        let declarations = rows.declarations?;
        let rows = declarations
            .rows
            .into_iter()
            .zip(rows.direct)
            .map(|(declarations, direct)| ProviderRow {
                declarations,
                direct,
            })
            .collect();
        Ok(ProviderPlanCandidate {
            source,
            rows: PreparedProviderRows {
                placed: declarations.placed,
                rows,
            },
        })
    }
}
