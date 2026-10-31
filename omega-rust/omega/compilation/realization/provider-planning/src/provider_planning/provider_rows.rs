//! Keep original provider rows and their optional direct-call inputs together.
//!
//! Source/layout preparation retains original row coordinates and deferred
//! foreign ABI failures and immutable intrinsic declaration facts. It creates
//! no projected plan or host grant. Selected execution joins target/accepted
//! bindings and constructs complete leaves without reopening checked source.

use crate::DerivedProviderPlan;
use diagnostics::Diagnostic;
use effects::provider_plan::{ProviderBinding, ProviderPlan, ProviderPlanRow};

/// Planning precedes constant closure. This phase cannot supply intrinsic facts
/// to completion until the finalized checked snapshot has prepared them.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnpreparedProviderRows {
    direct: Vec<Option<DirectBoundaryInput>>,
}

/// Declaration facts are produced once from the finalized checked snapshot.
/// Both selected review and direct completion borrow these same original rows.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PreparedProviderRows {
    placed: Result<Option<crate::SealedPlacedAccessor>, Diagnostic>,
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

/// One candidate inventory owner. Toolchain-only plans have no authored source
/// provenance; they cannot manufacture direct declaration inputs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProviderPlanCandidate<Rows = PreparedProviderRows> {
    source: ProviderPlanCandidateSource,
    rows: Rows,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum ProviderPlanCandidateSource {
    Derived(DerivedProviderPlan),
    Toolchain(ProviderPlan),
}

impl<Rows> ProviderPlanCandidate<Rows> {
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

    /// Borrow the producer's original row, without truncating a copied plan or
    /// rebuilding its source associations under projected callable identities.
    pub fn direct_source(
        &self,
        input: &DirectBoundaryInput,
    ) -> Result<DirectBoundarySource<'_>, Diagnostic> {
        self.source.direct_source(input.row_index)
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

    /// Rejoin exact producer provenance, never a first same-name candidate.
    /// Toolchain rows have no authored realization and cannot supply row facts.
    pub fn matches_selection(&self, selected: &crate::SelectedProviderReviewProvenance) -> bool {
        self.plan() == &selected.plan
            && match &self.source {
                ProviderPlanCandidateSource::Derived(source) => {
                    source.provenance == selected.provider
                }
                ProviderPlanCandidateSource::Toolchain(_) => selected
                    .provider
                    .row_realizations
                    .iter()
                    .all(|row| matches!(row, crate::ProviderRowRealization::Toolchain)),
            }
    }

    pub fn placed_accessor(&self) -> Result<Option<&crate::SealedPlacedAccessor>, Diagnostic> {
        self.rows
            .placed
            .as_ref()
            .map(Option::as_ref)
            .map_err(Clone::clone)
    }
}

impl ProviderPlanCandidateSource {
    fn direct_source(&self, row_index: usize) -> Result<DirectBoundarySource<'_>, Diagnostic> {
        let Self::Derived(source) = self else {
            return Err(Diagnostic::error(
                "direct boundary has no original provider provenance",
            ));
        };
        let provenance = &source.provenance;
        let row =
            source.plan.rows.get(row_index).ok_or_else(|| {
                Diagnostic::error("direct boundary lost its original provider row")
            })?;
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
}

pub struct DirectBoundarySource<'a> {
    pub plan: &'a ProviderPlan,
    pub row: &'a ProviderPlanRow,
    pub schema: crate::ProviderSchemaDeclaration,
    pub provider: Option<symbols::SymbolHandle>,
    pub requirement: symbols::SymbolHandle,
    pub realization: symbols::SymbolHandle,
}

/// Prepared declaration/physical facts are separate from catalog/host admission.
/// Unsupported execution and deferred shape errors are settled only when the
/// original target and ownership gates demand them.
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
        std::sync::Arc::make_mut(&mut plan.schema.declaration)
            .methods
            .retain(|method| {
                original.schema.identity_text(method.requirement_identity) == requirement
            });
        plan.schema.calling_applications.retain(|application| {
            plan.schema
                .declaration
                .methods
                .iter()
                .any(|method| method.requirement_identity == application.requirement)
        });
        let satisfied_schema = plan.schema.clone();
        let declaration = std::sync::Arc::make_mut(&mut plan.schema.declaration);
        declaration.receiver_applications.clear();
        plan.name = format!("direct-boundary:{}:{}", plan.target, self.callable_identity);
        plan.provider_type = self.realization_owner.clone();
        plan.provider_type_package_identity = self.realization_package;
        declaration.trait_name = declaration.identities.intern(&self.callable_identity);
        declaration.trait_package_identity = self.realization_package;
        plan.rows[0].requirement_identity = plan.identities.intern(&self.callable_identity);
        let [method] = declaration.methods.as_mut_slice() else {
            return Err(Diagnostic::error(
                "boundary leaf does not bind one exact requirement contract",
            ));
        };
        let previous_requirement = method.requirement_identity;
        method.requirement_identity = declaration.identities.intern(&self.callable_identity);
        for application in &mut plan.schema.calling_applications {
            if application.requirement == previous_requirement {
                application.requirement = method.requirement_identity;
            }
        }
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
) -> Result<ProviderPlanCandidate<UnpreparedProviderRows>, Vec<Diagnostic>> {
    let typed = context.services.context().typed;
    let mut direct = vec![None; source.plan.rows.len()];
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
            .declaration
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
                .row(&source.plan, row_index, &source.provenance)
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
        direct[row_index] = Some(DirectBoundaryInput {
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
        rows: UnpreparedProviderRows { direct },
    })
}

impl ProviderPlanCandidate<UnpreparedProviderRows> {
    pub(crate) fn toolchain(plan: ProviderPlan) -> Self {
        let direct = vec![None; plan.rows.len()];
        Self {
            source: ProviderPlanCandidateSource::Toolchain(plan),
            rows: UnpreparedProviderRows { direct },
        }
    }

    /// Transfer original ownership into completion-ready inputs after constant
    /// closure. No source plan or direct payload is cloned or projected here.
    pub(crate) fn prepare_row_declarations(
        self,
        typed: &checked_trees::typed_trees::TypedTrees,
        exact_toolchain_sources: &[(source::SourceId, [u8; 32])],
    ) -> Result<ProviderPlanCandidate, Diagnostic> {
        let placed = crate::SealedPlacedAccessor::rejoin_plan(
            crate::ProviderPlanningContext::new(typed, source::DependencyScope::Product),
            self.plan(),
        );
        let Self { source, rows } = self;
        let mut prepared = Vec::with_capacity(rows.direct.len());
        for (row_index, direct) in rows.direct.into_iter().enumerate() {
            let authored = match &source {
                ProviderPlanCandidateSource::Derived(source) => source
                    .provenance
                    .row_realizations
                    .get(row_index)
                    .and_then(crate::ProviderRowRealization::authored)
                    .is_some(),
                ProviderPlanCandidateSource::Toolchain(_) => false,
            };
            let declarations = if authored {
                let row = source.direct_source(row_index)?;
                let identity = crate::service_declarations::exact_identity(
                    typed,
                    row.schema.symbol(),
                    exact_toolchain_sources,
                );
                Some(ProviderRowDeclarations {
                    intrinsic: matches!(row.row.binding, ProviderBinding::CompilerIntrinsic { .. })
                        .then(|| {
                            crate::intrinsic_declarations::IntrinsicDeclaration::prepare(
                                typed,
                                row.plan,
                                row.schema,
                                row.row,
                                row.requirement,
                                row.realization,
                                identity,
                            )
                        }),
                    exit_process: crate::intrinsic_declarations::ExitProcessDeclaration::prepare(
                        typed,
                        row.plan,
                        row.row,
                        row.schema.symbol(),
                        row.requirement,
                        row.realization,
                    ),
                })
            } else {
                None
            };
            prepared.push(ProviderRow {
                declarations,
                direct,
            });
        }
        Ok(ProviderPlanCandidate {
            source,
            rows: PreparedProviderRows {
                placed,
                rows: prepared,
            },
        })
    }
}
