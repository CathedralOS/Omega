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
    origins: OnceLock<Vec<language_semantics::provider_declarations::SelectedTargetMachineOrigin>>,
    direct_calls: OnceLock<DirectCallDeclarations<SymbolHandle>>,
    services: typed_trees::service_declarations::PreparedServiceCatalog,
}

impl PreparedProviderInventory {
    pub fn new(checked: Arc<CheckedTrees>) -> Self {
        // The Product projection has reached its final immutable Arc before
        // source-owned service formation. No target can extend this census.
        let services = typed_trees::service_declarations::PreparedServiceCatalog::prepare(
            typed_trees::provider_declarations::ProviderPlanningContext::new(
                &checked.typed,
                source::DependencyScope::Product,
            ),
        );
        Self {
            checked,
            uses: OnceLock::new(),
            origins: OnceLock::new(),
            direct_calls: OnceLock::new(),
            services,
        }
    }

    pub fn program(&self) -> &Arc<CheckedTrees> {
        &self.checked
    }

    pub fn services_for(
        &self,
        checked: &CheckedTrees,
    ) -> Result<typed_trees::service_declarations::ServiceCatalog<'_>, Diagnostic> {
        self.require_owner(checked)?;
        self.services.bind(
            typed_trees::provider_declarations::ProviderPlanningContext::new(
                &self.checked.typed,
                source::DependencyScope::Product,
            ),
        )
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

    pub fn origins_for(
        &self,
        checked: &CheckedTrees,
    ) -> Result<&[language_semantics::provider_declarations::SelectedTargetMachineOrigin], Diagnostic>
    {
        self.require_owner(checked)?;
        Ok(self.origins())
    }

    pub fn origins(
        &self,
    ) -> &[language_semantics::provider_declarations::SelectedTargetMachineOrigin] {
        self.origins.get_or_init(|| {
            typed_trees::provider_declarations::provider_target_declarations(
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

    /// Bind this private coordinate inventory to its original checked owner.
    /// Equal-looking source in another arena cannot reuse these coordinates.
    pub fn uses_for(&self, checked: &CheckedTrees) -> Result<&HashSet<SymbolHandle>, Diagnostic> {
        self.require_owner(checked)?;
        Ok(self.uses.get_or_init(|| {
            let template = |symbol| {
                self.checked
                    .typed
                    .closed_requirement_application(symbol)
                    .map_or(symbol, |application| application.template)
            };
            // A conformance's nominal reference is not an invocation. Keep the
            // same complete all-body callable inventory, not entry reachability.
            let mut uses =
                crate::typed_trees::references::collect_program_references(&self.checked.typed)
                    .callable_uses
                    .into_iter()
                    .map(template)
                    .collect::<HashSet<_>>();
            uses.extend(
                self.checked
                    .facts
                    .operators
                    .boundary_applications
                    .iter()
                    .map(|application| template(application.requirement_symbol)),
            );
            uses.extend(
                self.checked
                    .facts
                    .operators
                    .symbolic_boundary_applications
                    .iter()
                    .map(|application| template(application.requirement_symbol)),
            );
            // Implicit descriptor operations (including Slice index/range)
            // need not have a callable name in the source reference inventory.
            uses.extend(
                self.checked
                    .facts
                    .provider_closed_requirements
                    .iter()
                    .map(|row| row.template),
            );
            uses
        }))
    }
}
