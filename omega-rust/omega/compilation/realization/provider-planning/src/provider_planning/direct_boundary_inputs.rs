//! Prepare direct-call inputs beside their original provider plan.
//!
//! Source/layout preparation retains original row coordinates and deferred
//! foreign ABI failures. It creates no projected plan or intrinsic execution.
//! Selected execution constructs complete target leaves after host admission.

use crate::DerivedProviderPlan;
use diagnostics::Diagnostic;
use effects::provider_plan::{ProviderBinding, ProviderPlan, ProviderPlanRow};

/// One candidate inventory owner. Toolchain-only plans have no authored source
/// provenance; they cannot manufacture direct declaration inputs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProviderPlanCandidate {
    source: ProviderPlanCandidateSource,
    direct: Vec<DirectBoundaryInput>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum ProviderPlanCandidateSource {
    Derived(DerivedProviderPlan),
    Toolchain(ProviderPlan),
}

impl ProviderPlanCandidate {
    pub fn plan(&self) -> &ProviderPlan {
        match &self.source {
            ProviderPlanCandidateSource::Derived(source) => &source.plan,
            ProviderPlanCandidateSource::Toolchain(plan) => plan,
        }
    }

    pub(crate) fn derived(&self) -> Option<&DerivedProviderPlan> {
        match &self.source {
            ProviderPlanCandidateSource::Derived(source) => Some(source),
            ProviderPlanCandidateSource::Toolchain(_) => None,
        }
    }

    pub fn direct_boundaries(&self) -> &[DirectBoundaryInput] {
        &self.direct
    }

    pub(crate) fn toolchain(plan: ProviderPlan) -> Self {
        Self {
            source: ProviderPlanCandidateSource::Toolchain(plan),
            direct: Vec::new(),
        }
    }

    /// Borrow the producer's original row, without truncating a copied plan or
    /// rebuilding its source associations under projected callable identities.
    pub fn direct_source(
        &self,
        input: &DirectBoundaryInput,
    ) -> Result<DirectBoundarySource<'_>, Diagnostic> {
        let source = self.derived().ok_or_else(|| {
            Diagnostic::error("direct boundary has no original provider provenance")
        })?;
        let provenance = &source.provenance;
        let row =
            source.plan.rows.get(input.row_index).ok_or_else(|| {
                Diagnostic::error("direct boundary lost its original provider row")
            })?;
        let requirement = provenance
            .row_requirements
            .get(input.row_index)
            .copied()
            .ok_or_else(|| Diagnostic::error("direct boundary lost its original requirement"))?;
        let realization = provenance
            .row_realizations
            .get(input.row_index)
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
}

pub struct DirectBoundarySource<'a> {
    pub plan: &'a ProviderPlan,
    pub row: &'a ProviderPlanRow,
    pub schema: crate::ProviderSchemaDeclaration,
    pub provider: Option<symbols::SymbolHandle>,
    pub requirement: symbols::SymbolHandle,
    pub realization: symbols::SymbolHandle,
}

/// Physical preparation is separate from catalog/host admission. Unsupported
/// intrinsic execution is determined later, never represented by a placeholder.
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
    /// Project the final callable plan only after its execution has been settled.
    /// The original satisfied schema remains separate from that projection and
    /// from any independently accepted public plan.
    pub fn finish(
        &self,
        original: &ProviderPlan,
        mut execution: backend_plan::DirectBoundaryExecution,
    ) -> Result<backend_plan::DirectBoundaryLeaf, Diagnostic> {
        let row = original.rows.get(self.row_index).ok_or_else(|| {
            Diagnostic::error("direct boundary projection lost its original provider row")
        })?;
        let requirement = original.identities.text(row.requirement_identity);
        let mut plan = original.clone();
        plan.rows = vec![row.clone()];
        plan.schema.methods.retain(|method| {
            original.schema.identity_text(method.requirement_identity) == requirement
        });
        let satisfied_schema = plan.schema.clone();
        plan.schema.receiver_applications.clear();
        plan.name = format!("direct-boundary:{}:{}", plan.target, self.callable_identity);
        plan.provider_type = self.realization_owner.clone();
        plan.provider_type_package_identity = self.realization_package;
        plan.schema.trait_name = plan.schema.identities.intern(&self.callable_identity);
        plan.schema.trait_package_identity = self.realization_package;
        plan.rows[0].requirement_identity = plan.identities.intern(&self.callable_identity);
        let [method] = plan.schema.methods.as_mut_slice() else {
            return Err(Diagnostic::error(
                "boundary leaf does not bind one exact requirement contract",
            ));
        };
        method.requirement_identity = plan.schema.identities.intern(&self.callable_identity);
        if let backend_plan::DirectBoundaryExecution::Foreign(Ok(binding)) = &mut execution {
            binding.trait_name = self.callable_identity.clone();
            binding.requirement_identity = self.callable_identity.clone();
            binding.table_type = self.realization_owner.clone();
        }
        Ok(backend_plan::DirectBoundaryLeaf {
            declaration_identity: self.declaration_identity.clone(),
            callable_identity: self.callable_identity.clone(),
            satisfied_schema,
            plan,
            satisfied_requirement_identity: requirement.to_owned(),
            execution,
        })
    }
}

pub(crate) fn prepare(
    source: DerivedProviderPlan,
    context: &super::external_binding_rows::ExternalBindingContext<'_>,
) -> Result<ProviderPlanCandidate, Vec<Diagnostic>> {
    let typed = context.typed;
    let mut direct = Vec::new();
    for (row_index, row) in source.plan.rows.iter().enumerate() {
        if !matches!(
            row.binding,
            ProviderBinding::CompilerIntrinsic { .. }
                | ProviderBinding::Import { .. }
                | ProviderBinding::Syscall { .. }
                | ProviderBinding::VtableField { .. }
                | ProviderBinding::TableFunction { .. }
        ) {
            continue;
        }
        let Some(machine) = source
            .provenance
            .row_realizations
            .get(row_index)
            .and_then(crate::ProviderRowRealization::authored)
            .and_then(|symbol| typed.machine_by_symbol(symbol))
        else {
            continue;
        };
        if machine.body_is_present {
            continue;
        }
        let callable_identity = typed
            .machine_callable_identity_text(machine)
            .ok_or_else(|| {
                vec![Diagnostic::error(
                    "boundary leaf has no exact callable identity",
                )]
            })?;
        let requirement = source.plan.identities.text(row.requirement_identity);
        if source
            .plan
            .schema
            .methods
            .iter()
            .filter(|method| {
                source
                    .plan
                    .schema
                    .identity_text(method.requirement_identity)
                    == requirement
            })
            .count()
            != 1
        {
            return Err(vec![Diagnostic::error(
                "boundary leaf does not bind one exact requirement contract",
            )]);
        }
        let mechanism = if matches!(row.binding, ProviderBinding::CompilerIntrinsic { .. }) {
            DirectBoundaryMechanism::Intrinsic
        } else {
            let binding = context
                .row(&source.plan, row_index, Some(&source.provenance))
                .and_then(|row| {
                    row.ok_or_else(|| {
                        vec![Diagnostic::error(
                            "foreign leaf lost its normalized external binding",
                        )]
                    })
                });
            DirectBoundaryMechanism::Foreign(binding)
        };
        // Invocation-local intrinsic declarations remain private. Foreign leaves
        // must have the same hermetic owner as the published boundary catalog.
        let declaration_identity = match typed.normalized_hermetic_symbol_identity(machine.symbol) {
            Ok(identity) => identity,
            Err(_) if matches!(mechanism, DirectBoundaryMechanism::Intrinsic) => format!(
                "program-local::{}",
                typed.symbols.display_path(machine.symbol, "::")
            ),
            Err(reason) => return Err(vec![Diagnostic::error(reason)]),
        };
        direct.push(DirectBoundaryInput {
            row_index,
            declaration_identity,
            callable_identity,
            realization_owner: typed.symbols.display_path(machine.symbol, "::"),
            realization_package: typed.symbols.symbol_package_identity(machine.symbol),
            mechanism,
        });
    }
    Ok(ProviderPlanCandidate {
        source: ProviderPlanCandidateSource::Derived(source),
        direct,
    })
}
