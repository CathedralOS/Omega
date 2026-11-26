//! Check a returned Build again on its opened Product's sources, either at once
//! or while the caller keeps working on the Products that consume it.

use super::{PlannedProduct, locate_failure, refusal};
use artifacts::compile_timings::CompileTimings;
use build_execution::{BuildConstants, BuildSubjectValidation, CompileFailure};
use diagnostics::Diagnostic;
use package_compilation::PackageCompilationInputs;
use source_compiler::SourcePreparation;
use source_compiler::compile_time_invocations::SourceCompletionAccounting;
use std::sync::{Arc, Condvar, Mutex, MutexGuard};

/// The returned Build entry checked again on its Product's sources, so the
/// constants it read are judged in the context the Product compiles. The check
/// owns everything it reads, so it can run while the Product is used to form
/// its handoff and while the Products that consume it are opened and checked.
pub(super) struct BuildRecheck {
    stage: Result<package_compilation::source_assembly::BuildScopeStageInputs, Vec<Diagnostic>>,
    files: Arc<source_compiler::representations::source_inputs::SourceMap>,
    anonymous_owner: Option<package_compilation::AnonymousSourceCapture>,
    preparation: SourcePreparation,
    host: &'static str,
    inputs: Option<PackageCompilationInputs>,
    declaration: Option<source_admission::RetainedDeclaration>,
    constants: BuildConstants,
    libraries: Arc<[package_compilation::source_imports::SuppliedBuildLibrary]>,
    interpreter_providers: source_compiler::InterpretedProviderSelections,
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
        let reads_non_toolchain_constant = constants.read.iter().any(|read| {
            read.declaration
                .as_ref()
                .is_none_or(|decl| !decl.source().origin.is_toolchain())
        });
        if evaluated.subject_validation != BuildSubjectValidation::NeedsOwnerCheck
            && !reads_non_toolchain_constant
            && !constants.folded_under_providers
        {
            return None;
        }
        let token = Arc::new(());
        let stage = product.source.build_scope_stage_inputs();
        let files = stage
            .as_ref()
            .map(|inputs| Arc::clone(inputs.files()))
            .unwrap_or_else(|_| Arc::clone(&product.source.parsed.files));
        let recheck = Self {
            stage,
            files,
            anonymous_owner: product.source.anonymous_owner.clone(),
            preparation: product.preparation.clone(),
            host: product.build_profile.target_name(),
            inputs: product.package_inputs.clone(),
            declaration: evaluated
                .configuration
                .selected_build_declaration()
                .cloned(),
            constants: constants.clone(),
            libraries: Arc::clone(&product.supplied_build_libraries),
            interpreter_providers: evaluated.interpreter_providers.clone(),
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

    /// Start this recheck on an idle worker of the invocation's budget.
    pub(super) fn start(self) -> PendingRecheck {
        PendingRecheck::start(move || self.run())
    }

    fn check(
        self,
        timings: &mut CompileTimings,
    ) -> Result<SourceCompletionAccounting, CompileFailure> {
        let declaration = self
            .declaration
            .as_ref()
            .ok_or_else(|| refusal("rejoined Build source lost its issued entry".into()))?;
        let build_owner = declaration.source().package_identity;
        let inputs = self
            .inputs
            .as_ref()
            .filter(|inputs| build_owner.is_none_or(|owner| owner == inputs.root()));
        let subject = build_owner.or_else(|| inputs.map(|inputs| inputs.root()));
        let authority = inputs
            .map(|inputs| inputs.declaration_scope(build_declarations::DependencyPurpose::Build))
            .or_else(|| {
                package_compilation::source_imports::SuppliedBuildLibrary::declaration_scope(
                    &self.libraries,
                    subject,
                )
            })
            .map(Arc::new);
        // Checking the rejoined subject reads only Build-scope sources and
        // names any refusal by its file, line, and column.
        let checked = (|| {
            let stage = self.stage?.prepare(timings)?;
            let (typed, entry) = stage.rejoin_build_entry(
                &self.preparation,
                self.host,
                authority,
                subject,
                declaration,
                timings,
            )?;
            build_execution::settle_rejoined_build_slice(
                &self.constants,
                &self.interpreter_providers,
                &typed,
                entry,
                inputs,
                self.anonymous_owner.as_ref(),
                &self.libraries,
                self.subject_validation,
                timings,
            )
        })();
        checked.map_err(|failure| locate_failure(failure, &self.files, inputs))
    }
}

type RecheckJob = Box<dyn FnOnce() -> CheckedBuildRecheck + Send>;

enum RecheckState {
    /// No thread has claimed the check yet.
    Waiting(RecheckJob),
    Running,
    Finished(std::thread::Result<CheckedBuildRecheck>),
    /// The result was taken, or the check was dropped before it started.
    Spent,
}

/// One recheck shared by the worker that may run it and the Product that
/// waits for it. Whichever claims it first runs it: a worker that picks it
/// up, or the thread that needs its result. So waiting never depends on a
/// free worker, and a check nobody started is never run after it is dropped.
struct RecheckSlot {
    state: Mutex<RecheckState>,
    finished: Condvar,
}

impl RecheckSlot {
    fn state(&self) -> MutexGuard<'_, RecheckState> {
        // The check runs outside the lock, so a poisoned lock holds a
        // complete state.
        self.state
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    fn claim(&self) -> Option<RecheckJob> {
        let mut state = self.state();
        if !matches!(*state, RecheckState::Waiting(_)) {
            return None;
        }
        match std::mem::replace(&mut *state, RecheckState::Running) {
            RecheckState::Waiting(job) => Some(job),
            _ => None,
        }
    }

    fn run(&self, job: RecheckJob) {
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(job));
        *self.state() = RecheckState::Finished(result);
        self.finished.notify_all();
    }

    fn run_if_waiting(&self) {
        if let Some(job) = self.claim() {
            self.run(job);
        }
    }
}

/// A Build recheck that may still be running. [`Self::join`] waits for it,
/// running it on the calling thread if no worker has started it.
pub(super) struct PendingRecheck(Arc<RecheckSlot>);

impl PendingRecheck {
    fn start(job: impl FnOnce() -> CheckedBuildRecheck + Send + 'static) -> Self {
        let slot = Arc::new(RecheckSlot {
            state: Mutex::new(RecheckState::Waiting(Box::new(job))),
            finished: Condvar::new(),
        });
        // Off a budget worker no long-lived pool exists to run the check, so
        // it runs when its result is needed.
        if worker_budget::on_budget_worker() {
            let shared = Arc::clone(&slot);
            rayon::spawn(move || shared.run_if_waiting());
        }
        Self(slot)
    }

    /// The recheck's result. A panic inside the check resumes here.
    fn join(self) -> Option<CheckedBuildRecheck> {
        self.0.run_if_waiting();
        let mut state = self.0.state();
        while matches!(*state, RecheckState::Running) {
            state = self
                .0
                .finished
                .wait(state)
                .unwrap_or_else(|poisoned| poisoned.into_inner());
        }
        match std::mem::replace(&mut *state, RecheckState::Spent) {
            RecheckState::Finished(Ok(checked)) => Some(checked),
            RecheckState::Finished(Err(panic)) => {
                drop(state);
                std::panic::resume_unwind(panic)
            }
            _ => None,
        }
    }
}

impl Drop for PendingRecheck {
    /// A recheck whose Product was abandoned is not started; one already
    /// running finishes on its worker and its result is discarded.
    fn drop(&mut self) {
        let mut state = self.0.state();
        if matches!(*state, RecheckState::Waiting(_)) {
            *state = RecheckState::Spent;
        }
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
        if self.evaluated.subject_validation == BuildSubjectValidation::NeedsOwnerCheck {
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
    owed: Option<(Arc<()>, PendingRecheck)>,
}

impl PendingProduct {
    pub(super) fn new(product: PlannedProduct, owed: Option<(Arc<()>, PendingRecheck)>) -> Self {
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

    /// Wait for this Product's recheck and return the Product once it passed.
    pub fn settle(self) -> Result<PlannedProduct, CompileFailure> {
        let (owed, checked) = match self.owed {
            Some((token, pending)) => (Some(token), pending.join()),
            None => (None, None),
        };
        self.product.settle_recheck(owed, checked)
    }

    /// Release the Product to a caller that only checks it, while its Build
    /// recheck may still run. `omega --check` writes nothing, so it can check
    /// the Product meanwhile; it must settle the returned recheck before it
    /// reports anything about the Product, and a refused recheck replaces the
    /// Product's own result. The released Product's timings and accounting
    /// are those of its original Build: the recheck's are reported and
    /// validated by [`OwedBuildRecheck::settle`].
    pub fn release_for_check(self) -> (PlannedProduct, Option<OwedBuildRecheck>) {
        let owed = self
            .owed
            .map(|(token, pending)| OwedBuildRecheck { token, pending });
        (self.product, owed)
    }
}

/// The Build recheck of a Product released by [`PendingProduct::release_for_check`].
#[must_use = "a refused Build recheck refuses the Product it checked"]
pub struct OwedBuildRecheck {
    token: Arc<()>,
    pending: PendingRecheck,
}

impl OwedBuildRecheck {
    /// Wait for the recheck and return the Build work it measured, or its
    /// refusal. A released Product's check validated only its original
    /// Build's charges, so the charges this recheck spent are validated here.
    pub fn settle(self) -> Result<CompileTimings, CompileFailure> {
        let checked = CheckedBuildRecheck::owed_to(&self.token, self.pending.join())?;
        checked
            .result?
            .validate_charges()
            .map_err(|reason| CompileFailure::from(vec![Diagnostic::error(reason)]))?;
        Ok(checked.timings)
    }
}

/// Run `body`, which opens Products and settles their Build rechecks, on a
/// worker of the invocation's budget. A recheck then runs on that budget, or
/// on the thread that settles it, and never on a small-stack thread.
pub fn with_build_rechecks<R: Send>(body: impl FnOnce() -> R + Send) -> R {
    worker_budget::within_budget(body)
}

/// Whether a recheck started now keeps its workers after the caller returns.
/// Off the invocation's budget, `with_build_rechecks` lends a pool only for
/// its own call, so its rechecks must settle inside it.
pub fn rechecks_outlive_caller() -> bool {
    worker_budget::on_budget_worker()
}

#[cfg(test)]
mod tests {
    use super::{CheckedBuildRecheck, PendingRecheck};
    use std::sync::Arc;
    use std::sync::atomic::{AtomicBool, Ordering};

    fn passed(product: &Arc<()>) -> CheckedBuildRecheck {
        CheckedBuildRecheck {
            product: Arc::clone(product),
            result: Ok(Default::default()),
            timings: Default::default(),
        }
    }

    /// A recheck's result settles only the Product that owes it.
    #[test]
    fn a_recheck_result_settles_only_the_product_that_owes_it() {
        let [first, second] = [Arc::new(()), Arc::new(())];
        let owner = Arc::clone(&first);
        let checked = PendingRecheck::start(move || passed(&owner)).join();
        assert!(CheckedBuildRecheck::owed_to(&second, checked).is_err());
        let owner = Arc::clone(&first);
        let checked = PendingRecheck::start(move || passed(&owner)).join();
        assert!(CheckedBuildRecheck::owed_to(&first, checked).is_ok());
        // A missing result is refused.
        assert!(CheckedBuildRecheck::owed_to(&first, None).is_err());
    }

    /// A refused recheck keeps its refusal for the Product that settles it.
    #[test]
    fn a_refused_recheck_refuses_its_product() {
        let product = Arc::new(());
        let owner = Arc::clone(&product);
        let checked = PendingRecheck::start(move || CheckedBuildRecheck {
            result: Err(super::refusal("recheck refused".into())),
            ..passed(&owner)
        })
        .join();
        let settled = CheckedBuildRecheck::owed_to(&product, checked).unwrap();
        let refusal = settled.result.unwrap_err().into_diagnostics();
        assert_eq!(
            refusal[0].message,
            "Build activation failed: recheck refused"
        );
    }

    /// With every budget worker busy, the thread that waits runs the check
    /// itself, and a worker that reaches it later does not run it again.
    #[test]
    fn the_waiting_thread_runs_a_recheck_no_worker_started() {
        let budget = worker_budget::ExecutionBudget::with_workers(1).unwrap();
        let runs = Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let checked = budget.run(|| {
            let product = Arc::new(());
            let counted = Arc::clone(&runs);
            let owner = Arc::clone(&product);
            // The only worker is this thread, so the spawned job cannot start
            // before `join`.
            let pending = PendingRecheck::start(move || {
                counted.fetch_add(1, Ordering::SeqCst);
                passed(&owner)
            });
            CheckedBuildRecheck::owed_to(&product, pending.join()).is_ok()
        });
        // Running another operation lets the worker reach the queued job,
        // which finds the check already claimed.
        budget.run(|| ());
        assert!(checked);
        assert_eq!(runs.load(Ordering::SeqCst), 1);
    }

    /// A recheck nobody started never runs once its Product is dropped.
    #[test]
    fn a_dropped_recheck_that_never_started_never_runs() {
        let ran = Arc::new(AtomicBool::new(false));
        let observed = Arc::clone(&ran);
        let product = Arc::new(());
        drop(PendingRecheck::start(move || {
            observed.store(true, Ordering::SeqCst);
            passed(&product)
        }));
        assert!(!ran.load(Ordering::SeqCst));
    }

    /// A recheck owed by a released Product settles only with the result
    /// made for that Product, and a refused one keeps its refusal.
    #[test]
    fn a_released_products_recheck_settles_only_with_its_own_result() {
        let owed = |token: &Arc<()>, made_for: &Arc<()>, refused: bool| {
            let made_for = Arc::clone(made_for);
            super::OwedBuildRecheck {
                token: Arc::clone(token),
                pending: PendingRecheck::start(move || CheckedBuildRecheck {
                    result: if refused {
                        Err(super::refusal("recheck refused".into()))
                    } else {
                        Ok(Default::default())
                    },
                    ..passed(&made_for)
                }),
            }
        };
        let [product, other] = [Arc::new(()), Arc::new(())];
        assert!(owed(&product, &product, false).settle().is_ok());
        assert!(owed(&product, &other, false).settle().is_err());
        let refusal = owed(&product, &product, true)
            .settle()
            .unwrap_err()
            .into_diagnostics();
        assert_eq!(
            refusal[0].message,
            "Build activation failed: recheck refused"
        );
    }

    /// A panic inside the check resumes on the thread that waits for it.
    #[test]
    fn a_panicking_recheck_panics_where_it_is_awaited() {
        let pending = PendingRecheck::start(|| panic!("recheck panicked"));
        let unwound = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| pending.join()));
        assert!(unwound.is_err());
    }

    fn configure_library() {
        static CONFIGURED: std::sync::OnceLock<()> = std::sync::OnceLock::new();
        CONFIGURED.get_or_init(|| {
            package_compilation::toolchain_library::configure(
                &std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                    .join("../../../../source/library"),
            )
            .unwrap()
        });
    }

    fn open_unchecked_project(
        entry: &std::path::Path,
        dependency_activation: bool,
    ) -> super::super::PlannedProduct {
        use package_compilation::BuildExecutionInputs;
        use package_manager::operations::begin_local_project_preparation;
        use package_manager::resolution::source::ExecutedSourceResolution;
        use target::TargetProfile;

        configure_library();
        let target = TargetProfile::LinuxX64;
        let project = begin_local_project_preparation(entry, &[target], true).unwrap();
        let acquired = project.acquire_root().unwrap();
        let stage = super::super::prepare(
            super::super::PlanningRequest {
                root: &acquired.entry_path(),
                sources: super::super::PlanningSources::Captured(acquired.source_capture()),
                execution: BuildExecutionInputs {
                    dependency_activation,
                    ..Default::default()
                },
                artifact_only: false,
            },
            Default::default(),
            Default::default(),
        )
        .unwrap();
        let mut returned = crate::interpret(stage, &mut ()).unwrap();
        let declared = acquired.declare_build(returned.returned()).unwrap();
        let subject = declared.identity();
        let ExecutedSourceResolution::Complete(graph) = declared.resolve().unwrap() else {
            panic!("fixture declares no dependencies");
        };
        let inputs = graph
            .join_source_result(subject, &mut returned.source_result().unwrap())
            .unwrap();
        let (source, evaluated) = returned.complete_activation(Some(&inputs)).unwrap();
        source
            .open_unchecked(evaluated, Some(&inputs), false)
            .unwrap()
    }

    #[test]
    fn entry_only_build_reading_only_toolchain_constants_owes_no_recheck() {
        let temp = platform_custody::temporary_directory::TemporaryDirectory::create(
            "recheck-entry-only-toolchain-consts",
            false,
        )
        .unwrap();
        let root = temp.path();
        std::fs::write(root.join("main.omg"), "public machine ping() {}").unwrap();
        let entry = root.join("build.omg");
        std::fs::write(
            &entry,
            r#"use {omega}/language/core/float_format.omg as formats;
use {omega}/language/core/units.omg;
machine build(builder: @mutable BuildPlan) {
    builder.product = BuildProduct::Package { name: "dep_pkg" };
    let _f: |formats|::FloatFormat = |formats|::FloatFormat::BINARY32;
    let _u: |units|::Unit = |units|::Units::METER;
    let _t: BuildTarget = linux_x86_64;
}
"#,
        )
        .unwrap();
        let product = open_unchecked_project(&entry, true);
        assert_eq!(
            product.evaluated.subject_validation,
            build_execution::BuildSubjectValidation::EntryOnly
        );
        assert!(
            product.evaluated.constants.read.is_empty(),
            "immutable toolchain literals should not force recheck: {:?}",
            product.evaluated.constants.read
        );
        assert!(super::BuildRecheck::owed_by(&product).is_none());
    }

    #[test]
    fn build_scope_recheck_excludes_product_and_rejects_invalid_subject() {
        let temp = platform_custody::temporary_directory::TemporaryDirectory::create(
            "recheck-build-scope-subject",
            false,
        )
        .unwrap();
        let root = temp.path();
        let main = root.join("main.omg");
        let helper = root.join("helper.omg");
        let entry = root.join("build.omg");
        std::fs::write(&main, "public machine main() {}").unwrap();
        std::fs::write(
            &helper,
            "public machine configure(builder: @mutable BuildPlan) {\n    builder.product = BuildProduct::Application { name: \"scoped_app\" };\n}\n",
        )
        .unwrap();
        std::fs::write(
            &entry,
            r#"use helper.omg as helper;
use main.omg as app;
machine build(builder: @mutable BuildPlan) {
    |helper|::configure(builder);
    Build::bind(builder, linux_x86_64::ProgramEntry, |app|::main);
}
"#,
        )
        .unwrap();

        let product = open_unchecked_project(&entry, false);
        assert_eq!(
            product.evaluated.subject_validation,
            build_execution::BuildSubjectValidation::NeedsOwnerCheck
        );
        assert!(
            product
                .source
                .parsed
                .files
                .selected_files()
                .any(|file| file.dependency_scope == source_provenance::DependencyScope::Product),
            "Product assembly includes Product sources"
        );
        let (recheck, token) =
            super::BuildRecheck::owed_by(&product).expect("provisional owner owes recheck");
        assert!(
            !recheck.files.selected_files().any(|file| {
                file.dependency_scope == source_provenance::DependencyScope::Product
            }),
            "BuildRecheck must exclude all Product-scope sources"
        );
        assert!(
            recheck
                .files
                .selected_files()
                .any(|file| file.path == entry.canonicalize().unwrap_or_else(|_| entry.clone()))
        );
        let checked = recheck.run();
        let settled = product.settle_recheck(Some(token), Some(checked)).unwrap();
        assert_eq!(
            settled.evaluated.subject_validation,
            build_execution::BuildSubjectValidation::Checked
        );

        // Now verify an invalid Build subject helper is caught under its real owner
        // even when BuildRecheck receives only Build-scope sources.
        std::fs::write(
            &helper,
            "public machine configure(builder: @mutable BuildPlan) {\n    builder.product = BuildProduct::Application { name: \"scoped_app\" };\n}\nmachine unused_invalid() {\n    let bad: u8 = true;\n}\n",
        )
        .unwrap();
        let mut invalid_product = open_unchecked_project(&entry, true);
        invalid_product.evaluated.subject_validation =
            build_execution::BuildSubjectValidation::NeedsOwnerCheck;
        let (recheck, _) =
            super::BuildRecheck::owed_by(&invalid_product).expect("provisional owner owes recheck");
        let failure = recheck
            .run()
            .result
            .expect_err("Build-scope-only BuildRecheck must reject invalid Build subject");
        let messages = diagnostics::join_messages(&failure.into_diagnostics());
        assert!(
            messages.contains("unused_invalid")
                && messages.contains("`bool`")
                && messages.contains("`u8`"),
            "{messages}"
        );
    }

    #[test]
    fn build_scope_recheck_catches_rejoined_constant_mismatch() {
        let temp = platform_custody::temporary_directory::TemporaryDirectory::create(
            "recheck-constant-mismatch",
            false,
        )
        .unwrap();
        let root = temp.path();
        std::fs::write(root.join("main.omg"), "public machine main() {}").unwrap();
        let entry = root.join("build.omg");
        std::fs::write(
            &entry,
            r#"use main.omg as app;
const BUILD_LIMIT: u64 = limit();
const OTHER_VALUE: u64 = 9u64;
machine limit() -> u64 { 4u64 }
machine build(builder: @mutable BuildPlan) {
    let total: u64 = BUILD_LIMIT + OTHER_VALUE;
    let matches: bool = total == 13u64;
    transition matches {
        true -> named(builder),
        false -> wrong(builder),
    }
    machine named(builder: @mutable BuildPlan) {
        builder.product = BuildProduct::Application { name: "const_app" };
        Build::bind(builder, linux_x86_64::ProgramEntry, |app|::main);
    }
    machine wrong(builder: @mutable BuildPlan) {
        builder.product = BuildProduct::Application { name: "other_app" };
    }
}
"#,
        )
        .unwrap();

        let product = open_unchecked_project(&entry, true);
        assert_eq!(
            product.evaluated.subject_validation,
            build_execution::BuildSubjectValidation::EntryOnly
        );
        assert_eq!(product.evaluated.constants.read.len(), 2);

        let (recheck, _) =
            super::BuildRecheck::owed_by(&product).expect("user Build constants owe recheck");
        assert!(
            !recheck.files.selected_files().any(|file| {
                file.dependency_scope == source_provenance::DependencyScope::Product
            }),
            "BuildRecheck must exclude Product-scope sources"
        );
        recheck
            .run()
            .result
            .expect("matching Build constants settle cleanly");

        let mut mismatched = open_unchecked_project(&entry, true);
        let mismatched_value = mismatched.evaluated.constants.read[1].value.clone();
        mismatched.evaluated.constants.read[0].value = mismatched_value;
        let (recheck, _) = super::BuildRecheck::owed_by(&mismatched)
            .expect("mismatched Build constant owes recheck");
        let failure = recheck
            .run()
            .result
            .expect_err("Build-scope-only BuildRecheck must catch rejoined constant mismatch");
        let messages = diagnostics::join_messages(&failure.into_diagnostics());
        assert!(
            messages.contains("retyped Build constant demand or value differs"),
            "{messages}"
        );
    }
}
