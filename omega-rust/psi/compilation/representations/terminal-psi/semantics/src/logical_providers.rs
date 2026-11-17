//! Close chosen provider bodies over accepted canonical declarations.
//! ABI planning consumes these exact choices and cannot select replacements.

use semantic_vocabulary::{BoundaryMachineId, PackageKeyIdentity};
use terminal_psi::{TerminalModule, TerminalProviderSelection, TerminalServiceSchema};
use crate::provider_candidates::{CheckedProviderQuery, CheckedProviderRow, selected_candidate_positions};

fn text(
    module: &TerminalModule,
    index: terminal_psi::TerminalIdentityIndex,
) -> Result<&str, String> {
    module
        .identities
        .text(index)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| "provider realization contains an unresolved canonical declaration".into())
}

struct Candidate<'a> {
    provider: terminal_psi::TerminalIdentityIndex,
    owner: terminal_psi::TerminalIdentityIndex,
    requirement: &'a str,
    source_callable: &'a str,
    row: &'a terminal_psi::ProviderCandidateConformance,
}

fn candidates(module: &TerminalModule) -> Result<Vec<Candidate<'_>>, String> {
    let mut rows = Vec::with_capacity(module.provider_candidates.len());
    for row in &module.provider_candidates {
        let body = module
            .machine(row.candidate)
            .ok_or("canonical provider candidate has no executable body")?;
        let declared = body
            .declaration
            .as_ref()
            .ok_or("canonical provider candidate has no checked declaration")?;
        if declared.callable != row.candidate_identity {
            return Err("canonical provider candidate changes its checked body identity".into());
        }
        module
            .boundary_machine(row.boundary)
            .ok_or("canonical provider candidate lost its required boundary")?;
        text(module, row.provider_identity)?;
        text(module, row.provider_declaration.identity())?;
        rows.push(Candidate {
            provider: row.provider_identity,
            owner: row.provider_declaration.identity(),
            requirement: text(module, row.source_requirement_identity)?,
            source_callable: text(module, row.source_candidate_identity)?,
            row,
        });
    }
    rows.sort_by_key(|row| {
        (
            row.provider,
            row.owner,
            row.requirement,
            row.source_callable,
        )
    });
    Ok(rows)
}

fn chosen_candidates<'a, 'rows>(
    choice: &TerminalProviderSelection,
    candidates: &'rows [Candidate<'a>],
) -> &'rows [Candidate<'a>] {
    let key = (choice.provider_identity, choice.provider_declaration);
    let begin = candidates.partition_point(|row| (row.provider, row.owner) < key);
    let end = begin + candidates[begin..].partition_point(|row| (row.provider, row.owner) == key);
    &candidates[begin..end]
}

fn schema_is_demanded(
    module: &TerminalModule,
    schema: &TerminalServiceSchema,
    candidates: &[Candidate<'_>],
    reached: &[BoundaryMachineId],
) -> Result<bool, String> {
    for &boundary in reached {
        let declaration = module
            .boundary_machine(boundary)
            .ok_or("provider demand names an absent boundary")?;
        let requirement = text(module, declaration.identity)?;
        if schema
            .surface
            .methods
            .iter()
            .any(|method| schema.surface.identity_text(method.requirement_identity) == requirement)
            || candidates.iter().any(|candidate| {
                candidate.row.boundary == boundary
                    && schema.surface.methods.iter().any(|method| {
                        schema.surface.identity_text(method.requirement_identity)
                            == candidate.requirement
                    })
            })
        {
            return Ok(true);
        }
    }
    Ok(false)
}

/// Preserve the exact failed canonical join for a caller that retains source
/// labels. Labels are diagnostic only; matching continues to use these IDs.
#[derive(Debug)]
pub enum ProviderSelectionError {
    MissingSupplies {
        target: String,
        boundaries: Vec<BoundaryMachineId>,
    },
    MissingCheckedRow {
        provider_declaration: terminal_psi::TerminalIdentityIndex,
        provider_identity: terminal_psi::TerminalIdentityIndex,
        schema_declaration: terminal_psi::TerminalIdentityIndex,
        schema_application: terminal_psi::TerminalIdentityIndex,
        requirement_identity: String,
    },
    ExistingDiagnostics(String),
}

impl From<String> for ProviderSelectionError {
    fn from(reason: String) -> Self {
        Self::ExistingDiagnostics(reason)
    }
}

impl From<&str> for ProviderSelectionError {
    fn from(reason: &str) -> Self {
        Self::ExistingDiagnostics(reason.to_owned())
    }
}

impl ProviderSelectionError {
    pub fn diagnostic(&self, module: &TerminalModule) -> String {
        match self {
            Self::MissingSupplies { target, boundaries } => format!(
                "reached requirements have no normalized chosen provider or exact native leaf: target `{target}`, boundaries {boundaries:?}"
            ),
            Self::MissingCheckedRow {
                provider_identity,
                requirement_identity,
                ..
            } => match module.identities.text(*provider_identity) {
                Some(provider) => format!(
                    "chosen provider `{provider}` has no proved row for `{requirement_identity}`"
                ),
                None => {
                    "chosen provider missing a checked row also lost its diagnostic identity".into()
                }
            },
            Self::ExistingDiagnostics(reason) => reason.clone(),
        }
    }
}


#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChosenProviderRow {
    pub requirement: String,
    pub machine: String,
    pub machine_package: Option<PackageKeyIdentity>,
    pub candidate_positions: Vec<usize>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LogicalProvider {
    pub choice: TerminalProviderSelection,
    pub schema: TerminalServiceSchema,
    pub rows: Vec<ChosenProviderRow>,
    pub origin_package: Option<PackageKeyIdentity>,
    pub covered: Vec<BoundaryMachineId>,
}

pub fn satisfies_plan_name(target: &str, trait_name: &str, provider_type: &str) -> String {
    match (target.is_empty(), provider_type.is_empty()) {
        (true, true) => format!("satisfies::{trait_name}"),
        (false, true) => format!("{target}::satisfies::{trait_name}"),
        (true, false) => format!("{provider_type}::satisfies::{trait_name}"),
        (false, false) => format!("{target}::{provider_type}::satisfies::{trait_name}"),
    }
}

fn selected_logical_providers(
    module: &TerminalModule,
    target: &str,
    reached: &[BoundaryMachineId],
) -> Result<Vec<LogicalProvider>, ProviderSelectionError> {
    let candidates = candidates(module)?;
    let mut lifetime_families = module.closed_conformance_applications.iter()
        .flat_map(|application| &application.schemas)
        .filter(|schema| schema.lifetime_parameter_count != 0)
        .map(|schema| text(module, schema.public_requirement_identity))
        .collect::<Result<Vec<_>, _>>()?;
    lifetime_families.sort_unstable();
    lifetime_families.dedup();
    let mut selections = Vec::new();
    for choice in module.provider_selections.iter().filter(|choice| choice.target == target) {
        let schema = module.service_schema(choice.schema_declaration, choice.schema_application)?;
        let chosen = chosen_candidates(choice, &candidates);
        if !schema_is_demanded(module, schema, chosen, reached)? { continue; }
        let provider = text(module, choice.provider_identity)?;
        let owner = text(module, choice.provider_declaration)?;
        let mut package = None::<Option<PackageKeyIdentity>>;
        let mut query_rows = Vec::with_capacity(schema.surface.methods.len());
        for method in &schema.surface.methods {
            let requirement = schema.surface.identity_text(method.requirement_identity);
            let begin = chosen.partition_point(|row| row.requirement < requirement);
            let end = begin + chosen[begin..].partition_point(|row| row.requirement == requirement);
            let selected = &chosen[begin..end];
            let first = selected.first().ok_or_else(|| ProviderSelectionError::MissingCheckedRow {
                provider_declaration: choice.provider_declaration,
                provider_identity: choice.provider_identity,
                schema_declaration: schema.declaration,
                schema_application: schema.application,
                requirement_identity: requirement.to_owned(),
            })?;
            if selected.iter().any(|row| row.source_callable != first.source_callable
                || row.row.source_candidate_package_identity != first.row.source_candidate_package_identity) {
                return Err(format!("chosen provider `{provider}` has competing exact realizations for `{requirement}`").into());
            }
            if lifetime_families.binary_search(&requirement).is_ok() {
                return Err("selected provider lifetime family needs its exact retained partition coordinate".into());
            }
            let row_package = first.row.source_candidate_package_identity;
            if package.is_some_and(|previous| previous != row_package) {
                return Err("chosen map needs an unrepresented cross-package adapter closure".into());
            }
            package = Some(row_package);
            query_rows.push(CheckedProviderRow {
                requirement,
                declared_requirement: requirement,
                machine: first.source_callable,
                machine_package: row_package,
            });
        }
        let positions = selected_candidate_positions(CheckedProviderQuery {
            name: &satisfies_plan_name(target, schema.surface.identity_text(schema.surface.trait_name), provider),
            declaration: owner,
            provider,
            surface: &schema.surface,
            rows: &query_rows,
        }, module)?;
        if positions.len() != query_rows.len() || positions.iter().any(Vec::is_empty) {
            return Err("chosen plan does not rejoin every canonical checked candidate".into());
        }
        let rows = query_rows.into_iter().zip(positions).map(|(row, candidate_positions)| ChosenProviderRow {
            requirement: row.requirement.to_owned(),
            machine: row.machine.to_owned(),
            machine_package: row.machine_package,
            candidate_positions,
        }).collect();
        let mut covered = Vec::new();
        for &boundary in reached {
            let declaration = module.boundary_machine(boundary).ok_or("provider demand names an absent boundary")?;
            let identity = text(module, declaration.identity)?;
            if chosen.iter().any(|candidate| candidate.row.boundary == boundary)
                || schema.surface.methods.iter().any(|method| schema.surface.identity_text(method.requirement_identity) == identity) {
                covered.push(boundary);
            }
        }
        selections.push(LogicalProvider { choice: choice.clone(), schema: schema.clone(), rows, origin_package: package.flatten(), covered });
    }
    Ok(selections)
}

/// Logical provider choices fixed by source settlement. These records contain
/// exact canonical declarations and candidate coordinates, never source state.
/// Native planning may attach ABI plans but cannot rerun logical selection.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClosedLogicalProviders {
    program: terminal_psi::TerminalPsiIdentity,
    target: String,
    reached: Vec<BoundaryMachineId>,
    providers: Vec<LogicalProvider>,
}

impl ClosedLogicalProviders {
    /// Source settlement calls this while closing its body worklist. The caller
    /// supplies the identity of the accepted module being settled; receiving
    /// native admission checks that identity against its canonical input.
    pub fn select(
        module: &TerminalModule,
        program: terminal_psi::TerminalPsiIdentity,
        target: &str,
        reached: &[BoundaryMachineId],
    ) -> Result<Self, ProviderSelectionError> {
        Ok(Self {
            program,
            target: target.to_owned(),
            reached: reached.to_vec(),
            providers: selected_logical_providers(module, target, reached)?,
        })
    }

    pub fn program(&self) -> terminal_psi::TerminalPsiIdentity { self.program }
    pub fn target(&self) -> &str { &self.target }
    pub fn requirements(&self) -> &[BoundaryMachineId] { &self.reached }
    pub fn providers(&self) -> &[LogicalProvider] { &self.providers }

    pub fn require_program(&self, program: terminal_psi::TerminalPsiIdentity) -> Result<(), &'static str> {
        if self.program != program {
            return Err("logical provider choices belong to a different accepted program");
        }
        Ok(())
    }
}
