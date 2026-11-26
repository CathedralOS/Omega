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
use judge_product_roster::{prepare_roster, product_provider_defaults, program_roots};

mod behavior_exclusions;
pub(crate) mod declaration_lookup;
pub use declaration_lookup::ProductDeclarations;
mod declarations;
mod entry_contracts;
mod entry_roots;
pub(super) use entry_roots::ProductRoots;
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
    /// Owner of the exact Build source selected by the issuing stage.
    pub activation_owner: Option<semantic_vocabulary::PackageKeyIdentity>,
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
                    self.activation_owner.or(subject),
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
    pub dependency_edges: Arc<[semantic_vocabulary::dependency_reach::DependencyServiceEdge]>,
    /// Costs of the Build that opened this Product, when it belongs to a project.
    pub build_accounting: Option<&'a crate::compile_time_invocations::SourceCompletionAccounting>,
    pub declaration_scope: Option<Arc<declaration_scope::PackageDeclarationScope>>,
    pub build_host: semantic_vocabulary::source_selections::ProductTarget,
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
    dependency_edges: Arc<[semantic_vocabulary::dependency_reach::DependencyServiceEdge]>,
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

    /// A target-free result still rechecks all the deployments the Product requested.
    pub(crate) fn admit_dependency_uses(&self) -> Result<(), Vec<Diagnostic>> {
        if self.dependency_edges.is_empty() {
            return Ok(());
        }
        if self.deployments.is_empty() {
            self.terminal_publication.prepare_provider_demand(
                None,
                &self.explicit_providers,
                &self.native_supplies,
                self.review_package,
                &self.dependency_edges,
            )?;
        } else {
            for target in &self.deployments {
                self.prepare_provider_demand(target)?;
            }
        }
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
            &self.dependency_edges,
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
    inputs: ProductSourceInputs<'_>,
    entry: ProductEntry<'_>,
    accounting: &mut crate::compile_time_invocations::SourceCompletionAccounting,
    timings: &mut crate::CompileTimings,
) -> Result<CompletedProductSource, Vec<Diagnostic>> {
    let resolve_and_type::CompletedSource {
        mut typed,
        accounting: initial_source_charges,
    } = continuation.complete(timings)?;
    let test = match entry {
        ProductEntry::RegisteredTest { root, .. } => Some(root),
        _ => None,
    };
    let (activation, mut admitted) = inputs.policy.admit(&mut typed, inputs.subject, test)?;
    // An interpreter choice has already had its source checked. Only the
    // interpreted successor prepares its body and acquires its dependencies.
    admitted.providers.retain(|choice| {
        choice.target != Some(semantic_vocabulary::source_selections::ProductTarget::Interpreter)
    });
    let native_supplies = inputs
        .native_bindings
        .resolve(&mut typed, admitted.native_bindings)?;
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
    let roots = match entry {
        ProductEntry::RegisteredTest { root, target } => {
            root.bind(&mut typed, &admitted.declarations.test_groups, target)?
        }
        _ => ProductRoots::Direct(
            crate::compile_product::declaration_lookup::collect_root_bindings(
                &typed,
                inputs.root_binding_requests,
            )?,
        ),
    };
    let unprepared = has_defaults.then(|| typed.clone());
    let mut pass = ProductSourcePass {
        inputs,
        activation,
        explicit_providers: admitted.providers,
        representations: admitted.representations,
        declarations: admitted.declarations,
        native_supplies,
        roots,
        default_provider_selections,
        entry,
        initial_accounting: accounting.clone(),
        accounting,
        timings,
    };
    // Prepare the source once for the defaults the targets are predicted to
    // choose. Without a prediction, each level of providers a chosen
    // default's body reaches costs another preparation and lowering.
    let mut layouts = match &unprepared {
        Some(_) => predicted_default_layouts(&mut pass, &typed)?,
        None => Vec::new(),
    };
    let mut predicted = !layouts.is_empty();
    let mut current = match prepare_roster(&mut pass, typed, &layouts) {
        Ok(current) => current,
        Err(_) if predicted => {
            predicted = false;
            prepare_unpredicted(&mut pass, &unprepared, &mut layouts)?
        }
        Err(diagnostics) => return Err(diagnostics),
    };
    let judged = loop {
        use provider_settlement::{
            LoweredSupplySource, LoweredWork, ProductSupplySource, ProviderWork,
            next_provider_work, next_provider_work_from_lowered,
        };
        // Settle layout defaults and provider supplies on the lowered closure
        // before completing and checking it. Settlement reads only the
        // closure, so a roster it replaces is never given its conformance
        // witnesses or checked by Stage 06.
        let lowered = loop {
            let closure = match judge_lowered_source::SourcePublication::lower_closure(
                &current.source,
                current.roster,
                pass.timings,
            ) {
                Ok(closure) => closure,
                Err(_) if predicted => {
                    predicted = false;
                    current = prepare_unpredicted(&mut pass, &unprepared, &mut layouts)?;
                    continue;
                }
                Err(diagnostics) => return Err(diagnostics),
            };
            let work = pass.timings.record_result(
                artifacts::compile_timings::source::PROVIDER_SETTLEMENT,
                || {
                    next_provider_work_from_lowered(LoweredSupplySource {
                        targets: pass.inputs.targets,
                        lowered: &closure,
                        build: &pass.explicit_providers,
                        native_bindings: &pass.native_supplies,
                        defaults: &current.provider_defaults,
                        layout_prepared: &layouts,
                        predicted,
                    })
                },
            );
            let work = match work {
                Err(_) if predicted => LoweredWork::Mispredicted,
                work => work?,
            };
            predicted = false;
            match work {
                LoweredWork::Settle(ProviderWork::Complete) => {
                    break judge_lowered_source::SourcePublication::complete_lowering(
                        closure,
                        pass.timings,
                    );
                }
                LoweredWork::Settle(ProviderWork::JudgeProviders(supplies)) => {
                    let mut roster = closure.into_roster();
                    roster.supplies.extend(supplies);
                    current.roster = roster;
                }
                LoweredWork::Settle(ProviderWork::PrepareLayouts(chosen)) => {
                    drop(closure);
                    layouts = chosen;
                    let typed = unprepared.clone().ok_or_else(|| vec![Diagnostic::error(
                        "a target chose a provider default absent from the source preparation frontier"
                    )])?;
                    current = prepare_roster(&mut pass, typed, &layouts)?;
                }
                LoweredWork::Mispredicted => {
                    drop(closure);
                    current = prepare_unpredicted(&mut pass, &unprepared, &mut layouts)?;
                }
            }
        };
        let publication = Arc::new(judge_lowered_source::SourcePublication::judge_lowered(
            lowered,
            pass.inputs.optimizations.clone(),
            pass.timings,
        )?);
        let judged = JudgedSource {
            program: current.program,
            publication,
            provider_defaults: current.provider_defaults,
        };
        // Confirm completeness against the checked publication as well.
        let confirm = pass.timings.record_result(
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
        match confirm {
            ProviderWork::Complete => break judged,
            ProviderWork::JudgeProviders(supplies) => {
                let mut roster = judged.publication.roster()?.clone();
                roster.supplies.extend(supplies);
                current = PreparedProduct {
                    program: judged.program,
                    source: judged.publication.accepted()?.prepared().clone(),
                    roster,
                    provider_defaults: judged.provider_defaults,
                };
            }
            ProviderWork::PrepareLayouts(chosen) => {
                layouts = chosen;
                let typed = unprepared.clone().ok_or_else(|| vec![Diagnostic::error(
                    "a target chose a provider default absent from the source preparation frontier"
                )])?;
                current = prepare_roster(&mut pass, typed, &layouts)?;
            }
        }
    };
    if let (
        ProductRoots::Applied { test, application },
        ProductEntry::RegisteredTest { target, .. },
    ) = (&pass.roots, pass.entry)
    {
        entry_roots::check_test_invocation(&judged.publication, test, *application, target)?;
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
        dependency_edges: pass.inputs.dependency_edges,
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

/// The defaults [`provider_settlement::predict_default_layouts`] expects the
/// targets to choose from the Product's entries, read from the unprepared source.
fn predicted_default_layouts(
    pass: &mut ProductSourcePass<'_, '_>,
    typed: &crate::source_state::TypedTrees,
) -> Result<Vec<provider_settlement::ChosenDefaultLayout>, Vec<Diagnostic>> {
    let defaults = product_provider_defaults(typed, &pass.default_provider_selections)?;
    let roots = program_roots(pass.roots.direct());
    let targets = pass
        .inputs
        .targets
        .iter()
        .map(|profile| profile.numeric_support.target)
        .collect::<Vec<_>>();
    pass.timings.record_result(
        artifacts::compile_timings::source::PROVIDER_SETTLEMENT,
        || {
            Ok(provider_settlement::predict_default_layouts(
                typed,
                &roots,
                &targets,
                &pass.explicit_providers,
                &pass.native_supplies,
                &defaults,
            ))
        },
    )
}

/// Discard a source prepared for a wrong prediction, with every charge its
/// preparation added, and prepare the source with no defaults, exactly as
/// settlement starts without a prediction.
fn prepare_unpredicted(
    pass: &mut ProductSourcePass<'_, '_>,
    unprepared: &Option<crate::source_state::TypedTrees>,
    layouts: &mut Vec<provider_settlement::ChosenDefaultLayout>,
) -> Result<PreparedProduct, Vec<Diagnostic>> {
    layouts.clear();
    *pass.accounting = pass.initial_accounting.clone();
    let typed = unprepared.clone().ok_or_else(|| {
        vec![Diagnostic::error(
            "a provider prediction was made without an unprepared source to settle again",
        )]
    })?;
    prepare_roster(pass, typed, layouts)
}

struct ProductSourcePass<'a, 'f> {
    inputs: ProductSourceInputs<'a>,
    activation: Option<symbols::SymbolHandle>,
    explicit_providers: Vec<provider_choice_inputs::ProviderSelection>,
    representations:
        Vec<crate::source_state::opaque_representations::OpaqueRepresentationSelection>,
    declarations: crate::compile_product::declarations::AdmittedProductDeclarations,
    native_supplies: Vec<provider_choice_inputs::NativeBuildSupply>,
    roots: ProductRoots,
    default_provider_selections: Vec<provider_choice_inputs::ProviderSelection>,
    entry: ProductEntry<'f>,
    initial_accounting: crate::compile_time_invocations::SourceCompletionAccounting,
    accounting: &'f mut crate::compile_time_invocations::SourceCompletionAccounting,
    timings: &'f mut crate::CompileTimings,
}

/// A Product source prepared for one set of chosen default layouts, with the
/// roster to lower from it.
struct PreparedProduct {
    program: Arc<crate::source_state::FrozenTypedTrees>,
    source: prepared_typed_trees_to_lowered_program::PreparedTypedTrees,
    roster: prepared_typed_trees_to_lowered_program::source_roster::SourceCompilationRoster,
    provider_defaults: Vec<provider_choice_inputs::TargetProviderDefaults>,
}

struct JudgedSource {
    program: Arc<crate::source_state::FrozenTypedTrees>,
    publication: Arc<judge_lowered_source::SourcePublication>,
    provider_defaults: Vec<provider_choice_inputs::TargetProviderDefaults>,
}
