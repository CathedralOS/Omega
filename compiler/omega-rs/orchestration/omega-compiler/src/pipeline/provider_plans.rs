//! Provider plans derive from checked `satisfies` closures and are admitted
//! through the chapter-10 trust path. Own-package plans remain dev-active with
//! a standing warning until the final build grants them; lockfile receipts hash
//! normalized plan identity so a changed plan drifts. A unique covering
//! candidate may still supply the declaration-era default, while explicit
//! selection remains under slot-owner authority.

use omega_effects::provider_plan::{ProviderBinding, ProviderPlan, ProviderPlanRow, ServiceSchema};
use psi_typed_trees::TypedTrees;

/// Exact selected provider-plan input consumed by external-root construction.
///
/// The schema is retained beside the normalized plan identity so installation
/// can bind the source callable shape—including domain/carry-qualified
/// parameter types—to the same provider selection later carried by
/// `ProviderExecution` and per-invocation entry receipts.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SelectedExternalRootProviderPlan {
    pub identity: omega_external_roots::ProviderPlanId,
    pub schema: ServiceSchema,
}

/// One exact selected source schema, AOT-lowered writer, installed resolver,
/// and activated unpublished destination sealed to the provider-populated
/// preparation they admitted. All four precede source resolution.
#[derive(Debug)]
pub struct SelectedExternalRootPostHandoffWriterPreparation<'installed, 'mapping, 'bytes> {
    selected_provider: SelectedExternalRootProviderPlan,
    lowered: omega_instruction_selection::LoweredPostHandoffWriter,
    installed_code: &'installed omega_executable_installation::InstalledCode,
    prepared: omega_external_roots::PreparedExternalRootPostHandoffWriterInvocation,
    destination:
        omega_executable_installation::PreparedPostHandoffWriterDestination<'mapping, 'bytes>,
}

impl<'installed, 'mapping, 'bytes>
    SelectedExternalRootPostHandoffWriterPreparation<'installed, 'mapping, 'bytes>
{
    pub const fn selected_provider(&self) -> &SelectedExternalRootProviderPlan {
        &self.selected_provider
    }

    pub const fn prepared(
        &self,
    ) -> &omega_external_roots::PreparedExternalRootPostHandoffWriterInvocation {
        &self.prepared
    }

    pub const fn lowered(&self) -> &omega_instruction_selection::LoweredPostHandoffWriter {
        &self.lowered
    }

    pub const fn installed_code(&self) -> &'installed omega_executable_installation::InstalledCode {
        self.installed_code
    }

    pub const fn destination(
        &self,
    ) -> &omega_executable_installation::PreparedPostHandoffWriterDestination<'mapping, 'bytes>
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
    lowered: omega_instruction_selection::LoweredPostHandoffWriter,
    destination:
        omega_executable_installation::PreparedPostHandoffWriterDestination<'mapping, 'bytes>,
    diagnostic: omega_external_roots::ExternalRootDiagnostic,
}

impl<'mapping, 'bytes> SelectedExternalRootWriterPreparationError<'mapping, 'bytes> {
    pub const fn diagnostic(&self) -> &omega_external_roots::ExternalRootDiagnostic {
        &self.diagnostic
    }

    pub fn into_parts(
        self,
    ) -> (
        SelectedExternalRootProviderPlan,
        omega_instruction_selection::LoweredPostHandoffWriter,
        omega_executable_installation::PreparedPostHandoffWriterDestination<'mapping, 'bytes>,
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
    lowered: omega_instruction_selection::LoweredPostHandoffWriter,
    installed_code: &'installed omega_executable_installation::InstalledCode,
    prepared: omega_external_roots::PreparedExternalRootPostHandoffWriterInvocation,
    destination:
        omega_executable_installation::PreparedPostHandoffWriterDestination<'mapping, 'bytes>,
}

/// Still-unpublished destination after one exact bound external-root writer
/// executes. The AOT-lowered fragment remains attached to the installation
/// context and installed resolver instead of being reduced to copied
/// fingerprints at this boundary. This carrier establishes neither consumer
/// semantics nor publication.
#[derive(Debug)]
pub struct WrittenBoundExternalRootPostHandoffWriterDestination<'installed, 'mapping, 'bytes> {
    selected_provider: SelectedExternalRootProviderPlan,
    lowered: omega_instruction_selection::LoweredPostHandoffWriter,
    installed_code: &'installed omega_executable_installation::InstalledCode,
    written:
        omega_external_roots::WrittenExternalRootPostHandoffWriterDestination<'mapping, 'bytes>,
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
    lowered: omega_instruction_selection::LoweredPostHandoffWriter,
    installed_code: &'installed omega_executable_installation::InstalledCode,
    written: omega_external_roots::ValidatedWrittenExternalRootPostHandoffWriterDestination<
        'mapping,
        'bytes,
    >,
}

/// Outward-consumer rejection returns the complete written carrier unchanged,
/// so validation can be retried against the correct installed realization.
#[derive(Debug)]
pub struct WrittenBoundExternalRootConsumerValidationError<'installed, 'mapping, 'bytes> {
    written: WrittenBoundExternalRootPostHandoffWriterDestination<'installed, 'mapping, 'bytes>,
    diagnostic: psi_layout_plans::MaterializationDiagnostic,
}

impl<'installed, 'mapping, 'bytes>
    WrittenBoundExternalRootConsumerValidationError<'installed, 'mapping, 'bytes>
{
    pub const fn diagnostic(&self) -> &psi_layout_plans::MaterializationDiagnostic {
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
    diagnostic: psi_layout_plans::MaterializationDiagnostic,
}

impl<'installed, 'mapping, 'bytes>
    WrittenBoundExternalRootWriterRecoveryError<'installed, 'mapping, 'bytes>
{
    pub const fn diagnostic(&self) -> &psi_layout_plans::MaterializationDiagnostic {
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

    pub const fn lowered(&self) -> &omega_instruction_selection::LoweredPostHandoffWriter {
        &self.lowered
    }

    pub const fn installed_code(&self) -> &'installed omega_executable_installation::InstalledCode {
        self.installed_code
    }

    /// Independently replay the retained AOT bytes, footprint, invocation,
    /// opaque installation context, and exact borrowed installed realization.
    /// Rejection only borrows this carrier, preserving every input for retry.
    pub fn validate_for_consumer(&self) -> Result<(), psi_layout_plans::MaterializationDiagnostic> {
        omega_instruction_selection::validate_lowered_post_handoff_writer(&self.lowered).map_err(
            |diagnostic| psi_layout_plans::MaterializationDiagnostic(diagnostic.message),
        )?;
        self.written.validate_for_consumer(self.installed_code)?;
        if self.lowered.fragment().target().architecture != self.installed_code.architecture()
            || self.written.invocation() != self.lowered.invocation()
        {
            return Err(psi_layout_plans::MaterializationDiagnostic(
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
        installed_code: &omega_executable_installation::InstalledCode,
    ) -> Result<
        ValidatedWrittenBoundExternalRootPostHandoffWriterDestination<'installed, 'mapping, 'bytes>,
        Box<WrittenBoundExternalRootConsumerValidationError<'installed, 'mapping, 'bytes>>,
    > {
        let diagnostic = if !std::ptr::eq(self.installed_code, installed_code) {
            Some(psi_layout_plans::MaterializationDiagnostic(
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

    pub const fn installed_code(&self) -> &'installed omega_executable_installation::InstalledCode {
        self.installed_code
    }

    pub const fn provider_execution(
        &self,
    ) -> omega_external_roots::AdmittedTerminalProviderExecution {
        self.written.provider_execution()
    }

    pub const fn selected_entry(&self) -> psi_layout_plans::EntryStubId {
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
        omega_instruction_selection::LoweredPostHandoffWriter,
        &'installed omega_executable_installation::InstalledCode,
        omega_external_roots::ValidatedWrittenExternalRootPostHandoffWriterDestination<
            'mapping,
            'bytes,
        >,
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
    diagnostic: psi_diagnostics::Diagnostic,
}

impl<'installed, 'mapping, 'bytes>
    ExternalRootPostHandoffWriterBindingError<'installed, 'mapping, 'bytes>
{
    pub const fn diagnostic(&self) -> &psi_diagnostics::Diagnostic {
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
    diagnostic: psi_layout_plans::MaterializationDiagnostic,
}

impl<'installed, 'mapping, 'bytes>
    BoundExternalRootWriterExecutionError<'installed, 'mapping, 'bytes>
{
    pub const fn diagnostic(&self) -> &psi_layout_plans::MaterializationDiagnostic {
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
    fn validate_execution(&self) -> Result<(), psi_layout_plans::MaterializationDiagnostic> {
        omega_instruction_selection::validate_lowered_post_handoff_writer(&self.lowered).map_err(
            |diagnostic| psi_layout_plans::MaterializationDiagnostic(diagnostic.message),
        )?;
        if self.prepared.architecture() != self.lowered.fragment().target().architecture {
            return Err(psi_layout_plans::MaterializationDiagnostic(
                "bound external-root writer architecture no longer matches its provider preparation"
                    .into(),
            ));
        }
        if self.prepared.invocation() != self.lowered.invocation()
            || self.prepared.context().normalized_fragment_fingerprint()
                != self.lowered.fragment().normalized_plan_fingerprint()
            || !self
                .prepared
                .context()
                .binds_invocation(self.lowered.invocation())
        {
            return Err(psi_layout_plans::MaterializationDiagnostic(
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
            .map_err(|diagnostic| psi_layout_plans::MaterializationDiagnostic(diagnostic.0))?;
        validate_selected_provider_writer_source(&self.selected_provider, &self.prepared)
    }

    pub const fn lowered(&self) -> &omega_instruction_selection::LoweredPostHandoffWriter {
        &self.lowered
    }

    pub const fn installed_code(&self) -> &'installed omega_executable_installation::InstalledCode {
        self.installed_code
    }

    pub const fn selected_provider(&self) -> &SelectedExternalRootProviderPlan {
        &self.selected_provider
    }

    pub const fn prepared(
        &self,
    ) -> &omega_external_roots::PreparedExternalRootPostHandoffWriterInvocation {
        &self.prepared
    }

    pub const fn destination(
        &self,
    ) -> &omega_executable_installation::PreparedPostHandoffWriterDestination<'mapping, 'bytes>
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
    prepared: &omega_external_roots::PreparedExternalRootPostHandoffWriterInvocation,
) -> Result<(), psi_layout_plans::MaterializationDiagnostic> {
    validate_selected_provider_source(
        selected_provider,
        prepared.selected_requirement_identity(),
        prepared.selected_boundary_parameter_count(),
        prepared.selected_boundary_contract_fingerprint(),
        prepared.selected_entry_claims(),
        prepared.provider_execution().provider_plan(),
    )
}

fn validate_selected_provider_written_source(
    selected_provider: &SelectedExternalRootProviderPlan,
    written: &omega_external_roots::WrittenExternalRootPostHandoffWriterDestination<'_, '_>,
) -> Result<(), psi_layout_plans::MaterializationDiagnostic> {
    validate_selected_provider_source(
        selected_provider,
        written.selected_requirement_identity(),
        written.selected_boundary_parameter_count(),
        written.selected_boundary_contract_fingerprint(),
        written.selected_entry_claims(),
        written.provider_execution().provider_plan(),
    )
}

fn validate_selected_provider_source(
    selected_provider: &SelectedExternalRootProviderPlan,
    requirement_identity: &str,
    boundary_parameter_count: usize,
    boundary_contract_fingerprint: u64,
    root_entry_claims: &[omega_external_roots::ExternalRootEntryClaim],
    provider_plan: u64,
) -> Result<(), psi_layout_plans::MaterializationDiagnostic> {
    let matches = selected_provider
        .schema
        .methods
        .iter()
        .filter(|method| method.requirement_identity == requirement_identity)
        .collect::<Vec<_>>();
    let [method] = matches.as_slice() else {
        return Err(psi_layout_plans::MaterializationDiagnostic(format!(
            "selected external-root provider schema retains {} rows for exact writer requirement `{requirement_identity}`",
            matches.len()
        )));
    };
    let selected_entry_claims = selected_provider
        .entry_claims(requirement_identity)
        .map_err(|diagnostic| psi_layout_plans::MaterializationDiagnostic(diagnostic.0))?;
    if selected_provider.identity.normalized_identity() != provider_plan
        || selected_provider.schema.trait_name.is_empty()
        || method.name.is_empty()
        || method.requirement_owner.is_empty()
        || method.parameter_count != boundary_parameter_count
        || method.parameter_type_identities.len() != method.parameter_count
        || method.calling_plan_fingerprint != Some(boundary_contract_fingerprint)
        || selected_entry_claims != root_entry_claims
    {
        return Err(psi_layout_plans::MaterializationDiagnostic(
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
    if let Err(diagnostic) =
        omega_instruction_selection::validate_lowered_post_handoff_writer(lowered)
    {
        return Err(ExternalRootPostHandoffWriterBindingError {
            preparation,
            diagnostic,
        });
    }
    if prepared.architecture() != lowered.fragment().target().architecture
        || prepared.architecture() != installed_code.architecture()
    {
        let diagnostic = psi_diagnostics::Diagnostic::error(format!(
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
        || prepared.context().normalized_fragment_fingerprint()
            != lowered.fragment().normalized_plan_fingerprint()
        || !prepared.context().binds_invocation(lowered.invocation())
    {
        return Err(ExternalRootPostHandoffWriterBindingError {
            preparation,
            diagnostic: psi_diagnostics::Diagnostic::error(
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
            diagnostic: psi_diagnostics::Diagnostic::error(diagnostic.0),
        });
    }
    if let Err(diagnostic) = validate_selected_provider_writer_source(selected_provider, prepared) {
        return Err(ExternalRootPostHandoffWriterBindingError {
            preparation,
            diagnostic: psi_diagnostics::Diagnostic::error(diagnostic.0),
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
    provider_plan: omega_external_roots::ProviderPlanId,
    requirement_identity: String,
    parameter_index: usize,
    domain: String,
    effective_carry: psi_language_semantics::CarryPolicy,
    implementation_machine: psi_symbols::SymbolHandle,
    implementation_state: psi_symbols::SymbolHandle,
    parameter_symbol: psi_symbols::SymbolHandle,
    domain_symbol: psi_symbols::SymbolHandle,
    checked_fact: psi_facts::FactHandle,
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
    occurrence: &'entry omega_external_roots::AdmittedEntryQualification,
    implementation_machine: psi_symbols::SymbolHandle,
    implementation_state: psi_symbols::SymbolHandle,
    checked_fact: psi_facts::FactHandle,
    parameter_symbol: psi_symbols::SymbolHandle,
    storage: &'storage omega_instruction_selection::DerivedBoundaryEntryParameterStorage,
}

impl<'entry, 'storage> AdmittedExternalRootEntryFactHandoff<'entry, 'storage> {
    pub const fn occurrence(&self) -> &'entry omega_external_roots::AdmittedEntryQualification {
        self.occurrence
    }

    pub const fn implementation_machine(&self) -> psi_symbols::SymbolHandle {
        self.implementation_machine
    }

    pub const fn implementation_state(&self) -> psi_symbols::SymbolHandle {
        self.implementation_state
    }

    pub const fn checked_fact(&self) -> psi_facts::FactHandle {
        self.checked_fact
    }

    pub const fn parameter_symbol(&self) -> psi_symbols::SymbolHandle {
        self.parameter_symbol
    }

    pub const fn storage(
        &self,
    ) -> &'storage omega_instruction_selection::DerivedBoundaryEntryParameterStorage {
        self.storage
    }
}

impl SelectedExternalRootEntryFactBinding {
    pub const fn provider_plan(&self) -> omega_external_roots::ProviderPlanId {
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

    pub const fn effective_carry(&self) -> psi_language_semantics::CarryPolicy {
        self.effective_carry
    }

    pub const fn implementation_machine(&self) -> psi_symbols::SymbolHandle {
        self.implementation_machine
    }

    pub const fn implementation_state(&self) -> psi_symbols::SymbolHandle {
        self.implementation_state
    }

    pub const fn parameter_symbol(&self) -> psi_symbols::SymbolHandle {
        self.parameter_symbol
    }

    pub const fn domain_symbol(&self) -> psi_symbols::SymbolHandle {
        self.domain_symbol
    }

    pub const fn checked_fact(&self) -> psi_facts::FactHandle {
        self.checked_fact
    }

    /// Match concrete runtime occurrence evidence without interpreting source
    /// names or accepting a selected-plan receipt by itself.
    pub fn matches_occurrence(
        &self,
        occurrence: &omega_external_roots::AdmittedEntryQualification,
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
        acknowledgement: &'entry omega_external_roots::InterruptAcknowledgement,
    ) -> Result<
        &'entry omega_external_roots::AdmittedEntryQualification,
        omega_external_roots::ExternalRootDiagnostic,
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
    fn admit_acknowledgement_handoff<'entry, 'storage>(
        &self,
        acknowledgement: &'entry omega_external_roots::InterruptAcknowledgement,
        storage: &'storage omega_instruction_selection::DerivedBoundaryEntryStorage,
    ) -> Result<
        AdmittedExternalRootEntryFactHandoff<'entry, 'storage>,
        omega_external_roots::ExternalRootDiagnostic,
    > {
        let occurrence = self.admit_acknowledgement(acknowledgement)?;
        let parameter = storage.parameter(self.parameter_index).ok_or_else(|| {
            omega_external_roots::ExternalRootDiagnostic(format!(
                "generated entry prologue has no capture for admitted semantic parameter {}",
                self.parameter_index
            ))
        })?;
        if !occurrence.matches_parameter_placement(self.parameter_index, &parameter.placement) {
            return Err(omega_external_roots::ExternalRootDiagnostic(format!(
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
    pub fn dispatch_checked_adapter_body<'entry, 'storage, Output>(
        &self,
        acknowledgement: &'entry omega_external_roots::InterruptAcknowledgement,
        storage: &'storage omega_instruction_selection::DerivedBoundaryEntryStorage,
        execute: impl FnOnce(AdmittedExternalRootEntryFactHandoff<'entry, 'storage>) -> Output,
    ) -> Result<Output, omega_external_roots::ExternalRootDiagnostic> {
        let handoff = self.admit_acknowledgement_handoff(acknowledgement, storage)?;
        Ok(execute(handoff))
    }
}

impl SelectedExternalRootProviderPlan {
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
        lowered: omega_instruction_selection::LoweredPostHandoffWriter,
        execution: &omega_external_roots::ProviderExecution,
        installed_code: &'installed omega_executable_installation::InstalledCode,
        writer: &psi_layout_plans::PostHandoffWriterPlan,
        destination: omega_executable_installation::PreparedPostHandoffWriterDestination<
            'mapping,
            'bytes,
        >,
    ) -> Result<
        SelectedExternalRootPostHandoffWriterPreparation<'installed, 'mapping, 'bytes>,
        SelectedExternalRootWriterPreparationError<'mapping, 'bytes>,
    > {
        if self.identity != execution.provider_plan() {
            return Err(SelectedExternalRootWriterPreparationError {
                selected_provider: self,
                lowered,
                destination,
                diagnostic: omega_external_roots::ExternalRootDiagnostic(
                    "post-handoff writer selected provider plan does not match the admitted provider execution"
                        .into(),
                ),
            });
        }
        if let Err(diagnostic) =
            omega_instruction_selection::validate_lowered_post_handoff_writer(&lowered)
        {
            return Err(SelectedExternalRootWriterPreparationError {
                selected_provider: self,
                lowered,
                destination,
                diagnostic: omega_external_roots::ExternalRootDiagnostic(diagnostic.message),
            });
        }
        if lowered.fragment().target().architecture != installed_code.architecture() {
            return Err(SelectedExternalRootWriterPreparationError {
                selected_provider: self,
                lowered,
                destination,
                diagnostic: omega_external_roots::ExternalRootDiagnostic(
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
                    diagnostic: omega_external_roots::ExternalRootDiagnostic(diagnostic.0),
                });
            }
        };
        if lowered.invocation() != &replayed_invocation {
            return Err(SelectedExternalRootWriterPreparationError {
                selected_provider: self,
                lowered,
                destination,
                diagnostic: omega_external_roots::ExternalRootDiagnostic(
                    "AOT-lowered post-handoff writer does not match the exact provider writer plan"
                        .into(),
                ),
            });
        }
        if let Err(diagnostic) = validate_selected_provider_source(
            &self,
            execution.selected_requirement_identity(),
            execution.selected_boundary_parameter_count(),
            execution.selected_boundary_contract_fingerprint(),
            execution.selected_entry_claims(),
            execution.provider_plan().normalized_identity(),
        ) {
            return Err(SelectedExternalRootWriterPreparationError {
                selected_provider: self,
                lowered,
                destination,
                diagnostic: omega_external_roots::ExternalRootDiagnostic(diagnostic.0),
            });
        }
        if let Err(diagnostic) = destination.validate_for_writer_preparation() {
            return Err(SelectedExternalRootWriterPreparationError {
                selected_provider: self,
                lowered,
                destination,
                diagnostic: omega_external_roots::ExternalRootDiagnostic(diagnostic.0),
            });
        }
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
                destination,
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
    ) -> Result<
        Vec<omega_external_roots::ExternalRootEntryClaim>,
        omega_external_roots::ExternalRootDiagnostic,
    > {
        let matches = self
            .schema
            .methods
            .iter()
            .filter(|method| method.requirement_identity == requirement_identity)
            .collect::<Vec<_>>();
        let [method] = matches.as_slice() else {
            return Err(omega_external_roots::ExternalRootDiagnostic(
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
                return Err(omega_external_roots::ExternalRootDiagnostic(format!(
                    "external-root entry claim `{}` carries predicate obligations and requires a specialized installation handoff",
                    claim.domain
                )));
            }
            claims.push(omega_external_roots::ExternalRootEntryClaim {
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
    ) -> Result<
        Vec<omega_external_roots::ExternalRootResultClaim>,
        omega_external_roots::ExternalRootDiagnostic,
    > {
        let matches = self
            .schema
            .methods
            .iter()
            .filter(|method| method.requirement_identity == requirement_identity)
            .collect::<Vec<_>>();
        let [method] = matches.as_slice() else {
            return Err(omega_external_roots::ExternalRootDiagnostic(
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
            .map(|claim| omega_external_roots::ExternalRootResultClaim {
                provider_plan: self.identity,
                requirement_identity: requirement_identity.to_owned(),
                domain: claim.domain.clone(),
                effective_carry: claim.effective_carry,
            })
            .collect::<Vec<_>>();
        claims.sort_by(|left, right| left.domain.cmp(&right.domain));
        Ok(claims)
    }
}

/// Build the exact Omega-owned selection sidecar and bind its stable receipt
/// identities into checked semantic evidence. Provider execution and
/// compiler-generated helper machines consume the returned carrier; neither
/// may reconstruct a plan by scanning authored `satisfies` rows.
pub(crate) fn bind_selected_provider_plan_facts(
    checked: &mut psi_checked_trees::CheckedTrees,
    candidates: &[ProviderPlan],
    facts: omega_effects::SelectedProviderPlanFacts,
    root_grants: &[String],
) -> Result<omega_effects::SelectedProviderPlanFacts, Vec<psi_diagnostics::Diagnostic>> {
    let provider_grants = resolve_selected_provider_grants(candidates, &facts, root_grants)
        .map_err(|diagnostic| vec![diagnostic])?;
    let mut granted_plan_identities = Vec::new();
    for grant in &provider_grants {
        if !granted_plan_identities.contains(&grant.selected_plan_identity) {
            granted_plan_identities.push(grant.selected_plan_identity);
        }
    }
    let granted_plans = granted_plan_identities
        .iter()
        .filter_map(|identity| facts.plan_by_identity(*identity))
        .collect::<Vec<_>>();
    let mut receipt_updates = Vec::new();
    let mut receipt_diagnostics = Vec::new();
    let traits = checked.typed.traits();
    if traits.len() != checked.typed.roots.traits.count() as usize {
        receipt_diagnostics.push(psi_diagnostics::Diagnostic::error(
            "admitted qualification receipt binding has an invalid typed trait span",
        ));
    }
    for definition in traits {
        if checked.typed.trait_machine_signatures(definition).len()
            != definition.machines.count() as usize
        {
            receipt_diagnostics.push(psi_diagnostics::Diagnostic::error(format!(
                "admitted qualification receipt binding has an invalid typed signature span for trait {:?}",
                definition.symbol,
            )));
        }
    }
    if !receipt_diagnostics.is_empty() {
        return Err(receipt_diagnostics);
    }
    for (handle, fact) in checked.facts.semantic.facts.iter().filter(|(_, fact)| {
        fact.evidence.origin == psi_language_semantics::QualificationEvidenceOrigin::AdmittedReceipt
            && fact.evidence.receipt_identity == 0
    }) {
        let owners = checked
            .typed
            .traits()
            .iter()
            .filter(|definition| definition.symbol == fact.evidence.source_symbol)
            .collect::<Vec<_>>();
        let owner = match owners.as_slice() {
            [owner] if owner.is_boundary => *owner,
            [owner] => {
                receipt_diagnostics.push(psi_diagnostics::Diagnostic::error(format!(
                    "admitted qualification evidence source {:?} names non-boundary trait `{}`",
                    fact.evidence.source_symbol, owner.name,
                )));
                continue;
            }
            _ => {
                receipt_diagnostics.push(psi_diagnostics::Diagnostic::error(format!(
                    "admitted qualification evidence source {:?} resolves to {} exact typed boundary requirement owners",
                    fact.evidence.source_symbol,
                    owners.len(),
                )));
                continue;
            }
        };
        let requirement_owners = checked
            .typed
            .traits()
            .iter()
            .flat_map(|candidate_owner| {
                checked
                    .typed
                    .trait_machine_signatures(candidate_owner)
                    .iter()
                    .filter(move |requirement| {
                        requirement.symbol == fact.evidence.requirement_symbol
                    })
                    .map(move |requirement| (candidate_owner, requirement))
            })
            .collect::<Vec<_>>();
        let requirement = match requirement_owners.as_slice() {
            [(requirement_owner, requirement)] if requirement_owner.symbol == owner.symbol => {
                *requirement
            }
            [(requirement_owner, _)] => {
                receipt_diagnostics.push(psi_diagnostics::Diagnostic::error(format!(
                    "admitted qualification evidence requirement {:?} belongs to exact trait {:?}, not owner {:?}",
                    fact.evidence.requirement_symbol,
                    requirement_owner.symbol,
                    owner.symbol,
                )));
                continue;
            }
            _ => {
                receipt_diagnostics.push(psi_diagnostics::Diagnostic::error(format!(
                    "admitted qualification evidence requirement {:?} resolves to {} exact typed signatures",
                    fact.evidence.requirement_symbol,
                    requirement_owners.len(),
                )));
                continue;
            }
        };
        let requirement_identity = checked
            .typed
            .normalized_trait_requirement_overload_identity(owner, requirement)
            .identity();
        let matches = granted_plans
            .iter()
            .filter(|plan| {
                plan.schema
                    .methods
                    .iter()
                    .any(|method| method.requirement_identity == requirement_identity)
            })
            .collect::<Vec<_>>();
        match matches.as_slice() {
            [] => {}
            [plan] => receipt_updates.push((handle, plan.identity_fingerprint())),
            _ => receipt_diagnostics.push(psi_diagnostics::Diagnostic::error(format!(
                "admitted qualification requirement `{requirement_identity}` matches {} granted selected provider plans",
                matches.len()
            ))),
        }
    }
    if !receipt_diagnostics.is_empty() {
        return Err(receipt_diagnostics);
    }
    retain_selected_operator_provider_evidence(checked, candidates, &facts)?;
    for (fact, identity) in receipt_updates {
        checked
            .facts
            .semantic
            .facts
            .get_mut(fact)
            .evidence
            .receipt_identity = identity;
    }
    Ok(facts)
}

fn retain_selected_operator_provider_evidence(
    checked: &mut psi_checked_trees::CheckedTrees,
    candidates: &[ProviderPlan],
    selected: &omega_effects::SelectedProviderPlanFacts,
) -> Result<(), Vec<psi_diagnostics::Diagnostic>> {
    // Validate selected operator plans independently of use-site discovery.
    // A malformed realization is invalid policy even when dead code happens
    // not to mention its requirement, and later annotation may consume only
    // plans that passed this gate.
    let mut diagnostics = Vec::new();
    for plan in selected.plans() {
        let operator = checked.typed.operators().iter().find(|operator| {
            operator.is_boundary
                && psi_typed_trees::operator::boundary_operator_requirement_identity(
                    &checked.typed,
                    operator,
                ) == plan.schema.trait_name
        });
        let Some(operator) = operator else {
            continue;
        };
        if let Err(diagnostic) =
            selected_operator_provider_identity(checked, candidates, selected, operator.symbol)
        {
            diagnostics.push(diagnostic);
        }
    }
    if !diagnostics.is_empty() {
        return Err(diagnostics);
    }

    let spelled = checked
        .facts
        .operators
        .uses
        .iter()
        .map(|(handle, operator_use)| (handle, operator_use.selected_operator_symbol))
        .collect::<Vec<_>>();
    let named = checked
        .facts
        .operators
        .named_uses
        .iter()
        .map(|(handle, operator_use)| (handle, operator_use.selected_operator_symbol))
        .collect::<Vec<_>>();
    for (handle, symbol) in spelled {
        match selected_operator_provider_identity(checked, candidates, selected, symbol) {
            Ok(Some(identity)) => {
                checked
                    .facts
                    .operators
                    .uses
                    .get_mut(handle)
                    .provider_plan_identity = identity;
            }
            Ok(None) => {}
            Err(diagnostic) => diagnostics.push(diagnostic),
        }
    }
    for (handle, symbol) in named {
        match selected_operator_provider_identity(checked, candidates, selected, symbol) {
            Ok(Some(identity)) => {
                checked
                    .facts
                    .operators
                    .named_uses
                    .get_mut(handle)
                    .provider_plan_identity = identity;
            }
            Ok(None) => {}
            Err(diagnostic) => diagnostics.push(diagnostic),
        }
    }

    if diagnostics.is_empty() {
        Ok(())
    } else {
        Err(diagnostics)
    }
}

fn selected_operator_provider_identity(
    checked: &psi_checked_trees::CheckedTrees,
    candidates: &[ProviderPlan],
    selected: &omega_effects::SelectedProviderPlanFacts,
    operator_symbol: psi_symbols::SymbolHandle,
) -> Result<Option<u64>, psi_diagnostics::Diagnostic> {
    let Some(operator) = checked
        .typed
        .operators()
        .iter()
        .find(|operator| operator.symbol == operator_symbol)
    else {
        return Ok(None);
    };
    let slot =
        psi_typed_trees::operator::boundary_operator_requirement_identity(&checked.typed, operator);
    if !candidates
        .iter()
        .any(|candidate| candidate.schema.trait_name == slot)
    {
        return Ok(None);
    }
    let Some(plan) = selected
        .plans()
        .iter()
        .find(|plan| plan.schema.trait_name == slot)
    else {
        return Err(psi_diagnostics::Diagnostic::error(format!(
            "boundary operator `{slot}` has provider candidates but no exact selected ProviderPlan realization for this target"
        )));
    };
    let [row] = plan.rows.as_slice() else {
        return Err(psi_diagnostics::Diagnostic::error(format!(
            "selected boundary-operator ProviderPlan `{}` must contain exactly one realization row",
            plan.name,
        )));
    };
    if let ProviderBinding::CheckedAdapter { machine } = &row.binding {
        let [namespace, requirement] = checked.typed.operator_path_members(operator.name) else {
            return Err(psi_diagnostics::Diagnostic::error(format!(
                "selected checked boundary-operator ProviderPlan `{}` targets `{slot}`, whose source path is not the supported `Namespace::requirement` shape",
                plan.name,
            )));
        };
        let checked_provider = checked
            .typed
            .machines()
            .iter()
            .find(|candidate| candidate.name.as_str() == machine)
            .filter(|candidate| {
                checked
                    .typed
                    .machine_trait_conformances(candidate)
                    .iter()
                    .any(|conformance| {
                        conformance.external_binding.is_none()
                            && psi_typed_trees::operator::resolve_satisfied_checked_operator(
                                &checked.typed,
                                candidate,
                                namespace.as_str(),
                                requirement.as_str(),
                            )
                            .is_some_and(|resolved| resolved.symbol == operator.symbol)
                    })
            });
        if checked_provider.is_none() {
            return Err(psi_diagnostics::Diagnostic::error(format!(
                "selected boundary-operator ProviderPlan `{}` binds checked adapter `{machine}`, but that machine does not satisfy exact slot `{slot}` with a checked body",
                plan.name,
            )));
        }
        return Ok(Some(plan.identity_fingerprint()));
    }
    let ProviderBinding::CompilerIntrinsic { machine, .. } = &row.binding else {
        return Err(psi_diagnostics::Diagnostic::error(format!(
            "selected boundary-operator ProviderPlan `{}` uses unsupported binding `{:?}`; boundary operators require a checked adapter or compiler intrinsic",
            plan.name, row.binding,
        )));
    };
    compiler_intrinsic_diagnostic_label(&checked.typed, operator).ok_or_else(|| {
        psi_diagnostics::Diagnostic::error(format!(
            "selected boundary-operator ProviderPlan `{}` targets `{slot}`, which has no compiler-known migrated intrinsic",
            plan.name,
        ))
    })?;
    if !intrinsic_realization_matches_operator(&checked.typed, machine, operator) {
        return Err(psi_diagnostics::Diagnostic::error(format!(
            "selected boundary-operator ProviderPlan `{}` binds realization `{machine}`, but it does not satisfy exact slot `{slot}` as an external leaf",
            plan.name,
        )));
    }
    Ok(Some(plan.identity_fingerprint()))
}

pub(super) fn intrinsic_realization_matches_operator(
    typed: &TypedTrees,
    realization_machine_identity: &str,
    operator: &psi_typed_trees::operator::OperatorDefinition,
) -> bool {
    let [namespace, requirement] = typed.operator_path_members(operator.name) else {
        return false;
    };
    typed.machines().iter().any(|machine| {
        typed
            .normalized_machine_overload_identity(machine)
            .is_some_and(|identity| identity.identity() == realization_machine_identity)
            && typed
                .machine_trait_conformances(machine)
                .iter()
                .any(|conformance| conformance.external_binding.is_some())
            && psi_typed_trees::operator::resolve_satisfied_boundary_operator(
                typed,
                machine,
                namespace.as_str(),
                requirement.as_str(),
            )
            .is_some_and(|resolved| resolved.symbol == operator.symbol)
    })
}

/// Render the compiler-known float realization selected by an exact checked
/// operator. This label is diagnostic-only: provider identity and dispatch use
/// the normalized realization-machine symbol retained in `ProviderBinding`.
pub fn compiler_intrinsic_diagnostic_label(
    typed: &TypedTrees,
    operator: &psi_typed_trees::operator::OperatorDefinition,
) -> Option<String> {
    let path = typed.operator_path_members(operator.name);
    let [namespace, requirement] = path else {
        return None;
    };
    let parameters = typed.operator_parameters(operator);
    let (operation, primitive, expected_result) = match namespace.as_str() {
        "Float" => {
            let operation = match requirement.as_str() {
                "add" | "subtract" | "multiply" | "divide" | "equal" | "not_equal" | "less"
                | "less_or_equal" | "greater" | "greater_or_equal" => requirement.as_str(),
                _ => return None,
            };
            let [left, right] = parameters else {
                return None;
            };
            let primitive = typed.primitive_type_reference(left.type_reference)?;
            if typed.primitive_type_reference(right.type_reference) != Some(primitive) {
                return None;
            }
            let expected_result = if matches!(operation, "add" | "subtract" | "multiply" | "divide")
            {
                primitive
            } else {
                psi_typed_trees::types::PrimitiveType::Bool
            };
            (operation, primitive, expected_result)
        }
        "F32" | "F64" => {
            if matches!(requirement.as_str(), "from_f64" | "from_f32") {
                let (expected_source, expected_result, source_name) =
                    match (namespace.as_str(), requirement.as_str()) {
                        ("F32", "from_f64") => (
                            psi_typed_trees::types::PrimitiveType::F64,
                            psi_typed_trees::types::PrimitiveType::F32,
                            "f64",
                        ),
                        ("F64", "from_f32") => (
                            psi_typed_trees::types::PrimitiveType::F32,
                            psi_typed_trees::types::PrimitiveType::F64,
                            "f32",
                        ),
                        _ => return None,
                    };
                let [value] = parameters else {
                    return None;
                };
                if typed.primitive_type_reference(value.type_reference) != Some(expected_source)
                    || typed.primitive_type_reference(operator.return_type) != Some(expected_result)
                {
                    return None;
                }
                return Some(format!(
                    "{}::{}.{source_name}",
                    namespace.as_str(),
                    requirement.as_str()
                ));
            }
            if let Some(source_name) = requirement.as_str().strip_prefix("from_") {
                let expected_source = match source_name {
                    "i8" => psi_typed_trees::types::PrimitiveType::I8,
                    "i16" => psi_typed_trees::types::PrimitiveType::I16,
                    "i32" => psi_typed_trees::types::PrimitiveType::I32,
                    "i64" => psi_typed_trees::types::PrimitiveType::I64,
                    "u8" => psi_typed_trees::types::PrimitiveType::U8,
                    "u16" => psi_typed_trees::types::PrimitiveType::U16,
                    "u32" => psi_typed_trees::types::PrimitiveType::U32,
                    "u64" => psi_typed_trees::types::PrimitiveType::U64,
                    _ => return None,
                };
                let expected_result = if namespace.as_str() == "F32" {
                    psi_typed_trees::types::PrimitiveType::F32
                } else {
                    psi_typed_trees::types::PrimitiveType::F64
                };
                let [value] = parameters else {
                    return None;
                };
                if typed.primitive_type_reference(value.type_reference) != Some(expected_source)
                    || typed.primitive_type_reference(operator.return_type) != Some(expected_result)
                {
                    return None;
                }
                return Some(format!(
                    "{}::{}.{source_name}",
                    namespace.as_str(),
                    requirement.as_str()
                ));
            }
            let operation = match requirement.as_str() {
                "minimum" | "maximum" => requirement.as_str(),
                "negate"
                | "square_root"
                | "square_root_toward_zero"
                | "square_root_toward_positive"
                | "square_root_toward_negative"
                | "classify"
                | "is_nan"
                | "is_finite"
                | "is_infinite"
                | "is_normal"
                | "is_subnormal" => requirement.as_str(),
                "multiply_then_add"
                | "fused_multiply_add"
                | "fused_multiply_add_toward_zero"
                | "fused_multiply_add_toward_positive"
                | "fused_multiply_add_toward_negative" => requirement.as_str(),
                "add_toward_zero" | "add_toward_positive" | "add_toward_negative" => {
                    requirement.as_str()
                }
                "subtract_toward_zero"
                | "subtract_toward_positive"
                | "subtract_toward_negative" => requirement.as_str(),
                "multiply_toward_zero"
                | "multiply_toward_positive"
                | "multiply_toward_negative" => requirement.as_str(),
                "divide_toward_zero" | "divide_toward_positive" | "divide_toward_negative" => {
                    requirement.as_str()
                }
                _ => return None,
            };
            let expected_primitive = if namespace.as_str() == "F32" {
                psi_typed_trees::types::PrimitiveType::F32
            } else {
                psi_typed_trees::types::PrimitiveType::F64
            };
            match parameters {
                [value]
                    if matches!(
                        operation,
                        "negate"
                            | "square_root"
                            | "square_root_toward_zero"
                            | "square_root_toward_positive"
                            | "square_root_toward_negative"
                            | "classify"
                            | "is_nan"
                            | "is_finite"
                            | "is_infinite"
                            | "is_normal"
                            | "is_subnormal"
                    ) =>
                {
                    if typed.primitive_type_reference(value.type_reference)
                        != Some(expected_primitive)
                    {
                        return None;
                    }
                }
                [left, right]
                    if matches!(
                        operation,
                        "minimum"
                            | "maximum"
                            | "add_toward_zero"
                            | "add_toward_positive"
                            | "add_toward_negative"
                            | "subtract_toward_zero"
                            | "subtract_toward_positive"
                            | "subtract_toward_negative"
                            | "multiply_toward_zero"
                            | "multiply_toward_positive"
                            | "multiply_toward_negative"
                            | "divide_toward_zero"
                            | "divide_toward_positive"
                            | "divide_toward_negative"
                    ) =>
                {
                    if typed.primitive_type_reference(left.type_reference)
                        != Some(expected_primitive)
                        || typed.primitive_type_reference(right.type_reference)
                            != Some(expected_primitive)
                    {
                        return None;
                    }
                }
                [left, right, addend]
                    if matches!(
                        operation,
                        "multiply_then_add"
                            | "fused_multiply_add"
                            | "fused_multiply_add_toward_zero"
                            | "fused_multiply_add_toward_positive"
                            | "fused_multiply_add_toward_negative"
                    ) =>
                {
                    if typed.primitive_type_reference(left.type_reference)
                        != Some(expected_primitive)
                        || typed.primitive_type_reference(right.type_reference)
                            != Some(expected_primitive)
                        || typed.primitive_type_reference(addend.type_reference)
                            != Some(expected_primitive)
                    {
                        return None;
                    }
                }
                _ => return None,
            }
            if operation == "classify" {
                if typed.display_type_reference(operator.return_type) != "FloatClass" {
                    return None;
                }
                let format = if expected_primitive == psi_typed_trees::types::PrimitiveType::F32 {
                    "f32"
                } else {
                    "f64"
                };
                return Some(format!("{}::classify.{format}", namespace.as_str()));
            }
            let expected_result = if matches!(
                operation,
                "is_nan" | "is_finite" | "is_infinite" | "is_normal" | "is_subnormal"
            ) {
                psi_typed_trees::types::PrimitiveType::Bool
            } else {
                expected_primitive
            };
            (operation, expected_primitive, expected_result)
        }
        "I8" | "I16" | "I32" | "I64" | "U8" | "U16" | "U32" | "U64" => {
            let expected_result = match namespace.as_str() {
                "I8" => psi_typed_trees::types::PrimitiveType::I8,
                "I16" => psi_typed_trees::types::PrimitiveType::I16,
                "I32" => psi_typed_trees::types::PrimitiveType::I32,
                "I64" => psi_typed_trees::types::PrimitiveType::I64,
                "U8" => psi_typed_trees::types::PrimitiveType::U8,
                "U16" => psi_typed_trees::types::PrimitiveType::U16,
                "U32" => psi_typed_trees::types::PrimitiveType::U32,
                "U64" => psi_typed_trees::types::PrimitiveType::U64,
                _ => unreachable!(),
            };
            let (expected_source, source_name) = match requirement.as_str() {
                "from_f32" => (psi_typed_trees::types::PrimitiveType::F32, "f32"),
                "from_f64" => (psi_typed_trees::types::PrimitiveType::F64, "f64"),
                _ => return None,
            };
            let [value] = parameters else {
                return None;
            };
            if typed.primitive_type_reference(value.type_reference) != Some(expected_source)
                || typed.primitive_type_reference(operator.return_type) != Some(expected_result)
            {
                return None;
            }
            let policy = match typed
                .type_reference_table
                .arithmetic_domain(operator.return_type)
            {
                psi_numerics::arithmetic::ArithmeticDomain::Exact => "exact",
                psi_numerics::arithmetic::ArithmeticDomain::Trapping => "trapping",
                psi_numerics::arithmetic::ArithmeticDomain::Saturating => "saturating",
                psi_numerics::arithmetic::ArithmeticDomain::Wrapping => return None,
            };
            return Some(format!(
                "{}::{}.{source_name}.{policy}",
                namespace.as_str(),
                requirement.as_str()
            ));
        }
        _ => return None,
    };
    if typed.primitive_type_reference(operator.return_type) != Some(expected_result) {
        return None;
    }
    let format = match primitive {
        psi_typed_trees::types::PrimitiveType::F32 => "f32",
        psi_typed_trees::types::PrimitiveType::F64 => "f64",
        _ => return None,
    };
    Some(format!("{}::{operation}.{format}", namespace.as_str()))
}

/// Resolve one external-root boundary slot only from the immutable provider
/// selection retained on the checked program. The returned ID is the exact
/// normalized `ProviderPlan` fingerprint consumed by root validation; source
/// declarations and unselected candidates are no longer in scope here.
pub fn selected_external_root_provider_plan_id(
    selected_provider_plans: &omega_effects::SelectedProviderPlanFacts,
    boundary_trait: &str,
) -> Result<omega_external_roots::ProviderPlanId, omega_external_roots::ExternalRootDiagnostic> {
    selected_external_root_provider_plan(selected_provider_plans, boundary_trait)
        .map(|selected| selected.identity)
}

/// Resolve one external-root boundary slot to the exact retained provider
/// identity and normalized source schema. Root-installation artifacts can
/// therefore report the authority-bearing inputs bound by the receipt chain
/// without re-reading source or trusting display names.
pub fn selected_external_root_provider_plan(
    selected_provider_plans: &omega_effects::SelectedProviderPlanFacts,
    boundary_trait: &str,
) -> Result<SelectedExternalRootProviderPlan, omega_external_roots::ExternalRootDiagnostic> {
    let matches = selected_provider_plans
        .plans()
        .iter()
        .filter(|plan| same_semantic_name(&plan.schema.trait_name, boundary_trait))
        .collect::<Vec<_>>();
    let [plan] = matches.as_slice() else {
        return Err(omega_external_roots::ExternalRootDiagnostic(
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
    Ok(SelectedExternalRootProviderPlan {
        identity: omega_external_roots::ProviderPlanId::from_normalized_identity(
            plan.identity_fingerprint(),
        )?,
        schema: plan.schema.clone(),
    })
}

/// Resolve an external-root provider selection when the current artifact has
/// one. An absent plan remains an honest pending installation dependency;
/// multiple matching selections are still invalid.
pub(crate) fn optional_selected_external_root_provider_plan(
    selected_provider_plans: &omega_effects::SelectedProviderPlanFacts,
    boundary_trait: &str,
) -> Result<Option<SelectedExternalRootProviderPlan>, omega_external_roots::ExternalRootDiagnostic>
{
    let matches = selected_provider_plans
        .plans()
        .iter()
        .filter(|plan| same_semantic_name(&plan.schema.trait_name, boundary_trait))
        .collect::<Vec<_>>();
    match matches.as_slice() {
        [] => Ok(None),
        [plan] => Ok(Some(SelectedExternalRootProviderPlan {
            identity: omega_external_roots::ProviderPlanId::from_normalized_identity(
                plan.identity_fingerprint(),
            )?,
            schema: plan.schema.clone(),
        })),
        plans => Err(omega_external_roots::ExternalRootDiagnostic(format!(
            "external-root boundary slot `{boundary_trait}` matches {} retained selected provider plans",
            plans.len()
        ))),
    }
}

/// Resolve every routed entry claim on one selected external root onto the
/// exact checked source-parameter fact consumed by its checked adapter.
pub fn selected_external_root_entry_fact_bindings(
    checked: &psi_checked_trees::CheckedTrees,
    selected_provider_plans: &omega_effects::SelectedProviderPlanFacts,
    boundary_trait: &str,
) -> Result<Vec<SelectedExternalRootEntryFactBinding>, omega_external_roots::ExternalRootDiagnostic>
{
    use omega_effects::provider_plan::ProviderBinding;
    use psi_facts::{FactOrigin, FactPayload, FactPlace, PlaceRoot, ProgramPoint};

    let matches = selected_provider_plans
        .plans()
        .iter()
        .filter(|plan| same_semantic_name(&plan.schema.trait_name, boundary_trait))
        .collect::<Vec<_>>();
    let [plan] = matches.as_slice() else {
        return Err(omega_external_roots::ExternalRootDiagnostic(
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
    let provider_plan = omega_external_roots::ProviderPlanId::from_normalized_identity(
        plan.identity_fingerprint(),
    )?;
    let mut bindings = Vec::new();

    for method in &plan.schema.methods {
        for claim in &method.entry_claims {
            if claim.predicate_body.is_present() {
                return Err(omega_external_roots::ExternalRootDiagnostic(format!(
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
                return Err(omega_external_roots::ExternalRootDiagnostic(format!(
                    "selected external-root plan `{}` binds routed requirement `{}::{}` {} times",
                    plan.name,
                    method.requirement_owner,
                    method.name,
                    rows.len()
                )));
            };
            let ProviderBinding::CheckedAdapter { machine } = &row.binding else {
                return Err(omega_external_roots::ExternalRootDiagnostic(format!(
                    "selected external-root routed requirement `{}::{}` has no checked adapter fact to bind",
                    method.requirement_owner, method.name
                )));
            };
            let implementation = checked
                .typed
                .machines()
                .iter()
                .find(|candidate| candidate.name.as_str() == machine)
                .ok_or_else(|| {
                    omega_external_roots::ExternalRootDiagnostic(format!(
                        "selected external-root checked adapter `{machine}` is absent from checked semantics"
                    ))
                })?;
            let state = checked
                .typed
                .machine_states(implementation)
                .first()
                .ok_or_else(|| {
                    omega_external_roots::ExternalRootDiagnostic(format!(
                        "selected external-root checked adapter `{machine}` has no entry state"
                    ))
                })?;
            let parameters = checked
                .typed
                .state_parameters(state)
                .iter()
                .filter(|parameter| !parameter.is_self)
                .collect::<Vec<_>>();
            let parameter = parameters.get(claim.parameter_index).ok_or_else(|| {
                omega_external_roots::ExternalRootDiagnostic(format!(
                    "selected external-root claim parameter {} is absent from checked adapter `{machine}`",
                    claim.parameter_index
                ))
            })?;
            let domain = checked
                .typed
                .domain_definitions()
                .iter()
                .find(|domain| domain.name.as_str() == claim.domain)
                .ok_or_else(|| {
                    omega_external_roots::ExternalRootDiagnostic(format!(
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
                            == psi_language_semantics::QualificationEvidenceOrigin::Propagated
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
                return Err(omega_external_roots::ExternalRootDiagnostic(format!(
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

/// PRV4 order step (2): derive plans from explicit SATISFIES edges -- one
/// plan per (provider type, boundary trait, target), assembled only from
/// that provider's conformance closure. External leaves and checked adapters
/// attached to the same provider type join one plan. External leaves may be
/// free declarations; checked adapters must belong to a nominal provider type
/// so execution can only dispatch through a retained whole-provider selection.
/// Coverage never combines unrelated provider types. Coverage/signatures come from the typed schema
/// (signature refinement is enforced by the conformance checker on each
/// edge); the effect surface is the union of the SATISFIED requirements'
/// declared effects -- the requirement supplies the ceiling, never the
/// leaf. Selection v1: a slot whose (trait, target) has exactly one FULLY
/// COVERING derived plan selects it implicitly; ambiguity or partial
/// coverage is loud at the consumer (the trust report shows coverage).
pub(crate) fn derive_satisfies_plans(
    typed: &TypedTrees,
    selected_target: Option<&str>,
) -> Vec<ProviderPlan> {
    let mut plans: Vec<ProviderPlan> = Vec::new();
    // Target filtering has already admitted only unscoped and selected-target
    // machines into typed trees. Derive from their exact retained conformance
    // and supply identities; source syntax is no longer a binding authority.
    for machine in typed.machines() {
        for clause in typed.machine_trait_conformances(machine) {
            if clause.requirement.as_ref().is_some_and(|requirement| {
                psi_typed_trees::operator::resolve_satisfied_boundary_operator(
                    typed,
                    machine,
                    clause.name.as_str(),
                    requirement.as_str(),
                )
                .is_some()
            }) {
                // Exact boundary-operator requirements use one overloaded
                // signature per provider slot; derive them below rather than
                // manufacturing an empty boundary-trait schema here.
                continue;
            }
            // A bodyless leaf carries `via`; a CHECKED ADAPTER is an
            // ordinary machine with a body and a requirement-named
            // satisfies edge (no via). Both contribute rows; whole-trait
            // conformances (no requirement) are the trait system's
            // ordinary business and derive nothing here.
            let Some(requirement) = clause.requirement.as_ref() else {
                continue;
            };
            let binding_kind = match (machine.supply_mode, clause.external_binding) {
                (
                    psi_language_semantics::MachineSupplyMode::ExternalRealization {
                        binding: supply_binding,
                        ..
                    },
                    Some(conformance_binding),
                ) if supply_binding == conformance_binding => {
                    let Some(binding) = exact_external_binding_identity(
                        typed,
                        machine,
                        clause.name.as_str(),
                        requirement.as_str(),
                    ) else {
                        continue;
                    };
                    Some(binding)
                }
                (psi_language_semantics::MachineSupplyMode::CheckedBody, None) => {
                    // A CHECKED ADAPTER derives a plan row only over a
                    // BOUNDARY trait (a service schema). A plain trait's
                    // conformance -- including its service-reach ceiling -- is the
                    // existing trait machinery's business (the decision-20
                    // admission fixtures pin it) and derives nothing here.
                    let is_boundary_trait = typed.traits().iter().any(|definition| {
                        definition.is_boundary && definition.symbol == clause.symbol
                    });
                    if !is_boundary_trait {
                        continue;
                    }
                    None
                }
                _ => continue, // refused elsewhere (via rungs)
            };
            let binding = binding_kind;
            let target = selected_target.unwrap_or_default().to_owned();
            let trait_leaf = clause.name.as_str().to_owned();
            let provider_type = machine
                .attached_data
                .as_ref()
                .map(|name| name.as_str().to_owned())
                .unwrap_or_default();
            let row_binding = match binding {
                None => ProviderBinding::CheckedAdapter {
                    machine: machine.name.as_str().to_owned(),
                },
                Some(binding) => external_provider_binding(
                    binding,
                    &provider_type,
                    &realization_machine_identity(typed, machine.name.as_str()),
                ),
            };
            let requirement_identity = satisfied_requirement_identity(
                typed,
                machine.name.as_str(),
                clause.name.as_str(),
                requirement.as_str(),
            );
            let semantic_requirement_identity = exact_satisfied_requirement_identity(
                typed,
                machine.name.as_str(),
                clause.name.as_str(),
                requirement.as_str(),
            );
            for (schema_trait, schema) in provider_plan_schema_targets(
                typed,
                &provider_type,
                &trait_leaf,
                &semantic_requirement_identity,
            ) {
                let plan_name = satisfies_plan_name(&target, &schema_trait, &provider_type);
                let position = plans
                    .iter()
                    .position(|plan| plan.name == plan_name)
                    .unwrap_or_else(|| {
                        plans.push(ProviderPlan {
                            name: plan_name.clone(),
                            provider_type: provider_type.clone(),
                            target: target.clone(),
                            schema,
                            rows: Vec::new(),
                            origin_package: String::new(),
                        });
                        plans.len() - 1
                    });
                plans[position].rows.push(ProviderPlanRow {
                    method: requirement.as_str().to_owned(),
                    requirement_identity: requirement_identity.clone(),
                    binding: row_binding.clone(),
                });
            }
        }
    }
    plans.extend(derive_boundary_operator_plans(typed, selected_target));
    plans
}

/// Select the boundary schema under which an exact inherited routed-input
/// requirement is installed. A provider may implement the stable parent
/// requirement while explicitly conforming to a target root that inherits it
/// and adds `Calling<C>`. In that case the descendant schema owns plan/ABI
/// refinement, but the row keeps the parent's exact requirement identity.
/// Requirements without accepted entry claims retain the established direct
/// provider-plan behavior.
fn provider_plan_schema_targets(
    typed: &TypedTrees,
    provider_type: &str,
    satisfied_trait: &str,
    requirement_identity: &str,
) -> Vec<(String, ServiceSchema)> {
    let direct = typed.traits().iter().find(|definition| {
        definition.is_boundary && same_semantic_name(definition.name.as_str(), satisfied_trait)
    });

    let mut refined = typed
        .conformances()
        .iter()
        .filter(|conformance| {
            conformance
                .carrier_name()
                .is_some_and(|carrier| same_semantic_name(carrier.as_str(), provider_type))
        })
        .filter_map(|conformance| {
            let definition = typed.traits().iter().find(|definition| {
                definition.is_boundary
                    && same_semantic_name(definition.name.as_str(), conformance.trait_name.as_str())
            })?;
            let arguments = provider_boundary_arguments(typed, definition, provider_type);
            let schema = ServiceSchema::from_typed_instance(typed, definition, &arguments)?;
            schema
                .methods
                .iter()
                .any(|method| {
                    method.requirement_identity == requirement_identity
                        && !method.entry_claims.is_empty()
                })
                .then(|| (definition.name.as_str().to_owned(), schema))
        })
        .collect::<Vec<_>>();
    refined.sort_by(|left, right| left.0.cmp(&right.0));
    refined.dedup_by(|left, right| left.0 == right.0);

    let has_descendant = refined.iter().any(|(name, _)| {
        direct.is_some_and(|definition| !same_semantic_name(name, definition.name.as_str()))
    });
    if has_descendant {
        refined.retain(|(name, _)| {
            direct.is_none_or(|definition| !same_semantic_name(name, definition.name.as_str()))
        });
    }
    if !refined.is_empty() {
        return refined;
    }

    direct
        .and_then(|definition| {
            let arguments = provider_boundary_arguments(typed, definition, provider_type);
            ServiceSchema::from_typed_instance(typed, definition, &arguments)
                .map(|schema| (definition.name.as_str().to_owned(), schema))
        })
        .into_iter()
        .collect()
}

fn derive_boundary_operator_plans(
    typed: &TypedTrees,
    selected_target: Option<&str>,
) -> Vec<ProviderPlan> {
    let mut plans = Vec::<ProviderPlan>::new();
    for machine in typed.machines() {
        for clause in typed.machine_trait_conformances(machine) {
            let Some(requirement) = clause.requirement.as_ref() else {
                continue;
            };
            let Some(operator) = psi_typed_trees::operator::resolve_satisfied_boundary_operator(
                typed,
                machine,
                clause.name.as_str(),
                requirement.as_str(),
            ) else {
                continue;
            };
            let binding = match (machine.supply_mode, clause.external_binding) {
                (
                    psi_language_semantics::MachineSupplyMode::ExternalRealization {
                        binding: supply_binding,
                        ..
                    },
                    Some(conformance_binding),
                ) if supply_binding == conformance_binding => {
                    let Some(binding) = exact_external_binding_identity(
                        typed,
                        machine,
                        clause.name.as_str(),
                        requirement.as_str(),
                    ) else {
                        continue;
                    };
                    external_provider_binding(
                        binding,
                        machine
                            .attached_data
                            .as_ref()
                            .map(|name| name.as_str())
                            .unwrap_or_default(),
                        &typed
                            .normalized_machine_overload_identity(machine)
                            .map(|identity| identity.identity())
                            .unwrap_or_default(),
                    )
                }
                (psi_language_semantics::MachineSupplyMode::CheckedBody, None) => {
                    ProviderBinding::CheckedAdapter {
                        machine: machine.name.as_str().to_owned(),
                    }
                }
                _ => continue, // invalid via/body combinations are refused elsewhere
            };
            let Some(schema) = ServiceSchema::from_typed_operator(typed, operator) else {
                continue;
            };
            let target = selected_target.unwrap_or_default().to_owned();
            let provider_type = machine
                .attached_data
                .as_ref()
                .map(|name| name.as_str().to_owned())
                .unwrap_or_default();
            let plan_name = satisfies_plan_name(&target, &schema.trait_name, &provider_type);
            let position = plans
                .iter()
                .position(|plan| plan.name == plan_name)
                .unwrap_or_else(|| {
                    plans.push(ProviderPlan {
                        name: plan_name.clone(),
                        provider_type: provider_type.clone(),
                        target: target.clone(),
                        schema: schema.clone(),
                        rows: Vec::new(),
                        origin_package: String::new(),
                    });
                    plans.len() - 1
                });
            plans[position].rows.push(ProviderPlanRow {
                method: "realize".to_owned(),
                requirement_identity: schema.methods[0].requirement_identity.clone(),
                binding,
            });
        }
    }
    plans
}

pub(crate) fn satisfied_requirement_identity(
    typed: &TypedTrees,
    machine_name: &str,
    trait_name: &str,
    requirement_name: &str,
) -> String {
    let Some(machine) = typed
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == machine_name)
    else {
        return String::new();
    };
    let Some(definition) = typed.traits().iter().find(|definition| {
        definition.name.as_str() == trait_name
            || definition
                .name
                .as_str()
                .rsplit("::")
                .next()
                .is_some_and(|leaf| leaf == trait_name)
    }) else {
        return String::new();
    };
    let named = typed
        .trait_machine_signatures(definition)
        .iter()
        .filter(|signature| signature.name.as_str() == requirement_name)
        .collect::<Vec<_>>();
    let selected = match named.as_slice() {
        [single] => Some(*single),
        many => {
            let implementation_dispatch = typed
                .machine_states(machine)
                .first()
                .map(|entry| typed.normalized_result_dispatch_set(entry.return_type));
            let mut matching = many.iter().copied().filter(|signature| {
                implementation_dispatch.as_ref().is_some_and(|dispatch| {
                    typed.normalized_result_dispatch_set(signature.return_type) == *dispatch
                })
            });
            let selected = matching.next();
            selected.filter(|_| matching.next().is_none())
        }
    };
    selected
        .map(|signature| {
            typed
                .normalized_trait_requirement_overload_identity(definition, signature)
                .identity()
        })
        .unwrap_or_default()
}

fn exact_satisfied_requirement_identity(
    typed: &TypedTrees,
    machine_name: &str,
    trait_name: &str,
    requirement_name: &str,
) -> String {
    let Some(definition) = typed
        .traits()
        .iter()
        .find(|definition| same_semantic_name(definition.name.as_str(), trait_name))
    else {
        return String::new();
    };
    let named = typed
        .trait_machine_signatures(definition)
        .iter()
        .filter(|signature| signature.name.as_str() == requirement_name)
        .collect::<Vec<_>>();
    match named.as_slice() {
        [single] => typed
            .normalized_trait_requirement_overload_identity(definition, single)
            .identity(),
        _ => satisfied_requirement_identity(typed, machine_name, trait_name, requirement_name),
    }
}

fn exact_external_binding_identity<'typed>(
    typed: &'typed TypedTrees,
    machine: &psi_typed_trees::machine::Machine,
    trait_name: &str,
    requirement_name: &str,
) -> Option<&'typed psi_language_semantics::ExternalBindingIdentity> {
    let mut matching = typed
        .machine_trait_conformances(machine)
        .iter()
        .filter(|conformance| same_semantic_name(conformance.name.as_str(), trait_name))
        .filter(|conformance| {
            conformance.requirement.as_ref().map(|name| name.as_str()) == Some(requirement_name)
        })
        .filter_map(|conformance| conformance.external_binding);
    let binding = matching.next()?;
    if matching.next().is_some() {
        return None;
    }
    typed.external_bindings.identity(binding)
}

fn external_provider_binding(
    binding: &psi_language_semantics::ExternalBindingIdentity,
    provider_type: &str,
    intrinsic_machine_identity: &str,
) -> ProviderBinding {
    use psi_language_semantics::ExternalBindingIdentity;

    match binding {
        ExternalBindingIdentity::Syscall { number } => ProviderBinding::Syscall { number: *number },
        ExternalBindingIdentity::Import { library, symbol } => ProviderBinding::Import {
            library: library.clone(),
            symbol: symbol.clone(),
        },
        ExternalBindingIdentity::CompilerIntrinsic => ProviderBinding::CompilerIntrinsic {
            machine: intrinsic_machine_identity.to_owned(),
        },
        ExternalBindingIdentity::VtableSlot { index } => {
            ProviderBinding::VtableSlot { index: *index }
        }
        ExternalBindingIdentity::VtableField { field } => ProviderBinding::VtableField {
            table: provider_type.to_owned(),
            field: field.clone(),
        },
        ExternalBindingIdentity::TableFunction { field } => ProviderBinding::TableFunction {
            table: provider_type.to_owned(),
            field: field.clone(),
        },
    }
}

fn realization_machine_identity(typed: &TypedTrees, machine_name: &str) -> String {
    typed
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == machine_name)
        .and_then(|machine| typed.normalized_machine_overload_identity(machine))
        .map(|identity| identity.identity())
        .unwrap_or_default()
}

fn provider_boundary_arguments(
    typed: &TypedTrees,
    boundary: &psi_typed_trees::trait_definition::TraitDefinition,
    provider_type: &str,
) -> Vec<psi_typed_trees::types::TypeReferenceHandle> {
    typed
        .conformances()
        .iter()
        .find(|conformance| {
            conformance
                .carrier_name()
                .is_some_and(|carrier| same_semantic_name(carrier.as_str(), provider_type))
                && same_semantic_name(conformance.trait_name.as_str(), boundary.name.as_str())
        })
        .map(|conformance| {
            typed
                .type_reference_table
                .type_reference_handles(conformance.arguments)
                .to_vec()
        })
        .unwrap_or_default()
}

fn same_semantic_name(left: &str, right: &str) -> bool {
    left == right
        || (!left.contains("::") && right.rsplit("::").next().is_some_and(|leaf| leaf == left))
        || (!right.contains("::") && left.rsplit("::").next().is_some_and(|leaf| leaf == right))
}

/// The stable name shared by derivation, reports, selection, and backend row
/// extraction. External leaves may use the anonymous form; a real provider
/// type is deliberately visible in artifact identity.
pub(crate) fn satisfies_plan_name(target: &str, trait_name: &str, provider_type: &str) -> String {
    match (target.is_empty(), provider_type.is_empty()) {
        (true, true) => format!("satisfies::{trait_name}"),
        (false, true) => format!("{target}::satisfies::{trait_name}"),
        (true, false) => format!("{provider_type}::satisfies::{trait_name}"),
        (false, false) => format!("{target}::{provider_type}::satisfies::{trait_name}"),
    }
}

fn checked_adapter_has_exact_conformance(
    typed: &TypedTrees,
    adapter: &psi_typed_trees::machine::Machine,
    plan: &omega_effects::provider_plan::ProviderPlan,
    row: &omega_effects::provider_plan::ProviderPlanRow,
) -> bool {
    let operator = typed.operators().iter().find(|operator| {
        operator.is_boundary
            && psi_typed_trees::operator::boundary_operator_requirement_identity(typed, operator)
                == plan.schema.trait_name
    });
    if let Some(operator) = operator {
        let identity =
            psi_typed_trees::operator::boundary_operator_requirement_identity(typed, operator);
        let [namespace, requirement] = typed.operator_path_members(operator.name) else {
            return false;
        };
        return row.method == "realize"
            && row.requirement_identity == identity
            && typed
                .machine_trait_conformances(adapter)
                .iter()
                .any(|conformance| {
                    conformance.external_binding.is_none()
                        && conformance.name.as_str() == namespace.as_str()
                        && conformance.requirement.as_ref().map(|name| name.as_str())
                            == Some(requirement.as_str())
                        && psi_typed_trees::operator::resolve_satisfied_checked_operator(
                            typed,
                            adapter,
                            namespace.as_str(),
                            requirement.as_str(),
                        )
                        .is_some_and(|resolved| resolved.symbol == operator.symbol)
                });
    }

    typed
        .machine_trait_conformances(adapter)
        .iter()
        .filter(|conformance| conformance.external_binding.is_none())
        .filter_map(|conformance| {
            let requirement = conformance.requirement.as_ref()?;
            let definition = typed
                .traits()
                .iter()
                .find(|definition| definition.symbol == conformance.symbol)?;
            Some(satisfied_requirement_identity(
                typed,
                adapter.name.as_str(),
                definition.name.as_str(),
                requirement.as_str(),
            ))
        })
        .any(|identity| identity == row.requirement_identity)
}

fn exact_schema_method_for_row<'plan>(
    plan: &'plan ProviderPlan,
    row: &ProviderPlanRow,
) -> Result<&'plan omega_effects::provider_plan::ServiceMethod, psi_diagnostics::Diagnostic> {
    if row.requirement_identity.is_empty() {
        return Err(psi_diagnostics::Diagnostic::error(format!(
            "ProviderPlan `{}` row `{}` has no exact synchronous-invocation overload identity",
            plan.name, row.method,
        )));
    }
    let methods = plan
        .schema
        .methods
        .iter()
        .filter(|method| plan.schema.row_binds_method(row, method))
        .collect::<Vec<_>>();
    let [method] = methods.as_slice() else {
        return Err(psi_diagnostics::Diagnostic::error(format!(
            "ProviderPlan `{}` row `{}` / `{}` binds {} exact synchronous-invocation schema methods",
            plan.name,
            row.method,
            row.requirement_identity,
            methods.len(),
        )));
    };
    Ok(*method)
}

fn exact_canonical_provider_schema(
    typed: &TypedTrees,
    plan: &ProviderPlan,
) -> Result<ServiceSchema, psi_diagnostics::Diagnostic> {
    if plan.schema.trait_name.is_empty() {
        return Err(psi_diagnostics::Diagnostic::error(format!(
            "ProviderPlan `{}` has no exact canonical typed schema identity",
            plan.name,
        )));
    }

    let trait_matches = typed
        .traits()
        .iter()
        .filter(|definition| {
            definition.is_boundary && definition.name.as_str() == plan.schema.trait_name
        })
        .collect::<Vec<_>>();
    let operator_matches = typed
        .operators()
        .iter()
        .filter(|operator| {
            operator.is_boundary
                && psi_typed_trees::operator::boundary_operator_requirement_identity(
                    typed, operator,
                ) == plan.schema.trait_name
        })
        .collect::<Vec<_>>();

    match (trait_matches.as_slice(), operator_matches.as_slice()) {
        ([definition], []) => {
            let argument_matches = typed
                .conformances()
                .iter()
                .filter(|conformance| {
                    conformance
                        .carrier_name()
                        .is_some_and(|carrier| carrier.as_str() == plan.provider_type)
                        && conformance.trait_name.as_str() == definition.name.as_str()
                })
                .collect::<Vec<_>>();
            let arguments = match argument_matches.as_slice() {
                [] => Vec::new(),
                [conformance] => typed
                    .type_reference_table
                    .type_reference_handles(conformance.arguments)
                    .to_vec(),
                _ => {
                    return Err(psi_diagnostics::Diagnostic::error(format!(
                        "ProviderPlan `{}` provider `{}` resolves to {} exact carrier argument rows for canonical typed schema `{}`",
                        plan.name,
                        plan.provider_type,
                        argument_matches.len(),
                        plan.schema.trait_name,
                    )));
                }
            };
            ServiceSchema::from_typed_instance(typed, definition, &arguments).ok_or_else(|| {
                psi_diagnostics::Diagnostic::error(format!(
                    "ProviderPlan `{}` exact schema `{}` did not reconstruct as a canonical typed boundary schema",
                    plan.name, plan.schema.trait_name,
                ))
            })
        }
        ([], [operator]) => ServiceSchema::from_typed_operator(typed, operator).ok_or_else(|| {
            psi_diagnostics::Diagnostic::error(format!(
                "ProviderPlan `{}` exact schema `{}` did not reconstruct as a canonical typed boundary-operator schema",
                plan.name, plan.schema.trait_name,
            ))
        }),
        _ => Err(psi_diagnostics::Diagnostic::error(format!(
            "ProviderPlan `{}` exact schema `{}` resolves to {} canonical typed boundary traits and {} canonical typed boundary operators",
            plan.name,
            plan.schema.trait_name,
            trait_matches.len(),
            operator_matches.len(),
        ))),
    }
}

fn exact_row_for_schema_method<'plan>(
    plan: &'plan ProviderPlan,
    method: &omega_effects::provider_plan::ServiceMethod,
) -> Result<&'plan ProviderPlanRow, psi_diagnostics::Diagnostic> {
    if method.requirement_identity.is_empty() {
        return Err(psi_diagnostics::Diagnostic::error(format!(
            "ProviderPlan `{}` schema method `{}` has no exact synchronous-invocation overload identity",
            plan.name, method.name,
        )));
    }
    let method_count = plan
        .schema
        .methods
        .iter()
        .filter(|candidate| candidate.requirement_identity == method.requirement_identity)
        .count();
    if method_count != 1 {
        return Err(psi_diagnostics::Diagnostic::error(format!(
            "ProviderPlan `{}` contains {method_count} schema methods for exact synchronous-invocation overload `{}`",
            plan.name, method.requirement_identity,
        )));
    }
    let rows = plan
        .rows
        .iter()
        .filter(|row| plan.schema.row_binds_method(row, method))
        .collect::<Vec<_>>();
    let [row] = rows.as_slice() else {
        return Err(psi_diagnostics::Diagnostic::error(format!(
            "ProviderPlan `{}` schema method `{}` / `{}` binds {} exact synchronous-invocation rows",
            plan.name,
            method.name,
            method.requirement_identity,
            rows.len(),
        )));
    };
    Ok(*row)
}

fn exact_checked_adapter<'typed>(
    typed: &'typed TypedTrees,
    plan: &ProviderPlan,
    row: &ProviderPlanRow,
    machine: &str,
) -> Result<&'typed psi_typed_trees::machine::Machine, psi_diagnostics::Diagnostic> {
    if machine.is_empty() {
        return Err(psi_diagnostics::Diagnostic::error(format!(
            "checked adapter for ProviderPlan `{}` row `{}` has no complete machine identity",
            plan.name, row.requirement_identity,
        )));
    }
    let matches = typed
        .machines()
        .iter()
        .filter(|candidate| candidate.name.as_str() == machine)
        .collect::<Vec<_>>();
    let adapter = match matches.as_slice() {
        [adapter] => *adapter,
        [] => {
            return Err(psi_diagnostics::Diagnostic::error(format!(
                "checked adapter `{machine}` for `{}::{}` is absent from typed machines",
                plan.schema.trait_name, row.method,
            )));
        }
        _ => {
            return Err(psi_diagnostics::Diagnostic::error(format!(
                "checked adapter `{machine}` for ProviderPlan `{}` row `{}` resolves to {} exact typed machines",
                plan.name,
                row.requirement_identity,
                matches.len(),
            )));
        }
    };
    if !adapter.symbol.is_valid() {
        return Err(psi_diagnostics::Diagnostic::error(format!(
            "checked adapter `{machine}` for ProviderPlan `{}` has no exact typed machine symbol",
            plan.name,
        )));
    }
    Ok(adapter)
}

fn exact_invocation_service_name(
    typed: &TypedTrees,
    machine: &psi_typed_trees::machine::Machine,
    target: psi_effects::InvocationTarget,
) -> Result<String, psi_diagnostics::Diagnostic> {
    let symbol = match target {
        psi_effects::InvocationTarget::Parameter(index) => {
            let Some(entry) = typed.machine_states(machine).first() else {
                return Err(psi_diagnostics::Diagnostic::error(format!(
                    "checked adapter `{}` has no entry state for synchronous-invocation parameter {index}",
                    machine.name,
                )));
            };
            let Ok(parameter_index) = usize::try_from(index) else {
                return Err(psi_diagnostics::Diagnostic::error(format!(
                    "checked adapter `{}` synchronous-invocation parameter index {index} is outside the target index range",
                    machine.name,
                )));
            };
            let Some(parameter) = typed
                .state_parameters(entry)
                .iter()
                .filter(|parameter| !parameter.is_self)
                .nth(parameter_index)
            else {
                return Err(psi_diagnostics::Diagnostic::error(format!(
                    "checked adapter `{}` has no exact non-self synchronous-invocation parameter {index}",
                    machine.name,
                )));
            };
            if !parameter.type_reference.is_valid() {
                return Err(psi_diagnostics::Diagnostic::error(format!(
                    "checked adapter `{}` synchronous-invocation parameter {index} has no exact type reference",
                    machine.name,
                )));
            }
            typed
                .type_reference_table
                .type_reference(parameter.type_reference)
                .type_symbol(&typed.type_reference_table)
        }
        psi_effects::InvocationTarget::Service(symbol) => symbol,
    };
    if !symbol.is_valid() {
        return Err(psi_diagnostics::Diagnostic::error(format!(
            "checked adapter `{}` has an invalid exact synchronous-invocation service symbol",
            machine.name,
        )));
    }
    let matches = typed
        .traits()
        .iter()
        .filter(|definition| definition.is_boundary && definition.symbol == symbol)
        .collect::<Vec<_>>();
    let [definition] = matches.as_slice() else {
        return Err(psi_diagnostics::Diagnostic::error(format!(
            "checked adapter `{}` synchronous-invocation symbol {:?} resolves to {} exact boundary traits",
            machine.name,
            symbol,
            matches.len(),
        )));
    };
    Ok(definition.name.as_str().to_owned())
}

fn exact_checked_adapter_invocations(
    typed: &TypedTrees,
    inferred: &psi_effects::InvocationInferencePlan,
    plan: &ProviderPlan,
    method: &omega_effects::provider_plan::ServiceMethod,
    row: &ProviderPlanRow,
    machine: &str,
) -> Result<Vec<String>, psi_diagnostics::Diagnostic> {
    let adapter = exact_checked_adapter(typed, plan, row, machine)?;
    let summaries = inferred
        .machines
        .iter()
        .filter(|summary| summary.machine == adapter.symbol)
        .collect::<Vec<_>>();
    let [summary] = summaries.as_slice() else {
        return Err(psi_diagnostics::Diagnostic::error(format!(
            "checked adapter `{machine}` resolves to {} exact synchronous-invocation inference summaries",
            summaries.len(),
        )));
    };
    let boundaries = typed
        .traits()
        .iter()
        .filter(|definition| {
            definition.is_boundary && definition.name.as_str() == method.requirement_owner
        })
        .collect::<Vec<_>>();
    let boundary = match boundaries.as_slice() {
        [boundary] => Some(*boundary),
        [] => {
            let operators = typed
                .operators()
                .iter()
                .filter(|operator| {
                    operator.is_boundary
                        && psi_typed_trees::operator::boundary_operator_requirement_identity(
                            typed, operator,
                        ) == method.requirement_owner
                })
                .count();
            if operators == 1 {
                None
            } else {
                return Err(psi_diagnostics::Diagnostic::error(format!(
                    "ProviderPlan `{}` requirement owner `{}` resolves to neither one exact boundary trait nor one exact boundary operator for synchronous invocation",
                    plan.name, method.requirement_owner,
                )));
            }
        }
        _ => {
            return Err(psi_diagnostics::Diagnostic::error(format!(
                "ProviderPlan `{}` requirement owner `{}` resolves to {} exact boundary traits for self-forwarded synchronous invocation",
                plan.name,
                method.requirement_owner,
                boundaries.len(),
            )));
        }
    };

    let mut names = Vec::new();
    for target in &summary.inferred_transitive {
        let target_name = exact_invocation_service_name(typed, adapter, *target)?;
        let self_forwarded = *target == psi_effects::InvocationTarget::Parameter(0)
            && boundary.is_some_and(|boundary| {
                let parameter_count = typed
                    .machine_states(adapter)
                    .first()
                    .map(|entry| {
                        typed
                            .state_parameters(entry)
                            .iter()
                            .filter(|parameter| !parameter.is_self)
                            .count()
                    })
                    .unwrap_or_default();
                method.parameter_count.checked_add(1) == Some(parameter_count)
                    && target_name == boundary.name.as_str()
            });
        if self_forwarded {
            continue;
        }
        names.push(target_name);
    }
    names.sort_unstable();
    names.dedup();
    Ok(names)
}

fn exact_authored_invocations(
    plan: &ProviderPlan,
    method: &omega_effects::provider_plan::ServiceMethod,
) -> Result<Vec<String>, psi_diagnostics::Diagnostic> {
    if method
        .synchronous_invocations
        .iter()
        .any(|target| target.is_empty())
    {
        return Err(psi_diagnostics::Diagnostic::error(format!(
            "ProviderPlan `{}` exact overload `{}` has an empty synchronous-invocation identity",
            plan.name, method.requirement_identity,
        )));
    }
    if method
        .synchronous_invocations
        .windows(2)
        .any(|pair| pair[0] >= pair[1])
    {
        return Err(psi_diagnostics::Diagnostic::error(format!(
            "ProviderPlan `{}` exact overload `{}` synchronous-invocation identities are not strictly increasing",
            plan.name, method.requirement_identity,
        )));
    }
    Ok(method.synchronous_invocations.clone())
}

/// Validate every derived candidate before coverage and selection. A partial
/// candidate may wait for more conformances, but duplicate/stray rows and
/// malformed binding shapes are invalid in their own right. The freely
/// constructible retained schema must first equal the canonical typed schema;
/// only then may checked-adapter reach be compared with its public ceiling.
/// Independent operational refinement is validated by the machine-conformance
/// checker that produced the candidate.
pub(crate) fn validate_provider_plan_candidates(
    typed: &TypedTrees,
    plans: &[omega_effects::provider_plan::ProviderPlan],
) -> Vec<psi_diagnostics::Diagnostic> {
    let mut diagnostics = Vec::new();
    let effect_plan = psi_effects::infer_operational_may(typed);
    let service_reach_plan = psi_effects::infer_service_reaches(typed, &effect_plan);
    let invocation_plan = psi_effects::infer_synchronous_invocations(typed);
    for plan in plans {
        let structural_diagnostics = plan.validate_candidate_against_schema();
        if structural_diagnostics.is_empty() {
            let canonical_schema = match exact_canonical_provider_schema(typed, plan) {
                Ok(schema) => schema,
                Err(diagnostic) => {
                    diagnostics.push(diagnostic);
                    continue;
                }
            };
            if plan.schema != canonical_schema {
                diagnostics.push(psi_diagnostics::Diagnostic::error(format!(
                    "ProviderPlan `{}` retained schema `{}` does not equal its exact canonical typed schema",
                    plan.name, plan.schema.trait_name,
                )));
                continue;
            }
        }
        diagnostics.extend(
            structural_diagnostics
                .into_iter()
                .map(psi_diagnostics::Diagnostic::error),
        );
        for row in &plan.rows {
            let ProviderBinding::CheckedAdapter { machine } = &row.binding else {
                continue;
            };
            let method = match exact_schema_method_for_row(plan, row) {
                Ok(method) => method,
                Err(diagnostic) => {
                    diagnostics.push(diagnostic);
                    continue;
                }
            };
            let adapter = match exact_checked_adapter(typed, plan, row, machine) {
                Ok(adapter) => adapter,
                Err(diagnostic) => {
                    diagnostics.push(diagnostic);
                    continue;
                }
            };
            if adapter.attached_data.as_ref().map(|owner| owner.as_str())
                != Some(plan.provider_type.as_str())
            {
                diagnostics.push(psi_diagnostics::Diagnostic::error(format!(
                    "checked adapter `{machine}` for `{}::{}` belongs to provider `{}`, not selected provider `{}`",
                    plan.schema.trait_name,
                    row.method,
                    adapter
                        .attached_data
                        .as_ref()
                        .map_or("<none>", |owner| owner.as_str()),
                    plan.provider_type,
                )));
                continue;
            }
            if adapter.supply_mode != psi_language_semantics::MachineSupplyMode::CheckedBody
                || typed.machine_states(adapter).is_empty()
            {
                diagnostics.push(psi_diagnostics::Diagnostic::error(format!(
                    "checked adapter `{machine}` for `{}::{}` does not name a checked body with an entry state",
                    plan.schema.trait_name, row.method,
                )));
                continue;
            }
            let has_exact_conformance =
                checked_adapter_has_exact_conformance(typed, adapter, plan, row);
            if !has_exact_conformance {
                diagnostics.push(psi_diagnostics::Diagnostic::error(format!(
                    "checked adapter `{machine}` for `{}::{}` has no exact checked satisfies edge for requirement identity `{}`",
                    plan.schema.trait_name, row.method, row.requirement_identity,
                )));
                continue;
            }
            let service_ceiling = method.service_reach.as_slice();
            let invocation_ceiling = method.synchronous_invocations.as_slice();
            let hidden_invocations = match exact_checked_adapter_invocations(
                typed,
                &invocation_plan,
                plan,
                method,
                row,
                machine,
            ) {
                Ok(invocations) => invocations
                    .into_iter()
                    .filter(|target| !invocation_ceiling.contains(target))
                    .collect::<Vec<_>>(),
                Err(diagnostic) => {
                    diagnostics.push(diagnostic);
                    Vec::new()
                }
            };
            if !hidden_invocations.is_empty() {
                diagnostics.push(psi_diagnostics::Diagnostic::error(format!(
                    "adapter `{}` does not refine `{}::{}`: its body may synchronously invoke boundary binding(s) [{}], but the requirement omits those `invokes` edges",
                    machine,
                    plan.schema.trait_name,
                    row.method,
                    hidden_invocations.join(", "),
                )));
            }
            let hidden_services = service_reach_plan
                .for_machine(adapter.symbol)
                .into_iter()
                .flat_map(|summary| service_reach_plan.services(summary.effective).iter())
                .filter_map(|service| typed.service_reaches.definition(*service))
                .map(|definition| definition.name.as_str())
                .filter(|name| {
                    !service_ceiling
                        .iter()
                        .any(|allowed| allowed.as_str() == *name)
                })
                .collect::<Vec<_>>();
            if !hidden_services.is_empty() {
                diagnostics.push(psi_diagnostics::Diagnostic::error(format!(
                    "adapter `{}` does not refine `{}::{}`: its body reaches boundary service(s) [{}] outside the requirement's declared service ceiling [{}] -- the satisfied requirement is the public contract; widen it or drop the service reach",
                    machine,
                    plan.schema.trait_name,
                    row.method,
                    hidden_services.join(", "),
                    service_ceiling.join(", "),
                )));
            }
        }
    }
    diagnostics
}

/// Reject a cycle in the direct synchronous graph realized by the concrete
/// provider selection. Reach closure is intentionally irrelevant: only a
/// selected method's authored `invokes` edges participate, and a missing
/// selected target cannot manufacture an edge.
pub(crate) fn validate_selected_synchronous_invocation_cycles(
    typed: &TypedTrees,
    plans: &[omega_effects::provider_plan::ProviderPlan],
    selected_names: &[String],
) -> Result<(), Vec<psi_diagnostics::Diagnostic>> {
    let selected = exact_selected_synchronous_plans(plans, selected_names)?;
    let inferred = psi_effects::infer_synchronous_invocations(typed);
    let mut edges = vec![Vec::<usize>::new(); selected.len()];
    let mut diagnostics = Vec::new();
    for (source_index, source) in selected.iter().enumerate() {
        for method in &source.schema.methods {
            let row = match exact_row_for_schema_method(source, method) {
                Ok(row) => row,
                Err(diagnostic) => {
                    diagnostics.push(diagnostic);
                    continue;
                }
            };
            let target_names = match &row.binding {
                ProviderBinding::CheckedAdapter { machine } => exact_checked_adapter_invocations(
                    typed, &inferred, source, method, row, machine,
                ),
                _ => exact_authored_invocations(source, method),
            };
            let target_names = match target_names {
                Ok(target_names) => target_names,
                Err(diagnostic) => {
                    diagnostics.push(diagnostic);
                    continue;
                }
            };
            for target_name in target_names {
                if let Some(target_index) = selected
                    .iter()
                    .position(|target| target.schema.trait_name == target_name)
                    && !edges[source_index].contains(&target_index)
                {
                    edges[source_index].push(target_index);
                }
            }
        }
    }
    if !diagnostics.is_empty() {
        return Err(diagnostics);
    }

    let mut color = vec![0u8; selected.len()];
    let mut path = Vec::new();
    for start in 0..selected.len() {
        if color[start] == 0
            && let Some(cycle) = synchronous_cycle_from(start, &edges, &mut color, &mut path)
        {
            let names = cycle
                .iter()
                .map(|index| selected[*index].schema.trait_name.as_str())
                .chain(std::iter::once(
                    selected[cycle[0]].schema.trait_name.as_str(),
                ))
                .collect::<Vec<_>>()
                .join(" -> ");
            return Err(vec![psi_diagnostics::Diagnostic::error(format!(
                "selected providers realize a cyclic synchronous `invokes` graph: {names}; break one edge with a mailbox, queue, scheduler handoff, or other new activation",
            ))]);
        }
    }
    Ok(())
}

fn exact_selected_synchronous_plans<'plans>(
    plans: &'plans [ProviderPlan],
    selected_names: &[String],
) -> Result<Vec<&'plans ProviderPlan>, Vec<psi_diagnostics::Diagnostic>> {
    let mut selected = Vec::new();
    let mut diagnostics = Vec::new();
    let mut seen_names = Vec::new();
    for name in selected_names {
        if name.is_empty() {
            diagnostics.push(psi_diagnostics::Diagnostic::error(
                "selected synchronous-invocation ProviderPlan name is empty",
            ));
            continue;
        }
        if seen_names.contains(name) {
            diagnostics.push(psi_diagnostics::Diagnostic::error(format!(
                "selected synchronous-invocation ProviderPlan `{name}` is listed more than once",
            )));
            continue;
        }
        seen_names.push(name.clone());
        let matches = plans
            .iter()
            .filter(|plan| plan.name == *name)
            .collect::<Vec<_>>();
        match matches.as_slice() {
            [plan] => selected.push(*plan),
            _ => diagnostics.push(psi_diagnostics::Diagnostic::error(format!(
                "selected synchronous-invocation ProviderPlan `{name}` resolves to {} exact candidate plans",
                matches.len(),
            ))),
        }
    }

    let mut seen_schemas = Vec::new();
    for plan in &selected {
        if plan.schema.trait_name.is_empty() {
            diagnostics.push(psi_diagnostics::Diagnostic::error(format!(
                "selected synchronous-invocation ProviderPlan `{}` has an empty exact schema identity",
                plan.name,
            )));
            continue;
        }
        if seen_schemas.contains(&plan.schema.trait_name) {
            diagnostics.push(psi_diagnostics::Diagnostic::error(format!(
                "selected synchronous-invocation schema `{}` is realized by more than one selected ProviderPlan",
                plan.schema.trait_name,
            )));
            continue;
        }
        seen_schemas.push(plan.schema.trait_name.clone());
    }

    if diagnostics.is_empty() {
        Ok(selected)
    } else {
        Err(diagnostics)
    }
}

fn synchronous_cycle_from(
    node: usize,
    edges: &[Vec<usize>],
    color: &mut [u8],
    path: &mut Vec<usize>,
) -> Option<Vec<usize>> {
    color[node] = 1;
    path.push(node);
    for target in &edges[node] {
        if color[*target] == 0 {
            if let Some(cycle) = synchronous_cycle_from(*target, edges, color, path) {
                return Some(cycle);
            }
        } else if color[*target] == 1 {
            let start = path.iter().position(|member| member == target)?;
            return Some(path[start..].to_vec());
        }
    }
    path.pop();
    color[node] = 2;
    None
}

fn authored_name_matches(authored: &str, candidate: &str, exact_exists: bool) -> bool {
    authored == candidate
        || (!exact_exists
            && !authored.contains("::")
            && candidate
                .rsplit("::")
                .next()
                .is_some_and(|leaf| leaf == authored))
}

fn resolve_provider_selection_slots(
    slot_names: &[String],
    declarations: &[crate::pipeline::build_config::ProviderSelection],
    owner: &str,
) -> (
    Vec<(String, crate::pipeline::build_config::ProviderSelection)>,
    Vec<psi_diagnostics::Diagnostic>,
) {
    let mut resolved = Vec::new();
    let mut diagnostics = Vec::new();
    for declaration in declarations {
        let exact_exists = slot_names
            .iter()
            .any(|slot| slot == &declaration.boundary_trait);
        let matching_slots = slot_names
            .iter()
            .map(String::as_str)
            .filter(|slot| authored_name_matches(&declaration.boundary_trait, slot, exact_exists))
            .collect::<Vec<_>>();
        match matching_slots.as_slice() {
            [slot] => resolved.push(((*slot).to_owned(), declaration.clone())),
            [] => diagnostics.push(psi_diagnostics::Diagnostic::error(format!(
                "{owner} selects provider `{}` for unknown boundary slot `{}`; the slot must exist in the loaded dependency closure",
                declaration.provider_type, declaration.boundary_trait,
            ))),
            many => diagnostics.push(psi_diagnostics::Diagnostic::error(format!(
                "{owner} names ambiguous boundary slot `{}`; it matches {} -- qualify the slot type",
                declaration.boundary_trait,
                many.iter()
                    .map(|slot| format!("`{slot}`"))
                    .collect::<Vec<_>>()
                    .join(", "),
            ))),
        }
    }
    (resolved, diagnostics)
}

/// PRV4c: select one fully covering provider type per applicable boundary
/// slot. An explicit build-root declaration wins over the selected target
/// package's ordinary default declaration. Without either, a unique covering
/// candidate supplies the declaration-era default. Rows are never selected
/// individually and partial candidates never combine.
pub(crate) fn select_provider_plan_names(
    plans: &[omega_effects::provider_plan::ProviderPlan],
    selected_target: omega_target::NativeTarget,
    defaults: &[crate::pipeline::build_config::ProviderSelection],
    requested: &[crate::pipeline::build_config::ProviderSelection],
) -> Result<Vec<String>, Vec<psi_diagnostics::Diagnostic>> {
    // Target inertness (the fail-canary host-portability convention): a
    // plan scoped to a NON-selected target is inert and never collides --
    // only plans that RESOLVE to the selected target participate.
    let applies = |target: &str| -> bool {
        if target.is_empty() {
            return true; // portable: every target
        }
        omega_target::NativeTarget::from_omega_target_name(Some(target))
            .is_ok_and(|resolved| resolved == selected_target)
    };
    let mut diagnostics = Vec::new();
    let mut selected = Vec::new();
    let mut slot_names: Vec<String> = plans
        .iter()
        .filter(|plan| !plan.schema.methods.is_empty())
        .map(|plan| plan.schema.trait_name.clone())
        .collect();
    slot_names.sort_unstable();
    slot_names.dedup();

    // Resolve every authored slot path to ONE canonical loaded trait before
    // applying precedence. A short leaf is convenient when unique, but it is
    // not an identity: letting `Pick` match both `first::Pick` and
    // `second::Pick` would turn one type-per-slot declaration into two grants.
    // Canonicalization here also makes differently spelled aliases of the
    // same slot participate in duplicate detection.
    let (resolved_requests, request_diagnostics) =
        resolve_provider_selection_slots(&slot_names, requested, "build");
    diagnostics.extend(request_diagnostics);
    let (resolved_defaults, default_diagnostics) =
        resolve_provider_selection_slots(&slot_names, defaults, "target package");
    diagnostics.extend(default_diagnostics);
    for slot_name in &slot_names {
        let declarations = resolved_requests
            .iter()
            .filter(|(slot, _)| slot == slot_name)
            .map(|(_, declaration)| declaration)
            .collect::<Vec<_>>();
        if declarations.len() > 1 {
            diagnostics.push(psi_diagnostics::Diagnostic::error(format!(
                "build declares provider selection for slot `{slot_name}` more than once: {}",
                declarations
                    .iter()
                    .map(|declaration| format!(
                        "`{} -> {}`",
                        declaration.boundary_trait, declaration.provider_type
                    ))
                    .collect::<Vec<_>>()
                    .join(", "),
            )));
        }
    }

    for slot_name in slot_names {
        let explicit = resolved_requests
            .iter()
            .find(|(slot, _)| *slot == slot_name)
            .map(|(_, selection)| selection);
        let slot_defaults: Vec<_> = resolved_defaults
            .iter()
            .filter(|(slot, _)| *slot == slot_name)
            .map(|(_, selection)| selection)
            .collect();
        let candidates: Vec<&ProviderPlan> = plans
            .iter()
            .filter(|plan| plan.schema.trait_name == slot_name && applies(&plan.target))
            .collect();
        let covering: Vec<&ProviderPlan> = candidates
            .iter()
            .copied()
            .filter(|plan| plan.covers_schema())
            .collect();

        let selected_declaration = if let Some(explicit) = explicit {
            // A slot-owner override intentionally replaces every target
            // default for this slot, including a default whose provider is
            // absent from the selected dependency closure.
            Some(("build", explicit))
        } else if let Some(first) = slot_defaults.first().copied() {
            // Compare target defaults by the canonical candidate type when a
            // spelling resolves uniquely. `Provider` and `pkg::Provider`
            // naming the same loaded type are one default, not a conflict.
            // Ambiguous or absent spellings stay distinct here and receive
            // the more specific candidate diagnostic below.
            let mut distinct_provider_types: Vec<String> = slot_defaults
                .iter()
                .map(|selection| {
                    let exact_exists = candidates
                        .iter()
                        .any(|plan| plan.provider_type == selection.provider_type);
                    let matching = candidates
                        .iter()
                        .copied()
                        .filter(|plan| {
                            authored_name_matches(
                                &selection.provider_type,
                                &plan.provider_type,
                                exact_exists,
                            )
                        })
                        .collect::<Vec<_>>();
                    match matching.as_slice() {
                        [plan] => plan.provider_type.clone(),
                        _ => selection.provider_type.clone(),
                    }
                })
                .collect();
            distinct_provider_types.sort_unstable();
            distinct_provider_types.dedup();
            if distinct_provider_types.len() > 1 {
                diagnostics.push(psi_diagnostics::Diagnostic::error(format!(
                    "slot `{slot_name}` has conflicting target-package defaults: {} -- a target supplies at most one default provider type per slot",
                    distinct_provider_types
                        .iter()
                        .map(|provider| format!("`{provider}`"))
                        .collect::<Vec<_>>()
                        .join(", "),
                )));
                continue;
            }
            Some(("target package", first))
        } else {
            None
        };

        if let Some((owner, declaration)) = selected_declaration {
            let exact_exists = candidates
                .iter()
                .any(|plan| plan.provider_type == declaration.provider_type);
            let matching: Vec<&ProviderPlan> = candidates
                .iter()
                .copied()
                .filter(|plan| {
                    authored_name_matches(
                        &declaration.provider_type,
                        &plan.provider_type,
                        exact_exists,
                    )
                })
                .collect();
            match matching.as_slice() {
                [plan] if plan.covers_schema() => selected.push(plan.name.clone()),
                [plan] => diagnostics.push(psi_diagnostics::Diagnostic::error(format!(
                    "{owner} selects provider `{}` for slot `{slot_name}`, but candidate `{}` is partial ({}/{}) and cannot be selected",
                    declaration.provider_type,
                    plan.name,
                    plan.rows.len(),
                    plan.schema.methods.len(),
                ))),
                [] => {
                    let exact_exists = plans.iter().any(|plan| {
                        plan.schema.trait_name == slot_name
                            && plan.provider_type == declaration.provider_type
                    });
                    let wrong_target = plans.iter().any(|plan| {
                        plan.schema.trait_name == slot_name
                            && authored_name_matches(
                                &declaration.provider_type,
                                &plan.provider_type,
                                exact_exists,
                            )
                    });
                    diagnostics.push(psi_diagnostics::Diagnostic::error(format!(
                        "{owner} selects provider `{}` for slot `{slot_name}`, but no {}candidate exists in the loaded dependency closure",
                        declaration.provider_type,
                        if wrong_target { "selected-target " } else { "" },
                    )));
                }
                _ => diagnostics.push(psi_diagnostics::Diagnostic::error(format!(
                    "{owner} selection `{}` for slot `{slot_name}` resolves to multiple provider candidates; qualify the provider type",
                    declaration.provider_type,
                ))),
            }
            continue;
        }

        match covering.as_slice() {
            [] => {}
            [plan] => selected.push(plan.name.clone()),
            many => {
                let count = if many.len() == 2 {
                    "two".to_owned()
                } else {
                    many.len().to_string()
                };
                diagnostics.push(psi_diagnostics::Diagnostic::error(format!(
                    "slot `{slot_name}` has {count} covering provider plans for the selected target: {} -- choose one in build.omg with `b.select_provider<{slot_name}, ProviderType>();`",
                    many.iter()
                        .map(|plan| format!("`{}` [{:016x}]", plan.name, plan.identity_fingerprint()))
                        .collect::<Vec<_>>()
                        .join(", "),
                )));
            }
        }
    }
    if diagnostics.is_empty() {
        Ok(selected)
    } else {
        Err(diagnostics)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ProviderGrantSelectorKind {
    PlanName,
    ProviderSlot,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ResolvedSelectedProviderGrant {
    pub selector: String,
    pub selector_kind: ProviderGrantSelectorKind,
    pub selected_plan_identity: u64,
    pub selected_plan_name: String,
    pub selected_slot_name: String,
}

impl ResolvedSelectedProviderGrant {
    pub fn commitment(&self) -> String {
        match self.selector_kind {
            ProviderGrantSelectorKind::PlanName => {
                format!("provider plan: {}", self.selected_plan_name)
            }
            ProviderGrantSelectorKind::ProviderSlot => {
                format!("provider slot: {}", self.selected_slot_name)
            }
        }
    }
}

/// Resolve all provider grants once against the complete candidate inventory
/// and the already-selected closure. Plan and slot spellings are distinct
/// selector subjects: the same spelling may name both only when both resolve
/// to the same selected plan, in which case the exact plan-name form is the
/// canonical commitment. Non-provider selectors remain absent for the shared
/// non-provider trust resolver.
pub(crate) fn resolve_selected_provider_grants(
    candidates: &[ProviderPlan],
    selected: &omega_effects::SelectedProviderPlanFacts,
    root_grants: &[String],
) -> Result<Vec<ResolvedSelectedProviderGrant>, psi_diagnostics::Diagnostic> {
    let mut resolved = Vec::new();
    for grant in root_grants {
        let plan_name_candidates = candidates
            .iter()
            .filter(|plan| plan.name == *grant)
            .collect::<Vec<_>>();
        let slot_is_known = candidates
            .iter()
            .any(|plan| plan.schema.trait_name == *grant);
        if plan_name_candidates.is_empty() && !slot_is_known {
            continue;
        }
        if plan_name_candidates.len() > 1 {
            return Err(psi_diagnostics::Diagnostic::error(format!(
                "root grant `{grant}` names {} exact provider plan candidates",
                plan_name_candidates.len(),
            )));
        }

        let selected_by_plan_name = selected
            .plans()
            .iter()
            .filter(|plan| plan.name == *grant)
            .collect::<Vec<_>>();
        let selected_by_slot = selected
            .plans()
            .iter()
            .filter(|plan| plan.schema.trait_name == *grant)
            .collect::<Vec<_>>();
        if selected_by_plan_name.len() > 1 || selected_by_slot.len() > 1 {
            return Err(psi_diagnostics::Diagnostic::error(format!(
                "root grant `{grant}` resolves to multiple selected provider plans",
            )));
        }
        let plan_name_plan = selected_by_plan_name.first().copied();
        let slot_plan = selected_by_slot.first().copied();
        if !plan_name_candidates.is_empty() && plan_name_plan.is_none() {
            return Err(psi_diagnostics::Diagnostic::error(format!(
                "root grant `{grant}` names an unselected provider plan",
            )));
        }
        if slot_is_known && slot_plan.is_none() && plan_name_plan.is_none() {
            return Err(psi_diagnostics::Diagnostic::error(format!(
                "root grant `{grant}` names a provider slot with no selected provider plan",
            )));
        }
        let (plan, selector_kind) = match (plan_name_plan, slot_plan) {
            (Some(plan), Some(slot_plan))
                if plan.identity_fingerprint() == slot_plan.identity_fingerprint() =>
            {
                (plan, ProviderGrantSelectorKind::PlanName)
            }
            (Some(_), Some(_)) => {
                return Err(psi_diagnostics::Diagnostic::error(format!(
                    "root grant `{grant}` names distinct provider plan and slot subjects",
                )));
            }
            (Some(plan), None) => (plan, ProviderGrantSelectorKind::PlanName),
            (None, Some(plan)) if plan_name_candidates.is_empty() => {
                (plan, ProviderGrantSelectorKind::ProviderSlot)
            }
            (None, Some(_)) => {
                return Err(psi_diagnostics::Diagnostic::error(format!(
                    "root grant `{grant}` names an unselected provider plan and a different selected provider slot",
                )));
            }
            (None, None) => {
                return Err(psi_diagnostics::Diagnostic::error(format!(
                    "root grant `{grant}` names a provider plan or slot with no selected provider plan",
                )));
            }
        };
        let exact_candidate_matches = candidates
            .iter()
            .filter(|candidate| *candidate == plan)
            .count();
        if exact_candidate_matches != 1 {
            return Err(psi_diagnostics::Diagnostic::error(format!(
                "root grant `{grant}` selected provider plan `{}` resolves to {exact_candidate_matches} exact candidate rows",
                plan.name,
            )));
        }
        resolved.push(ResolvedSelectedProviderGrant {
            selector: grant.clone(),
            selector_kind,
            selected_plan_identity: plan.identity_fingerprint(),
            selected_plan_name: plan.name.clone(),
            selected_slot_name: plan.schema.trait_name.clone(),
        });
    }
    Ok(resolved)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn derive_provider_fixture(source: &str) -> (TypedTrees, ProviderPlan) {
        let tokens = psi_source_files_to_tokens::Lexer::new(source)
            .tokenize()
            .expect("tokenize provider fixture");
        let syntax = psi_tokens_to_syntax_trees::parse_syntax_trees(&tokens)
            .expect("parse provider fixture");
        let resolved = psi_syntax_trees_to_symbol_resolved_trees::lower_syntax_trees(&syntax)
            .expect("resolve provider fixture");
        let typed =
            psi_symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees(&resolved)
                .expect("type provider fixture");
        let plans = derive_satisfies_plans(&typed, None);
        let [plan] = plans.as_slice() else {
            panic!(
                "provider fixture must derive exactly one plan, got {}",
                plans.len()
            );
        };
        (typed, plan.clone())
    }

    #[test]
    fn provider_derivation_consumes_typed_external_binding_identity() {
        let source = |library: &str, symbol: &str| {
            format!(
                r#"
                    boundary trait Process {{
                        machine exit(code: i32);
                    }}

                    machine exit_leaf(code: i32)
                    satisfies Process::exit
                    via Binding::DllImport("{library}", "{symbol}");
                "#
            )
        };
        let retained_source = source("retained-library", "retained-symbol");
        let retained_tokens = psi_source_files_to_tokens::Lexer::new(&retained_source)
            .tokenize()
            .expect("tokenize retained binding");
        let retained_syntax = psi_tokens_to_syntax_trees::parse_syntax_trees(&retained_tokens)
            .expect("parse retained binding");
        let resolved =
            psi_syntax_trees_to_symbol_resolved_trees::lower_syntax_trees(&retained_syntax)
                .expect("resolve retained binding");
        let typed =
            psi_symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees(&resolved)
                .expect("type retained binding");

        // Derivation accepts no syntax tree: the exact typed id/table is its
        // only external-binding authority.
        let plans = derive_satisfies_plans(&typed, None);
        let [plan] = plans.as_slice() else {
            panic!("one external provider plan")
        };

        assert_eq!(
            plan.rows[0].binding,
            ProviderBinding::Import {
                library: "retained-library".to_owned(),
                symbol: "retained-symbol".to_owned(),
            }
        );
    }

    fn selection_plan(name: &str, methods: &[&str], rows: &[&str]) -> ProviderPlan {
        ProviderPlan {
            name: name.to_owned(),
            provider_type: name.to_owned(),
            target: String::new(),
            schema: ServiceSchema {
                trait_name: "Pair".to_owned(),
                methods: methods
                    .iter()
                    .map(|method| omega_effects::provider_plan::ServiceMethod {
                        name: (*method).to_owned(),
                        requirement_owner: "Pair".to_owned(),
                        requirement_identity: format!("Pair::{method}"),
                        parameter_count: 0,
                        parameter_type_identities: Vec::new(),
                        entry_claims: Vec::new(),
                        has_result: false,
                        result_type_identity: None,
                        result_claims: Vec::new(),
                        service_reach: vec!["Pair".to_owned()],
                        synchronous_invocations: Vec::new(),
                        may_suspend: false,
                        may_block: false,
                        terminates_guarantee: false,
                        calling_plan_fingerprint: None,
                    })
                    .collect(),
            },
            rows: rows
                .iter()
                .map(|method| ProviderPlanRow {
                    method: (*method).to_owned(),
                    requirement_identity: format!("Pair::{method}"),
                    binding: ProviderBinding::VtableSlot { index: 0 },
                })
                .collect(),
            origin_package: String::new(),
        }
    }

    #[test]
    fn provider_grant_ledger_resolves_one_exact_selector_subject() {
        let first = selection_plan("FirstProvider", &["first"], &["first"]);
        let candidates = vec![first.clone()];
        let selected = omega_effects::SelectedProviderPlanFacts::from_selection(
            &candidates,
            &[first.name.clone()],
        )
        .expect("selected provider");
        let grants = resolve_selected_provider_grants(
            &candidates,
            &selected,
            &[
                "FirstProvider".to_owned(),
                "Pair".to_owned(),
                "OtherFact".to_owned(),
                "Pair".to_owned(),
            ],
        )
        .expect("exact provider selectors");

        assert_eq!(grants.len(), 3);
        assert_eq!(grants[0].selector_kind, ProviderGrantSelectorKind::PlanName);
        assert_eq!(grants[0].commitment(), "provider plan: FirstProvider");
        assert_eq!(
            grants[1].selector_kind,
            ProviderGrantSelectorKind::ProviderSlot
        );
        assert_eq!(grants[1].commitment(), "provider slot: Pair");
        assert_eq!(grants[2], grants[1]);
        assert!(
            grants
                .iter()
                .all(|grant| grant.selected_plan_identity == first.identity_fingerprint())
        );

        let mut same_subject = first.clone();
        same_subject.name = same_subject.schema.trait_name.clone();
        let selected = omega_effects::SelectedProviderPlanFacts::from_selection(
            std::slice::from_ref(&same_subject),
            &[same_subject.name.clone()],
        )
        .expect("same plan and slot subject");
        let grants = resolve_selected_provider_grants(
            std::slice::from_ref(&same_subject),
            &selected,
            &[same_subject.name.clone()],
        )
        .expect("same subject is canonical");
        assert_eq!(grants[0].selector_kind, ProviderGrantSelectorKind::PlanName);
    }

    #[test]
    fn provider_grant_ledger_rejects_ambiguity_and_unselected_subjects() {
        enum Corruption {
            DuplicatePlanName,
            UnselectedPlan,
            MissingSelectedSlot,
            DistinctPlanAndSlot,
            UnselectedPlanAndSelectedSlot,
            SelectedCandidateDrift,
        }
        let cases = [
            (
                Corruption::DuplicatePlanName,
                "2 exact provider plan candidates",
            ),
            (Corruption::UnselectedPlan, "unselected provider plan"),
            (
                Corruption::MissingSelectedSlot,
                "provider slot with no selected provider plan",
            ),
            (
                Corruption::DistinctPlanAndSlot,
                "distinct provider plan and slot subjects",
            ),
            (
                Corruption::UnselectedPlanAndSelectedSlot,
                "unselected provider plan",
            ),
            (
                Corruption::SelectedCandidateDrift,
                "resolves to 0 exact candidate rows",
            ),
        ];

        for (corruption, expected) in cases {
            let mut first = selection_plan("FirstProvider", &["first"], &["first"]);
            first.schema.trait_name = "FirstSlot".to_owned();
            let mut second = selection_plan("SecondProvider", &["first"], &["first"]);
            second.schema.trait_name = "SecondSlot".to_owned();
            let (candidates, selected_candidates, selected_names, grant) = match corruption {
                Corruption::DuplicatePlanName => {
                    second.name = first.name.clone();
                    (
                        vec![first.clone(), second],
                        vec![first.clone()],
                        vec![first.name.clone()],
                        first.name.clone(),
                    )
                }
                Corruption::UnselectedPlan => (
                    vec![first.clone(), second.clone()],
                    vec![first.clone()],
                    vec![first.name.clone()],
                    second.name.clone(),
                ),
                Corruption::MissingSelectedSlot => (
                    vec![first.clone(), second.clone()],
                    vec![first.clone()],
                    vec![first.name.clone()],
                    second.schema.trait_name.clone(),
                ),
                Corruption::DistinctPlanAndSlot => {
                    second.schema.trait_name = first.name.clone();
                    (
                        vec![first.clone(), second.clone()],
                        vec![first.clone(), second.clone()],
                        vec![first.name.clone(), second.name.clone()],
                        first.name.clone(),
                    )
                }
                Corruption::UnselectedPlanAndSelectedSlot => {
                    second.schema.trait_name = first.name.clone();
                    (
                        vec![first.clone(), second.clone()],
                        vec![second.clone()],
                        vec![second.name.clone()],
                        first.name.clone(),
                    )
                }
                Corruption::SelectedCandidateDrift => {
                    let mut drifted = first.clone();
                    drifted.origin_package = "drifted".to_owned();
                    (
                        vec![first.clone()],
                        vec![drifted],
                        vec![first.name.clone()],
                        first.name.clone(),
                    )
                }
            };
            let selected = omega_effects::SelectedProviderPlanFacts::from_selection(
                &selected_candidates,
                &selected_names,
            )
            .expect("selected fixture");
            let diagnostic = resolve_selected_provider_grants(&candidates, &selected, &[grant])
                .expect_err("invalid provider selector custody must reject");
            assert!(
                diagnostic.message.contains(expected),
                "expected {expected:?}, got {diagnostic:?}",
            );
        }
    }

    fn push_boundary_requirement(
        checked: &mut psi_checked_trees::CheckedTrees,
        owner_symbol: psi_symbols::SymbolHandle,
        owner_name: &str,
        requirement_symbol: psi_symbols::SymbolHandle,
        requirement_name: &str,
    ) -> String {
        let mut owner = psi_typed_trees::trait_definition::TraitDefinition {
            symbol: owner_symbol,
            is_boundary: true,
            name: psi_typed_trees::name::Identifier::generated(owner_name),
            ..Default::default()
        };
        checked.typed.push_trait_machine_signature(
            &mut owner,
            psi_typed_trees::signature::StateSignature {
                symbol: requirement_symbol,
                name: psi_typed_trees::name::Identifier::generated(requirement_name),
                ..Default::default()
            },
        );
        checked.typed.push_trait_definition(owner);
        let owner = checked
            .typed
            .traits()
            .iter()
            .find(|definition| definition.symbol == owner_symbol)
            .expect("inserted boundary owner");
        let requirement = checked
            .typed
            .trait_machine_signatures(owner)
            .iter()
            .find(|requirement| requirement.symbol == requirement_symbol)
            .expect("inserted boundary requirement");
        checked
            .typed
            .normalized_trait_requirement_overload_identity(owner, requirement)
            .identity()
    }

    fn set_exact_requirement(
        plan: &mut ProviderPlan,
        schema: &str,
        owner: &str,
        requirement_identity: &str,
    ) {
        plan.schema.trait_name = schema.to_owned();
        plan.schema.methods[0].requirement_owner = owner.to_owned();
        plan.schema.methods[0].requirement_identity = requirement_identity.to_owned();
        plan.rows[0].requirement_identity = requirement_identity.to_owned();
    }

    fn append_admitted_fact(
        checked: &mut psi_checked_trees::CheckedTrees,
        subject_symbol: psi_symbols::SymbolHandle,
        domain_symbol: psi_symbols::SymbolHandle,
        owner_symbol: psi_symbols::SymbolHandle,
        requirement_symbol: psi_symbols::SymbolHandle,
    ) -> psi_facts::FactHandle {
        let place = checked.facts.semantic.append_symbol_place(subject_symbol);
        checked.facts.semantic.append_fact(psi_facts::Fact {
            place: psi_facts::FactPlace::Place(place),
            point: psi_facts::ProgramPoint::Global,
            origin: psi_facts::FactOrigin::CallEnsures,
            evidence: psi_facts::QualificationEvidence::from_admitted_requirement(
                owner_symbol,
                requirement_symbol,
            ),
            payload: psi_facts::FactPayload::DomainMembership {
                value: Default::default(),
                domain: Default::default(),
                domain_symbol,
            },
        })
    }

    #[test]
    fn selected_synchronous_invocation_graph_rejects_cycles_only_after_selection() {
        let mut alpha = selection_plan("alpha", &["run"], &["run"]);
        alpha.schema.trait_name = "Alpha".to_owned();
        alpha.schema.methods[0].synchronous_invocations = vec!["Beta".to_owned()];
        let mut beta = selection_plan("beta", &["run"], &["run"]);
        beta.schema.trait_name = "Beta".to_owned();
        beta.schema.methods[0].synchronous_invocations = vec!["Alpha".to_owned()];

        validate_selected_synchronous_invocation_cycles(
            &TypedTrees::default(),
            &[alpha.clone(), beta.clone()],
            &["alpha".to_owned()],
        )
        .expect("an unselected potential return edge is not realized");

        let diagnostics = validate_selected_synchronous_invocation_cycles(
            &TypedTrees::default(),
            &[alpha, beta],
            &["alpha".to_owned(), "beta".to_owned()],
        )
        .expect_err("the selected Alpha -> Beta -> Alpha graph must reject");
        assert!(diagnostics.iter().any(|diagnostic| {
            diagnostic
                .message
                .contains("cyclic synchronous `invokes` graph")
                && diagnostic.message.contains("Alpha -> Beta -> Alpha")
        }));
    }

    #[derive(Clone, Copy, Debug)]
    enum SelectedInvocationDrift {
        None,
        EmptySelectedName,
        MissingSelectedPlan,
        DuplicateSelectedName,
        DuplicatePlanName,
        DuplicateSelectedSchema,
        EmptyMethodIdentity,
        DuplicateMethodIdentity,
        EmptyRowIdentity,
        CrossRowIdentity,
        MissingRow,
        DuplicateRow,
        EmptyInvocation,
        DuplicateInvocation,
    }

    #[test]
    fn selected_synchronous_invocation_identity_drift_rejects_exactly() {
        let cases = [
            (SelectedInvocationDrift::None, None),
            (
                SelectedInvocationDrift::EmptySelectedName,
                Some("name is empty"),
            ),
            (
                SelectedInvocationDrift::MissingSelectedPlan,
                Some("resolves to 0 exact candidate plans"),
            ),
            (
                SelectedInvocationDrift::DuplicateSelectedName,
                Some("listed more than once"),
            ),
            (
                SelectedInvocationDrift::DuplicatePlanName,
                Some("resolves to 2 exact candidate plans"),
            ),
            (
                SelectedInvocationDrift::DuplicateSelectedSchema,
                Some("realized by more than one selected ProviderPlan"),
            ),
            (
                SelectedInvocationDrift::EmptyMethodIdentity,
                Some("schema method `run` has no exact"),
            ),
            (
                SelectedInvocationDrift::DuplicateMethodIdentity,
                Some("contains 2 schema methods"),
            ),
            (
                SelectedInvocationDrift::EmptyRowIdentity,
                Some("binds 0 exact synchronous-invocation rows"),
            ),
            (
                SelectedInvocationDrift::CrossRowIdentity,
                Some("binds 0 exact synchronous-invocation rows"),
            ),
            (
                SelectedInvocationDrift::MissingRow,
                Some("binds 0 exact synchronous-invocation rows"),
            ),
            (
                SelectedInvocationDrift::DuplicateRow,
                Some("binds 2 exact synchronous-invocation rows"),
            ),
            (
                SelectedInvocationDrift::EmptyInvocation,
                Some("empty synchronous-invocation identity"),
            ),
            (
                SelectedInvocationDrift::DuplicateInvocation,
                Some("not strictly increasing"),
            ),
        ];

        for (drift, expected) in cases {
            let mut alpha = selection_plan("alpha", &["run"], &["run"]);
            alpha.schema.trait_name = "pkg::Alpha".to_owned();
            alpha.schema.methods[0].synchronous_invocations = vec!["pkg::Beta".to_owned()];
            let mut beta = selection_plan("beta", &["run"], &["run"]);
            beta.schema.trait_name = "pkg::Beta".to_owned();
            let mut plans = vec![alpha, beta];
            let mut selected = vec!["alpha".to_owned(), "beta".to_owned()];
            match drift {
                SelectedInvocationDrift::None => {}
                SelectedInvocationDrift::EmptySelectedName => selected[0].clear(),
                SelectedInvocationDrift::MissingSelectedPlan => {
                    selected[0] = "missing".to_owned();
                }
                SelectedInvocationDrift::DuplicateSelectedName => {
                    selected[1] = selected[0].clone();
                }
                SelectedInvocationDrift::DuplicatePlanName => {
                    let duplicate = plans[0].clone();
                    plans.push(duplicate);
                }
                SelectedInvocationDrift::DuplicateSelectedSchema => {
                    plans[1].schema.trait_name = plans[0].schema.trait_name.clone();
                }
                SelectedInvocationDrift::EmptyMethodIdentity => {
                    plans[0].schema.methods[0].requirement_identity.clear();
                }
                SelectedInvocationDrift::DuplicateMethodIdentity => {
                    let duplicate = plans[0].schema.methods[0].clone();
                    plans[0].schema.methods.push(duplicate);
                }
                SelectedInvocationDrift::EmptyRowIdentity => {
                    plans[0].rows[0].requirement_identity.clear();
                }
                SelectedInvocationDrift::CrossRowIdentity => {
                    plans[0].rows[0].requirement_identity = "pkg::Other::run".to_owned();
                }
                SelectedInvocationDrift::MissingRow => plans[0].rows.clear(),
                SelectedInvocationDrift::DuplicateRow => {
                    let duplicate = plans[0].rows[0].clone();
                    plans[0].rows.push(duplicate);
                }
                SelectedInvocationDrift::EmptyInvocation => {
                    plans[0].schema.methods[0].synchronous_invocations = vec![String::new()];
                }
                SelectedInvocationDrift::DuplicateInvocation => {
                    plans[0].schema.methods[0].synchronous_invocations =
                        vec!["pkg::Beta".to_owned(), "pkg::Beta".to_owned()];
                }
            }

            let result = validate_selected_synchronous_invocation_cycles(
                &TypedTrees::default(),
                &plans,
                &selected,
            );
            match expected {
                None => result.expect("exact selected direct graph is valid"),
                Some(expected) => {
                    let diagnostics = result.expect_err("identity drift must fail closed");
                    assert!(
                        diagnostics
                            .iter()
                            .any(|diagnostic| diagnostic.message.contains(expected)),
                        "{drift:?}: expected `{expected}`, got {diagnostics:?}",
                    );
                }
            }
        }
    }

    #[test]
    fn selected_synchronous_invocation_edges_require_complete_schema_identity() {
        let mut alpha = selection_plan("alpha", &["run"], &["run"]);
        alpha.schema.trait_name = "a::Alpha".to_owned();
        alpha.schema.methods[0].synchronous_invocations = vec!["a::Beta".to_owned()];
        let mut beta = selection_plan("beta", &["run"], &["run"]);
        beta.schema.trait_name = "b::Beta".to_owned();
        beta.schema.methods[0].synchronous_invocations = vec!["a::Alpha".to_owned()];

        validate_selected_synchronous_invocation_cycles(
            &TypedTrees::default(),
            &[alpha.clone(), beta.clone()],
            &["alpha".to_owned(), "beta".to_owned()],
        )
        .expect("same-leaf foreign schema must not manufacture an edge");

        alpha.schema.methods[0].synchronous_invocations = vec!["b::Beta".to_owned()];
        let diagnostics = validate_selected_synchronous_invocation_cycles(
            &TypedTrees::default(),
            &[alpha, beta],
            &["alpha".to_owned(), "beta".to_owned()],
        )
        .expect_err("the exact canonical Alpha -> Beta -> Alpha graph must reject");
        assert!(diagnostics.iter().any(|diagnostic| {
            diagnostic
                .message
                .contains("a::Alpha -> b::Beta -> a::Alpha")
        }));
    }

    fn boundary_trait(
        symbol: u32,
        name: &str,
    ) -> psi_typed_trees::trait_definition::TraitDefinition {
        psi_typed_trees::trait_definition::TraitDefinition {
            symbol: psi_symbols::SymbolHandle::from_arena_index(symbol),
            name: psi_typed_trees::name::Identifier::generated(name),
            is_boundary: true,
            ..Default::default()
        }
    }

    fn checked_invocation_fixture(
        parameter_trait: u32,
        parameter_count: usize,
    ) -> (
        TypedTrees,
        ProviderPlan,
        psi_effects::InvocationInferencePlan,
    ) {
        let source = psi_symbols::SymbolHandle::from_arena_index(31);
        let target = psi_symbols::SymbolHandle::from_arena_index(32);
        let foreign_source = psi_symbols::SymbolHandle::from_arena_index(33);
        let machine_symbol = psi_symbols::SymbolHandle::from_arena_index(34);
        let mut typed = TypedTrees::default();
        typed.push_trait_definition(boundary_trait(31, "pkg::Source"));
        typed.push_trait_definition(boundary_trait(32, "pkg::Target"));
        typed.push_trait_definition(boundary_trait(33, "other::Source"));
        let type_symbol = match parameter_trait {
            31 => source,
            32 => target,
            33 => foreign_source,
            other => psi_symbols::SymbolHandle::from_arena_index(other),
        };
        let type_reference =
            typed
                .type_reference_table
                .insert(psi_typed_trees::types::TypeReferenceNode::Named {
                    symbol: type_symbol,
                    name: psi_typed_trees::name::Identifier::generated("binding"),
                });
        let mut entry = psi_typed_trees::state::State::default();
        typed.push_state_parameter(
            &mut entry,
            psi_typed_trees::signature::StateParameter {
                type_reference,
                name: psi_typed_trees::name::Identifier::generated("binding"),
                ..Default::default()
            },
        );
        let mut machine = psi_typed_trees::machine::Machine {
            symbol: machine_symbol,
            name: psi_typed_trees::name::Identifier::generated("Provider::run"),
            attached_data: Some(psi_typed_trees::name::Identifier::generated("Provider")),
            ..Default::default()
        };
        typed.push_machine_state(&mut machine, entry);
        typed.push_machine(machine);

        let mut plan = selection_plan("provider", &["run"], &["run"]);
        plan.provider_type = "Provider".to_owned();
        plan.schema.trait_name = "pkg::Source".to_owned();
        plan.schema.methods[0].requirement_owner = "pkg::Source".to_owned();
        plan.schema.methods[0].parameter_count = parameter_count;
        plan.rows[0].binding = ProviderBinding::CheckedAdapter {
            machine: "Provider::run".to_owned(),
        };
        let inferred = psi_effects::InvocationInferencePlan {
            machines: vec![psi_effects::MachineInvocationInference {
                machine: machine_symbol,
                published: Vec::new(),
                inferred_direct: vec![psi_effects::InvocationTarget::Parameter(0)],
                inferred_transitive: vec![psi_effects::InvocationTarget::Parameter(0)],
                effective: vec![psi_effects::InvocationTarget::Parameter(0)],
            }],
        };
        (typed, plan, inferred)
    }

    #[derive(Clone, Copy, Debug)]
    enum CheckedInvocationDrift {
        None,
        MissingOwner,
        DuplicateOwner,
        AbsentMachine,
        DuplicateMachine,
        AbsentInference,
        DuplicateInference,
        OutOfRangeParameter,
        UnknownParameterType,
        InvalidService,
        UnknownService,
        NonBoundaryService,
        DuplicateBoundarySymbol,
    }

    #[test]
    fn checked_synchronous_invocation_targets_reject_every_exact_drift() {
        let cases = [
            (CheckedInvocationDrift::None, None),
            (
                CheckedInvocationDrift::MissingOwner,
                Some("neither one exact boundary trait nor one exact boundary operator"),
            ),
            (
                CheckedInvocationDrift::DuplicateOwner,
                Some("resolves to 2 exact boundary traits"),
            ),
            (
                CheckedInvocationDrift::AbsentMachine,
                Some("is absent from typed machines"),
            ),
            (
                CheckedInvocationDrift::DuplicateMachine,
                Some("resolves to 2 exact typed machines"),
            ),
            (
                CheckedInvocationDrift::AbsentInference,
                Some("0 exact synchronous-invocation inference summaries"),
            ),
            (
                CheckedInvocationDrift::DuplicateInference,
                Some("2 exact synchronous-invocation inference summaries"),
            ),
            (
                CheckedInvocationDrift::OutOfRangeParameter,
                Some("no exact non-self synchronous-invocation parameter 1"),
            ),
            (
                CheckedInvocationDrift::UnknownParameterType,
                Some("resolves to 0 exact boundary traits"),
            ),
            (
                CheckedInvocationDrift::InvalidService,
                Some("invalid exact synchronous-invocation service symbol"),
            ),
            (
                CheckedInvocationDrift::UnknownService,
                Some("resolves to 0 exact boundary traits"),
            ),
            (
                CheckedInvocationDrift::NonBoundaryService,
                Some("resolves to 0 exact boundary traits"),
            ),
            (
                CheckedInvocationDrift::DuplicateBoundarySymbol,
                Some("resolves to 2 exact boundary traits"),
            ),
        ];

        for (drift, expected) in cases {
            let parameter_trait = if matches!(drift, CheckedInvocationDrift::UnknownParameterType) {
                99
            } else {
                32
            };
            let (mut typed, mut plan, mut inferred) =
                checked_invocation_fixture(parameter_trait, 1);
            match drift {
                CheckedInvocationDrift::None => {}
                CheckedInvocationDrift::MissingOwner => {
                    plan.schema.methods[0].requirement_owner = "pkg::Missing".to_owned();
                }
                CheckedInvocationDrift::DuplicateOwner => {
                    typed.push_trait_definition(boundary_trait(35, "pkg::Source"));
                }
                CheckedInvocationDrift::AbsentMachine => {
                    plan.rows[0].binding = ProviderBinding::CheckedAdapter {
                        machine: "Provider::missing".to_owned(),
                    };
                }
                CheckedInvocationDrift::DuplicateMachine => {
                    let duplicate = typed.machines()[0].clone();
                    typed.push_machine(duplicate);
                }
                CheckedInvocationDrift::AbsentInference => inferred.machines.clear(),
                CheckedInvocationDrift::DuplicateInference => {
                    let duplicate = inferred.machines[0].clone();
                    inferred.machines.push(duplicate);
                }
                CheckedInvocationDrift::OutOfRangeParameter => {
                    inferred.machines[0].inferred_transitive =
                        vec![psi_effects::InvocationTarget::Parameter(1)];
                }
                CheckedInvocationDrift::UnknownParameterType => {}
                CheckedInvocationDrift::InvalidService => {
                    inferred.machines[0].inferred_transitive =
                        vec![psi_effects::InvocationTarget::Service(
                            psi_symbols::SymbolHandle::invalid(),
                        )];
                }
                CheckedInvocationDrift::UnknownService => {
                    inferred.machines[0].inferred_transitive =
                        vec![psi_effects::InvocationTarget::Service(
                            psi_symbols::SymbolHandle::from_arena_index(99),
                        )];
                }
                CheckedInvocationDrift::NonBoundaryService => {
                    typed.push_trait_definition(
                        psi_typed_trees::trait_definition::TraitDefinition {
                            symbol: psi_symbols::SymbolHandle::from_arena_index(99),
                            name: psi_typed_trees::name::Identifier::generated("pkg::Plain"),
                            ..Default::default()
                        },
                    );
                    inferred.machines[0].inferred_transitive =
                        vec![psi_effects::InvocationTarget::Service(
                            psi_symbols::SymbolHandle::from_arena_index(99),
                        )];
                }
                CheckedInvocationDrift::DuplicateBoundarySymbol => {
                    typed.push_trait_definition(boundary_trait(32, "pkg::DuplicateTarget"));
                }
            }
            let method = &plan.schema.methods[0];
            let row = &plan.rows[0];
            let ProviderBinding::CheckedAdapter { machine } = &row.binding else {
                unreachable!()
            };
            let result =
                exact_checked_adapter_invocations(&typed, &inferred, &plan, method, row, machine);
            match expected {
                None => assert_eq!(
                    result.expect("exact checked target resolves"),
                    vec!["pkg::Target".to_owned()],
                ),
                Some(expected) => assert!(
                    result
                        .expect_err("checked invocation identity drift must reject")
                        .message
                        .contains(expected),
                    "{drift:?}: expected `{expected}`",
                ),
            }
        }
    }

    #[test]
    fn self_forwarding_erases_only_the_exact_schema_receiver() {
        let (typed, plan, inferred) = checked_invocation_fixture(31, 0);
        let ProviderBinding::CheckedAdapter { machine } = &plan.rows[0].binding else {
            unreachable!()
        };
        assert_eq!(
            exact_checked_adapter_invocations(
                &typed,
                &inferred,
                &plan,
                &plan.schema.methods[0],
                &plan.rows[0],
                machine,
            )
            .expect("exact receiver forwarding resolves"),
            Vec::<String>::new(),
        );

        let (typed, plan, inferred) = checked_invocation_fixture(33, 0);
        let ProviderBinding::CheckedAdapter { machine } = &plan.rows[0].binding else {
            unreachable!()
        };
        assert_eq!(
            exact_checked_adapter_invocations(
                &typed,
                &inferred,
                &plan,
                &plan.schema.methods[0],
                &plan.rows[0],
                machine,
            )
            .expect("same-leaf foreign receiver remains an external edge"),
            vec!["other::Source".to_owned()],
        );
    }

    #[test]
    fn implicit_selection_never_combines_partial_candidates() {
        let plans = vec![
            selection_plan("FirstProvider", &["first", "second"], &["first"]),
            selection_plan("SecondProvider", &["first", "second"], &["second"]),
        ];
        assert_eq!(
            select_provider_plan_names(&plans, omega_target::NativeTarget::host(), &[], &[])
                .expect("partial candidates are reportable, not ambiguous"),
            Vec::<String>::new(),
            "two partial candidates are not one provider"
        );
    }

    #[test]
    fn implicit_selection_returns_the_unique_covering_candidate() {
        let plans = vec![
            selection_plan(
                "CompleteProvider",
                &["first", "second"],
                &["first", "second"],
            ),
            selection_plan("PartialProvider", &["first", "second"], &["first"]),
        ];
        assert_eq!(
            select_provider_plan_names(&plans, omega_target::NativeTarget::host(), &[], &[])
                .expect("one covering candidate selects"),
            vec!["CompleteProvider".to_owned()]
        );
    }

    #[test]
    fn external_root_bridge_requires_one_exact_retained_boundary_slot() {
        let mut first = selection_plan("FirstProvider", &["run"], &["run"]);
        first.schema.trait_name = "first::Pair".into();
        let mut second = selection_plan("SecondProvider", &["run"], &["run"]);
        second.schema.trait_name = "second::Pair".into();
        let facts = omega_effects::SelectedProviderPlanFacts::from_selection(
            &[first.clone(), second],
            &["FirstProvider".into(), "SecondProvider".into()],
        )
        .expect("distinct qualified boundary slots may both be selected");
        assert_eq!(
            selected_external_root_provider_plan_id(&facts, "first::Pair")
                .expect("qualified slot resolves")
                .normalized_identity(),
            first.identity_fingerprint()
        );
        assert!(
            selected_external_root_provider_plan_id(&facts, "Pair")
                .expect_err("an ambiguous leaf slot must reject")
                .0
                .contains("matches 2 retained selected provider plans")
        );
    }

    #[test]
    fn granted_selected_plan_attaches_receipt_by_exact_inherited_requirement() {
        let owner_symbol = psi_symbols::SymbolHandle::from_arena_index(7);
        let subject_symbol = psi_symbols::SymbolHandle::from_arena_index(8);
        let domain_symbol = psi_symbols::SymbolHandle::from_arena_index(9);
        let requirement_symbol = psi_symbols::SymbolHandle::from_arena_index(10);
        let mut checked = psi_checked_trees::CheckedTrees::default();
        let requirement_identity = push_boundary_requirement(
            &mut checked,
            owner_symbol,
            "PairBase",
            requirement_symbol,
            "first",
        );
        let fact = append_admitted_fact(
            &mut checked,
            subject_symbol,
            domain_symbol,
            owner_symbol,
            requirement_symbol,
        );
        let mut selected = selection_plan("FirstProvider", &["first"], &["first"]);
        set_exact_requirement(
            &mut selected,
            "PairChild",
            "PairBase",
            &requirement_identity,
        );
        let identity = selected.identity_fingerprint();

        bind_selected_provider_plan_facts(
            &mut checked,
            std::slice::from_ref(&selected),
            omega_effects::SelectedProviderPlanFacts::from_selection(
                std::slice::from_ref(&selected),
                &["FirstProvider".to_owned()],
            )
            .expect("canonical selected facts"),
            &["PairChild".to_owned()],
        )
        .expect("exact inherited requirement binds the selected child-schema plan");

        assert_eq!(
            checked
                .facts
                .semantic
                .facts
                .get(fact)
                .evidence
                .receipt_identity,
            identity
        );
    }

    #[test]
    fn granted_selected_plan_does_not_stamp_a_different_exact_requirement() {
        let owner_symbol = psi_symbols::SymbolHandle::from_arena_index(7);
        let selected_requirement = psi_symbols::SymbolHandle::from_arena_index(10);
        let evidence_requirement = psi_symbols::SymbolHandle::from_arena_index(11);
        let mut checked = psi_checked_trees::CheckedTrees::default();
        let mut owner = psi_typed_trees::trait_definition::TraitDefinition {
            symbol: owner_symbol,
            is_boundary: true,
            name: psi_typed_trees::name::Identifier::generated("PairBase"),
            ..Default::default()
        };
        for (symbol, name) in [
            (selected_requirement, "first"),
            (evidence_requirement, "second"),
        ] {
            checked.typed.push_trait_machine_signature(
                &mut owner,
                psi_typed_trees::signature::StateSignature {
                    symbol,
                    name: psi_typed_trees::name::Identifier::generated(name),
                    ..Default::default()
                },
            );
        }
        checked.typed.push_trait_definition(owner);
        let owner = checked
            .typed
            .traits()
            .iter()
            .find(|definition| definition.symbol == owner_symbol)
            .expect("inserted boundary owner");
        let requirement = checked
            .typed
            .trait_machine_signatures(owner)
            .iter()
            .find(|requirement| requirement.symbol == selected_requirement)
            .expect("selected boundary requirement");
        let requirement_identity = checked
            .typed
            .normalized_trait_requirement_overload_identity(owner, requirement)
            .identity();
        let fact = append_admitted_fact(
            &mut checked,
            psi_symbols::SymbolHandle::from_arena_index(8),
            psi_symbols::SymbolHandle::from_arena_index(9),
            owner_symbol,
            evidence_requirement,
        );
        let mut selected = selection_plan("FirstProvider", &["first"], &["first"]);
        set_exact_requirement(
            &mut selected,
            "PairChild",
            "PairBase",
            &requirement_identity,
        );

        bind_selected_provider_plan_facts(
            &mut checked,
            std::slice::from_ref(&selected),
            omega_effects::SelectedProviderPlanFacts::from_selection(
                std::slice::from_ref(&selected),
                &["FirstProvider".to_owned()],
            )
            .expect("canonical selected facts"),
            &["PairChild".to_owned()],
        )
        .expect("a different exact requirement is simply not stamped");

        assert_eq!(
            checked
                .facts
                .semantic
                .facts
                .get(fact)
                .evidence
                .receipt_identity,
            0
        );
    }

    #[test]
    fn admitted_receipt_rejects_a_requirement_outside_its_exact_owner() {
        let owner_symbol = psi_symbols::SymbolHandle::from_arena_index(7);
        let requirement_symbol = psi_symbols::SymbolHandle::from_arena_index(10);
        let mut checked = psi_checked_trees::CheckedTrees::default();
        let requirement_identity = push_boundary_requirement(
            &mut checked,
            owner_symbol,
            "PairBase",
            requirement_symbol,
            "first",
        );
        append_admitted_fact(
            &mut checked,
            psi_symbols::SymbolHandle::from_arena_index(8),
            psi_symbols::SymbolHandle::from_arena_index(9),
            owner_symbol,
            psi_symbols::SymbolHandle::from_arena_index(11),
        );
        let mut selected = selection_plan("FirstProvider", &["first"], &["first"]);
        set_exact_requirement(
            &mut selected,
            "PairChild",
            "PairBase",
            &requirement_identity,
        );

        let diagnostics = bind_selected_provider_plan_facts(
            &mut checked,
            std::slice::from_ref(&selected),
            omega_effects::SelectedProviderPlanFacts::from_selection(
                std::slice::from_ref(&selected),
                &["FirstProvider".to_owned()],
            )
            .expect("canonical selected facts"),
            &["PairChild".to_owned()],
        )
        .expect_err("an admitted requirement outside the exact owner must reject");
        assert!(diagnostics.iter().any(|diagnostic| {
            diagnostic
                .message
                .contains("resolves to 0 exact typed signatures")
        }));
    }

    #[test]
    fn admitted_receipt_owner_and_signature_custody_is_exact_and_atomic() {
        let owner_symbol = psi_symbols::SymbolHandle::from_arena_index(7);
        let requirement_symbol = psi_symbols::SymbolHandle::from_arena_index(10);
        let mut checked = psi_checked_trees::CheckedTrees::default();
        let requirement_identity = push_boundary_requirement(
            &mut checked,
            owner_symbol,
            "PairBase",
            requirement_symbol,
            "first",
        );
        let valid = append_admitted_fact(
            &mut checked,
            psi_symbols::SymbolHandle::from_arena_index(8),
            psi_symbols::SymbolHandle::from_arena_index(9),
            owner_symbol,
            requirement_symbol,
        );
        append_admitted_fact(
            &mut checked,
            psi_symbols::SymbolHandle::from_arena_index(11),
            psi_symbols::SymbolHandle::from_arena_index(12),
            owner_symbol,
            psi_symbols::SymbolHandle::from_arena_index(90),
        );
        let mut selected = selection_plan("FirstProvider", &["first"], &["first"]);
        set_exact_requirement(
            &mut selected,
            "PairChild",
            "PairBase",
            &requirement_identity,
        );

        let diagnostics = bind_selected_provider_plan_facts(
            &mut checked,
            std::slice::from_ref(&selected),
            omega_effects::SelectedProviderPlanFacts::from_selection(
                std::slice::from_ref(&selected),
                &[selected.name.clone()],
            )
            .expect("selected provider"),
            &[selected.schema.trait_name.clone()],
        )
        .expect_err("late missing signature must reject every staged receipt");
        assert!(diagnostics.iter().any(|diagnostic| {
            diagnostic
                .message
                .contains("resolves to 0 exact typed signatures")
        }));
        assert_eq!(
            checked
                .facts
                .semantic
                .facts
                .get(valid)
                .evidence
                .receipt_identity,
            0,
            "late failure must not publish an earlier valid receipt",
        );

        let mut duplicate_owner = checked.clone();
        duplicate_owner.typed.push_trait_definition(
            psi_typed_trees::trait_definition::TraitDefinition {
                symbol: owner_symbol,
                is_boundary: true,
                name: psi_typed_trees::name::Identifier::generated("DuplicatePairBase"),
                ..Default::default()
            },
        );
        let diagnostics = bind_selected_provider_plan_facts(
            &mut duplicate_owner,
            std::slice::from_ref(&selected),
            omega_effects::SelectedProviderPlanFacts::from_selection(
                std::slice::from_ref(&selected),
                &[selected.name.clone()],
            )
            .expect("selected provider"),
            &[selected.schema.trait_name.clone()],
        )
        .expect_err("duplicate exact owner must reject");
        assert!(diagnostics.iter().any(|diagnostic| {
            diagnostic
                .message
                .contains("resolves to 2 exact typed boundary requirement owners")
        }));

        let other_owner = psi_symbols::SymbolHandle::from_arena_index(20);
        let other_requirement = psi_symbols::SymbolHandle::from_arena_index(21);
        let mut cross_owned = psi_checked_trees::CheckedTrees::default();
        push_boundary_requirement(
            &mut cross_owned,
            owner_symbol,
            "PairBase",
            requirement_symbol,
            "first",
        );
        push_boundary_requirement(
            &mut cross_owned,
            other_owner,
            "OtherBase",
            other_requirement,
            "other",
        );
        append_admitted_fact(
            &mut cross_owned,
            psi_symbols::SymbolHandle::from_arena_index(22),
            psi_symbols::SymbolHandle::from_arena_index(23),
            owner_symbol,
            other_requirement,
        );
        let diagnostics = bind_selected_provider_plan_facts(
            &mut cross_owned,
            std::slice::from_ref(&selected),
            omega_effects::SelectedProviderPlanFacts::from_selection(
                std::slice::from_ref(&selected),
                &[selected.name.clone()],
            )
            .expect("selected provider"),
            &[selected.schema.trait_name.clone()],
        )
        .expect_err("cross-owned exact signature must reject");
        assert!(
            diagnostics
                .iter()
                .any(|diagnostic| { diagnostic.message.contains("belongs to exact trait") })
        );

        let mut duplicate_signature = psi_checked_trees::CheckedTrees::default();
        push_boundary_requirement(
            &mut duplicate_signature,
            owner_symbol,
            "PairBase",
            requirement_symbol,
            "first",
        );
        push_boundary_requirement(
            &mut duplicate_signature,
            other_owner,
            "OtherBase",
            requirement_symbol,
            "duplicate",
        );
        append_admitted_fact(
            &mut duplicate_signature,
            psi_symbols::SymbolHandle::from_arena_index(24),
            psi_symbols::SymbolHandle::from_arena_index(25),
            owner_symbol,
            requirement_symbol,
        );
        let diagnostics = bind_selected_provider_plan_facts(
            &mut duplicate_signature,
            std::slice::from_ref(&selected),
            omega_effects::SelectedProviderPlanFacts::from_selection(
                std::slice::from_ref(&selected),
                &[selected.name.clone()],
            )
            .expect("selected provider"),
            &[selected.schema.trait_name.clone()],
        )
        .expect_err("duplicate exact signature must reject");
        assert!(diagnostics.iter().any(|diagnostic| {
            diagnostic
                .message
                .contains("resolves to 2 exact typed signatures")
        }));
    }

    #[test]
    fn admitted_receipt_rejects_duplicate_exact_granted_plan_matches() {
        let owner_symbol = psi_symbols::SymbolHandle::from_arena_index(7);
        let requirement_symbol = psi_symbols::SymbolHandle::from_arena_index(10);
        let mut checked = psi_checked_trees::CheckedTrees::default();
        let requirement_identity = push_boundary_requirement(
            &mut checked,
            owner_symbol,
            "PairBase",
            requirement_symbol,
            "first",
        );
        append_admitted_fact(
            &mut checked,
            psi_symbols::SymbolHandle::from_arena_index(8),
            psi_symbols::SymbolHandle::from_arena_index(9),
            owner_symbol,
            requirement_symbol,
        );
        let mut first = selection_plan("FirstProvider", &["first"], &["first"]);
        set_exact_requirement(&mut first, "PairChildA", "PairBase", &requirement_identity);
        let mut second = selection_plan("SecondProvider", &["first"], &["first"]);
        set_exact_requirement(&mut second, "PairChildB", "PairBase", &requirement_identity);

        let diagnostics = bind_selected_provider_plan_facts(
            &mut checked,
            &[first.clone(), second.clone()],
            omega_effects::SelectedProviderPlanFacts::from_selection(
                &[first, second],
                &["FirstProvider".to_owned(), "SecondProvider".to_owned()],
            )
            .expect("distinct selected slots may retain duplicate requirement identities"),
            &["PairChildA".to_owned(), "PairChildB".to_owned()],
        )
        .expect_err("two granted exact matches must reject rather than choose by order");
        assert!(diagnostics.iter().any(|diagnostic| {
            diagnostic
                .message
                .contains("matches 2 granted selected provider plans")
        }));
    }

    #[test]
    fn explicit_selection_resolves_covering_ambiguity_by_provider_type() {
        let plans = vec![
            selection_plan("FirstProvider", &["first"], &["first"]),
            selection_plan("SecondProvider", &["first"], &["first"]),
        ];
        let selected = select_provider_plan_names(
            &plans,
            omega_target::NativeTarget::host(),
            &[],
            &[crate::pipeline::build_config::ProviderSelection {
                boundary_trait: "Pair".to_owned(),
                provider_type: "SecondProvider".to_owned(),
            }],
        )
        .expect("the build root owns the slot choice");
        assert_eq!(selected, vec!["SecondProvider".to_owned()]);
    }

    #[test]
    fn unqualified_selection_refuses_an_ambiguous_boundary_slot_leaf() {
        let mut first = selection_plan("FirstProvider", &["choose"], &["choose"]);
        first.schema.trait_name = "first::Pick".to_owned();
        let mut second = selection_plan("SecondProvider", &["choose"], &["choose"]);
        second.schema.trait_name = "second::Pick".to_owned();

        let diagnostics = select_provider_plan_names(
            &[first, second],
            omega_target::NativeTarget::host(),
            &[],
            &[crate::pipeline::build_config::ProviderSelection {
                boundary_trait: "Pick".to_owned(),
                provider_type: "FirstProvider".to_owned(),
            }],
        )
        .expect_err("one short slot name must not grant two qualified slots");
        assert!(
            diagnostics.iter().any(|diagnostic| {
                diagnostic
                    .message
                    .contains("ambiguous boundary slot `Pick`")
                    && diagnostic.message.contains("`first::Pick`")
                    && diagnostic.message.contains("`second::Pick`")
            }),
            "expected a qualification diagnostic, got {diagnostics:#?}"
        );
    }

    #[test]
    fn exact_slot_name_wins_over_a_qualified_leaf_fallback() {
        let exact = selection_plan("ExactProvider", &["choose"], &["choose"]);
        let mut qualified = selection_plan("QualifiedProvider", &["choose"], &[]);
        qualified.schema.trait_name = "package::Pair".to_owned();

        let selected = select_provider_plan_names(
            &[exact, qualified],
            omega_target::NativeTarget::host(),
            &[],
            &[crate::pipeline::build_config::ProviderSelection {
                boundary_trait: "Pair".to_owned(),
                provider_type: "ExactProvider".to_owned(),
            }],
        )
        .expect("an exact canonical slot name outranks leaf fallback");
        assert_eq!(selected, vec!["ExactProvider".to_owned()]);
    }

    #[test]
    fn exact_provider_name_wins_over_a_qualified_leaf_fallback() {
        let exact = selection_plan("exact-plan", &["choose"], &["choose"]);
        let mut qualified = selection_plan("qualified-plan", &["choose"], &["choose"]);
        qualified.provider_type = "package::exact-plan".to_owned();

        let selected = select_provider_plan_names(
            &[exact, qualified],
            omega_target::NativeTarget::host(),
            &[],
            &[crate::pipeline::build_config::ProviderSelection {
                boundary_trait: "Pair".to_owned(),
                provider_type: "exact-plan".to_owned(),
            }],
        )
        .expect("an exact canonical provider name outranks leaf fallback");
        assert_eq!(selected, vec!["exact-plan".to_owned()]);
    }

    #[test]
    fn canonical_slot_resolution_catches_duplicate_selection_spellings() {
        let mut plan = selection_plan("FirstProvider", &["choose"], &["choose"]);
        plan.schema.trait_name = "package::Pick".to_owned();

        let diagnostics = select_provider_plan_names(
            &[plan],
            omega_target::NativeTarget::host(),
            &[],
            &[
                crate::pipeline::build_config::ProviderSelection {
                    boundary_trait: "Pick".to_owned(),
                    provider_type: "FirstProvider".to_owned(),
                },
                crate::pipeline::build_config::ProviderSelection {
                    boundary_trait: "package::Pick".to_owned(),
                    provider_type: "SecondProvider".to_owned(),
                },
            ],
        )
        .expect_err("one canonical slot cannot be selected twice through aliases");
        assert!(
            diagnostics.iter().any(|diagnostic| diagnostic
                .message
                .contains("provider selection for slot `package::Pick` more than once")),
            "expected canonical duplicate-slot diagnostic, got {diagnostics:#?}"
        );
    }

    #[test]
    fn target_default_refuses_an_ambiguous_boundary_slot_leaf() {
        let mut first = selection_plan("FirstProvider", &["choose"], &["choose"]);
        first.schema.trait_name = "first::Pick".to_owned();
        let mut second = selection_plan("SecondProvider", &["choose"], &["choose"]);
        second.schema.trait_name = "second::Pick".to_owned();

        let diagnostics = select_provider_plan_names(
            &[first, second],
            omega_target::NativeTarget::host(),
            &[crate::pipeline::build_config::ProviderSelection {
                boundary_trait: "Pick".to_owned(),
                provider_type: "FirstProvider".to_owned(),
            }],
            &[],
        )
        .expect_err("a target default must name one canonical slot");
        assert!(diagnostics.iter().any(|diagnostic| {
            diagnostic
                .message
                .contains("target package names ambiguous boundary slot `Pick`")
        }));
    }

    #[test]
    fn explicit_selection_refuses_partial_provider() {
        let plans = vec![selection_plan(
            "PartialProvider",
            &["first", "second"],
            &["first"],
        )];
        let diagnostics = select_provider_plan_names(
            &plans,
            omega_target::NativeTarget::host(),
            &[],
            &[crate::pipeline::build_config::ProviderSelection {
                boundary_trait: "Pair".to_owned(),
                provider_type: "PartialProvider".to_owned(),
            }],
        )
        .expect_err("selection never manufactures missing rows");
        assert!(diagnostics[0].message.contains("is partial"));
    }

    #[test]
    fn target_default_resolves_covering_ambiguity() {
        let plans = vec![
            selection_plan("FirstProvider", &["first"], &["first"]),
            selection_plan("SecondProvider", &["first"], &["first"]),
        ];
        let selected = select_provider_plan_names(
            &plans,
            omega_target::NativeTarget::host(),
            &[crate::pipeline::build_config::ProviderSelection {
                boundary_trait: "Pair".to_owned(),
                provider_type: "FirstProvider".to_owned(),
            }],
            &[],
        )
        .expect("the selected target package supplies the slot default");
        assert_eq!(selected, vec!["FirstProvider".to_owned()]);
    }

    #[test]
    fn target_default_aliases_of_one_provider_do_not_conflict() {
        let mut plan = selection_plan("package-provider", &["first"], &["first"]);
        plan.provider_type = "package::FirstProvider".to_owned();
        let selected = select_provider_plan_names(
            &[plan],
            omega_target::NativeTarget::host(),
            &[
                crate::pipeline::build_config::ProviderSelection {
                    boundary_trait: "Pair".to_owned(),
                    provider_type: "FirstProvider".to_owned(),
                },
                crate::pipeline::build_config::ProviderSelection {
                    boundary_trait: "Pair".to_owned(),
                    provider_type: "package::FirstProvider".to_owned(),
                },
            ],
            &[],
        )
        .expect("aliases of one canonical provider type are one target default");
        assert_eq!(selected, vec!["package-provider".to_owned()]);
    }

    #[test]
    fn build_override_wins_over_target_default() {
        let plans = vec![
            selection_plan("FirstProvider", &["first"], &["first"]),
            selection_plan("SecondProvider", &["first"], &["first"]),
        ];
        let selected = select_provider_plan_names(
            &plans,
            omega_target::NativeTarget::host(),
            &[crate::pipeline::build_config::ProviderSelection {
                boundary_trait: "Pair".to_owned(),
                provider_type: "FirstProvider".to_owned(),
            }],
            &[crate::pipeline::build_config::ProviderSelection {
                boundary_trait: "Pair".to_owned(),
                provider_type: "SecondProvider".to_owned(),
            }],
        )
        .expect("the build root owns the final slot choice");
        assert_eq!(selected, vec!["SecondProvider".to_owned()]);
    }

    #[test]
    fn conflicting_target_defaults_are_loud() {
        let plans = vec![
            selection_plan("FirstProvider", &["first"], &["first"]),
            selection_plan("SecondProvider", &["first"], &["first"]),
        ];
        let diagnostics = select_provider_plan_names(
            &plans,
            omega_target::NativeTarget::host(),
            &[
                crate::pipeline::build_config::ProviderSelection {
                    boundary_trait: "Pair".to_owned(),
                    provider_type: "FirstProvider".to_owned(),
                },
                crate::pipeline::build_config::ProviderSelection {
                    boundary_trait: "Pair".to_owned(),
                    provider_type: "SecondProvider".to_owned(),
                },
            ],
            &[],
        )
        .expect_err("a target has one default provider per slot");
        assert!(
            diagnostics[0]
                .message
                .contains("conflicting target-package defaults")
        );
    }

    #[test]
    fn table_field_leaf_requires_an_attached_layout_owner() {
        let mut plan = selection_plan("field-leaf", &["first"], &[]);
        plan.provider_type.clear();
        plan.rows.push(ProviderPlanRow {
            method: "first".to_owned(),
            requirement_identity: "Pair::first".to_owned(),
            binding: ProviderBinding::VtableField {
                table: String::new(),
                field: "first".to_owned(),
            },
        });

        let diagnostics = validate_provider_plan_candidates(&TypedTrees::default(), &[plan]);

        assert_eq!(diagnostics.len(), 1);
        assert!(
            diagnostics[0]
                .message
                .contains("without an attached provider data type")
        );
    }

    #[test]
    fn checked_adapter_requires_a_nominal_provider_type() {
        let mut plan = selection_plan("free-adapter", &["first"], &[]);
        plan.provider_type.clear();
        plan.rows.push(ProviderPlanRow {
            method: "first".to_owned(),
            requirement_identity: "Pair::first".to_owned(),
            binding: ProviderBinding::CheckedAdapter {
                machine: "first_adapter".to_owned(),
            },
        });

        let diagnostics = validate_provider_plan_candidates(&TypedTrees::default(), &[plan]);

        // Candidate-shape and typed-resolution validation remain cumulative:
        // the impossible free adapter has neither a nominal owner nor a typed
        // machine that could supply one.
        assert_eq!(diagnostics.len(), 2);
        assert!(
            diagnostics
                .iter()
                .any(|diagnostic| diagnostic.message.contains("has no nominal provider type"))
        );
        assert!(
            diagnostics
                .iter()
                .any(|diagnostic| diagnostic.message.contains("is absent from typed machines"))
        );
    }

    #[test]
    fn checked_adapter_must_resolve_to_its_exact_checked_provider_conformance() {
        let source = r#"
            boundary trait Readable {
                machine read() -> i32;
            }

            boundary trait OtherBoundary {
                machine other() -> i32;
            }

            data Provider {}
            data OtherProvider {}

            machine Provider::read() -> i32 satisfies Readable::read { 1 }
            machine Provider::helper() -> i32 { 2 }
            machine OtherProvider::helper() -> i32 { 3 }
            machine Provider::external() -> i32
            satisfies OtherBoundary::other
            via Binding::CompilerIntrinsic;
        "#;
        let tokens = psi_source_files_to_tokens::Lexer::new(source)
            .tokenize()
            .expect("tokenize adapter ownership fixture");
        let syntax = psi_tokens_to_syntax_trees::parse_syntax_trees(&tokens)
            .expect("parse adapter ownership fixture");
        let resolved = psi_syntax_trees_to_symbol_resolved_trees::lower_syntax_trees(&syntax)
            .expect("resolve adapter ownership fixture");
        let typed =
            psi_symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees(&resolved)
                .expect("type adapter ownership fixture");
        let plan = derive_satisfies_plans(&typed, None)
            .into_iter()
            .find(|plan| plan.schema.trait_name == "Readable")
            .expect("Readable provider plan");
        assert!(
            validate_provider_plan_candidates(&typed, std::slice::from_ref(&plan)).is_empty(),
            "the exact checked provider conformance remains valid"
        );

        let mut absent = plan.clone();
        absent.rows[0].binding = ProviderBinding::CheckedAdapter {
            machine: "Provider::absent".to_owned(),
        };
        assert!(
            validate_provider_plan_candidates(&typed, &[absent])
                .iter()
                .any(|diagnostic| diagnostic.message.contains("is absent from typed machines"))
        );

        let mut wrong_provider = plan.clone();
        wrong_provider.rows[0].binding = ProviderBinding::CheckedAdapter {
            machine: "OtherProvider::helper".to_owned(),
        };
        assert!(
            validate_provider_plan_candidates(&typed, &[wrong_provider])
                .iter()
                .any(|diagnostic| diagnostic.message.contains(
                    "belongs to provider `OtherProvider`, not selected provider `Provider`"
                ))
        );

        let mut external = plan.clone();
        external.rows[0].binding = ProviderBinding::CheckedAdapter {
            machine: "Provider::external".to_owned(),
        };
        assert!(
            validate_provider_plan_candidates(&typed, &[external])
                .iter()
                .any(|diagnostic| diagnostic
                    .message
                    .contains("does not name a checked body with an entry state"))
        );

        let mut unrelated = plan;
        unrelated.rows[0].binding = ProviderBinding::CheckedAdapter {
            machine: "Provider::helper".to_owned(),
        };
        assert!(
            validate_provider_plan_candidates(&typed, &[unrelated])
                .iter()
                .any(|diagnostic| diagnostic
                    .message
                    .contains("has no exact checked satisfies edge for requirement identity"))
        );
    }

    #[test]
    fn checked_operator_adapter_must_resolve_to_its_exact_operator_conformance() {
        let source = r#"
            data CheckedMath {}
            boundary operator CheckedMath::offset_zero(value: i32) -> i32;

            data OtherMath {}
            boundary operator OtherMath::offset_zero(value: i32) -> i32;

            data CheckedMathProvider {}
            machine CheckedMathProvider::offset_zero_impl(input: i32) -> i32
            satisfies CheckedMath::offset_zero
            {
                transition { _ -> (input) }
            }
            machine CheckedMathProvider::decoy_impl(input: i32) -> i32
            satisfies OtherMath::offset_zero
            {
                transition { _ -> (input) }
            }
            machine CheckedMathProvider::wrong_signature(input: u64) -> u64
            satisfies CheckedMath::offset_zero
            {
                transition { _ -> (input) }
            }
        "#;
        let tokens = psi_source_files_to_tokens::Lexer::new(source)
            .tokenize()
            .expect("tokenize checked operator adapter fixture");
        let syntax = psi_tokens_to_syntax_trees::parse_syntax_trees(&tokens)
            .expect("parse checked operator adapter fixture");
        let resolved = psi_syntax_trees_to_symbol_resolved_trees::lower_syntax_trees(&syntax)
            .expect("resolve checked operator adapter fixture");
        let typed =
            psi_symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees(&resolved)
                .expect("type checked operator adapter fixture");
        let operator = typed
            .operators()
            .iter()
            .find(|operator| {
                typed
                    .operator_path_members(operator.name)
                    .iter()
                    .map(|member| member.as_str())
                    .eq(["CheckedMath", "offset_zero"])
            })
            .expect("CheckedMath::offset_zero operator");
        let identity =
            psi_typed_trees::operator::boundary_operator_requirement_identity(&typed, operator);
        let plan = derive_satisfies_plans(&typed, None)
            .into_iter()
            .find(|plan| plan.schema.trait_name == identity)
            .expect("CheckedMath::offset_zero provider plan");
        assert!(
            validate_provider_plan_candidates(&typed, std::slice::from_ref(&plan)).is_empty(),
            "the exact checked operator conformance remains valid"
        );

        for unrelated in [
            "CheckedMathProvider::decoy_impl",
            "CheckedMathProvider::wrong_signature",
        ] {
            let mut invalid = plan.clone();
            invalid.rows[0].binding = ProviderBinding::CheckedAdapter {
                machine: unrelated.to_owned(),
            };
            assert!(
                validate_provider_plan_candidates(&typed, &[invalid])
                    .iter()
                    .any(|diagnostic| diagnostic
                        .message
                        .contains("has no exact checked satisfies edge for requirement identity")),
                "operator adapter `{unrelated}` must not satisfy the exact operator row"
            );
        }
    }

    #[test]
    fn syscall_derivation_retains_exact_number_before_range_validation() {
        fn derive(number: i64) -> (TypedTrees, Vec<omega_effects::provider_plan::ProviderPlan>) {
            let source = format!(
                r#"
                    boundary trait Process {{
                        machine exit(code: i32);
                    }}

                    machine exit_leaf(code: i32)
                    satisfies Process::exit
                    via Binding::Syscall({number});
                "#
            );
            let tokens = psi_source_files_to_tokens::Lexer::new(&source)
                .tokenize()
                .expect("tokenize syscall leaf");
            let syntax = psi_tokens_to_syntax_trees::parse_syntax_trees(&tokens)
                .expect("parse syscall leaf");
            let resolved = psi_syntax_trees_to_symbol_resolved_trees::lower_syntax_trees(&syntax)
                .expect("resolve syscall leaf");
            let typed =
                psi_symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees(&resolved)
                    .expect("type syscall leaf");
            let plans = derive_satisfies_plans(&typed, None);
            (typed, plans)
        }

        let maximum = i64::from(u32::MAX);
        let (typed, plans) = derive(maximum);
        let ProviderBinding::Syscall { number } = &plans[0].rows[0].binding else {
            panic!("source syscall leaf must retain a syscall binding");
        };
        assert_eq!(*number, maximum);
        assert!(validate_provider_plan_candidates(&typed, &plans).is_empty());

        let oversized = maximum + 1;
        let (typed, plans) = derive(oversized);
        let ProviderBinding::Syscall { number } = &plans[0].rows[0].binding else {
            panic!("source syscall leaf must retain a syscall binding");
        };
        assert_eq!(*number, oversized);
        assert_ne!(*number, 0, "oversized syscall must not normalize to zero");
        let diagnostics = validate_provider_plan_candidates(&typed, &plans);
        assert!(diagnostics.iter().any(|diagnostic| {
            diagnostic
                .message
                .contains("target syscall plan requires a value in 0..=4294967295")
        }));
    }

    #[test]
    fn checked_adapter_rejects_symbol_resolved_service_widening() {
        let source = r#"
            boundary trait Queryable {
                machine query();
            }

            boundary trait Readable {
                machine read(queryable: &mut Queryable);
            }

            data Provider {}

            machine Provider::read(queryable: &mut Queryable)
            satisfies Readable::read {
                queryable.query();
            }
        "#;
        let tokens = psi_source_files_to_tokens::Lexer::new(source)
            .tokenize()
            .expect("tokenize");
        let syntax =
            psi_tokens_to_syntax_trees::parse_syntax_trees(&tokens).expect("parse provider");
        let resolved = psi_syntax_trees_to_symbol_resolved_trees::lower_syntax_trees(&syntax)
            .expect("resolve provider");
        let typed =
            psi_symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees(&resolved)
                .expect("type provider");
        let plans = derive_satisfies_plans(&typed, None);

        let diagnostics = validate_provider_plan_candidates(&typed, &plans);

        assert!(diagnostics.iter().any(|diagnostic| {
            diagnostic
                .message
                .contains("boundary service(s) [Queryable]")
                && diagnostic
                    .message
                    .contains("declared service ceiling [Readable]")
        }));
    }

    #[test]
    fn provider_candidate_requires_exact_canonical_typed_schema() {
        #[derive(Clone, Copy, Debug)]
        enum Drift {
            Method,
            RequirementOwner,
            RequirementIdentity,
            ParameterShape,
            EntryClaim,
            ResultShape,
            ResultClaim,
            ServiceReach,
            SynchronousInvocation,
            Suspension,
            Blocking,
            Termination,
            CallingPlan,
        }

        let source = r#"
            boundary trait Readable {
                machine read();
            }

            data Provider {}

            machine Provider::read()
            satisfies Readable::read {}
        "#;
        let (typed, plan) = derive_provider_fixture(source);
        assert!(validate_provider_plan_candidates(&typed, std::slice::from_ref(&plan)).is_empty());

        for drift in [
            Drift::Method,
            Drift::RequirementOwner,
            Drift::RequirementIdentity,
            Drift::ParameterShape,
            Drift::EntryClaim,
            Drift::ResultShape,
            Drift::ResultClaim,
            Drift::ServiceReach,
            Drift::SynchronousInvocation,
            Drift::Suspension,
            Drift::Blocking,
            Drift::Termination,
            Drift::CallingPlan,
        ] {
            let mut drifted = plan.clone();
            let method = &mut drifted.schema.methods[0];
            match drift {
                Drift::Method => {
                    method.name = "other".to_owned();
                    drifted.rows[0].method = method.name.clone();
                }
                Drift::RequirementOwner => method.requirement_owner = "Other".to_owned(),
                Drift::RequirementIdentity => {
                    method.requirement_identity = "Other::read()".to_owned();
                    drifted.rows[0].requirement_identity = method.requirement_identity.clone();
                }
                Drift::ParameterShape => {
                    method.parameter_count = 1;
                    method.parameter_type_identities = vec!["i32".to_owned()];
                }
                Drift::EntryClaim => {
                    method.parameter_count = 1;
                    method.parameter_type_identities = vec!["i32 in Accepted".to_owned()];
                    method.entry_claims = vec![omega_effects::provider_plan::ServiceEntryClaim {
                        parameter_index: 0,
                        domain: "Accepted".to_owned(),
                        predicate_body: psi_language_semantics::DomainPredicateBody::Bodyless,
                        effective_carry: psi_language_semantics::CarryPolicy::STRICT,
                        authority_flow:
                            omega_effects::provider_plan::ServiceEntryAuthorityFlow::Accepts,
                    }];
                }
                Drift::ResultShape => {
                    method.has_result = true;
                    method.result_type_identity = Some("i32".to_owned());
                }
                Drift::ResultClaim => {
                    method.has_result = true;
                    method.result_type_identity = Some("i32 in Returned".to_owned());
                    method.result_claims = vec![omega_effects::provider_plan::ServiceResultClaim {
                        domain: "Returned".to_owned(),
                        effective_carry: psi_language_semantics::CarryPolicy::STRICT,
                    }];
                }
                Drift::ServiceReach => {
                    method.service_reach.push("Writable".to_owned());
                    method.service_reach.sort_unstable();
                }
                Drift::SynchronousInvocation => {
                    method.synchronous_invocations.push("Writable".to_owned())
                }
                Drift::Suspension => method.may_suspend = true,
                Drift::Blocking => method.may_block = true,
                Drift::Termination => method.terminates_guarantee = true,
                Drift::CallingPlan => method.calling_plan_fingerprint = Some(1),
            }

            let diagnostics = validate_provider_plan_candidates(&typed, &[drifted]);
            assert!(
                diagnostics.iter().any(|diagnostic| diagnostic
                    .message
                    .contains("does not equal its exact canonical typed schema")),
                "{drift:?} must fail canonical typed schema custody: {diagnostics:?}",
            );
        }
    }

    #[test]
    fn forged_service_ceiling_cannot_launder_checked_adapter_reach() {
        let source = r#"
            boundary trait Queryable {
                machine query();
            }

            boundary trait Readable {
                machine read(queryable: &mut Queryable);
            }

            data Provider {}

            machine Provider::read(queryable: &mut Queryable)
            satisfies Readable::read {
                queryable.query();
            }
        "#;
        let (typed, mut plan) = derive_provider_fixture(source);
        plan.schema.methods[0]
            .service_reach
            .push("Queryable".to_owned());
        plan.schema.methods[0].service_reach.sort_unstable();
        plan.schema.methods[0].service_reach.dedup();

        let diagnostics = validate_provider_plan_candidates(&typed, &[plan]);
        assert!(diagnostics.iter().any(|diagnostic| {
            diagnostic
                .message
                .contains("does not equal its exact canonical typed schema")
        }));
        assert!(!diagnostics.iter().any(|diagnostic| {
            diagnostic
                .message
                .contains("boundary service(s) [Queryable] outside")
        }));
    }

    #[test]
    fn canonical_schema_resolution_is_exact_and_unique() {
        let source = r#"
            boundary trait Readable {
                machine read();
            }

            boundary trait Unrelated {
                machine inspect();
            }

            data Provider {}

            machine Provider::read()
            satisfies Readable::read {}
        "#;
        let (typed, plan) = derive_provider_fixture(source);
        assert_eq!(
            exact_canonical_provider_schema(&typed, &plan).expect("exact schema"),
            plan.schema,
        );

        for identity in ["Missing", "pkg::Readable"] {
            let mut drifted = plan.clone();
            drifted.schema.trait_name = identity.to_owned();
            let diagnostic = exact_canonical_provider_schema(&typed, &drifted)
                .expect_err("unknown and qualified-leaf impostor schemas must reject");
            assert!(diagnostic.message.contains("resolves to 0 canonical typed"));
        }

        let mut duplicated = typed.clone();
        let duplicate = duplicated
            .traits()
            .iter()
            .find(|definition| definition.name.as_str() == "Readable")
            .expect("Readable trait")
            .clone();
        duplicated.push_trait_definition(duplicate);
        let diagnostic = exact_canonical_provider_schema(&duplicated, &plan)
            .expect_err("duplicate exact schema authority must reject");
        assert!(
            diagnostic
                .message
                .contains("resolves to 2 canonical typed boundary traits")
        );
    }

    #[test]
    fn canonical_schema_accepts_exact_inherited_requirement() {
        let source = r#"
            boundary trait Parent {
                machine read();
            }

            boundary trait Child {
                requires Parent;
            }

            data Provider {}

            ProviderChild: Provider satisfies Child;

            machine Provider::read()
            satisfies Parent::read {}
        "#;
        let (typed, mut plan) = derive_provider_fixture(source);
        let child = typed
            .traits()
            .iter()
            .find(|definition| definition.name.as_str() == "Child")
            .expect("Child boundary schema");
        plan.schema = ServiceSchema::from_typed(&typed, child).expect("typed child schema");

        assert!(
            validate_provider_plan_candidates(&typed, &[plan]).is_empty(),
            "an exact child schema may retain its inherited parent requirement",
        );
    }

    #[test]
    fn canonical_schema_rejects_duplicate_exact_carrier_arguments() {
        let source = r#"
            boundary trait Readable {
                machine read(&mut self);
            }

            data Provider {}

            ProviderReadable: Provider satisfies Readable;

            machine Provider::read(&mut self)
            satisfies Readable::read {}
        "#;
        let (mut typed, plan) = derive_provider_fixture(source);
        assert_eq!(typed.conformances().len(), 1);
        let duplicate = typed.conformances()[0].clone();
        typed.push_conformance(duplicate);

        let diagnostic = exact_canonical_provider_schema(&typed, &plan)
            .expect_err("duplicate carrier argument custody must reject");
        assert!(
            diagnostic
                .message
                .contains("resolves to 2 exact carrier argument rows")
        );
    }
}
