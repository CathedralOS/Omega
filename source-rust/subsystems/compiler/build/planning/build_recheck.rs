//! Check a returned Build again on its opened Product's sources, either at once
//! or while the caller keeps opening the Products that consume it.

use super::{PlannedProduct, locate_failure, refusal};
use artifacts::compile_timings::CompileTimings;
use build_execution::{BuildConstants, BuildSubjectValidation, CompileFailure};
use diagnostics::Diagnostic;
use package_compilation::PackageCompilationInputs;
use source_compiler::SourcePreparation;
use source_compiler::compile_time_invocations::SourceCompletionAccounting;
use std::sync::{Arc, Mutex};

/// The returned Build entry checked again on its Product's sources, so the
/// constants it read are judged in the context the Product compiles. The check
/// owns everything it reads, so it can run while the Product is used to form
/// its handoff and while the Products that consume it are opened.
pub(super) struct BuildRecheck {
    stage: Result<source_compiler::BuildStageSource, Vec<Diagnostic>>,
    files: Arc<source_compiler::representations::source_inputs::SourceMap>,
    anonymous_owner: Option<package_compilation::AnonymousSourceCapture>,
    preparation: SourcePreparation,
    host: &'static str,
    inputs: Option<PackageCompilationInputs>,
    declaration: Option<source_admission::RetainedDeclaration>,
    constants: BuildConstants,
    subject_validation: BuildSubjectValidation,
    timings: bool,
    /// Pairs the result with the one Product it settles.
    product: Arc<()>,
}

/// The outcome of one [`BuildRecheck`], applied only to the Product it checked.
pub(super) struct CheckedBuildRecheck {
    product: Arc<()>,
    result: Result<SourceCompletionAccounting, CompileFailure>,
    timings: CompileTimings,
}

impl CheckedBuildRecheck {
    /// The result that settles the Product holding `owed`: a missing result,
    /// or one checked for another Product, is refused.
    fn owed_to(owed: &Arc<()>, checked: Option<Self>) -> Result<Self, CompileFailure> {
        checked
            .filter(|checked| Arc::ptr_eq(&checked.product, owed))
            .ok_or_else(|| refusal("the Product's Build recheck has no result".into()))
    }
}

impl BuildRecheck {
    /// The recheck `product` owes, or `None` when its Build read no constant
    /// and owes no check under its real owner.
    pub(super) fn owed_by(product: &PlannedProduct) -> Option<(Self, Arc<()>)> {
        let evaluated = &product.evaluated;
        let constants = &evaluated.constants;
        if evaluated.subject_validation != BuildSubjectValidation::CheckedUnderProvisionalOwner
            && constants.read.is_empty()
            && !constants.folded_under_providers
        {
            return None;
        }
        let token = Arc::new(());
        let recheck = Self {
            stage: product.source.build_stage_source(),
            files: Arc::clone(&product.source.parsed.files),
            anonymous_owner: product.source.anonymous_owner.clone(),
            preparation: product.preparation.clone(),
            host: product.build_profile.target_name(),
            inputs: product.package_inputs.clone(),
            declaration: evaluated
                .configuration
                .selected_build_declaration()
                .cloned(),
            constants: constants.clone(),
            subject_validation: evaluated.subject_validation,
            timings: product.timings.is_enabled(),
            product: Arc::clone(&token),
        };
        Some((recheck, token))
    }

    pub(super) fn run(self) -> CheckedBuildRecheck {
        let mut timings = if self.timings {
            CompileTimings::enabled()
        } else {
            CompileTimings::default()
        };
        let product = Arc::clone(&self.product);
        let result = self.check(&mut timings);
        CheckedBuildRecheck {
            product,
            result,
            timings,
        }
    }

    fn check(
        self,
        timings: &mut CompileTimings,
    ) -> Result<SourceCompletionAccounting, CompileFailure> {
        let inputs = self.inputs.as_ref();
        let authority = inputs.map(|inputs| {
            Arc::new(inputs.declaration_scope(build_declarations::DependencyPurpose::Build))
        });
        let declaration = self
            .declaration
            .as_ref()
            .ok_or_else(|| refusal("rejoined Build source lost its issued entry".into()))?;
        // Checking the rejoined subject can refuse any declaration in the
        // Product's files, so its refusals name their file, line and column.
        let checked = (|| {
            let (typed, entry) = self.stage?.rejoin_build_entry(
                &self.preparation,
                self.host,
                authority,
                inputs.map(|inputs| inputs.root()),
                declaration,
                timings,
            )?;
            build_execution::settle_rejoined_build_slice(
                &self.constants,
                &typed,
                entry,
                inputs,
                self.anonymous_owner.as_ref(),
                self.subject_validation,
                timings,
            )
        })();
        checked.map_err(|failure| locate_failure(failure, &self.files, inputs))
    }
}

impl PlannedProduct {
    /// Settle this Product with the recheck it owed. A result for another
    /// Product is refused, and so is a missing one.
    pub(super) fn settle_recheck(
        mut self,
        owed: Option<Arc<()>>,
        checked: Option<CheckedBuildRecheck>,
    ) -> Result<Self, CompileFailure> {
        let Some(owed) = owed else {
            return Ok(self);
        };
        let checked = CheckedBuildRecheck::owed_to(&owed, checked)?;
        self.timings.extend(&checked.timings);
        let spent = checked.result?;
        self.evaluated.source_completion_accounting.append(spent);
        if self.evaluated.subject_validation == BuildSubjectValidation::CheckedUnderProvisionalOwner
        {
            self.evaluated.subject_validation = BuildSubjectValidation::Checked;
        }
        Ok(self)
    }
}

/// A dependency Product whose Build recheck may still be running. Only its
/// handoff can be read; the Product itself is returned by [`Self::settle`]
/// once its recheck passes.
pub struct PendingProduct {
    product: PlannedProduct,
    owed: Option<(Arc<()>, usize)>,
}

impl PendingProduct {
    pub(super) fn new(product: PlannedProduct, owed: Option<(Arc<()>, usize)>) -> Self {
        Self { product, owed }
    }

    /// The Build handoff of this Product for `profile`. It reads nothing the
    /// recheck produces.
    pub fn build_handoff(
        &self,
        profile: target::TargetProfile,
    ) -> Result<package_compilation::PackageBuildHandoff, CompileFailure> {
        self.product.build_handoff(profile)
    }

    /// Return the Product once its recheck from `checked` has passed.
    pub fn settle(self, checked: &mut CheckedRechecks) -> Result<PlannedProduct, CompileFailure> {
        let (owed, result) = match self.owed {
            Some((token, slot)) => (Some(token), checked.take(slot)),
            None => (None, None),
        };
        self.product.settle_recheck(owed, result)
    }
}

/// Build rechecks running alongside the caller inside [`with_build_rechecks`].
pub struct BuildRechecks<'r, 's> {
    scope: &'r rayon::Scope<'s>,
    results: &'s Mutex<Vec<Option<CheckedBuildRecheck>>>,
}

impl<'s> BuildRechecks<'_, 's> {
    pub(super) fn start(&self, recheck: BuildRecheck) -> Result<usize, CompileFailure> {
        self.spawn(move || recheck.run())
    }

    fn spawn(
        &self,
        recheck: impl FnOnce() -> CheckedBuildRecheck + Send + 's,
    ) -> Result<usize, CompileFailure> {
        let mut results = self
            .results
            .lock()
            .map_err(|_| refusal("the Build recheck results are poisoned".into()))?;
        let slot = results.len();
        results.push(None);
        let shared = self.results;
        self.scope.spawn(move |_| {
            let checked = recheck();
            // A slot left empty is refused when its Product settles.
            if let Ok(mut results) = shared.lock() {
                results[slot] = Some(checked);
            }
        });
        Ok(slot)
    }
}

/// The finished rechecks of one [`with_build_rechecks`] call.
pub struct CheckedRechecks(Vec<Option<CheckedBuildRecheck>>);

impl CheckedRechecks {
    fn take(&mut self, slot: usize) -> Option<CheckedBuildRecheck> {
        self.0.get_mut(slot).and_then(Option::take)
    }
}

/// Run `body` while the Build rechecks it starts run on the invocation's
/// workers, and return once every recheck has finished.
pub fn with_build_rechecks<R: Send>(
    body: impl FnOnce(&BuildRechecks<'_, '_>) -> R + Send,
) -> (R, CheckedRechecks) {
    let results = Mutex::new(Vec::new());
    let value = worker_budget::within_budget(|| {
        rayon::scope(|scope| {
            body(&BuildRechecks {
                scope,
                results: &results,
            })
        })
    });
    // `rayon::scope` resumes any recheck panic before this point.
    let results = results
        .into_inner()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    (value, CheckedRechecks(results))
}

#[cfg(test)]
mod tests {
    use super::{CheckedBuildRecheck, with_build_rechecks};
    use std::sync::Arc;

    fn passed(product: &Arc<()>) -> CheckedBuildRecheck {
        CheckedBuildRecheck {
            product: Arc::clone(product),
            result: Ok(Default::default()),
            timings: Default::default(),
        }
    }

    /// Each started recheck finishes before `with_build_rechecks` returns, and
    /// its result settles only the Product that owes it.
    #[test]
    fn a_recheck_result_settles_only_the_product_that_owes_it() {
        let [first, second] = [Arc::new(()), Arc::new(())];
        let (slots, mut checked) = with_build_rechecks(|rechecks| {
            let first = Arc::clone(&first);
            let second = Arc::clone(&second);
            [
                rechecks.spawn(move || passed(&first)).unwrap(),
                rechecks.spawn(move || passed(&second)).unwrap(),
            ]
        });
        assert!(CheckedBuildRecheck::owed_to(&first, checked.take(slots[0])).is_ok());
        // The second Product's result cannot settle the first.
        assert!(CheckedBuildRecheck::owed_to(&first, checked.take(slots[1])).is_err());
        // Each result settles once, and a slot never started has none.
        assert!(CheckedBuildRecheck::owed_to(&first, checked.take(slots[0])).is_err());
        assert!(CheckedBuildRecheck::owed_to(&second, checked.take(2)).is_err());
    }

    /// A refused recheck keeps its refusal for the Product that settles it.
    #[test]
    fn a_refused_recheck_refuses_its_product() {
        let product = Arc::new(());
        let (slot, mut checked) = with_build_rechecks(|rechecks| {
            let owner = Arc::clone(&product);
            rechecks
                .spawn(move || CheckedBuildRecheck {
                    result: Err(super::refusal("recheck refused".into())),
                    ..passed(&owner)
                })
                .unwrap()
        });
        let settled = CheckedBuildRecheck::owed_to(&product, checked.take(slot)).unwrap();
        let refusal = settled.result.unwrap_err().into_diagnostics();
        assert_eq!(
            refusal[0].message,
            "Build activation failed: recheck refused"
        );
    }
}
