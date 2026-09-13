use super::*;

/// Exact selected provider-plan input consumed by external-root construction.
///
/// The schema is retained beside the normalized plan identity so installation
/// can bind the source callable shape—including domain/carry-qualified
/// parameter types—to the same provider selection later carried by
/// `ProviderExecution` and per-invocation entry receipts.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SelectedExternalRootProviderPlan {
    pub identity: external_roots::ProviderPlanId,
    pub digest: effects::provider_plan::ProviderPlanDigest,
    pub schema: ServiceSchema,
    pub(super) exact_plan: ProviderPlan,
}

/// One exact selected source schema, AOT-lowered writer, installed resolver,
/// and activated unpublished destination sealed to the provider-populated
/// preparation they admitted. All four precede source resolution.
#[derive(Debug)]
pub struct SelectedExternalRootPostHandoffWriterPreparation<'installed, 'mapping, 'bytes> {
    selected_provider: SelectedExternalRootProviderPlan,
    lowered: program_entry_plan::LoweredPostHandoffWriter,
    installed_code: &'installed executable_installation::InstalledCode,
    prepared: external_roots::PreparedExternalRootPostHandoffWriterInvocation,
    destination:
        executable_installation::ValidatedPreparedPostHandoffWriterDestination<'mapping, 'bytes>,
}

impl<'installed, 'mapping, 'bytes>
    SelectedExternalRootPostHandoffWriterPreparation<'installed, 'mapping, 'bytes>
{
    pub const fn selected_provider(&self) -> &SelectedExternalRootProviderPlan {
        &self.selected_provider
    }

    pub const fn prepared(
        &self,
    ) -> &external_roots::PreparedExternalRootPostHandoffWriterInvocation {
        &self.prepared
    }

    pub const fn lowered(&self) -> &program_entry_plan::LoweredPostHandoffWriter {
        &self.lowered
    }

    pub const fn installed_code(&self) -> &'installed executable_installation::InstalledCode {
        self.installed_code
    }

    pub const fn destination(
        &self,
    ) -> &executable_installation::ValidatedPreparedPostHandoffWriterDestination<'mapping, 'bytes>
    {
        &self.destination
    }
}

/// Preparation rejection returns the exact selected schema, lowered writer,
/// and destination. Provider execution, installed code, and writer are
/// borrowed inputs and therefore remain with the caller unchanged.
#[derive(Debug)]
pub struct SelectedExternalRootWriterPreparationError<'mapping, 'bytes> {
    selected_provider: SelectedExternalRootProviderPlan,
    lowered: program_entry_plan::LoweredPostHandoffWriter,
    destination: executable_installation::PreparedPostHandoffWriterDestination<'mapping, 'bytes>,
    diagnostic: external_roots::ExternalRootDiagnostic,
}

impl<'mapping, 'bytes> SelectedExternalRootWriterPreparationError<'mapping, 'bytes> {
    pub const fn diagnostic(&self) -> &external_roots::ExternalRootDiagnostic {
        &self.diagnostic
    }

    pub fn into_parts(
        self,
    ) -> (
        SelectedExternalRootProviderPlan,
        program_entry_plan::LoweredPostHandoffWriter,
        executable_installation::PreparedPostHandoffWriterDestination<'mapping, 'bytes>,
    ) {
        (self.selected_provider, self.lowered, self.destination)
    }
}

/// A generated writer whose exact AOT fragment and selected source schema are
/// joined to one admitted external-root execution and its provider-populated
/// invocation context, exact installed resolver, and activated destination.
/// It establishes no publication authority.
#[derive(Debug)]
pub struct BoundExternalRootPostHandoffWriterInvocation<'installed, 'mapping, 'bytes> {
    selected_provider: SelectedExternalRootProviderPlan,
    lowered: program_entry_plan::LoweredPostHandoffWriter,
    installed_code: &'installed executable_installation::InstalledCode,
    prepared: external_roots::PreparedExternalRootPostHandoffWriterInvocation,
    destination:
        executable_installation::ValidatedPreparedPostHandoffWriterDestination<'mapping, 'bytes>,
}

/// Still-unpublished destination after one exact bound external-root writer
/// executes. The AOT-lowered fragment remains attached to the installation
/// context and installed resolver instead of being reduced to copied
/// fingerprints at this boundary. This carrier establishes neither consumer
/// semantics nor publication.
#[derive(Debug)]
pub struct WrittenBoundExternalRootPostHandoffWriterDestination<'installed, 'mapping, 'bytes> {
    selected_provider: SelectedExternalRootProviderPlan,
    lowered: program_entry_plan::LoweredPostHandoffWriter,
    installed_code: &'installed executable_installation::InstalledCode,
    written: external_roots::WrittenExternalRootPostHandoffWriterDestination<'mapping, 'bytes>,
}

/// A written compiler-bound destination whose complete retained context has
/// been replayed against the exact installed realization selected by its
/// outward consumer. Only this carrier exposes the still-unpublished bytes.
/// It grants no publication or provider-operation authority.
#[derive(Debug)]
pub struct ValidatedWrittenBoundExternalRootPostHandoffWriterDestination<
    'installed,
    'mapping,
    'bytes,
> {
    selected_provider: SelectedExternalRootProviderPlan,
    lowered: program_entry_plan::LoweredPostHandoffWriter,
    installed_code: &'installed executable_installation::InstalledCode,
    written:
        external_roots::ValidatedWrittenExternalRootPostHandoffWriterDestination<'mapping, 'bytes>,
}

/// Outward-consumer rejection returns the complete written carrier unchanged,
/// so validation can be retried against the correct installed realization.
#[derive(Debug)]
pub struct WrittenBoundExternalRootConsumerValidationError<'installed, 'mapping, 'bytes> {
    written: WrittenBoundExternalRootPostHandoffWriterDestination<'installed, 'mapping, 'bytes>,
    diagnostic: layout_plans::MaterializationDiagnostic,
}

impl<'installed, 'mapping, 'bytes>
    WrittenBoundExternalRootConsumerValidationError<'installed, 'mapping, 'bytes>
{
    pub const fn diagnostic(&self) -> &layout_plans::MaterializationDiagnostic {
        &self.diagnostic
    }

    pub fn into_written(
        self,
    ) -> WrittenBoundExternalRootPostHandoffWriterDestination<'installed, 'mapping, 'bytes> {
        self.written
    }
}

/// Failed recovery of a compiler-bound writer destination. The error retains
/// the complete written carrier so no lowered, provider, installation, or
/// destination custody is reconstructed after rejection.
#[derive(Debug)]
pub struct WrittenBoundExternalRootWriterRecoveryError<'installed, 'mapping, 'bytes> {
    written: WrittenBoundExternalRootPostHandoffWriterDestination<'installed, 'mapping, 'bytes>,
    diagnostic: layout_plans::MaterializationDiagnostic,
}

impl<'installed, 'mapping, 'bytes>
    WrittenBoundExternalRootWriterRecoveryError<'installed, 'mapping, 'bytes>
{
    pub const fn diagnostic(&self) -> &layout_plans::MaterializationDiagnostic {
        &self.diagnostic
    }

    pub fn into_written(
        self,
    ) -> WrittenBoundExternalRootPostHandoffWriterDestination<'installed, 'mapping, 'bytes> {
        self.written
    }
}

impl<'installed, 'mapping, 'bytes>
    WrittenBoundExternalRootPostHandoffWriterDestination<'installed, 'mapping, 'bytes>
{
    pub const fn selected_provider(&self) -> &SelectedExternalRootProviderPlan {
        &self.selected_provider
    }

    pub const fn lowered(&self) -> &program_entry_plan::LoweredPostHandoffWriter {
        &self.lowered
    }

    pub const fn installed_code(&self) -> &'installed executable_installation::InstalledCode {
        self.installed_code
    }

    /// Independently replay the retained AOT bytes, footprint, invocation,
    /// opaque installation context, and exact borrowed installed realization.
    /// Rejection only borrows this carrier, preserving every input for retry.
    pub fn validate_for_consumer(&self) -> Result<(), layout_plans::MaterializationDiagnostic> {
        program_entry_plan::validate_lowered_post_handoff_writer(&self.lowered)
            .map_err(|diagnostic| layout_plans::MaterializationDiagnostic(diagnostic.message))?;
        self.written.validate_for_consumer(self.installed_code)?;
        if self.lowered.fragment().target().architecture != self.installed_code.architecture()
            || self.written.invocation() != self.lowered.invocation()
        {
            return Err(layout_plans::MaterializationDiagnostic(
                "written bound external-root destination does not retain its exact lowered fragment and installation context"
                    .into(),
            ));
        }
        validate_selected_provider_written_source(&self.selected_provider, &self.written)
    }

    /// Consume this still-unpublished carrier only after independently
    /// replaying it against the exact installed realization held by the
    /// outward consumer. Rejection exposes no bytes and returns full custody.
    pub fn into_validated_for_consumer(
        self,
        installed_code: &executable_installation::InstalledCode,
    ) -> Result<
        ValidatedWrittenBoundExternalRootPostHandoffWriterDestination<'installed, 'mapping, 'bytes>,
        Box<WrittenBoundExternalRootConsumerValidationError<'installed, 'mapping, 'bytes>>,
    > {
        let diagnostic = if !std::ptr::eq(self.installed_code, installed_code) {
            Some(layout_plans::MaterializationDiagnostic(
                "written bound external-root destination does not belong to the exact installed realization held by its consumer"
                    .into(),
            ))
        } else {
            self.validate_for_consumer().err()
        };
        if let Some(diagnostic) = diagnostic {
            return Err(Box::new(WrittenBoundExternalRootConsumerValidationError {
                written: self,
                diagnostic,
            }));
        }
        let Self {
            selected_provider,
            lowered,
            installed_code,
            written,
        } = self;
        match written.into_validated_for_consumer(installed_code) {
            Ok(written) => Ok(
                ValidatedWrittenBoundExternalRootPostHandoffWriterDestination {
                    selected_provider,
                    lowered,
                    installed_code,
                    written,
                },
            ),
            Err(error) => {
                let diagnostic = error.diagnostic().clone();
                Err(Box::new(WrittenBoundExternalRootConsumerValidationError {
                    written: WrittenBoundExternalRootPostHandoffWriterDestination {
                        selected_provider,
                        lowered,
                        installed_code,
                        written: (*error).into_written(),
                    },
                    diagnostic,
                }))
            }
        }
    }

    /// Recover the exact bound invocation with its still-unpublished
    /// destination for another execution attempt. Every retained layer is
    /// independently replayed before ownership crosses; rejection returns this
    /// complete written carrier unchanged.
    pub fn recover_for_retry(
        self,
    ) -> Result<
        BoundExternalRootPostHandoffWriterInvocation<'installed, 'mapping, 'bytes>,
        Box<WrittenBoundExternalRootWriterRecoveryError<'installed, 'mapping, 'bytes>>,
    > {
        if let Err(diagnostic) = self.validate_for_consumer() {
            return Err(Box::new(WrittenBoundExternalRootWriterRecoveryError {
                written: self,
                diagnostic,
            }));
        }
        let Self {
            selected_provider,
            lowered,
            installed_code,
            written,
        } = self;
        match written.recover_for_retry(installed_code) {
            Ok((prepared, destination)) => Ok(BoundExternalRootPostHandoffWriterInvocation {
                selected_provider,
                lowered,
                installed_code,
                prepared,
                destination,
            }),
            Err(error) => {
                let diagnostic = error.diagnostic().clone();
                let written = (*error).into_written();
                Err(Box::new(WrittenBoundExternalRootWriterRecoveryError {
                    written: WrittenBoundExternalRootPostHandoffWriterDestination {
                        selected_provider,
                        lowered,
                        installed_code,
                        written,
                    },
                    diagnostic,
                }))
            }
        }
    }
}

impl<'installed, 'mapping, 'bytes>
    ValidatedWrittenBoundExternalRootPostHandoffWriterDestination<'installed, 'mapping, 'bytes>
{
    /// Bytes remain unpublished; this is observation after complete replay,
    /// not a publication transition.
    pub fn bytes(&self) -> &[u8] {
        self.written.bytes()
    }

    pub const fn installed_code(&self) -> &'installed executable_installation::InstalledCode {
        self.installed_code
    }

    pub const fn provider_execution(&self) -> external_roots::AdmittedProviderExecution {
        self.written.provider_execution()
    }

    pub const fn selected_entry(&self) -> layout_plans::EntryStubId {
        self.written.selected_entry()
    }

    pub const fn selected_entry_source_slot(&self) -> usize {
        self.written.selected_entry_source_slot()
    }

    pub fn selected_requirement_identity(&self) -> &str {
        self.written.selected_requirement_identity()
    }

    pub fn recover_for_retry(
        self,
    ) -> Result<
        BoundExternalRootPostHandoffWriterInvocation<'installed, 'mapping, 'bytes>,
        Box<WrittenBoundExternalRootWriterRecoveryError<'installed, 'mapping, 'bytes>>,
    > {
        let Self {
            selected_provider,
            lowered,
            installed_code,
            written,
        } = self;
        match written.recover_for_retry() {
            Ok((prepared, destination)) => Ok(BoundExternalRootPostHandoffWriterInvocation {
                selected_provider,
                lowered,
                installed_code,
                prepared,
                destination,
            }),
            Err(error) => {
                let diagnostic = error.diagnostic().clone();
                Err(Box::new(WrittenBoundExternalRootWriterRecoveryError {
                    written: WrittenBoundExternalRootPostHandoffWriterDestination {
                        selected_provider,
                        lowered,
                        installed_code,
                        written: (*error).into_written(),
                    },
                    diagnostic,
                }))
            }
        }
    }

    pub fn into_parts(
        self,
    ) -> (
        SelectedExternalRootProviderPlan,
        program_entry_plan::LoweredPostHandoffWriter,
        &'installed executable_installation::InstalledCode,
        external_roots::ValidatedWrittenExternalRootPostHandoffWriterDestination<'mapping, 'bytes>,
    ) {
        (
            self.selected_provider,
            self.lowered,
            self.installed_code,
            self.written,
        )
    }
}

/// A failed replay of the sealed source-schema/AOT/provider preparation
/// returns that complete preparation without regenerating lowered bytes or
/// provider authority.
#[derive(Debug)]
pub struct ExternalRootPostHandoffWriterBindingError<'installed, 'mapping, 'bytes> {
    preparation: SelectedExternalRootPostHandoffWriterPreparation<'installed, 'mapping, 'bytes>,
    diagnostic: diagnostics::Diagnostic,
}

impl<'installed, 'mapping, 'bytes>
    ExternalRootPostHandoffWriterBindingError<'installed, 'mapping, 'bytes>
{
    pub const fn diagnostic(&self) -> &diagnostics::Diagnostic {
        &self.diagnostic
    }

    pub fn into_preparation(
        self,
    ) -> SelectedExternalRootPostHandoffWriterPreparation<'installed, 'mapping, 'bytes> {
        self.preparation
    }
}

#[derive(Debug)]
pub struct BoundExternalRootWriterExecutionError<'installed, 'mapping, 'bytes> {
    bound: BoundExternalRootPostHandoffWriterInvocation<'installed, 'mapping, 'bytes>,
    diagnostic: layout_plans::MaterializationDiagnostic,
}

impl<'installed, 'mapping, 'bytes>
    BoundExternalRootWriterExecutionError<'installed, 'mapping, 'bytes>
{
    pub const fn diagnostic(&self) -> &layout_plans::MaterializationDiagnostic {
        &self.diagnostic
    }

    pub fn into_bound(
        self,
    ) -> BoundExternalRootPostHandoffWriterInvocation<'installed, 'mapping, 'bytes> {
        self.bound
    }
}

impl<'installed, 'mapping, 'bytes>
    BoundExternalRootPostHandoffWriterInvocation<'installed, 'mapping, 'bytes>
{
    fn validate_execution(&self) -> Result<(), layout_plans::MaterializationDiagnostic> {
        program_entry_plan::validate_lowered_post_handoff_writer(&self.lowered)
            .map_err(|diagnostic| layout_plans::MaterializationDiagnostic(diagnostic.message))?;
        if self.prepared.architecture() != self.lowered.fragment().target().architecture {
            return Err(layout_plans::MaterializationDiagnostic(
                "bound external-root writer architecture no longer matches its provider preparation"
                    .into(),
            ));
        }
        if self.prepared.invocation() != self.lowered.invocation()
            || self
                .prepared
                .context()
                .normalized_fragment_report_fingerprint()
                != self.lowered.fragment().normalized_plan_report_fingerprint()
            || !self
                .prepared
                .context()
                .binds_invocation(self.lowered.invocation())
        {
            return Err(layout_plans::MaterializationDiagnostic(
                "bound external-root writer preparation no longer binds its exact lowered invocation"
                    .into(),
            ));
        }
        self.prepared
            .context()
            .validate_for_destination(
                self.installed_code,
                self.destination.site(),
                self.destination.len(),
            )
            .map_err(|diagnostic| layout_plans::MaterializationDiagnostic(diagnostic.0))?;
        validate_selected_provider_writer_source(&self.selected_provider, &self.prepared)
    }

    pub const fn lowered(&self) -> &program_entry_plan::LoweredPostHandoffWriter {
        &self.lowered
    }

    pub const fn installed_code(&self) -> &'installed executable_installation::InstalledCode {
        self.installed_code
    }

    pub const fn selected_provider(&self) -> &SelectedExternalRootProviderPlan {
        &self.selected_provider
    }

    pub const fn prepared(
        &self,
    ) -> &external_roots::PreparedExternalRootPostHandoffWriterInvocation {
        &self.prepared
    }

    pub const fn destination(
        &self,
    ) -> &executable_installation::ValidatedPreparedPostHandoffWriterDestination<'mapping, 'bytes>
    {
        &self.destination
    }

    /// Consume the exact AOT/preparation/destination join. Success remains
    /// unpublished for the owning consumer's semantic validation and
    /// publication transition.
    pub fn execute(
        self,
    ) -> Result<
        WrittenBoundExternalRootPostHandoffWriterDestination<'installed, 'mapping, 'bytes>,
        Box<BoundExternalRootWriterExecutionError<'installed, 'mapping, 'bytes>>,
    > {
        if let Err(diagnostic) = self.validate_execution() {
            return Err(Box::new(BoundExternalRootWriterExecutionError {
                bound: self,
                diagnostic,
            }));
        }
        let Self {
            selected_provider,
            lowered,
            installed_code,
            prepared,
            destination,
        } = self;
        match prepared.execute(installed_code, destination) {
            Ok(written) => Ok(WrittenBoundExternalRootPostHandoffWriterDestination {
                selected_provider,
                lowered,
                installed_code,
                written,
            }),
            Err(prepared_error) => {
                let diagnostic = prepared_error.diagnostic().clone();
                let (prepared, destination) = (*prepared_error).into_parts();
                Err(Box::new(BoundExternalRootWriterExecutionError {
                    bound: BoundExternalRootPostHandoffWriterInvocation {
                        selected_provider,
                        lowered,
                        installed_code,
                        prepared,
                        destination,
                    },
                    diagnostic,
                }))
            }
        }
    }
}

fn validate_selected_provider_writer_source(
    selected_provider: &SelectedExternalRootProviderPlan,
    prepared: &external_roots::PreparedExternalRootPostHandoffWriterInvocation,
) -> Result<(), layout_plans::MaterializationDiagnostic> {
    validate_selected_provider_source(
        selected_provider,
        prepared.selected_requirement_identity(),
        prepared.selected_boundary_parameter_count(),
        prepared.selected_boundary_contract_report_fingerprint(),
        prepared.selected_boundary_contract_commitment(),
        prepared.selected_entry_claims(),
        prepared
            .provider_execution()
            .provider_plan_report_identity(),
        prepared.provider_plan_digest(),
    )
}

fn validate_selected_provider_written_source(
    selected_provider: &SelectedExternalRootProviderPlan,
    written: &external_roots::WrittenExternalRootPostHandoffWriterDestination<'_, '_>,
) -> Result<(), layout_plans::MaterializationDiagnostic> {
    validate_selected_provider_source(
        selected_provider,
        written.selected_requirement_identity(),
        written.selected_boundary_parameter_count(),
        written.selected_boundary_contract_report_fingerprint(),
        written.selected_boundary_contract_commitment(),
        written.selected_entry_claims(),
        written.provider_execution().provider_plan_report_identity(),
        written.provider_plan_digest(),
    )
}

fn validate_selected_provider_source(
    selected_provider: &SelectedExternalRootProviderPlan,
    requirement_identity: &str,
    boundary_parameter_count: usize,
    boundary_contract_report_fingerprint: u64,
    boundary_contract_commitment: effects::provider_plan::BoundaryCallingPlanCommitment,
    root_entry_claims: &[external_roots::ExternalRootEntryClaim],
    provider_plan: u64,
    provider_plan_digest: effects::provider_plan::ProviderPlanDigest,
) -> Result<(), layout_plans::MaterializationDiagnostic> {
    if !selected_provider.has_valid_exact_identity() {
        return Err(layout_plans::MaterializationDiagnostic(
            "selected external-root provider plan lost its exact strong identity".into(),
        ));
    }
    let matches = selected_provider
        .schema
        .methods
        .iter()
        .filter(|method| method.requirement_identity == requirement_identity)
        .collect::<Vec<_>>();
    let [method] = matches.as_slice() else {
        return Err(layout_plans::MaterializationDiagnostic(format!(
            "selected external-root provider schema retains {} rows for exact writer requirement `{requirement_identity}`",
            matches.len()
        )));
    };
    let selected_entry_claims = selected_provider
        .entry_claims(requirement_identity)
        .map_err(|diagnostic| layout_plans::MaterializationDiagnostic(diagnostic.0))?;
    if selected_provider.identity.normalized_identity() != provider_plan
        || selected_provider.digest != provider_plan_digest
        || selected_provider.schema.trait_name.is_empty()
        || method.name.is_empty()
        || method.requirement_owner.is_empty()
        || method.parameter_count != boundary_parameter_count
        || method.parameter_type_identities.len() != method.parameter_count
        || method.calling_plan_report_fingerprint != Some(boundary_contract_report_fingerprint)
        || method.calling_plan_commitment != Some(boundary_contract_commitment)
        || selected_entry_claims != root_entry_claims
    {
        return Err(layout_plans::MaterializationDiagnostic(
            "selected external-root provider schema does not match the exact prepared writer requirement, boundary, and provider execution"
                .into(),
        ));
    }
    Ok(())
}

/// Independently replay one selected source schema, AOT-lowered writer, and
/// provider preparation before transferring them to executable custody.
pub fn bind_external_root_post_handoff_writer_invocation<'installed, 'mapping, 'bytes>(
    preparation: SelectedExternalRootPostHandoffWriterPreparation<'installed, 'mapping, 'bytes>,
) -> Result<
    BoundExternalRootPostHandoffWriterInvocation<'installed, 'mapping, 'bytes>,
    ExternalRootPostHandoffWriterBindingError<'installed, 'mapping, 'bytes>,
> {
    let selected_provider = preparation.selected_provider();
    let lowered = preparation.lowered();
    let installed_code = preparation.installed_code();
    let prepared = preparation.prepared();
    if let Err(diagnostic) = program_entry_plan::validate_lowered_post_handoff_writer(lowered) {
        return Err(ExternalRootPostHandoffWriterBindingError {
            preparation,
            diagnostic,
        });
    }
    if prepared.architecture() != lowered.fragment().target().architecture
        || prepared.architecture() != installed_code.architecture()
    {
        let diagnostic = diagnostics::Diagnostic::error(format!(
            "external-root post-handoff writer architecture {:?} does not match lowered fragment architecture {:?}",
            prepared.architecture(),
            lowered.fragment().target().architecture
        ));
        return Err(ExternalRootPostHandoffWriterBindingError {
            preparation,
            diagnostic,
        });
    }
    if prepared.invocation() != lowered.invocation()
        || prepared.context().normalized_fragment_report_fingerprint()
            != lowered.fragment().normalized_plan_report_fingerprint()
        || !prepared.context().binds_invocation(lowered.invocation())
    {
        return Err(ExternalRootPostHandoffWriterBindingError {
            preparation,
            diagnostic: diagnostics::Diagnostic::error(
                "external-root provider preparation does not bind the exact lowered post-handoff writer invocation",
            ),
        });
    }
    if let Err(diagnostic) = prepared.context().validate_for_destination(
        installed_code,
        preparation.destination().site(),
        preparation.destination().len(),
    ) {
        return Err(ExternalRootPostHandoffWriterBindingError {
            preparation,
            diagnostic: diagnostics::Diagnostic::error(diagnostic.0),
        });
    }
    if let Err(diagnostic) = validate_selected_provider_writer_source(selected_provider, prepared) {
        return Err(ExternalRootPostHandoffWriterBindingError {
            preparation,
            diagnostic: diagnostics::Diagnostic::error(diagnostic.0),
        });
    }
    let SelectedExternalRootPostHandoffWriterPreparation {
        selected_provider,
        lowered,
        installed_code,
        prepared,
        destination,
    } = preparation;
    Ok(BoundExternalRootPostHandoffWriterInvocation {
        selected_provider,
        lowered,
        installed_code,
        prepared,
        destination,
    })
}

/// Static bridge from one selected external-root `accepts` row to the exact
/// checked entry fact that the runtime occurrence must discharge.
///
/// This is Omega-owned realization state: Psi checks the body under its
/// declared parameter precondition, while installation and entry lowering bind
/// that precondition to provider/invocation evidence without mutating
/// `CheckedTrees` or manufacturing a second semantic fact.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SelectedExternalRootEntryFactBinding {
    provider_plan: external_roots::ProviderPlanId,
    requirement_identity: String,
    parameter_index: usize,
    domain: String,
    effective_carry: language_semantics::CarryPolicy,
    implementation_machine: symbols::SymbolHandle,
    implementation_state: symbols::SymbolHandle,
    parameter_symbol: symbols::SymbolHandle,
    domain_symbol: symbols::SymbolHandle,
    checked_fact: facts::FactHandle,
}

/// Sealed join between one live installed-root occurrence, the exact checked
/// parameter fact it introduces, and the generated prologue fragment that
/// captures that semantic parameter from its normalized ABI placement.
///
/// This carrier borrows the live occurrence and the derived storage plan. It
/// cannot detach a reusable qualification receipt from the linear
/// acknowledgement.
#[derive(Debug)]
pub struct AdmittedExternalRootEntryFactHandoff<'entry, 'storage> {
    occurrence: &'entry external_roots::AdmittedEntryQualification,
    implementation_machine: symbols::SymbolHandle,
    implementation_state: symbols::SymbolHandle,
    checked_fact: facts::FactHandle,
    parameter_symbol: symbols::SymbolHandle,
    storage: &'storage program_entry_plan::DerivedBoundaryEntryParameterStorage,
}

impl<'entry, 'storage> AdmittedExternalRootEntryFactHandoff<'entry, 'storage> {
    pub const fn occurrence(&self) -> &'entry external_roots::AdmittedEntryQualification {
        self.occurrence
    }

    pub const fn implementation_machine(&self) -> symbols::SymbolHandle {
        self.implementation_machine
    }

    pub const fn implementation_state(&self) -> symbols::SymbolHandle {
        self.implementation_state
    }

    pub const fn checked_fact(&self) -> facts::FactHandle {
        self.checked_fact
    }

    pub const fn parameter_symbol(&self) -> symbols::SymbolHandle {
        self.parameter_symbol
    }

    pub const fn storage(
        &self,
    ) -> &'storage program_entry_plan::DerivedBoundaryEntryParameterStorage {
        self.storage
    }
}

impl SelectedExternalRootEntryFactBinding {
    pub const fn provider_plan(&self) -> external_roots::ProviderPlanId {
        self.provider_plan
    }

    pub fn requirement_identity(&self) -> &str {
        &self.requirement_identity
    }

    pub const fn parameter_index(&self) -> usize {
        self.parameter_index
    }

    pub fn domain(&self) -> &str {
        &self.domain
    }

    pub const fn effective_carry(&self) -> language_semantics::CarryPolicy {
        self.effective_carry
    }

    pub const fn implementation_machine(&self) -> symbols::SymbolHandle {
        self.implementation_machine
    }

    pub const fn implementation_state(&self) -> symbols::SymbolHandle {
        self.implementation_state
    }

    pub const fn parameter_symbol(&self) -> symbols::SymbolHandle {
        self.parameter_symbol
    }

    pub const fn domain_symbol(&self) -> symbols::SymbolHandle {
        self.domain_symbol
    }

    pub const fn checked_fact(&self) -> facts::FactHandle {
        self.checked_fact
    }

    /// Match concrete runtime occurrence evidence without interpreting source
    /// names or accepting a selected-plan receipt by itself.
    pub fn matches_occurrence(
        &self,
        occurrence: &external_roots::AdmittedEntryQualification,
    ) -> bool {
        occurrence.matches_contract(
            self.provider_plan,
            &self.requirement_identity,
            self.parameter_index,
            &self.domain,
            self.effective_carry,
        )
    }

    /// Resolve this checked parameter fact only through the linear runtime
    /// acknowledgement minted for the installed-root occurrence. The returned
    /// evidence remains borrowed from that carrier and cannot be detached or
    /// replayed as an independently cloneable receipt. Its semantic parameter
    /// index is still present beside the exact placement selected by the
    /// installed root's validated boundary plan, so ABI lowering never has to
    /// rediscover the admitted subject by name or register.
    fn admit_acknowledgement<'entry>(
        &self,
        acknowledgement: &'entry external_roots::InterruptAcknowledgement,
    ) -> Result<
        &'entry external_roots::AdmittedEntryQualification,
        external_roots::ExternalRootDiagnostic,
    > {
        acknowledgement.qualification_for_contract(
            self.provider_plan,
            &self.requirement_identity,
            self.parameter_index,
            &self.domain,
            self.effective_carry,
        )
    }

    /// Join the live admitted occurrence to the exact generated prologue
    /// capture before the checked adapter body may rely on its propagated
    /// parameter fact.
    fn admit_acknowledgement_handoff<'entry, 'storage, Write>(
        &self,
        acknowledgement: &'entry external_roots::InterruptAcknowledgement,
        storage: &'storage program_entry_plan::DerivedBoundaryEntryStorage<Write>,
    ) -> Result<
        AdmittedExternalRootEntryFactHandoff<'entry, 'storage>,
        external_roots::ExternalRootDiagnostic,
    > {
        let occurrence = self.admit_acknowledgement(acknowledgement)?;
        let parameter = storage.parameter(self.parameter_index).ok_or_else(|| {
            external_roots::ExternalRootDiagnostic(format!(
                "generated entry prologue has no capture for admitted semantic parameter {}",
                self.parameter_index
            ))
        })?;
        if !occurrence.matches_parameter_placement(self.parameter_index, &parameter.placement) {
            return Err(external_roots::ExternalRootDiagnostic(format!(
                "generated entry prologue placement for semantic parameter {} does not match the live admitted occurrence",
                self.parameter_index
            )));
        }
        Ok(AdmittedExternalRootEntryFactHandoff {
            occurrence,
            implementation_machine: self.implementation_machine,
            implementation_state: self.implementation_state,
            checked_fact: self.checked_fact,
            parameter_symbol: self.parameter_symbol,
            storage: parameter,
        })
    }

    /// Dispatch the exact checked provider body only after joining its
    /// propagated parameter fact to the live installed-root occurrence and
    /// generated entry-prologue capture.
    ///
    /// The executor is deliberately invoked inside this operation rather than
    /// receiving a detachable admission result. A failed occurrence or ABI
    /// placement match returns before `execute` runs, while a successful call
    /// receives the non-constructible borrowed handoff naming the exact checked
    /// machine, state, fact, parameter, and prologue write range.
    pub fn dispatch_checked_adapter_body<'entry, 'storage, Write, Output>(
        &self,
        acknowledgement: &'entry external_roots::InterruptAcknowledgement,
        storage: &'storage program_entry_plan::DerivedBoundaryEntryStorage<Write>,
        execute: impl FnOnce(AdmittedExternalRootEntryFactHandoff<'entry, 'storage>) -> Output,
    ) -> Result<Output, external_roots::ExternalRootDiagnostic> {
        let handoff = self.admit_acknowledgement_handoff(acknowledgement, storage)?;
        Ok(execute(handoff))
    }
}

impl SelectedExternalRootProviderPlan {
    /// Construct a retained selection from one exact normalized provider plan.
    /// Compact and strong identities are always derived together; callers
    /// cannot pair either identity with a different schema.
    pub fn from_exact_plan(
        exact_plan: ProviderPlan,
    ) -> Result<Self, external_roots::ExternalRootDiagnostic> {
        Ok(Self {
            identity: external_roots::ProviderPlanId::from_normalized_identity(
                exact_plan.report_fingerprint(),
            )?,
            digest: exact_plan.identity_digest(),
            schema: exact_plan.schema.clone(),
            exact_plan,
        })
    }

    pub const fn exact_plan(&self) -> &ProviderPlan {
        &self.exact_plan
    }

    fn has_valid_exact_identity(&self) -> bool {
        self.identity.normalized_identity() == self.exact_plan.report_fingerprint()
            && self.digest == self.exact_plan.identity_digest()
            && self.schema == self.exact_plan.schema
    }

    /// Join this retained compiler selection to one admitted provider
    /// execution and its exact post-handoff entry-writer invocation.
    ///
    /// The selected-plan fingerprint remains identity rather than authority;
    /// the sealed execution and installed-code resolver provide the authority.
    /// Exact AOT bytes, architecture, writer invocation, the installed resolver
    /// itself, and a validated activated destination are sealed before that
    /// resolver may observe any symbolic source. Matching the schema here also
    /// prevents closure substitution during preparation.
    pub fn prepare_post_handoff_entry_writer<'installed, 'mapping, 'bytes>(
        self,
        lowered: program_entry_plan::LoweredPostHandoffWriter,
        execution: &external_roots::ProviderExecution,
        installed_code: &'installed executable_installation::InstalledCode,
        writer: &layout_plans::PostHandoffWriterPlan,
        destination: executable_installation::PreparedPostHandoffWriterDestination<
            'mapping,
            'bytes,
        >,
    ) -> Result<
        SelectedExternalRootPostHandoffWriterPreparation<'installed, 'mapping, 'bytes>,
        SelectedExternalRootWriterPreparationError<'mapping, 'bytes>,
    > {
        if !self.has_valid_exact_identity()
            || self.identity != execution.provider_plan()
            || self.digest != execution.provider_plan_digest()
        {
            return Err(SelectedExternalRootWriterPreparationError {
                selected_provider: self,
                lowered,
                destination,
                diagnostic: external_roots::ExternalRootDiagnostic(
                    "post-handoff writer selected provider plan does not match the admitted provider execution"
                        .into(),
                ),
            });
        }
        if let Err(diagnostic) = program_entry_plan::validate_lowered_post_handoff_writer(&lowered)
        {
            return Err(SelectedExternalRootWriterPreparationError {
                selected_provider: self,
                lowered,
                destination,
                diagnostic: external_roots::ExternalRootDiagnostic(diagnostic.message),
            });
        }
        if lowered.fragment().target().architecture != installed_code.architecture() {
            return Err(SelectedExternalRootWriterPreparationError {
                selected_provider: self,
                lowered,
                destination,
                diagnostic: external_roots::ExternalRootDiagnostic(
                    "post-handoff writer lowered architecture does not match the exact installed artifact"
                        .into(),
                ),
            });
        }
        let replayed_invocation = match writer.lower_reusable_fragment() {
            Ok(invocation) => invocation,
            Err(diagnostic) => {
                return Err(SelectedExternalRootWriterPreparationError {
                    selected_provider: self,
                    lowered,
                    destination,
                    diagnostic: external_roots::ExternalRootDiagnostic(diagnostic.0),
                });
            }
        };
        if lowered.invocation() != &replayed_invocation {
            return Err(SelectedExternalRootWriterPreparationError {
                selected_provider: self,
                lowered,
                destination,
                diagnostic: external_roots::ExternalRootDiagnostic(
                    "AOT-lowered post-handoff writer does not match the exact provider writer plan"
                        .into(),
                ),
            });
        }
        if let Err(diagnostic) = validate_selected_provider_source(
            &self,
            execution.selected_requirement_identity(),
            execution.selected_boundary_parameter_count(),
            execution.selected_boundary_contract_report_fingerprint(),
            execution.selected_boundary_contract_commitment(),
            execution.selected_entry_claims(),
            execution.provider_plan().normalized_identity(),
            execution.provider_plan_digest(),
        ) {
            return Err(SelectedExternalRootWriterPreparationError {
                selected_provider: self,
                lowered,
                destination,
                diagnostic: external_roots::ExternalRootDiagnostic(diagnostic.0),
            });
        }
        let destination = match destination.into_validated_for_writer_preparation() {
            Ok(destination) => destination,
            Err(error) => {
                let diagnostic =
                    external_roots::ExternalRootDiagnostic(error.diagnostic().0.clone());
                return Err(SelectedExternalRootWriterPreparationError {
                    selected_provider: self,
                    lowered,
                    destination: (*error).into_destination(),
                    diagnostic,
                });
            }
        };
        let destination_len = destination.len();
        let destination_site = destination.site();
        match execution.prepare_post_handoff_entry_writer(
            self.identity,
            installed_code,
            writer,
            destination_len,
            destination_site,
        ) {
            Ok(prepared) => Ok(SelectedExternalRootPostHandoffWriterPreparation {
                selected_provider: self,
                lowered,
                installed_code,
                prepared,
                destination,
            }),
            Err(diagnostic) => Err(SelectedExternalRootWriterPreparationError {
                selected_provider: self,
                lowered,
                destination: destination.into_destination(),
                diagnostic,
            }),
        }
    }

    /// Lower the compiler-owned accepted claims for one exact requirement into
    /// the provider-neutral runtime-ledger representation. External-root
    /// construction must use this retained selection rather than restating
    /// domains or carry policy from source display text.
    pub fn entry_claims(
        &self,
        requirement_identity: &str,
    ) -> Result<Vec<external_roots::ExternalRootEntryClaim>, external_roots::ExternalRootDiagnostic>
    {
        if !self.has_valid_exact_identity() {
            return Err(external_roots::ExternalRootDiagnostic(
                "selected external-root provider plan lost its exact strong identity".into(),
            ));
        }
        let matches = self
            .schema
            .methods
            .iter()
            .filter(|method| method.requirement_identity == requirement_identity)
            .collect::<Vec<_>>();
        let [method] = matches.as_slice() else {
            return Err(external_roots::ExternalRootDiagnostic(
                match matches.len() {
                    0 => format!(
                        "selected external-root provider plan has no requirement `{requirement_identity}`"
                    ),
                    count => format!(
                        "selected external-root provider plan has {count} copies of requirement `{requirement_identity}`"
                    ),
                },
            ));
        };
        let mut claims = Vec::with_capacity(method.entry_claims.len());
        for claim in &method.entry_claims {
            if claim.predicate_body.is_present() {
                return Err(external_roots::ExternalRootDiagnostic(format!(
                    "external-root entry claim `{}` carries predicate obligations and requires a specialized installation handoff",
                    claim.domain
                )));
            }
            claims.push(external_roots::ExternalRootEntryClaim {
                parameter_index: claim.parameter_index,
                domain: claim.domain.clone(),
                effective_carry: claim.effective_carry,
            });
        }
        claims.sort_by(|left, right| {
            left.parameter_index
                .cmp(&right.parameter_index)
                .then_with(|| left.domain.cmp(&right.domain))
        });
        Ok(claims)
    }

    /// Lower routed result qualifications for one exact selected requirement
    /// into the runtime receipt contract used by interrupt-mask transitions.
    pub fn result_claims(
        &self,
        requirement_identity: &str,
    ) -> Result<Vec<external_roots::ExternalRootResultClaim>, external_roots::ExternalRootDiagnostic>
    {
        if !self.has_valid_exact_identity() {
            return Err(external_roots::ExternalRootDiagnostic(
                "selected external-root provider plan lost its exact strong identity".into(),
            ));
        }
        let matches = self
            .schema
            .methods
            .iter()
            .filter(|method| method.requirement_identity == requirement_identity)
            .collect::<Vec<_>>();
        let [method] = matches.as_slice() else {
            return Err(external_roots::ExternalRootDiagnostic(
                match matches.len() {
                    0 => format!(
                        "selected external-root provider plan has no requirement `{requirement_identity}`"
                    ),
                    count => format!(
                        "selected external-root provider plan has {count} copies of requirement `{requirement_identity}`"
                    ),
                },
            ));
        };
        let mut claims = method
            .result_claims
            .iter()
            .map(|claim| external_roots::ExternalRootResultClaim {
                provider_plan: self.identity,
                provider_plan_digest: self.digest,
                requirement_identity: requirement_identity.to_owned(),
                domain: claim.domain.clone(),
                effective_carry: claim.effective_carry,
            })
            .collect::<Vec<_>>();
        claims.sort_by(|left, right| left.domain.cmp(&right.domain));
        Ok(claims)
    }
}

/// Resolve one external-root boundary slot only from the immutable provider
/// selection retained on the checked program. The returned ID is the exact
/// normalized `ProviderPlan` fingerprint consumed by root validation; source
/// declarations and unselected candidates are no longer in scope here.
pub fn selected_external_root_provider_plan_id(
    selected_provider_plans: &effects::SelectedProviderPlanFacts,
    boundary_trait: &str,
) -> Result<external_roots::ProviderPlanId, external_roots::ExternalRootDiagnostic> {
    selected_external_root_provider_plan(selected_provider_plans, boundary_trait)
        .map(|selected| selected.identity)
}

/// Resolve one external-root boundary slot to the exact retained provider
/// identity and normalized source schema. Root-installation artifacts can
/// therefore report the authority-bearing inputs bound by the receipt chain
/// without re-reading source or trusting display names.
pub fn selected_external_root_provider_plan(
    selected_provider_plans: &effects::SelectedProviderPlanFacts,
    boundary_trait: &str,
) -> Result<SelectedExternalRootProviderPlan, external_roots::ExternalRootDiagnostic> {
    let matches = selected_provider_plans
        .plans()
        .iter()
        .filter(|plan| same_semantic_name(&plan.schema.trait_name, boundary_trait))
        .collect::<Vec<_>>();
    let [plan] = matches.as_slice() else {
        return Err(external_roots::ExternalRootDiagnostic(
            match matches.len() {
                0 => format!(
                    "external-root boundary slot `{boundary_trait}` has no retained selected provider plan"
                ),
                count => format!(
                    "external-root boundary slot `{boundary_trait}` matches {count} retained selected provider plans"
                ),
            },
        ));
    };
    SelectedExternalRootProviderPlan::from_exact_plan((*plan).clone())
}

/// Resolve an external-root provider selection when the current artifact has
/// one. An absent plan remains an honest pending installation dependency;
/// multiple matching selections are still invalid.
pub fn optional_selected_external_root_provider_plan(
    selected_provider_plans: &effects::SelectedProviderPlanFacts,
    boundary_trait: &str,
) -> Result<Option<SelectedExternalRootProviderPlan>, external_roots::ExternalRootDiagnostic> {
    let matches = selected_provider_plans
        .plans()
        .iter()
        .filter(|plan| same_semantic_name(&plan.schema.trait_name, boundary_trait))
        .collect::<Vec<_>>();
    match matches.as_slice() {
        [] => Ok(None),
        [plan] => Ok(Some(SelectedExternalRootProviderPlan::from_exact_plan(
            (*plan).clone(),
        )?)),
        plans => Err(external_roots::ExternalRootDiagnostic(format!(
            "external-root boundary slot `{boundary_trait}` matches {} retained selected provider plans",
            plans.len()
        ))),
    }
}

/// Resolve every routed entry claim on one selected external root onto the
/// exact checked source-parameter fact consumed by its checked adapter.
pub fn selected_external_root_entry_fact_bindings(
    checked: &checked_trees::CheckedTrees,
    selected_provider_plans: &effects::SelectedProviderPlanFacts,
    boundary_trait: &str,
) -> Result<Vec<SelectedExternalRootEntryFactBinding>, external_roots::ExternalRootDiagnostic> {
    use effects::provider_plan::ProviderBinding;
    use facts::{FactOrigin, FactPayload, FactPlace, PlaceRoot, ProgramPoint};

    let matches = selected_provider_plans
        .plans()
        .iter()
        .filter(|plan| same_semantic_name(&plan.schema.trait_name, boundary_trait))
        .collect::<Vec<_>>();
    let [plan] = matches.as_slice() else {
        return Err(external_roots::ExternalRootDiagnostic(
            match matches.len() {
                0 => format!(
                    "external-root boundary slot `{boundary_trait}` has no retained selected provider plan"
                ),
                count => format!(
                    "external-root boundary slot `{boundary_trait}` matches {count} retained selected provider plans"
                ),
            },
        ));
    };
    let provider_plan =
        external_roots::ProviderPlanId::from_normalized_identity(plan.report_fingerprint())?;
    let mut bindings = Vec::new();

    for method in &plan.schema.methods {
        for claim in &method.entry_claims {
            if claim.predicate_body.is_present() {
                return Err(external_roots::ExternalRootDiagnostic(format!(
                    "selected external-root claim `{}` carries predicate obligations and requires a specialized installation handoff",
                    claim.domain
                )));
            }
            let rows = plan
                .rows
                .iter()
                .filter(|row| plan.schema.row_binds_method(row, method))
                .collect::<Vec<_>>();
            let [row] = rows.as_slice() else {
                return Err(external_roots::ExternalRootDiagnostic(format!(
                    "selected external-root plan `{}` binds routed requirement `{}::{}` {} times",
                    plan.name,
                    method.requirement_owner,
                    method.name,
                    rows.len()
                )));
            };
            let ProviderBinding::CheckedAdapter {
                machine_identity, ..
            } = &row.binding
            else {
                return Err(external_roots::ExternalRootDiagnostic(format!(
                    "selected external-root routed requirement `{}::{}` has no checked adapter fact to bind",
                    method.requirement_owner, method.name
                )));
            };
            let implementation = exact_checked_adapter(&checked.typed, plan, row)
                .map_err(|diagnostic| external_roots::ExternalRootDiagnostic(diagnostic.message))?;
            let state = checked
                .typed
                .machine_states(implementation)
                .first()
                .ok_or_else(|| {
                    external_roots::ExternalRootDiagnostic(format!(
                        "selected external-root checked adapter `{machine_identity}` has no entry state"
                    ))
                })?;
            let parameters = checked
                .typed
                .state_parameters(state)
                .iter()
                .filter(|parameter| !parameter.is_self)
                .collect::<Vec<_>>();
            let parameter = parameters.get(claim.parameter_index).ok_or_else(|| {
                external_roots::ExternalRootDiagnostic(format!(
                    "selected external-root claim parameter {} is absent from checked adapter `{machine_identity}`",
                    claim.parameter_index
                ))
            })?;
            let domain = checked
                .typed
                .domain_definitions()
                .iter()
                .find(|domain| domain.name.as_str() == claim.domain)
                .ok_or_else(|| {
                    external_roots::ExternalRootDiagnostic(format!(
                        "selected external-root claim domain `{}` is absent from checked semantics",
                        claim.domain
                    ))
                })?;
            let facts = checked
                .facts
                .semantic
                .facts
                .iter()
                .filter(|(_, fact)| {
                    fact.point
                        == ProgramPoint::State {
                            machine_symbol: implementation.symbol,
                            state_symbol: state.symbol,
                        }
                        && fact.origin
                            == FactOrigin::StateParameterDomain {
                                machine_symbol: implementation.symbol,
                                state_symbol: state.symbol,
                            }
                        && fact.evidence.origin
                            == language_semantics::QualificationEvidenceOrigin::Propagated
                        && matches!(
                            fact.payload,
                            FactPayload::DomainMembership { domain_symbol, .. }
                                if domain_symbol == domain.symbol
                        )
                        && matches!(fact.place, FactPlace::Place(place)
                        if {
                            let place = checked.facts.semantic.places.get(place);
                            place.root == PlaceRoot::Symbol(parameter.symbol)
                                && place.segments.is_empty()
                        })
                })
                .map(|(handle, _)| handle)
                .collect::<Vec<_>>();
            let [checked_fact] = facts.as_slice() else {
                return Err(external_roots::ExternalRootDiagnostic(format!(
                    "selected external-root claim `{}` parameter {} maps to {} checked entry facts",
                    claim.domain,
                    claim.parameter_index,
                    facts.len()
                )));
            };
            bindings.push(SelectedExternalRootEntryFactBinding {
                provider_plan,
                requirement_identity: method.requirement_identity.clone(),
                parameter_index: claim.parameter_index,
                domain: claim.domain.clone(),
                effective_carry: claim.effective_carry,
                implementation_machine: implementation.symbol,
                implementation_state: state.symbol,
                parameter_symbol: parameter.symbol,
                domain_symbol: domain.symbol,
                checked_fact: *checked_fact,
            });
        }
    }
    Ok(bindings)
}
