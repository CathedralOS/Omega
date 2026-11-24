//! Finish a Product's complete source worklist before native realization.
//! Layout retries and logical provider closure stay inside this owner. A retry
//! uses the original unprepared source, so demand remains lazy and target scoped.

use crate::driver::judge_lowered_source;
use crate::driver::resolve_and_type;
use checked_program_to_psi::provider_choice_inputs;
use checked_program_to_psi::provider_supply;
use diagnostics::Diagnostic;
use std::sync::Arc;

mod judge_product_roster;
use judge_product_roster::prepare_and_judge;

mod behavior_exclusions;
pub(crate) mod declaration_lookup;
pub use declaration_lookup::ProductDeclarations;
mod declarations;
mod entry_contracts;
mod entry_shapes;
pub(crate) mod layout_scopes;
pub mod native_binding_declarations;
mod native_bindings;
mod product_policy;
mod provider_requests;
use product_policy::AdmittedProductPolicy;
pub(crate) use product_policy::capture_package_native_bindings;
pub mod provider_choices;
pub mod provider_declarations;
pub(crate) mod provider_settlement;
mod retained_application;
pub mod source_targets;

pub use native_bindings::{NativeBindingInputs, PackageNativeBindings};

/// The source descriptions and events returned by the selected Build execution.
#[derive(Clone, Default)]
pub struct ProductPolicyInputs<'a> {
    pub activation: Option<&'a source_admission::RetainedDeclaration>,
    pub declared: &'a [Arc<crate::declaration_selection::ProductDeclarationRequest>],
    pub executed: &'a [crate::declaration_selection::ExecutedProductSelection],
    pub native_bindings: &'a [crate::declaration_selection::ExecutedNativeBinding],
    pub exclusions: &'a [crate::declaration_selection::ExecutedEnumExclusion],
}

impl ProductPolicyInputs<'_> {
    pub(crate) fn admit(
        &self,
        typed: &mut typed_trees::TypedTrees,
        subject: Option<semantic_vocabulary::PackageKeyIdentity>,
        test: Option<&crate::registered_tests::TestRoot>,
    ) -> Result<(Option<symbols::SymbolHandle>, AdmittedProductPolicy), Vec<Diagnostic>> {
        let activation = self
            .activation
            .map(|declaration| {
                crate::source_state::source_subjects::rejoin_build_activation(
                    typed,
                    subject,
                    declaration,
                )
                .map_err(|reason| vec![Diagnostic::error(reason)])
            })
            .transpose()?;
        let admitted = product_policy::admit_product_policy(typed, activation, self, test)?;
        Ok((activation, admitted))
    }
}

#[derive(Clone)]
pub struct ProductSourceInputs<'a> {
    /// Costs of the Build that opened this Product, when it belongs to a project.
    pub build_accounting: Option<&'a crate::compile_time_invocations::SourceCompletionAccounting>,
    pub declaration_scope: Option<Arc<declaration_scope::PackageDeclarationScope>>,
    pub build_host: semantic_vocabulary::source_selections::ProductTarget,
    /// The generated source admitted after this Build's authored source was captured.
    pub generated: Option<crate::SourceExtensionRequest>,
    pub targets: &'a [crate::compile_product::source_targets::SourceTarget],
    pub calling_targets: &'a [&'a str],
    pub subject: Option<semantic_vocabulary::PackageKeyIdentity>,
    /// Entry selections from the authored Build or the core default.
    pub root_binding_requests: &'a [crate::declaration_selection::ProductEntryBinding],
    pub policy: ProductPolicyInputs<'a>,
    /// Package-authenticated declarations and closed operands from dependency Builds.
    pub default_provider_selections: &'a [retained_product_selections::RetainedProviderSelection],
    pub native_bindings: NativeBindingInputs<'a>,
    pub optimizations: optimization::PsiOptimizationSelections,
}

pub(crate) struct CompletedProductSource {
    pub program: Arc<crate::source_state::FrozenTypedTrees>,
    pub grants: Vec<symbols::root_grants::AuthoredRootGrant>,
    pub declarations: crate::compile_product::declarations::AdmittedProductDeclarations,
    pub exclusions: Vec<behavior_exclusion_requests::AuthoredBehaviorExclusion>,
    pub(crate) terminal_publication: Arc<judge_lowered_source::SourcePublication>,
    review_package: bool,
    deployments: Vec<String>,
    pub(crate) explicit_providers: Vec<provider_choice_inputs::ProviderSelection>,
    pub(crate) native_supplies: Vec<provider_choice_inputs::NativeBuildSupply>,
}

impl CompletedProductSource {
    pub fn terminal_publication(&self) -> &Arc<judge_lowered_source::SourcePublication> {
        &self.terminal_publication
    }

    /// An occurrence reviews every native decision its own Build made. A dependency's
    /// Build has its own review, and only declarations present in this Product can use it.
    pub(crate) fn retain_package_review_bindings(
        &mut self,
        package: semantic_vocabulary::PackageKeyIdentity,
        target: &str,
    ) -> Result<(), Vec<Diagnostic>> {
        self.native_supplies
            .retain(|supply| supply.selection.target.canonical_name() == target);
        if self
            .native_supplies
            .iter()
            .all(|supply| supply.selection.supplier_package == package)
        {
            return Ok(());
        }
        let accepted = self.terminal_publication.accepted()?;
        let declared = accepted
            .boundary_requirements_declared()
            .map_err(|error| vec![error])?;
        let mut retained = Vec::new();
        for supply in std::mem::take(&mut self.native_supplies) {
            if supply.selection.supplier_package == package {
                retained.push(supply);
                continue;
            }
            let slot = supply.selection.subject;
            for requirement in declared.iter().filter(|row| row.slot == slot) {
                if supply
                    .selection
                    .requirement_identity_for(accepted.source(), requirement.requirement)
                    .map_err(|reason| vec![Diagnostic::error(reason)])?
                    .is_some()
                {
                    retained.push(supply);
                    break;
                }
            }
        }
        self.native_supplies = retained;
        Ok(())
    }

    /// Issue deployment facts from the same closed source and Build choices used for publication.
    pub fn prepare_provider_demand(
        &self,
        target: &str,
    ) -> Result<
        provider_supply::SourceProviderFacts<provider_supply::SourceProviderDemand>,
        Vec<Diagnostic>,
    > {
        if !self
            .deployments
            .iter()
            .any(|deployment| deployment == target)
        {
            return Err(vec![Diagnostic::error(format!(
                "source compilation did not select deployment `{target}`",
            ))]);
        }
        self.terminal_publication.prepare_provider_demand(
            Some(target),
            &self.explicit_providers,
            &self.native_supplies,
            self.review_package,
        )
    }
}

/// Which declaration supplies the entry of this Product judgment.
#[derive(Clone, Copy)]
pub(crate) enum ProductEntry<'a> {
    BuildBindings,
    PackageReview,
    Inspection(&'a str),
    RegisteredTest {
        root: &'a crate::registered_tests::TestRoot,
        target: &'a str,
    },
}

pub(crate) fn prepare_lower_and_check(
    continuation: crate::SourceTypingCheckpoint,
    mut inputs: ProductSourceInputs<'_>,
    entry: ProductEntry<'_>,
    accounting: &mut crate::compile_time_invocations::SourceCompletionAccounting,
    timings: &mut crate::CompileTimings,
) -> Result<CompletedProductSource, Vec<Diagnostic>> {
    let mut authored = continuation.complete_authored(timings)?;
    let typed = authored.authored_program_mut();
    let test = match entry {
        ProductEntry::RegisteredTest { root, .. } => Some(root),
        _ => None,
    };
    let (activation, admitted) = inputs.policy.admit(typed, inputs.subject, test)?;
    let native_supplies = inputs
        .native_bindings
        .resolve(typed, admitted.native_bindings)?;
    let resolve_and_type::CompletedTypedSource {
        mut typed,
        accounting: initial_source_charges,
    } = match inputs.generated.take() {
        Some(extension) => authored.extend(extension, inputs.declaration_scope.clone(), timings)?,
        None => authored.finish(),
    };
    *accounting = initial_source_charges;
    accounting
        .validate_charges()
        .map_err(|reason| vec![Diagnostic::error(reason)])?;
    let default_provider_selections =
        crate::compile_product::provider_declarations::rejoin_provider_defaults(
            &mut typed,
            inputs.default_provider_selections,
        )?;
    let has_defaults = !default_provider_selections.is_empty()
        || typed.machines().iter().any(|machine| {
            machine.purpose == crate::source_state::machine::MachinePurpose::ProviderDefaults
        });
    let unprepared = has_defaults.then(|| typed.clone());
    let root_bindings = match entry {
        ProductEntry::RegisteredTest { root, target } => {
            vec![root.bind(&typed, &admitted.declarations.test_groups, target)?]
        }
        _ => crate::compile_product::declaration_lookup::collect_root_bindings(
            &typed,
            inputs.root_binding_requests,
        )?,
    };
    let mut pass = ProductSourcePass {
        inputs,
        activation,
        explicit_providers: admitted.providers,
        representations: admitted.representations,
        declarations: admitted.declarations,
        native_supplies,
        root_bindings,
        default_provider_selections,
        entry,
        initial_accounting: accounting.clone(),
        accounting,
        timings,
    };
    let mut layouts = Vec::new();
    let mut judged = prepare_and_judge(&mut pass, typed, &layouts)?;
    loop {
        use provider_settlement::{ProductSupplySource, ProviderWork, next_provider_work};
        let work = pass.timings.record_result(
            artifacts::compile_timings::source::PROVIDER_SETTLEMENT,
            || {
                next_provider_work(ProductSupplySource {
                    targets: pass.inputs.targets,
                    publication: &judged.publication,
                    build: &pass.explicit_providers,
                    native_bindings: &pass.native_supplies,
                    defaults: &judged.provider_defaults,
                    layout_prepared: &layouts,
                })
            },
        )?;
        match work {
            ProviderWork::Complete => break,
            ProviderWork::JudgeProviders(supplies) => {
                let mut roster = judged.publication.roster()?.clone();
                roster.supplies.extend(supplies);
                let source = judged.publication.accepted()?.prepared().clone();
                judged.publication = Arc::new(judge_lowered_source::SourcePublication::judge(
                    source,
                    roster,
                    pass.inputs.optimizations.clone(),
                    pass.timings,
                )?);
            }
            ProviderWork::PrepareLayouts(chosen) => {
                layouts = chosen;
                let typed = unprepared.clone().ok_or_else(|| vec![Diagnostic::error(
                    "a target chose a provider default absent from the source preparation frontier"
                )])?;
                judged = prepare_and_judge(&mut pass, typed, &layouts)?;
            }
        }
    }
    // Layout retries can materialize additional Product receipts. Check the final
    // charges and the originating Build's charges in their own evaluation contexts.
    pass.accounting
        .validate_charges()
        .map_err(|reason| vec![Diagnostic::error(reason)])?;
    if let Some(build_accounting) = pass.inputs.build_accounting {
        build_accounting
            .validate_charges()
            .map_err(|reason| vec![Diagnostic::error(reason)])?;
    }
    Ok(CompletedProductSource {
        review_package: matches!(pass.entry, ProductEntry::PackageReview),
        program: judged.program,
        terminal_publication: judged.publication,
        deployments: pass
            .inputs
            .calling_targets
            .iter()
            .map(|target| (*target).to_owned())
            .collect(),
        explicit_providers: pass.explicit_providers,
        grants: admitted.grants,
        declarations: pass.declarations,
        exclusions: admitted.exclusions,
        native_supplies: pass.native_supplies,
    })
}

struct ProductSourcePass<'a, 'f> {
    inputs: ProductSourceInputs<'a>,
    activation: Option<symbols::SymbolHandle>,
    explicit_providers: Vec<provider_choice_inputs::ProviderSelection>,
    representations:
        Vec<crate::source_state::opaque_representations::OpaqueRepresentationSelection>,
    declarations: crate::compile_product::declarations::AdmittedProductDeclarations,
    native_supplies: Vec<provider_choice_inputs::NativeBuildSupply>,
    root_bindings: Vec<crate::RootBinding>,
    default_provider_selections: Vec<provider_choice_inputs::ProviderSelection>,
    entry: ProductEntry<'f>,
    initial_accounting: crate::compile_time_invocations::SourceCompletionAccounting,
    accounting: &'f mut crate::compile_time_invocations::SourceCompletionAccounting,
    timings: &'f mut crate::CompileTimings,
}

struct JudgedSource {
    program: Arc<crate::source_state::FrozenTypedTrees>,
    publication: Arc<judge_lowered_source::SourcePublication>,
    provider_defaults: Vec<provider_choice_inputs::TargetProviderDefaults>,
}
