//! A carried type depends on its supplier's checked Product. The review binds
//! the exact nominal to that occurrence's source commitment. Public exposure
//! subsumes package-private use in this coarse compatibility dependency.
use super::declarations::unavailable;
use super::input::PolicySource;
use diagnostics::Diagnostic;
use review_evidence::record::{
    PackagePolicySemanticDependency, PackageReviewNominalIdentity, PackageReviewNominalOwner,
    PackageReviewSemanticDependencyExposure as Exposure,
};
use semantic_vocabulary::PackageKeyIdentity;
use source_provenance::PackageSourceConsumptionCommitment;
use std::cell::RefCell;
use std::collections::{BTreeMap, HashSet};

pub(super) struct CarriedTypes {
    suppliers: BTreeMap<PackageKeyIdentity, PackageSourceConsumptionCommitment>,
    // Only the emitted review reads this inventory. Checking and name resolution
    // use the accepted source's own declaration scope.
    uses: RefCell<
        BTreeMap<PackageReviewNominalIdentity, (PackageSourceConsumptionCommitment, Exposure)>,
    >,
}

impl CarriedTypes {
    pub(super) fn new(
        inputs: &package_compilation::PackageCompilationInputs,
        target: target::TargetProfile,
    ) -> Result<Self, Vec<Diagnostic>> {
        let mut suppliers = BTreeMap::new();
        for (scope, handoff) in inputs.dependency_build_handoff_instances() {
            if scope != package_compilation::declarations::DependencyPurpose::Product {
                continue;
            }
            if handoff.target() != target
                || suppliers
                    .insert(handoff.package(), handoff.source_consumption_commitment())
                    .is_some()
            {
                return Err(unavailable(
                    "a unique checked supplier source occurrence on this target",
                ));
            }
        }
        Ok(Self {
            suppliers,
            uses: RefCell::default(),
        })
    }

    pub(super) fn retain(
        &self,
        dependency: PackageReviewNominalIdentity,
        exposure: Exposure,
    ) -> Result<(), Vec<Diagnostic>> {
        let PackageReviewNominalOwner::Package(owner) = dependency.owner() else {
            return Err(unavailable("the package owner of a carried type"));
        };
        let commitment = self.suppliers.get(&owner).ok_or_else(|| {
            unavailable("the checked Product source commitment of a carried type's supplier")
        })?;
        let mut uses = self.uses.borrow_mut();
        let entry = uses.entry(dependency).or_insert((*commitment, exposure));
        if exposure == Exposure::PublicInterface {
            entry.1 = exposure;
        }
        Ok(())
    }
}

impl PolicySource<'_> {
    pub(super) fn carried_dependencies(
        &self,
        package: PackageKeyIdentity,
    ) -> Result<Vec<PackagePolicySemanticDependency>, Vec<Diagnostic>> {
        use source_compiler::representations::source_state::references::{
            DeclarationSelection, collect_program_declarations,
            collect_selected_program_type_occurrences, collect_type_symbol_dependencies,
        };
        let source = self.typed();
        let symbols = &source.symbols;
        let owners = DeclarationSelection::from_symbols(
            collect_program_declarations(source)
                .into_iter()
                .filter(|&owner| {
                    symbols.symbol_package_identity(owner) == Some(package)
                        && symbols.symbol_is_in_dependency_scope(
                            owner,
                            source_provenance::DependencyScope::Product,
                        )
                }),
        );
        let mut visited = HashSet::new();
        for occurrence in collect_selected_program_type_occurrences(source, &owners) {
            if !visited.insert(occurrence.reference) {
                continue;
            }
            for symbol in collect_type_symbol_dependencies(source, occurrence.reference) {
                if !symbols.symbols().nodes().is_valid(symbol) {
                    return Err(unavailable("a stale declaration in a carried type"));
                }
                if let Some(owner) = symbols.symbol_package_identity(symbol)
                    && owner != package
                    && matches!(
                        symbols.get(symbol).kind,
                        symbols::SymbolKind::Data
                            | symbols::SymbolKind::TypeAlias
                            | symbols::SymbolKind::Domain
                    )
                {
                    self.carried_types
                        .retain(self.nominal(symbol)?, Exposure::PrivateImplementation)?;
                }
            }
        }
        Ok(self
            .carried_types
            .uses
            .borrow()
            .iter()
            .map(|(dependency, &(supplier_sources, exposure))| {
                PackagePolicySemanticDependency::CarriedType {
                    dependency: dependency.clone(),
                    supplier_sources,
                    exposure,
                }
            })
            .collect())
    }
}
