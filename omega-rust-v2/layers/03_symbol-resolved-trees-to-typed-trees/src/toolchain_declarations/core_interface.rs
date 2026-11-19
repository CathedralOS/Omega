//! Install a library's public declarations in the caller's type catalog. Its predicate nodes live in its own typed forest.
use crate::contracts::effect_propagation::ExecutionCalls;
use crate::declarations::declaration_catalog::{DeclarationCatalog, DeclaredItems};
use crate::type_system::{formation::FormationSites, type_catalog::DeclaredTypes};
use crate::{TypedStage, TypedTrees, ValueType};
use omega_toolchain::CoreInterface;

pub(crate) struct CheckedLibrary {
    trees: TypedTrees<'static>,
    items: DeclaredItems<'static>,
    types: DeclaredTypes<'static>,
    formations: FormationSites,
    pub(crate) execution: ExecutionCalls,
}

pub(crate) fn checked_library() -> &'static CheckedLibrary {
    static LIBRARY: std::sync::OnceLock<CheckedLibrary> = std::sync::OnceLock::new();
    LIBRARY.get_or_init(|| {
        let core = omega_toolchain::core_interface();
        CheckedLibrary::check(&core.declarations, Some(core)).expect("typed standard interface")
    })
}

pub fn standard_library() -> &'static TypedTrees<'static> {
    &checked_library().trees
}

impl CheckedLibrary {
    fn check(
        resolved: &'static omega_resolver::SymbolResolvedTrees<'static>,
        core: Option<&'static CoreInterface>,
    ) -> Result<Self, crate::TypeError> {
        let mut facts = TypedStage::default();
        let mut catalog = DeclarationCatalog::default();
        let execution = crate::typing::type_stage::check_stage(resolved, &Default::default(), &mut catalog, &mut facts, core, None)?;
        catalog.publish_declarations(&mut facts);
        Ok(Self {
            trees: TypedTrees::new(resolved.clone(), facts, 0),
            items: catalog.declarations,
            types: catalog.types.declarations,
            formations: catalog.types.formations.into_sites(),
            execution,
        })
    }
    pub(crate) fn import<'a>(
        &'static self,
        output: &mut TypedStage,
    ) -> DeclarationCatalog<'a> {
        output.library = Some(&self.trees);
        output.signatures.clone_from(&self.trees.signatures);
        output.callable_bounds.clone_from(&self.trees.callable_bounds);
        output.machine_effects.clone_from(&self.trees.machine_effects);
        output.data_properties.clone_from(&self.trees.data_properties);
        output.obligations.extend(
            self.trees
                .obligations
                .iter()
                .filter(|proof| matches!(proof.kind, crate::ObligationKind::VerifyLibraryImplementation(_)))
                .cloned(),
        );
        let mut catalog = DeclarationCatalog {
            declarations: self.items.clone(),
            ..Default::default()
        };
        catalog.types.declarations = self.types.clone();
        catalog.types.formations.imported = Some(&self.formations);
        catalog
    }
}

pub(crate) fn configure_roles<'a>(
    core: &'a CoreInterface,
    catalog: &mut DeclarationCatalog<'a>,
    output: &mut TypedStage,
) {
    for &machine in &catalog.local_machines {
        let signature = &catalog.signatures[machine];
        if signature.has_guarantee && core.supplies(machine) && core.collection_primitive(machine).is_none() {
            output.obligations.push(crate::TypingObligation {
                node: signature.node,
                site: signature.site.clone(),
                kind: crate::ObligationKind::VerifyLibraryImplementation(machine.clone()),
            });
        }
    }
    catalog.access_plan_requirement = Some(core.access_plan_requirement());
    for (view, canonical_default) in core.ranking_views() {
        let signature = &catalog.declarations.signatures[view];
        let subject = match signature.parameters.as_slice() {
            [(_, single)] => single.clone(),
            tuple => ValueType::Tuple(tuple.iter().map(|(_, ty)| ty.clone()).collect()),
        };
        catalog
            .declarations
            .ranking_views
            .to_mut()
            .push(crate::RankingView {
                declaration: view.clone(),
                subject,
                rank: signature.result.clone(),
                parameters: vec![],
                canonical_default,
            });
    }
}

#[cfg(test)]
#[path = "../../tests/support/checked_library.rs"]
mod tests;
