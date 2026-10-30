//! Complete checked-program provider demand, before target choice.
//!
//! These are invocation-local declaration coordinates, not executable reach or
//! portable owners. Inventory construction is infallible and independent of the
//! fallible external-provider projection, preserving that projection's phase.

use crate::CheckedTrees;
use diagnostics::Diagnostic;
use language_semantics::direct_calls::DirectCallDeclarations;
use std::collections::HashSet;
use std::sync::{Arc, OnceLock};
use symbols::SymbolHandle;

pub struct PreparedProviderInventory {
    checked: Arc<CheckedTrees>,
    uses: OnceLock<HashSet<SymbolHandle>>,
    direct_calls: OnceLock<DirectCallDeclarations<SymbolHandle>>,
    requirements: OnceLock<Vec<DeclarationProviderRequirements>>,
}

impl PreparedProviderInventory {
    pub fn new(checked: Arc<CheckedTrees>) -> Self {
        Self {
            checked,
            uses: OnceLock::new(),
            direct_calls: OnceLock::new(),
            requirements: OnceLock::new(),
        }
    }

    pub fn program(&self) -> &Arc<CheckedTrees> {
        &self.checked
    }

    pub fn direct_calls_for(
        &self,
        checked: &CheckedTrees,
    ) -> Result<&DirectCallDeclarations<SymbolHandle>, Diagnostic> {
        self.require_owner(checked)?;
        Ok(self.direct_calls())
    }

    pub fn direct_calls(&self) -> &DirectCallDeclarations<SymbolHandle> {
        self.direct_calls.get_or_init(|| {
            typed_trees::direct_calls::collect(
                &self.checked.typed,
                source::DependencyScope::Product,
            )
        })
    }

    fn require_owner(&self, checked: &CheckedTrees) -> Result<(), Diagnostic> {
        if !std::ptr::eq(self.checked.as_ref(), checked) {
            return Err(Diagnostic::error(
                "provider declarations belong to another checked program",
            ));
        }
        Ok(())
    }

    /// Project exact demand into an already selected declaration roster.
    /// Preparing all-body references is independent of choosing these roots;
    /// this query neither discovers provider edges nor grants execution.
    pub fn requirements_for_declarations(
        &self,
        checked: &CheckedTrees,
        declarations: &typed_trees::references::DeclarationSelection,
    ) -> Result<ProviderApplicationRequirements, Diagnostic> {
        self.require_owner(checked)?;
        let mut result = ProviderApplicationRequirements::default();
        for row in self
            .requirements()
            .iter()
            .filter(|row| declarations.contains(row.owner))
        {
            for &(coordinate, template) in &row.requirements {
                result.coordinates.insert(coordinate);
                result.templates.insert(template);
            }
        }
        Ok(result)
    }

    /// Complete all-body uses remain distinct from an application projection.
    pub fn uses_for(&self, checked: &CheckedTrees) -> Result<&HashSet<SymbolHandle>, Diagnostic> {
        self.require_owner(checked)?;
        Ok(self.uses.get_or_init(|| {
            self.requirements()
                .iter()
                .flat_map(|row| row.requirements.iter().map(|(_, template)| *template))
                .collect()
        }))
    }

    fn requirements(&self) -> &[DeclarationProviderRequirements] {
        self.requirements.get_or_init(|| {
            let mut rows = Vec::new();
            let template = |coordinate| {
                self.checked
                    .typed
                    .closed_requirement_application(coordinate)
                    .map_or(coordinate, |application| application.template)
            };
            // Associate each implicit operation with its exact generational
            // arena site once, rather than scanning all operations per owner.
            let mut expression_requirements = Vec::new();
            let mut statement_requirements = Vec::new();
            use crate::operators::CheckedBoundaryOperatorApplicationUseSite;
            for (site, requirement) in self
                .checked
                .facts
                .operators
                .boundary_applications
                .iter()
                .map(|application| (&application.site, application.requirement_symbol))
                .chain(
                    self.checked
                        .facts
                        .operators
                        .symbolic_boundary_applications
                        .iter()
                        .map(|application| (&application.site, application.requirement_symbol)),
                )
            {
                match site {
                    CheckedBoundaryOperatorApplicationUseSite::Expression {
                        expression, ..
                    }
                    | CheckedBoundaryOperatorApplicationUseSite::MatchEquality {
                        expression, ..
                    } => {
                        expression_requirements.push((
                            (expression.arena_index(), expression.generation()),
                            requirement,
                        ));
                    }
                    CheckedBoundaryOperatorApplicationUseSite::Statement(statement) => {
                        statement_requirements.push((
                            (statement.arena_index(), statement.generation()),
                            requirement,
                        ));
                    }
                }
            }
            expression_requirements.sort_unstable_by_key(|(site, _)| *site);
            statement_requirements.sort_unstable_by_key(|(site, _)| *site);
            typed_trees::references::for_each_declaration_references(
                &self.checked.typed,
                |owner, references| {
                    let mut coordinates = references.callable_uses;
                    for expression in &references.expressions {
                        coordinates.extend(requirements_at_site(
                            &expression_requirements,
                            (expression.arena_index(), expression.generation()),
                        ));
                    }
                    for statement in &references.statements {
                        coordinates.extend(requirements_at_site(
                            &statement_requirements,
                            (statement.arena_index(), statement.generation()),
                        ));
                    }
                    let mut requirements = coordinates
                        .into_iter()
                        .map(|coordinate| (coordinate, template(coordinate)))
                        .collect::<Vec<_>>();
                    requirements.sort_unstable_by_key(|(coordinate, _)| {
                        (coordinate.arena_index(), coordinate.generation())
                    });
                    if !requirements.is_empty() {
                        rows.push(DeclarationProviderRequirements {
                            owner,
                            requirements,
                        });
                    }
                },
            );
            // Descriptor operations may have no callable source expression.
            // Their exact checked provider owner determines scoped membership.
            for row in self.checked.facts.provider_closed_requirements.iter() {
                rows.push(DeclarationProviderRequirements {
                    owner: row.provider,
                    requirements: vec![(row.closed_requirement, template(row.closed_requirement))],
                });
            }
            rows
        })
    }
}

struct DeclarationProviderRequirements {
    owner: SymbolHandle,
    /// Each complete coordinate stays paired with its declaration template.
    requirements: Vec<(SymbolHandle, SymbolHandle)>,
}

#[derive(Default)]
pub struct ProviderApplicationRequirements {
    pub coordinates: HashSet<SymbolHandle>,
    pub templates: HashSet<SymbolHandle>,
}

fn requirements_at_site<Key: Ord + Copy>(
    requirements: &[(Key, SymbolHandle)],
    site: Key,
) -> impl Iterator<Item = SymbolHandle> + '_ {
    let start = requirements.partition_point(|(candidate, _)| *candidate < site);
    requirements[start..]
        .iter()
        .take_while(move |(candidate, _)| *candidate == site)
        .map(|(_, requirement)| *requirement)
}
