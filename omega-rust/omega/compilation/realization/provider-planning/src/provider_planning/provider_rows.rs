// We capture source declaration details needed to generate direct boundary calls
// and placed memory accesses (`CapturedProviderRows`) while keeping each
// operation paired with its original candidate (`ProviderPlanCandidate`).
// Direct-call identity errors are reported when attaching Calling details
// (`attach_physical_rows`), while intrinsic and exit-process declaration errors
// wait until after `ProgramEntry` checks (`finish_row_declarations`).

use crate::DerivedProviderPlan;
use diagnostics::Diagnostic;
use effects::provider_plan::{ProviderBinding, ProviderPlan, ProviderPlanRow};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeferredProviderRows {
    direct: Vec<Option<DirectBoundaryInput>>,
    declarations: Result<RowDeclarations, Diagnostic>,
}

pub(crate) struct CapturedProviderRows {
    direct: Result<Vec<Option<DirectBoundaryDeclaration>>, Vec<Diagnostic>>,
    declarations: Result<RowDeclarations, Diagnostic>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct RowDeclarations {
    placed: Result<Option<crate::SealedAccessor>, Diagnostic>,
    rows: Vec<Option<ProviderRowDeclarations>>,
}

#[derive(Clone)]
struct DirectBoundaryDeclaration {
    row_index: usize,
    declaration_identity: String,
    callable_identity: String,
    realization_owner: String,
    realization_package: Option<semantic_vocabulary::PackageKeyIdentity>,
    intrinsic: bool,
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

fn capture_direct_declarations(
    source: &DerivedProviderPlan,
    services: checked_trees::typed_trees::service_declarations::ServiceCatalog<'_>,
) -> Result<Vec<Option<DirectBoundaryDeclaration>>, Vec<Diagnostic>> {
    let typed = services.context().typed;
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
        let context = crate::ProviderPlanningContext::new(typed, source::DependencyScope::Product);
        let Some(machine) = (match source.provenance.row_realizations.get(row_index) {
            Some(crate::ProviderRowRealization::Authored(symbol)) => {
                typed.machine_by_symbol(*symbol)
            }
            Some(crate::ProviderRowRealization::BuildNative(origin)) => {
                typed.machines().iter().find(|machine| {
                    !machine.body_is_present
                        && typed
                            .machine_trait_conformances(machine)
                            .iter()
                            .any(|edge| edge.requirement_symbol == origin.requirement)
                        && crate::target_machines::realizes_machine(
                            context,
                            machine,
                            Some(source.plan.target.as_str()),
                        )
                })
            }
            _ => None,
        }) else {
            continue;
        };
        if machine.body_is_present {
            continue;
        }
        // A table binding reads one runtime `addr` field of the record its
        // leaf borrows. Once that record is known, whether it has the field is
        // a fact of the source, the same on every target and whether or not
        // the program entry reaches the leaf, so we refuse it here for every
        // candidate. The later consumers report it only from a selected plan
        // or a reached leaf, which let an unreached leaf name a field its
        // table does not have. A leaf whose table does not resolve from its
        // first parameter keeps its deferred refusal.
        if let ProviderBinding::VtableField {
            field,
            storage: Err(reason),
            ..
        }
        | ProviderBinding::TableFunction {
            field,
            storage: Err(reason),
            ..
        } = &row.binding
            && crate::dispatch_tables::dispatch_table(typed, machine).is_ok()
        {
            return Err(vec![Diagnostic::error(format!(
                "cannot realize table field `{field}` for `{}`: {reason}",
                row.method
            ))]);
        }
        let callable_identity = typed
            .machine_callable_identity_text(machine)
            .ok_or_else(|| {
                vec![Diagnostic::error(
                    "boundary leaf has no exact callable identity",
                )]
            })?;
        let requirement = source
            .plan
            .identities
            .text(row.declared_requirement_identity);
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
        let intrinsic = matches!(row.binding, ProviderBinding::CompilerIntrinsic { .. });
        let declaration_identity = match typed.normalized_hermetic_symbol_identity(machine.symbol) {
            Ok(identity) => identity,
            Err(_) if intrinsic => format!(
                "program-local::{}",
                typed.symbols.display_path(machine.symbol, "::")
            ),
            Err(reason) => return Err(vec![Diagnostic::error(reason)]),
        };
        direct[row_index] = Some(DirectBoundaryDeclaration {
            row_index,
            declaration_identity,
            callable_identity,
            realization_owner: typed.symbols.display_path(machine.symbol, "::"),
            realization_package: typed.symbols.symbol_package_identity(machine.symbol),
            intrinsic,
        });
    }
    Ok(direct)
}

impl CapturedProviderRows {
    pub(crate) fn capture(
        source: &DerivedProviderPlan,
        services: checked_trees::typed_trees::service_declarations::ServiceCatalog<'_>,
        exact_toolchain_sources: &[(source::SourceId, [u8; 32])],
    ) -> Self {
        Self {
            direct: capture_direct_declarations(source, services),
            declarations: capture_row_declarations(
                source,
                services.context().typed,
                exact_toolchain_sources,
            ),
        }
    }

    pub(crate) fn attach_physical_rows(
        self,
        source: DerivedProviderPlan,
        context: &super::external_binding_rows::ExternalBindingContext<'_>,
    ) -> Result<ProviderPlanCandidate<DeferredProviderRows>, Vec<Diagnostic>> {
        let direct =
            self.direct?
                .into_iter()
                .map(|declaration| {
                    declaration.map(|declaration| {
                        let mechanism = if declaration.intrinsic {
                            DirectBoundaryMechanism::Intrinsic
                        } else {
                            let binding = context
                                .row(&source.plan, declaration.row_index)
                                .and_then(|row| {
                                    row.ok_or_else(|| {
                                        vec![Diagnostic::error(
                                            "foreign leaf lost its normalized external binding",
                                        )]
                                    })
                                });
                            DirectBoundaryMechanism::Foreign(binding)
                        };
                        DirectBoundaryInput {
                            row_index: declaration.row_index,
                            declaration_identity: declaration.declaration_identity,
                            callable_identity: declaration.callable_identity,
                            realization_owner: declaration.realization_owner,
                            realization_package: declaration.realization_package,
                            mechanism,
                        }
                    })
                })
                .collect();
        Ok(ProviderPlanCandidate {
            source,
            rows: DeferredProviderRows {
                direct,
                declarations: self.declarations,
            },
        })
    }
}

fn capture_row_declarations(
    source: &DerivedProviderPlan,
    typed: &checked_trees::typed_trees::TypedTrees,
    exact_toolchain_sources: &[(source::SourceId, [u8; 32])],
) -> Result<RowDeclarations, Diagnostic> {
    let placed = crate::SealedAccessor::rejoin_plan(
        crate::ProviderPlanningContext::new(typed, source::DependencyScope::Product),
        &source.plan,
    );
    let mut rows = Vec::with_capacity(source.plan.rows.len());
    for row_index in 0..source.plan.rows.len() {
        let authored = source
            .provenance
            .row_realizations
            .get(row_index)
            .and_then(crate::ProviderRowRealization::authored)
            .is_some();
        let declarations = if authored {
            let row = derived_direct_source(source, row_index)?;
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
        rows.push(declarations);
    }
    Ok(RowDeclarations { placed, rows })
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
