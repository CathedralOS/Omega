use checked_trees::CheckedTrees;
use checked_trees_to_lowered_psi::{
    LoweringError, TerminalMachineSelection, lower_machine, select_terminal_machine,
};
use lowered_psi::{
    LoweredPsi, LoweredSelectedIeeeFloatComparisonOccurrence,
    LoweredSelectedIeeeFloatFmaOccurrence, LoweredSelectedIntegerComparisonOccurrence,
    LoweredSourceCallOccurrence,
};
use lowered_psi_to_lowered_psi::{
    PsiOptimizationStageError, PsiOptimizationStageResult, run_psi_optimization,
};
use lowered_psi_to_terminal_psi::{
    CheckedBoundaryOperatorApplicationScope, finalize_terminal_artifact,
};
use terminal_codec::terminal_psi_identity;
use terminal_psi::{CheckedProgramEntryTerminalReceipt, TerminalMachineResult};

use crate::stage_timings::{TerminalProductionStage, TerminalProductionTimings};

mod receiver_eligibility;

/// Canonical Terminal output beside the checked-source evidence the caller
/// asked production to retain: the checked D29 demand scope, the selected
/// floating-point and integer comparison occurrences, the source call
/// occurrences, the checked `ProgramEntry` receipt when an entry identity was
/// supplied, and the caller's opaque callback-use custody.
///
/// The retained native route rejoins the checked entry receipt after the
/// checked frontend is gone, so the receipt leaves production beside the
/// artifact instead of remaining a direct-route-only custody object. Psi
/// neither inspects the callback sidecar nor grants callback placement,
/// registration, invocation, address, or lifetime authority.
#[derive(Debug, PartialEq, Eq)]
#[must_use = "Terminal production retains checked-source and callback custody"]
pub struct ProducedTerminalArtifact<C> {
    artifact: terminal_codec::CanonicalTerminalArtifact,
    receipt: Option<CheckedProgramEntryTerminalReceipt>,
    unoptimized: Option<LoweredPsi>,
    boundary_operator_scope: CheckedBoundaryOperatorApplicationScope,
    callback_custody: C,
    source_call_occurrences: Vec<LoweredSourceCallOccurrence>,
    selected_ieee_float_fma_occurrences: Vec<LoweredSelectedIeeeFloatFmaOccurrence>,
    selected_ieee_float_comparison_occurrences: Vec<LoweredSelectedIeeeFloatComparisonOccurrence>,
    selected_integer_comparison_occurrences: Vec<LoweredSelectedIntegerComparisonOccurrence>,
}

impl<C> ProducedTerminalArtifact<C> {
    pub const fn artifact(&self) -> &terminal_codec::CanonicalTerminalArtifact {
        &self.artifact
    }

    /// The checked `ProgramEntry` receipt, present exactly when the request
    /// carried an entry identity.
    pub const fn receipt(&self) -> Option<&CheckedProgramEntryTerminalReceipt> {
        self.receipt.as_ref()
    }

    /// The lowered module before selected optimization, present exactly when
    /// the custody asked to retain it: the composition an admission check
    /// that must not see optimization reads, without lowering again.
    pub const fn unoptimized(&self) -> Option<&LoweredPsi> {
        self.unoptimized.as_ref()
    }

    pub const fn boundary_operator_scope(&self) -> &CheckedBoundaryOperatorApplicationScope {
        &self.boundary_operator_scope
    }

    pub const fn callback_custody(&self) -> &C {
        &self.callback_custody
    }

    pub fn source_call_occurrences(&self) -> &[LoweredSourceCallOccurrence] {
        &self.source_call_occurrences
    }

    pub fn selected_ieee_float_fma_occurrences(&self) -> &[LoweredSelectedIeeeFloatFmaOccurrence] {
        &self.selected_ieee_float_fma_occurrences
    }

    pub fn selected_ieee_float_comparison_occurrences(
        &self,
    ) -> &[LoweredSelectedIeeeFloatComparisonOccurrence] {
        &self.selected_ieee_float_comparison_occurrences
    }

    /// Selected integer comparison joins, the integer counterpart of the IEEE
    /// comparison roster. Omega rejoins each row to its exact selected
    /// provider before native realization may admit the operation.
    pub fn selected_integer_comparison_occurrences(
        &self,
    ) -> &[LoweredSelectedIntegerComparisonOccurrence] {
        &self.selected_integer_comparison_occurrences
    }

    /// Keep only the canonical portable artifact.
    pub fn into_artifact(self) -> terminal_codec::CanonicalTerminalArtifact {
        self.artifact
    }

    #[allow(clippy::type_complexity)]
    pub fn into_parts(
        self,
    ) -> (
        terminal_codec::CanonicalTerminalArtifact,
        Option<CheckedProgramEntryTerminalReceipt>,
        Option<LoweredPsi>,
        CheckedBoundaryOperatorApplicationScope,
        C,
        Vec<LoweredSourceCallOccurrence>,
        Vec<LoweredSelectedIeeeFloatFmaOccurrence>,
        Vec<LoweredSelectedIeeeFloatComparisonOccurrence>,
        Vec<LoweredSelectedIntegerComparisonOccurrence>,
    ) {
        (
            self.artifact,
            self.receipt,
            self.unoptimized,
            self.boundary_operator_scope,
            self.callback_custody,
            self.source_call_occurrences,
            self.selected_ieee_float_fma_occurrences,
            self.selected_ieee_float_comparison_occurrences,
            self.selected_integer_comparison_occurrences,
        )
    }
}

/// Transactional rejection from Terminal production.
///
/// The checked tree and selected machine are borrowed inputs. The only owned
/// input is the callback sidecar, so rejection returns it exactly for retry or
/// diagnosis instead of silently discarding it.
#[derive(Debug)]
#[must_use = "Terminal production rejection returns callback-use custody"]
pub struct CallbackCustodyTerminalArtifactProductionError<C> {
    error: TerminalArtifactProductionError,
    callback_custody: C,
}

impl<C> CallbackCustodyTerminalArtifactProductionError<C> {
    pub const fn error(&self) -> &TerminalArtifactProductionError {
        &self.error
    }

    pub const fn callback_custody(&self) -> &C {
        &self.callback_custody
    }

    pub fn into_parts(self) -> (TerminalArtifactProductionError, C) {
        (self.error, self.callback_custody)
    }
}

impl<C> std::fmt::Display for CallbackCustodyTerminalArtifactProductionError<C> {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.error.fmt(formatter)
    }
}

impl<C: std::fmt::Debug> std::error::Error for CallbackCustodyTerminalArtifactProductionError<C> {}

/// What one production run keeps in custody beside the artifact.
///
/// `entry_identity` is the checked source-signature identity a
/// `ProgramEntry` receipt retains; `None` publishes the machine as an
/// ordinary Terminal product with no entry receipt. `callback_custody` is the
/// caller's opaque callback-use sidecar, returned by value on success and on
/// rejection. `timings` records each production leg; a
/// [`TerminalProductionTimings::default`] carrier runs the same body
/// unmeasured.
pub struct TerminalProductionCustody<'t, C> {
    pub entry_identity: Option<[u8; 32]>,
    pub callback_custody: C,
    /// Retain the lowered module as it was before selected optimization,
    /// beside the artifact. An admission that must judge the unoptimized
    /// composition (the authored behavior exclusions) reads it from the
    /// product instead of lowering the entry a second time.
    pub retain_unoptimized: bool,
    pub timings: &'t mut TerminalProductionTimings,
}

impl<'t> TerminalProductionCustody<'t, ()> {
    /// No entry receipt and no callback sidecar: only the artifact and its
    /// checked-scope evidence are retained.
    pub fn artifact_only(timings: &'t mut TerminalProductionTimings) -> Self {
        Self {
            entry_identity: None,
            callback_custody: (),
            retain_unoptimized: false,
            timings,
        }
    }
}

/// Exact borrowed source inputs and target-neutral selection for Terminal production.
pub struct TerminalProductionRequest<'a> {
    pub checked: &'a CheckedTrees,
    pub machine: TerminalMachineSelection<'a>,
    pub optimization_selections: optimization::PsiOptimizationSelections,
}

impl<'a> TerminalProductionRequest<'a> {
    /// Select the identity optimization phase by default.
    pub fn new(checked: &'a CheckedTrees, machine: TerminalMachineSelection<'a>) -> Self {
        Self {
            checked,
            machine,
            optimization_selections: optimization::PsiOptimizationSelections::default(),
        }
    }

    /// Produce the canonical Terminal artifact beside the custody the caller
    /// asked to retain.
    ///
    /// Every product shares one instrumented body: ledger check, lowering,
    /// selected optimization, the `ProgramEntry` receipt legs when an entry
    /// identity is supplied, publication, and the checked boundary-operator
    /// scope. The source-signature digest remains opaque; later Omega
    /// settlement must independently compare it with the retained source
    /// signature. Rejection returns the callback sidecar unchanged.
    pub fn produce<C>(
        self,
        custody: TerminalProductionCustody<'_, C>,
    ) -> Result<ProducedTerminalArtifact<C>, CallbackCustodyTerminalArtifactProductionError<C>>
    {
        let TerminalProductionCustody {
            entry_identity,
            callback_custody,
            retain_unoptimized,
            timings,
        } = custody;
        match self.produce_retained(entry_identity, retain_unoptimized, timings) {
            Ok(produced) => Ok(ProducedTerminalArtifact {
                artifact: produced.artifact,
                receipt: produced.receipt,
                unoptimized: produced.unoptimized,
                boundary_operator_scope: produced.boundary_operator_scope,
                callback_custody,
                source_call_occurrences: produced.source_call_occurrences,
                selected_ieee_float_fma_occurrences: produced.selected_ieee_float_fma_occurrences,
                selected_ieee_float_comparison_occurrences: produced
                    .selected_ieee_float_comparison_occurrences,
                selected_integer_comparison_occurrences: produced
                    .selected_integer_comparison_occurrences,
            }),
            Err(error) => Err(CallbackCustodyTerminalArtifactProductionError {
                error,
                callback_custody,
            }),
        }
    }

    fn produce_retained(
        self,
        entry_identity: Option<[u8; 32]>,
        retain_unoptimized: bool,
        timings: &mut TerminalProductionTimings,
    ) -> Result<ProducedTerminalArtifact<()>, TerminalArtifactProductionError> {
        let checked = self.checked;
        // The receipt names the selected checked machine, so the selection is
        // rejoined before lowering only when an entry receipt is requested.
        let entry_selection = match entry_identity {
            Some(_) => Some(
                timings
                    .record_result(TerminalProductionStage::MachineSelection, || {
                        select_terminal_machine(checked, self.machine)
                    })
                    .map_err(TerminalArtifactProductionError::Lowering)?,
            ),
            None => None,
        };
        let (unoptimized, optimized) = self.lower_and_optimize(retain_unoptimized, timings)?;
        let entry_receipt = match (entry_identity, entry_selection) {
            (Some(source_signature_identity), Some(selection)) => Some(prepare_entry_receipt(
                checked,
                selection,
                source_signature_identity,
                optimized.lowered(),
                timings,
            )?),
            _ => None,
        };
        let (artifact, lowered) = timings
            .record_result(TerminalProductionStage::Publication, || {
                publish_terminal_artifact(optimized)
            })?;
        let receipt = match entry_receipt {
            Some(receipt) => {
                if artifact.manifest().semantic() != receipt.terminal_psi_identity() {
                    return Err(TerminalArtifactProductionError::EntryReceipt(
                        ProgramEntryTerminalReceiptError::ArtifactSemanticIdentityMismatch,
                    ));
                }
                Some(receipt)
            }
            None => None,
        };
        let boundary_operator_scope = timings
            .record_result(TerminalProductionStage::BoundaryOperatorScope, || {
                checked_boundary_operator_scope(checked, &artifact, &lowered)
            })
            .map_err(TerminalArtifactProductionError::Lowering)?;
        Ok(ProducedTerminalArtifact {
            artifact,
            receipt,
            unoptimized,
            boundary_operator_scope,
            callback_custody: (),
            source_call_occurrences: lowered.source_call_occurrences,
            selected_ieee_float_fma_occurrences: lowered.selected_ieee_float_fma_occurrences,
            selected_ieee_float_comparison_occurrences: lowered
                .selected_ieee_float_comparison_occurrences,
            selected_integer_comparison_occurrences: lowered
                .selected_integer_comparison_occurrences,
        })
    }

    /// Lower once, retain the unoptimized module when asked, then run the
    /// selected optimization on the same lowering.
    fn lower_and_optimize(
        self,
        retain_unoptimized: bool,
        timings: &mut TerminalProductionTimings,
    ) -> Result<(Option<LoweredPsi>, PsiOptimizationStageResult), TerminalArtifactProductionError>
    {
        timings
            .record_result(TerminalProductionStage::LedgerCheck, || {
                crate::checked_ledger::verify(self.checked)
            })
            .map_err(TerminalArtifactProductionError::Lowering)?;
        let lowered = timings
            .record_result(TerminalProductionStage::Lowering, || {
                lower_machine(self.checked, self.machine)
            })
            .map_err(TerminalArtifactProductionError::Lowering)?;
        let unoptimized = retain_unoptimized.then(|| lowered.clone());
        let optimized = timings
            .record_result(TerminalProductionStage::Optimization, || {
                run_psi_optimization(lowered, self.optimization_selections)
            })
            .map_err(TerminalArtifactProductionError::Optimization)?;
        Ok((unoptimized, optimized))
    }
}

/// Check the optimized module's unique Unit entry and derive the checked
/// `ProgramEntry` receipt before publication; the published artifact's
/// semantic identity is rejoined to the receipt afterwards.
fn prepare_entry_receipt(
    checked: &CheckedTrees,
    selection: &checked_trees::CheckedTerminalMachineSelection,
    source_signature_identity: [u8; 32],
    optimized_lowered: &LoweredPsi,
    timings: &mut TerminalProductionTimings,
) -> Result<CheckedProgramEntryTerminalReceipt, TerminalArtifactProductionError> {
    timings.record_result(TerminalProductionStage::EntryReceipt, || {
        let entry_matches = optimized_lowered
            .semantic_module
            .machines
            .iter()
            .filter(|machine| machine.id == optimized_lowered.semantic_module.entry)
            .collect::<Vec<_>>();
        let [entry] = entry_matches.as_slice() else {
            return Err(TerminalArtifactProductionError::EntryReceipt(
                ProgramEntryTerminalReceiptError::TerminalEntryMultiplicity(entry_matches.len()),
            ));
        };
        if entry.result != TerminalMachineResult::Unit {
            return Err(TerminalArtifactProductionError::EntryReceipt(
                ProgramEntryTerminalReceiptError::NonUnitEntry,
            ));
        }
        Ok(())
    })?;
    let terminal_psi_identity = timings
        .record_result(TerminalProductionStage::TerminalIdentity, || {
            terminal_psi_identity(&optimized_lowered.semantic_module)
        })
        .map_err(ProgramEntryTerminalReceiptError::TerminalIdentity)
        .map_err(TerminalArtifactProductionError::EntryReceipt)?;
    let receiver_eligibility = timings
        .record_result(TerminalProductionStage::ReceiverEligibility, || {
            Ok::<_, ()>(receiver_eligibility::derive(
                checked,
                selection,
                &optimized_lowered.semantic_module,
            ))
        })
        .expect("receiver eligibility derivation is infallible");
    Ok(CheckedProgramEntryTerminalReceipt::new(
        source_signature_identity,
        selection.name.clone(),
        selection.machine,
        terminal_psi_identity,
        optimized_lowered.semantic_module.entry,
        receiver_eligibility,
    ))
}

/// Publication consumes exactly the validated optimization result; retained
/// source evidence is extracted only after canonical publication succeeds.
fn publish_terminal_artifact(
    optimized: PsiOptimizationStageResult,
) -> Result<(terminal_codec::CanonicalTerminalArtifact, LoweredPsi), TerminalArtifactProductionError>
{
    let artifact = finalize_terminal_artifact(&optimized)
        .map_err(TerminalArtifactProductionError::Artifact)?;
    Ok((artifact, optimized.into_lowered()))
}

#[derive(Debug)]
pub enum TerminalArtifactProductionError {
    Lowering(LoweringError),
    Optimization(PsiOptimizationStageError),
    Artifact(terminal_codec::CanonicalTerminalArtifactError),
    EntryReceipt(ProgramEntryTerminalReceiptError),
}

#[derive(Debug)]
pub enum ProgramEntryTerminalReceiptError {
    TerminalEntryMultiplicity(usize),
    NonUnitEntry,
    TerminalIdentity(terminal_codec::CodecError),
    ArtifactSemanticIdentityMismatch,
}

impl std::fmt::Display for ProgramEntryTerminalReceiptError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "{self:?}")
    }
}

impl std::error::Error for ProgramEntryTerminalReceiptError {}

impl std::fmt::Display for TerminalArtifactProductionError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "{self:?}")
    }
}

impl std::error::Error for TerminalArtifactProductionError {}

fn checked_boundary_operator_scope(
    checked: &CheckedTrees,
    artifact: &terminal_codec::CanonicalTerminalArtifact,
    lowered: &LoweredPsi,
) -> Result<CheckedBoundaryOperatorApplicationScope, LoweringError> {
    lowered_psi_to_terminal_psi::checked_boundary_operator_scope(checked, artifact, lowered)
        .map_err(LoweringError::Unsupported)
}

#[cfg(test)]
mod tests {
    use checked_trees::CheckedTrees;

    use crate::{
        TerminalMachineSelection, TerminalProductionCustody, TerminalProductionRequest,
        TerminalProductionStage, TerminalProductionTimings,
    };

    fn check_source(source: &str) -> CheckedTrees {
        let tokens = source_files_to_tokens::Lexer::new(source)
            .tokenize()
            .unwrap();
        let syntax = tokens_to_syntax_trees::parse_syntax_trees(&tokens).unwrap();
        let resolved = syntax_trees_to_symbol_resolved_trees::resolve(
            syntax_trees_to_symbol_resolved_trees::ResolutionRequest::new(&syntax),
        )
        .unwrap();
        let typed =
            symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees(&resolved).unwrap();
        typed_trees_to_checked_trees::lower_typed_trees(
            typed,
            &typed_trees_to_checked_trees::CheckingRequest::settled(),
        )
        .unwrap()
    }

    /// Production lowering runs the correspondence retention route: the batch
    /// is extracted from the checked program's typed trees and installed on
    /// the module the artifact publishes. Ordinary programs produce an empty
    /// batch, so the published module carries no quotient rows and still
    /// decodes to identical identity.
    #[test]
    fn production_installs_the_extracted_quotient_correspondence_batch() {
        let checked = check_source(
            "data Main { value: i32; } machine Main::run(&mut self) { self.value = 7; }",
        );
        let produced =
            TerminalProductionRequest::new(&checked, TerminalMachineSelection::Name("Main::run"))
                .produce(TerminalProductionCustody {
                    retain_unoptimized: false,
                    entry_identity: Some([7; 32]),
                    callback_custody: (),
                    timings: &mut TerminalProductionTimings::default(),
                })
                .unwrap();
        assert!(produced.receipt().is_some());
        let module = terminal_codec::decode_module(produced.artifact().semantic_bytes()).unwrap();
        assert!(module.quotient_correspondences.is_empty());
    }

    /// Every product shares the instrumented body: without an entry identity
    /// an enabled carrier records the
    /// ledger/lowering/optimization/publication/boundary-scope ladder and no
    /// receipt, and a callback sidecar rides the same ladder back to the
    /// caller. An entry identity adds the machine-selection and receipt legs
    /// in their run positions.
    #[test]
    fn production_records_the_stage_ladder_for_each_retained_custody() {
        let checked = check_source(
            "data Main { value: i32; } machine Main::run(&mut self) { self.value = 7; }",
        );
        let mut timings = TerminalProductionTimings::enabled();
        let produced =
            TerminalProductionRequest::new(&checked, TerminalMachineSelection::Name("Main::run"))
                .produce(TerminalProductionCustody::artifact_only(&mut timings))
                .unwrap();
        assert!(!produced.artifact().semantic_bytes().is_empty());
        assert!(produced.receipt().is_none());
        let stages: Vec<TerminalProductionStage> =
            timings.rows().iter().map(|(stage, _)| *stage).collect();
        assert_eq!(
            stages,
            [
                TerminalProductionStage::LedgerCheck,
                TerminalProductionStage::Lowering,
                TerminalProductionStage::Optimization,
                TerminalProductionStage::Publication,
                TerminalProductionStage::BoundaryOperatorScope,
            ]
        );

        let mut callback_timings = TerminalProductionTimings::enabled();
        let produced =
            TerminalProductionRequest::new(&checked, TerminalMachineSelection::Name("Main::run"))
                .produce(TerminalProductionCustody {
                    retain_unoptimized: false,
                    entry_identity: None,
                    callback_custody: 42u8,
                    timings: &mut callback_timings,
                })
                .unwrap();
        assert_eq!(produced.callback_custody(), &42u8);
        let callback_stages: Vec<TerminalProductionStage> = callback_timings
            .rows()
            .iter()
            .map(|(stage, _)| *stage)
            .collect();
        assert_eq!(callback_stages, stages);

        let mut entry_timings = TerminalProductionTimings::enabled();
        let produced =
            TerminalProductionRequest::new(&checked, TerminalMachineSelection::Name("Main::run"))
                .produce(TerminalProductionCustody {
                    retain_unoptimized: false,
                    entry_identity: Some([9; 32]),
                    callback_custody: (),
                    timings: &mut entry_timings,
                })
                .unwrap();
        assert_eq!(
            produced
                .receipt()
                .map(|receipt| receipt.source_signature_identity()),
            Some([9; 32])
        );
        let entry_stages: Vec<TerminalProductionStage> = entry_timings
            .rows()
            .iter()
            .map(|(stage, _)| *stage)
            .collect();
        assert_eq!(
            entry_stages,
            [
                TerminalProductionStage::MachineSelection,
                TerminalProductionStage::LedgerCheck,
                TerminalProductionStage::Lowering,
                TerminalProductionStage::Optimization,
                TerminalProductionStage::EntryReceipt,
                TerminalProductionStage::TerminalIdentity,
                TerminalProductionStage::ReceiverEligibility,
                TerminalProductionStage::Publication,
                TerminalProductionStage::BoundaryOperatorScope,
            ]
        );
    }

    /// One source-produced rebound dynamic scalar call: the checked plan's
    /// `CallDynamicScalar` materializes a descriptor table (one rebound
    /// descriptor joining an initial and a latest conformance selection) and
    /// dispatches through it. The boundary-operator scope replays that exact
    /// occurrence back to its checked plan.
    const REBOUND_DYNAMIC_SOURCE: &str = r#"
        trait Measure {
            machine measure(&self) -> i32;
        }

        data Item [copy] {
            value: i32;
        }

        Primary: Item satisfies Measure {
            machine measure(&self) -> i32 {
                transition { _ -> self.value }
            }
        }

        data Main [copy] {
            decoy: Item;
            selected: Item;
        }

        machine Main::run(&mut self) {
            let mut erased: &dyn Measure = &self.decoy as &dyn Item::Primary;
            erased = &self.selected as &dyn Item::Primary;
            let result: i32 = erased.measure();
        }
    "#;

    /// The same descriptor-table materialization on the Unit-result lane:
    /// `CallDynamicUnit` instead of `CallDynamicScalar`.
    const REBOUND_DYNAMIC_UNIT_SOURCE: &str = r#"
        trait Touch {
            machine touch(&self);
        }

        data Item {
            value: i32;
        }

        Primary: Item satisfies Touch {
            machine touch(&self) {}
        }

        data Main {
            decoy: Item;
            selected: Item;
        }

        machine Main::run(&mut self) {
            let mut erased: &dyn Touch = &self.decoy as &dyn Item::Primary;
            erased = &self.selected as &dyn Item::Primary;
            erased.touch();
        }
    "#;

    /// The rebound whose two selections name different checked conformances,
    /// so the descriptor retains two distinct closed applications.
    const CHANGED_CONFORMANCE_REBOUND_SOURCE: &str = r#"
        trait Touch { machine touch(&self); }
        data Item { value: i32; }

        Primary: Item satisfies Touch { machine touch(&self) {} }
        Secondary: Item satisfies Touch { machine touch(&self) {} }

        data Main { decoy: Item; selected: Item; }

        machine Main::run(&mut self) {
            let mut erased: &dyn Touch = &self.decoy as &dyn Item::Primary;
            erased = &self.selected as &dyn Item::Secondary;
            erased.touch();
        }
    "#;

    fn rebound_dynamic_produced(
        source: &str,
    ) -> (
        checked_trees::CheckedTrees,
        crate::ProducedTerminalArtifact<()>,
    ) {
        let checked = check_source(source);
        let produced =
            TerminalProductionRequest::new(&checked, TerminalMachineSelection::Name("Main::run"))
                .produce(TerminalProductionCustody {
                    retain_unoptimized: true,
                    entry_identity: None,
                    callback_custody: (),
                    timings: &mut TerminalProductionTimings::default(),
                })
                .expect("rebound dynamic production publishes");
        (checked, produced)
    }

    /// The lowered module production retained beside the artifact: under the
    /// default identity optimization it is the module the boundary scope
    /// replayed, kept here so tests can mutate its sidecars without lowering
    /// again.
    fn retained_lowered(produced: &crate::ProducedTerminalArtifact<()>) -> lowered_psi::LoweredPsi {
        produced
            .unoptimized()
            .expect("custody retained the lowered module")
            .clone()
    }

    fn published_module(
        produced: &crate::ProducedTerminalArtifact<()>,
    ) -> terminal_psi::TerminalModule {
        terminal_codec::decode_module(produced.artifact().semantic_bytes())
            .expect("published Terminal module decodes")
    }

    #[test]
    fn a_rebound_dynamic_call_rejoins_one_exact_descriptor_table_occurrence() {
        use lowered_psi_to_terminal_psi::CheckedDynamicCallLane;
        let (checked, produced) = rebound_dynamic_produced(REBOUND_DYNAMIC_SOURCE);
        let module = published_module(&produced);
        let catalog = &module.dynamic_dispatch;
        let [dispatch] = catalog.indirect_dispatches.as_slice() else {
            panic!("one indirect dispatch row");
        };
        let [descriptor] = catalog.rebound_descriptors.as_slice() else {
            panic!("one rebound descriptor");
        };
        assert_eq!(catalog.selections.len(), 2);
        assert_eq!(catalog.stored_descriptors.len(), 0);
        assert!(catalog.direct_dispatches.is_empty());
        assert!(catalog.stored_dispatches.is_empty());
        assert!(catalog.parameter_dispatches.is_empty());
        assert_eq!(produced.source_call_occurrences().len(), 1);
        let operation = module
            .machines
            .iter()
            .flat_map(|machine| machine.blocks.iter())
            .flat_map(|block| block.operations.iter())
            .find(|operation| operation.id == dispatch.operation)
            .expect("the dispatch names an emitted operation");
        assert!(matches!(
            operation.kind,
            terminal_psi::OperationKind::CallDynamicScalar { .. }
        ));
        let scope = produced.boundary_operator_scope();
        let [occurrence] = scope.dynamic_call_occurrences() else {
            panic!("one replayed dynamic call occurrence");
        };
        assert_eq!(occurrence.lane(), CheckedDynamicCallLane::ReboundScalar);
        assert_eq!(occurrence.plan_index(), 0);
        assert_eq!(occurrence.terminal_machine(), dispatch.owner);
        assert_eq!(occurrence.terminal_operation(), dispatch.operation);
        assert_eq!(occurrence.descriptor_ordinal(), dispatch.descriptor_ordinal);
        assert_eq!(occurrence.descriptor_ordinal(), descriptor.ordinal,);
        assert_eq!(
            produced.source_call_occurrences()[0].terminal_operation,
            dispatch.operation
        );
        // The same roster replays directly against the published artifact.
        let lowered = retained_lowered(&produced);
        let replayed = lowered_psi_to_terminal_psi::checked_boundary_operator_scope(
            &checked,
            produced.artifact(),
            &lowered,
        )
        .expect("the recorded roster rejoins its checked rebound plan");
        assert_eq!(
            replayed.dynamic_call_occurrences(),
            scope.dynamic_call_occurrences()
        );
    }

    #[test]
    fn a_rebound_dynamic_unit_call_rejoins_its_descriptor_table_occurrence() {
        use lowered_psi_to_terminal_psi::CheckedDynamicCallLane;
        let (_checked, produced) = rebound_dynamic_produced(REBOUND_DYNAMIC_UNIT_SOURCE);
        let module = published_module(&produced);
        let catalog = &module.dynamic_dispatch;
        let [dispatch] = catalog.indirect_dispatches.as_slice() else {
            panic!("one indirect dispatch row");
        };
        let operation = module
            .machines
            .iter()
            .flat_map(|machine| machine.blocks.iter())
            .flat_map(|block| block.operations.iter())
            .find(|operation| operation.id == dispatch.operation)
            .expect("the dispatch names an emitted operation");
        assert!(matches!(
            operation.kind,
            terminal_psi::OperationKind::CallDynamicUnit { .. }
        ));
        let scope = produced.boundary_operator_scope();
        let [occurrence] = scope.dynamic_call_occurrences() else {
            panic!("one replayed dynamic call occurrence");
        };
        assert_eq!(occurrence.lane(), CheckedDynamicCallLane::ReboundUnit);
        assert_eq!(occurrence.plan_index(), 0);
        assert_eq!(occurrence.terminal_operation(), dispatch.operation);
    }

    #[test]
    fn a_changed_conformance_rebound_rejoins_two_closed_applications() {
        let (_checked, produced) = rebound_dynamic_produced(CHANGED_CONFORMANCE_REBOUND_SOURCE);
        let module = published_module(&produced);
        let catalog = &module.dynamic_dispatch;
        assert_eq!(catalog.selections.len(), 2);
        assert_ne!(
            catalog.selections[0].conformance_application_commitment,
            catalog.selections[1].conformance_application_commitment,
            "the changed conformance keeps two distinct closed applications",
        );
        let scope = produced.boundary_operator_scope();
        let [occurrence] = scope.dynamic_call_occurrences() else {
            panic!("one replayed dynamic call occurrence");
        };
        assert_eq!(occurrence.plan_index(), 0);
    }

    #[test]
    fn a_stale_or_duplicated_dynamic_occurrence_rejects() {
        use semantic_vocabulary::OperationId;
        let (checked, produced) = rebound_dynamic_produced(REBOUND_DYNAMIC_SOURCE);
        let lowered = retained_lowered(&produced);
        let replay = |corrupted: &lowered_psi::LoweredPsi| {
            lowered_psi_to_terminal_psi::checked_boundary_operator_scope(
                &checked,
                produced.artifact(),
                corrupted,
            )
            .unwrap_err()
        };
        // A row naming an operation the module no longer carries.
        let mut corrupted = lowered.clone();
        corrupted.source_call_occurrences[0].terminal_operation =
            OperationId::new(u64::MAX).expect("unused operation identity");
        assert_eq!(
            replay(&corrupted),
            "rebound dynamic call does not name one exact Terminal operation"
        );
        // A duplicated occurrence row.
        let mut corrupted = lowered.clone();
        let duplicate = corrupted.source_call_occurrences[0].clone();
        corrupted.source_call_occurrences.push(duplicate);
        assert_eq!(
            replay(&corrupted),
            "rebound dynamic call maps to duplicate Terminal occurrences"
        );
        // A stale coordinate leaves the surviving indirect dispatch unchecked.
        let mut corrupted = lowered.clone();
        corrupted.source_call_occurrences[0].call_ordinal += 1;
        assert_eq!(
            replay(&corrupted),
            "Terminal indirect dispatch does not rejoin one checked rebound call"
        );
    }

    #[test]
    fn a_substituted_dynamic_selection_identity_rejects() {
        let (checked, produced) = rebound_dynamic_produced(REBOUND_DYNAMIC_SOURCE);
        let lowered = retained_lowered(&produced);
        let replay = |corrupted: &checked_trees::CheckedTrees| {
            lowered_psi_to_terminal_psi::checked_boundary_operator_scope(
                corrupted,
                produced.artifact(),
                &lowered,
            )
            .unwrap_err()
        };
        let drifted = "rebound dynamic dispatch drifted from its checked selection";
        // A checked plan whose requirement identity no longer matches the
        // emitted dispatch row.
        let mut corrupted = checked.clone();
        corrupted
            .facts
            .flow
            .terminal_unit_effects
            .dynamic_dispatch
            .rebound_scalar_calls[0]
            .latest
            .requirement_identity = "foreign".to_owned();
        assert_eq!(replay(&corrupted), drifted);
        // A checked plan whose family tuple is substituted.
        let mut corrupted = checked.clone();
        corrupted
            .facts
            .flow
            .terminal_unit_effects
            .dynamic_dispatch
            .rebound_scalar_calls[0]
            .latest
            .family_tuple = Box::from(["foreign".to_owned()]);
        assert_eq!(replay(&corrupted), drifted);
        // A stale checked coordinate leaves the indirect dispatch unrejoined.
        let mut corrupted = checked.clone();
        corrupted
            .facts
            .flow
            .terminal_unit_effects
            .dynamic_dispatch
            .rebound_scalar_calls[0]
            .latest
            .coordinate
            .statement_index += 1;
        assert_eq!(
            replay(&corrupted),
            "Terminal indirect dispatch does not rejoin one checked rebound call"
        );
        // A retained conformance row whose requirement identity drifted.
        let mut corrupted = checked.clone();
        corrupted
            .facts
            .flow
            .terminal_unit_effects
            .dynamic_dispatch
            .rebound_scalar_calls[0]
            .latest
            .selection
            .rows[0]
            .requirement_identity = "foreign".to_owned();
        assert_eq!(
            replay(&corrupted),
            "rebound dynamic conformance application drifted from its checked selection"
        );
    }

    #[test]
    fn a_foreign_dynamic_call_scope_rejects() {
        let (_checked, produced) = rebound_dynamic_produced(REBOUND_DYNAMIC_SOURCE);
        let lowered = retained_lowered(&produced);
        // A program whose rebound plan names a different requirement symbol:
        // replaying the published roster under foreign checked custody either
        // drifts on the row identities or leaves the dispatch unrejoined.
        let foreign = check_source(&REBOUND_DYNAMIC_SOURCE.replace("measure", "gauge"));
        let error = lowered_psi_to_terminal_psi::checked_boundary_operator_scope(
            &foreign,
            produced.artifact(),
            &lowered,
        )
        .unwrap_err();
        assert!(
            [
                "rebound dynamic dispatch drifted from its checked selection",
                "Terminal indirect dispatch does not rejoin one checked rebound call",
            ]
            .contains(&error),
            "unexpected replay failure: {error}"
        );
    }
}
