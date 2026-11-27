//! The lowered program the checker accepted, with the evidence it issued.
//! Stage 06 produces these records and stage 07 publishes Psi from them.
#![forbid(unsafe_code)]

use checked_program_issuer::CheckerIssuer;
use semantic_vocabulary::MachineId;
use std::sync::Arc;
use terminal_check_tables::compilation::CompilationForPublication;
use terminal_check_tables::program::Elaboration;
use terminal_codec::ObligationEvidenceSeal;

/// The root closure the lowered-form checker accepted: the exact lowered
/// program it judged, one evidence seal per discharged operation obligation,
/// and the record of any optimization it executed and rechecked.
///
/// Only stage 06 can issue one. A consumer reads it and cannot change it.
#[derive(Debug)]
pub struct CheckedProgram {
    elaboration: Elaboration,
    seals: Vec<ObligationEvidenceSeal>,
    optimization: Option<terminal_codec::PsiOptimizationExecutionRecord>,
}

impl CheckedProgram {
    /// Keep the program the checker accepted. The issuer argument is the reason
    /// no other crate can call this: only stage 06 links the crate that supplies
    /// one, and `mbx xtask check-boundary` refuses any other link.
    pub fn from_judgment(
        _issuer: CheckerIssuer,
        elaboration: Elaboration,
        seals: Vec<ObligationEvidenceSeal>,
        optimization: Option<terminal_codec::PsiOptimizationExecutionRecord>,
    ) -> Self {
        Self {
            elaboration,
            seals,
            optimization,
        }
    }

    pub fn elaboration(&self) -> &Elaboration {
        &self.elaboration
    }

    pub fn optimization(&self) -> Option<&terminal_codec::PsiOptimizationExecutionRecord> {
        self.optimization.as_ref()
    }

    pub fn seals(&self) -> &[ObligationEvidenceSeal] {
        &self.seals
    }

    pub fn prepare_publication(&self) -> Result<CheckedPublicationModule, PublicationError> {
        prepare(&self.elaboration, &self.seals, self.optimization.as_ref())
    }

    pub fn publish(&self) -> Result<RoutePublication, PublicationError> {
        self.prepare_publication()?.publish()
    }

    pub fn publish_invocation(
        &self,
    ) -> Result<RoutePublication<terminal_codec::InvocationTerminalCode>, PublicationError> {
        self.prepare_publication()?.publish_invocation()
    }
}

/// Prepare the publication module of a closure the checker still holds with
/// its discharge. The checker's own tests publish closures this way; every
/// other consumer starts from a `CheckedProgram`.
pub fn prepare_accepted_publication(
    _issuer: CheckerIssuer,
    elaboration: &Elaboration,
    seals: &[ObligationEvidenceSeal],
    optimization: Option<&terminal_codec::PsiOptimizationExecutionRecord>,
) -> Result<CheckedPublicationModule, PublicationError> {
    prepare(elaboration, seals, optimization)
}

/// The checker's judgment of one lowered compilation. The input type names the
/// lowering that supplied both the terminal compilation and the source kept
/// beside the checked program, so a judgment of a different input is a
/// different type.
pub struct CheckedPublication<Input: CompilationForPublication> {
    source: Input::Source,
    program: Arc<CheckedProgram>,
    // These checked generic and witness forms have no executable publication authority.
    further_forms: Vec<Arc<Elaboration>>,
}

impl<Input: CompilationForPublication> CheckedPublication<Input> {
    /// Return the source of `Input` with the program the checker accepted for
    /// it. `further_forms` holds the generic forms and conformance witnesses
    /// accepted beside the root closure. Only stage 06 holds the issuer.
    pub fn from_judgment(
        _issuer: CheckerIssuer,
        source: Input::Source,
        program: Arc<CheckedProgram>,
        further_forms: Vec<Arc<Elaboration>>,
    ) -> Self {
        Self {
            source,
            program,
            further_forms,
        }
    }

    /// The retained source, the accepted root closure, and the further
    /// forms the checker accepted beside it.
    pub fn into_source_and_program(
        self,
    ) -> (Input::Source, Arc<CheckedProgram>, Vec<Arc<Elaboration>>) {
        (self.source, self.program, self.further_forms)
    }
}

/// An accepted closure published as Terminal Psi: the canonical artifact of
/// the module the route checked, and the evidence seals bound to that
/// module's identity, which travel beside it in memory to Omega 00.
#[derive(Debug)]
pub struct RoutePublication<Artifact = terminal_codec::CanonicalTerminalArtifact> {
    pub artifact: Artifact,
    pub seals: terminal_codec::EvidenceSeals,
}

impl RoutePublication {
    /// Uses the same nominal-entry receipt as every other Terminal producer.
    /// A same-spelled machine from another owner cannot satisfy this root.
    pub fn entry_receipt(
        &self,
        declaration: &terminal_psi::PreparedNominalRoot,
        source_signature_identity: [u8; 32],
    ) -> Result<
        psi_publication_records::CheckedProgramEntryTerminalReceipt,
        terminal_codec::entry_receipt::ProgramEntryTerminalReceiptError,
    > {
        terminal_codec::entry_receipt::prepare_program_entry_receipt(
            declaration,
            source_signature_identity,
            &self.artifact,
        )
    }
}

/// Why an accepted closure could not be published.
#[derive(Debug)]
pub enum PublicationError {
    /// A conditional installation refinement is a proof problem, not a program.
    InstallationReachRefinement,
    /// The closure retains abstract requirements, logical-frame envelopes,
    /// open source binders, static-value formals or opaque rigid values.
    /// These generic check forms are never executable source programs.
    GenericCheckForm(String),
    /// A machine of the module has no declaration identity, for the reason
    /// given (`Elaboration::undeclared`). product.md: "Source-produced
    /// machine bodies retain a portable checked declaration identity".
    Undeclared(MachineId, &'static str),
    /// The module fails a structural check of Terminal semantics.
    Semantic(String),
    Artifact(terminal_codec::CanonicalTerminalArtifactError),
    Seals(terminal_codec::CodecError),
}

/// The exact module and evidence the lowered-form checker approved for publication.
/// This value owns no typed tree, source correspondence or checker work tables.
pub struct CheckedPublicationModule {
    module: terminal_codec::ClosedTerminalModule,
    optimization: Option<terminal_codec::PsiOptimizationExecutionRecord>,
    seals: terminal_codec::EvidenceSeals,
}

impl CheckedPublicationModule {
    pub fn module(&self) -> &terminal_psi::TerminalModule {
        self.module.module()
    }

    pub fn publish(self) -> Result<RoutePublication, PublicationError> {
        let Self {
            module,
            seals,
            optimization,
        } = self;
        let identity = module.identity();
        let record = match optimization {
            Some(record) => record,
            None => terminal_codec::PsiOptimizationExecutionRecord::new(
                optimization::PsiOptimizationSelections::default(),
                identity,
                identity,
            )
            .map_err(|error| PublicationError::Semantic(error.to_string()))?,
        };
        let artifact =
            terminal_codec::CanonicalTerminalArtifact::from_closed_parts(module, record, None)
                .map_err(PublicationError::Artifact)?;
        Ok(RoutePublication { artifact, seals })
    }

    pub fn publish_invocation(
        self,
    ) -> Result<RoutePublication<terminal_codec::InvocationTerminalCode>, PublicationError> {
        let Self {
            module,
            seals,
            optimization,
        } = self;
        if optimization.is_some() {
            return Err(PublicationError::Semantic(
                "executed Psi optimizations require their portable publication record".into(),
            ));
        }
        let artifact = terminal_codec::InvocationTerminalCode::from_closed_module(module)
            .map_err(PublicationError::Seals)?;
        Ok(RoutePublication { artifact, seals })
    }
}

fn prepare(
    elaboration: &Elaboration,
    seals: &[ObligationEvidenceSeal],
    optimization: Option<&terminal_codec::PsiOptimizationExecutionRecord>,
) -> Result<CheckedPublicationModule, PublicationError> {
    validate(elaboration)?;
    let module = terminal_codec::ClosedTerminalModule::new(elaboration.module.clone())
        .map_err(PublicationError::Seals)?;
    let seals = terminal_codec::EvidenceSeals {
        module: module.identity(),
        seals: seals.to_vec(),
    };
    Ok(CheckedPublicationModule {
        module,
        seals,
        optimization: optimization.cloned(),
    })
}

fn validate(elaboration: &Elaboration) -> Result<(), PublicationError> {
    if elaboration.installation_bound_refinement.is_some() {
        return Err(PublicationError::InstallationReachRefinement);
    }
    if !elaboration.requirements.is_empty()
        || !elaboration.static_reach_parameters.is_empty()
        || !elaboration.static_reach_applications.is_empty()
        || !elaboration.static_reach_summaries.is_empty()
        || !elaboration.generic_callable_declarations.is_empty()
        || !elaboration.check_form_envelopes.is_empty()
        || !elaboration.generic_binders.is_empty()
        || !elaboration.generic_storage.is_empty()
        || !elaboration.nominal_calls.is_empty()
        || !elaboration.nominal_types.is_empty()
        || elaboration
            .module
            .types
            .structural_types
            .iter()
            .any(|row| matches!(row.shape, terminal_psi::StructuralTypeShape::TypeParameter))
        || elaboration
            .value_parameters
            .iter()
            .any(|(_, count)| *count != 0)
        || elaboration.tables.machines.iter().any(|(_, machine)| {
            machine
                .values
                .iter()
                .any(|(_, value)| value.rigid.is_some())
        })
    {
        // Report each guarded roster together so a refused publication can
        // be traced without dumping its module or changing its admission.
        let mut static_values = elaboration
            .value_parameters
            .iter()
            .filter(|(_, count)| **count != 0);
        let first_static = static_values.next();
        let static_count = static_values.count() + usize::from(first_static.is_some());
        let mut rigid_values = elaboration
            .tables
            .machines
            .iter()
            .flat_map(|(machine, row)| {
                row.values
                    .iter()
                    .filter_map(move |(value, row)| row.rigid.map(|rigid| (machine, value, rigid)))
            });
        let first_rigid = rigid_values.next();
        let rigid_count = rigid_values.count() + usize::from(first_rigid.is_some());
        return Err(PublicationError::GenericCheckForm(format!(
            "abstract requirements: {} (first {:?}); check-form envelopes: {} (first {:?}); open source binders: {} (first {:?}); static-value machines: {} (first {:?}); rigid values: {} (first {:?})",
            elaboration.requirements.len(),
            elaboration.requirements.first(),
            elaboration.check_form_envelopes.len(),
            elaboration
                .check_form_envelopes
                .first()
                .map(|row| row.machine),
            elaboration.generic_binders.len(),
            elaboration.generic_binders.first(),
            static_count,
            first_static,
            rigid_count,
            first_rigid,
        )));
    }
    if let Some((machine, reason)) = elaboration.undeclared.first() {
        return Err(PublicationError::Undeclared(*machine, reason));
    }
    terminal_semantics::activation_storage::validate_module(&elaboration.module)
        .map_err(|reason| PublicationError::Semantic(reason.to_owned()))?;
    Ok(())
}
