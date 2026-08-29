//! Provider plans derive from checked `satisfies` closures and are admitted
//! through the chapter-10 trust path. Own-package plans remain dev-active with
//! a standing warning until the final build grants them; lockfile receipts hash
//! normalized plan identity so a changed plan drifts. A unique covering
//! candidate may still supply the declaration-era default, while explicit
//! selection remains under slot-owner authority.

use omega_effects::provider_plan::{ProviderBinding, ProviderPlan, ProviderPlanRow, ServiceSchema};
#[cfg(test)]
use omega_trust_model::ProviderGrantSelectorKind;
use omega_trust_model::resolve_selected_provider_grants;
use psi_typed_trees::TypedTrees;
use std::sync::Arc;

#[path = "plans/external_binding_rows.rs"]
mod external_binding_rows;
pub use external_binding_rows::{extract_external_binding_rows, settle_external_binding_rows};
#[path = "plans/intrinsic_execution.rs"]
mod intrinsic_execution;
pub use intrinsic_execution::{
    CompilerPrimitiveFloatBinaryOperation, primitive_float_binary_intrinsic_execution_identity,
};

/// Exact selected provider-plan input consumed by external-root construction.
///
/// The schema is retained beside the normalized plan identity so installation
/// can bind the source callable shape—including domain/carry-qualified
/// parameter types—to the same provider selection later carried by
/// `ProviderExecution` and per-invocation entry receipts.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SelectedExternalRootProviderPlan {
    pub identity: omega_external_roots::ProviderPlanId,
    pub digest: omega_effects::provider_plan::ProviderPlanDigest,
    pub schema: ServiceSchema,
    exact_plan: ProviderPlan,
}

/// One exact selected source schema, AOT-lowered writer, installed resolver,
/// and activated unpublished destination sealed to the provider-populated
/// preparation they admitted. All four precede source resolution.
#[derive(Debug)]
pub struct SelectedExternalRootPostHandoffWriterPreparation<'installed, 'mapping, 'bytes> {
    selected_provider: SelectedExternalRootProviderPlan,
    lowered: omega_program_entry_plan::LoweredPostHandoffWriter,
    installed_code: &'installed omega_executable_installation::InstalledCode,
    prepared: omega_external_roots::PreparedExternalRootPostHandoffWriterInvocation,
    destination: omega_executable_installation::ValidatedPreparedPostHandoffWriterDestination<
        'mapping,
        'bytes,
    >,
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

    pub const fn lowered(&self) -> &omega_program_entry_plan::LoweredPostHandoffWriter {
        &self.lowered
    }

    pub const fn installed_code(&self) -> &'installed omega_executable_installation::InstalledCode {
        self.installed_code
    }

    pub const fn destination(
        &self,
    ) -> &omega_executable_installation::ValidatedPreparedPostHandoffWriterDestination<
        'mapping,
        'bytes,
    > {
        &self.destination
    }
}

/// Preparation rejection returns the exact selected schema, lowered writer,
/// and destination. Provider execution, installed code, and writer are
/// borrowed inputs and therefore remain with the caller unchanged.
#[derive(Debug)]
pub struct SelectedExternalRootWriterPreparationError<'mapping, 'bytes> {
    selected_provider: SelectedExternalRootProviderPlan,
    lowered: omega_program_entry_plan::LoweredPostHandoffWriter,
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
        omega_program_entry_plan::LoweredPostHandoffWriter,
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
    lowered: omega_program_entry_plan::LoweredPostHandoffWriter,
    installed_code: &'installed omega_executable_installation::InstalledCode,
    prepared: omega_external_roots::PreparedExternalRootPostHandoffWriterInvocation,
    destination: omega_executable_installation::ValidatedPreparedPostHandoffWriterDestination<
        'mapping,
        'bytes,
    >,
}

/// Still-unpublished destination after one exact bound external-root writer
/// executes. The AOT-lowered fragment remains attached to the installation
/// context and installed resolver instead of being reduced to copied
/// fingerprints at this boundary. This carrier establishes neither consumer
/// semantics nor publication.
#[derive(Debug)]
pub struct WrittenBoundExternalRootPostHandoffWriterDestination<'installed, 'mapping, 'bytes> {
    selected_provider: SelectedExternalRootProviderPlan,
    lowered: omega_program_entry_plan::LoweredPostHandoffWriter,
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
    lowered: omega_program_entry_plan::LoweredPostHandoffWriter,
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

    pub const fn lowered(&self) -> &omega_program_entry_plan::LoweredPostHandoffWriter {
        &self.lowered
    }

    pub const fn installed_code(&self) -> &'installed omega_executable_installation::InstalledCode {
        self.installed_code
    }

    /// Independently replay the retained AOT bytes, footprint, invocation,
    /// opaque installation context, and exact borrowed installed realization.
    /// Rejection only borrows this carrier, preserving every input for retry.
    pub fn validate_for_consumer(&self) -> Result<(), psi_layout_plans::MaterializationDiagnostic> {
        omega_program_entry_plan::validate_lowered_post_handoff_writer(&self.lowered).map_err(
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

    pub const fn provider_execution(&self) -> omega_external_roots::AdmittedProviderExecution {
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
        omega_program_entry_plan::LoweredPostHandoffWriter,
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
        omega_program_entry_plan::validate_lowered_post_handoff_writer(&self.lowered).map_err(
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

    pub const fn lowered(&self) -> &omega_program_entry_plan::LoweredPostHandoffWriter {
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
    ) -> &omega_executable_installation::ValidatedPreparedPostHandoffWriterDestination<
        'mapping,
        'bytes,
    > {
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
        prepared.provider_plan_digest(),
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
        written.provider_plan_digest(),
    )
}

fn validate_selected_provider_source(
    selected_provider: &SelectedExternalRootProviderPlan,
    requirement_identity: &str,
    boundary_parameter_count: usize,
    boundary_contract_fingerprint: u64,
    root_entry_claims: &[omega_external_roots::ExternalRootEntryClaim],
    provider_plan: u64,
    provider_plan_digest: omega_effects::provider_plan::ProviderPlanDigest,
) -> Result<(), psi_layout_plans::MaterializationDiagnostic> {
    if !selected_provider.has_valid_exact_identity() {
        return Err(psi_layout_plans::MaterializationDiagnostic(
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
        return Err(psi_layout_plans::MaterializationDiagnostic(format!(
            "selected external-root provider schema retains {} rows for exact writer requirement `{requirement_identity}`",
            matches.len()
        )));
    };
    let selected_entry_claims = selected_provider
        .entry_claims(requirement_identity)
        .map_err(|diagnostic| psi_layout_plans::MaterializationDiagnostic(diagnostic.0))?;
    if selected_provider.identity.normalized_identity() != provider_plan
        || selected_provider.digest != provider_plan_digest
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
    if let Err(diagnostic) = omega_program_entry_plan::validate_lowered_post_handoff_writer(lowered)
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
    storage: &'storage omega_program_entry_plan::DerivedBoundaryEntryParameterStorage,
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
    ) -> &'storage omega_program_entry_plan::DerivedBoundaryEntryParameterStorage {
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
    fn admit_acknowledgement_handoff<'entry, 'storage, Write>(
        &self,
        acknowledgement: &'entry omega_external_roots::InterruptAcknowledgement,
        storage: &'storage omega_program_entry_plan::DerivedBoundaryEntryStorage<Write>,
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
    pub fn dispatch_checked_adapter_body<'entry, 'storage, Write, Output>(
        &self,
        acknowledgement: &'entry omega_external_roots::InterruptAcknowledgement,
        storage: &'storage omega_program_entry_plan::DerivedBoundaryEntryStorage<Write>,
        execute: impl FnOnce(AdmittedExternalRootEntryFactHandoff<'entry, 'storage>) -> Output,
    ) -> Result<Output, omega_external_roots::ExternalRootDiagnostic> {
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
    ) -> Result<Self, omega_external_roots::ExternalRootDiagnostic> {
        Ok(Self {
            identity: omega_external_roots::ProviderPlanId::from_normalized_identity(
                exact_plan.identity_fingerprint(),
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
        self.identity.normalized_identity() == self.exact_plan.identity_fingerprint()
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
        lowered: omega_program_entry_plan::LoweredPostHandoffWriter,
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
        if !self.has_valid_exact_identity()
            || self.identity != execution.provider_plan()
            || self.digest != execution.provider_plan_digest()
        {
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
            omega_program_entry_plan::validate_lowered_post_handoff_writer(&lowered)
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
            execution.provider_plan_digest(),
        ) {
            return Err(SelectedExternalRootWriterPreparationError {
                selected_provider: self,
                lowered,
                destination,
                diagnostic: omega_external_roots::ExternalRootDiagnostic(diagnostic.0),
            });
        }
        let destination = match destination.into_validated_for_writer_preparation() {
            Ok(destination) => destination,
            Err(error) => {
                let diagnostic =
                    omega_external_roots::ExternalRootDiagnostic(error.diagnostic().0.clone());
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
    ) -> Result<
        Vec<omega_external_roots::ExternalRootEntryClaim>,
        omega_external_roots::ExternalRootDiagnostic,
    > {
        if !self.has_valid_exact_identity() {
            return Err(omega_external_roots::ExternalRootDiagnostic(
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
        if !self.has_valid_exact_identity() {
            return Err(omega_external_roots::ExternalRootDiagnostic(
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

/// Exact checked-program and selected-plan candidate after every provider
/// grant, receipt, operator-use, and installation-reach decision has replayed.
/// The candidate owns any separated Arc privately until its caller commits it.
#[derive(Debug)]
pub struct SelectedProviderPlanBinding {
    program: Arc<psi_checked_trees::CheckedTrees>,
    selected: omega_effects::SelectedProviderPlanFacts,
}

impl SelectedProviderPlanBinding {
    pub fn into_parts(
        self,
    ) -> (
        Arc<psi_checked_trees::CheckedTrees>,
        omega_effects::SelectedProviderPlanFacts,
    ) {
        (self.program, self.selected)
    }
}

#[derive(Default)]
struct SelectedProviderProgramUpdates {
    spelled_operator_uses: Vec<(
        psi_arena::Handle<psi_checked_trees::CheckedOperatorUseFact>,
        u64,
    )>,
    named_operator_uses: Vec<(
        psi_arena::Handle<psi_checked_trees::CheckedNamedOperatorUseFact>,
        u64,
    )>,
    admitted_receipts: Vec<(psi_facts::FactHandle, u64)>,
}

impl SelectedProviderProgramUpdates {
    fn is_empty(&self) -> bool {
        self.spelled_operator_uses.is_empty()
            && self.named_operator_uses.is_empty()
            && self.admitted_receipts.is_empty()
    }

    fn apply(self, checked: &mut psi_checked_trees::CheckedTrees) {
        for (handle, identity) in self.spelled_operator_uses {
            checked
                .facts
                .operators
                .uses
                .get_mut(handle)
                .provider_plan_identity = identity;
        }
        for (handle, identity) in self.named_operator_uses {
            checked
                .facts
                .operators
                .named_uses
                .get_mut(handle)
                .provider_plan_identity = identity;
        }
        for (handle, identity) in self.admitted_receipts {
            checked
                .facts
                .semantic
                .facts
                .get_mut(handle)
                .evidence
                .receipt_identity = identity;
        }
    }
}

/// Build the exact Omega-owned selection sidecar and bind its stable receipt
/// identities into checked semantic evidence. Provider execution and
/// compiler-generated helper machines consume the returned carrier; neither
/// may reconstruct a plan by scanning authored `satisfies` rows.
pub fn bind_selected_provider_plan_facts(
    program: &Arc<psi_checked_trees::CheckedTrees>,
    candidates: &[ProviderPlan],
    facts: omega_effects::SelectedProviderPlanFacts,
    root_grants: &[String],
) -> Result<SelectedProviderPlanBinding, Vec<psi_diagnostics::Diagnostic>> {
    let checked = program.as_ref();
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
    let (spelled_operator_uses, named_operator_uses) =
        plan_selected_operator_provider_evidence(checked, candidates, &facts)?;
    let installation_reach_resolutions =
        derive_selected_installation_reach_resolutions(checked, &facts)?;
    let selected = facts
        .with_installation_reach_resolutions(installation_reach_resolutions)
        .map_err(|reason| vec![psi_diagnostics::Diagnostic::error(reason)])?;
    let updates = SelectedProviderProgramUpdates {
        spelled_operator_uses,
        named_operator_uses,
        admitted_receipts: receipt_updates,
    };
    let mut bound_program = Arc::clone(program);
    if !updates.is_empty() {
        updates.apply(Arc::make_mut(&mut bound_program));
    }
    Ok(SelectedProviderPlanBinding {
        program: bound_program,
        selected,
    })
}

fn derive_selected_installation_reach_resolutions(
    checked: &psi_checked_trees::CheckedTrees,
    selected: &omega_effects::SelectedProviderPlanFacts,
) -> Result<Vec<omega_effects::InstallationReachResolution>, Vec<psi_diagnostics::Diagnostic>> {
    let mut resolutions = Vec::new();
    let mut diagnostics = Vec::new();
    for plan in selected.plans() {
        // Boundary operators share the provider-plan carrier, but they are
        // compiler-owned operator slots rather than boundary-trait
        // requirements. Candidate validation has already replayed their exact
        // typed operator schema. Do not make the trait-only installation-reach
        // pass reinterpret them as missing trait requirements.
        let is_boundary_operator_plan = checked.typed.operators().iter().any(|operator| {
            omega_effects::provider_plan::ServiceSchema::from_typed_operator(
                &checked.typed,
                operator,
            )
            .as_ref()
                == Some(&plan.schema)
        });
        if is_boundary_operator_plan {
            continue;
        }
        for row in &plan.rows {
            let requirements = checked
                .typed
                .traits()
                .iter()
                .flat_map(|owner| {
                    checked
                        .typed
                        .trait_machine_signatures(owner)
                        .iter()
                        .filter(move |requirement| {
                            checked
                                .typed
                                .normalized_trait_requirement_overload_identity(owner, requirement)
                                .identity()
                                == row.requirement_identity
                        })
                        .map(move |requirement| (owner, requirement))
                })
                .collect::<Vec<_>>();
            let [(_, requirement)] = requirements.as_slice() else {
                diagnostics.push(psi_diagnostics::Diagnostic::error(format!(
                    "selected provider row `{}` resolves to {} exact typed requirements",
                    row.requirement_identity,
                    requirements.len()
                )));
                continue;
            };
            if !requirement.service_reach_is_installation_bound {
                continue;
            }

            let realization_machines = checked
                .typed
                .machines()
                .iter()
                .filter(|machine| {
                    machine
                        .attached_data
                        .as_ref()
                        .map(|name| name.as_str())
                        .unwrap_or_default()
                        == plan.provider_type
                })
                .filter(|machine| {
                    checked
                        .typed
                        .machine_trait_conformances(machine)
                        .iter()
                        .any(|conformance| {
                            conformance.requirement.as_ref().is_some_and(|name| {
                                satisfied_requirement_identity(
                                    &checked.typed,
                                    machine.name.as_str(),
                                    conformance.name.as_str(),
                                    name.as_str(),
                                ) == row.requirement_identity
                            })
                        })
                })
                .collect::<Vec<_>>();
            let [realization] = realization_machines.as_slice() else {
                diagnostics.push(psi_diagnostics::Diagnostic::error(format!(
                    "selected provider row `{}` resolves to {} exact realization machines for provider `{}`",
                    row.requirement_identity,
                    realization_machines.len(),
                    plan.provider_type
                )));
                continue;
            };
            let Some(envelope) = checked
                .facts
                .contract_plans
                .realized_envelope(realization.symbol)
            else {
                diagnostics.push(psi_diagnostics::Diagnostic::error(format!(
                    "selected provider realization `{}` has no checked contract envelope",
                    realization.name
                )));
                continue;
            };
            let upper_bound = checked
                .facts
                .service_reaches
                .rows
                .services(requirement.service_reach_row)
                .iter()
                .filter_map(|service| checked.facts.service_reaches.services.definition(*service))
                .map(|definition| definition.name.clone())
                .collect();
            resolutions.push(omega_effects::InstallationReachResolution {
                requirement_identity: row.requirement_identity.clone(),
                provider_plan_identity: plan.identity_fingerprint(),
                upper_bound,
                resolved_row: envelope.effective_service_reach.clone(),
            });
        }
    }
    if diagnostics.is_empty() {
        Ok(resolutions)
    } else {
        Err(diagnostics)
    }
}

fn plan_selected_operator_provider_evidence(
    checked: &psi_checked_trees::CheckedTrees,
    candidates: &[ProviderPlan],
    selected: &omega_effects::SelectedProviderPlanFacts,
) -> Result<
    (
        Vec<(
            psi_arena::Handle<psi_checked_trees::CheckedOperatorUseFact>,
            u64,
        )>,
        Vec<(
            psi_arena::Handle<psi_checked_trees::CheckedNamedOperatorUseFact>,
            u64,
        )>,
    ),
    Vec<psi_diagnostics::Diagnostic>,
> {
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
    let mut spelled_updates = Vec::new();
    for (handle, symbol) in spelled {
        match selected_operator_provider_identity(checked, candidates, selected, symbol) {
            Ok(Some(identity)) => spelled_updates.push((handle, identity)),
            Ok(None) => {}
            Err(diagnostic) => diagnostics.push(diagnostic),
        }
    }
    let mut named_updates = Vec::new();
    for (handle, symbol) in named {
        match selected_operator_provider_identity(checked, candidates, selected, symbol) {
            Ok(Some(identity)) => named_updates.push((handle, identity)),
            Ok(None) => {}
            Err(diagnostic) => diagnostics.push(diagnostic),
        }
    }

    if diagnostics.is_empty() {
        Ok((spelled_updates, named_updates))
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
    if let ProviderBinding::CheckedAdapter {
        machine_identity, ..
    } = &row.binding
    {
        let [namespace, requirement] = checked.typed.operator_path_members(operator.name) else {
            return Err(psi_diagnostics::Diagnostic::error(format!(
                "selected checked boundary-operator ProviderPlan `{}` targets `{slot}`, whose source path is not the supported `Namespace::requirement` shape",
                plan.name,
            )));
        };
        let checked_provider = exact_checked_adapter(&checked.typed, plan, row)?;
        let satisfies_slot = {
            checked
                .typed
                .machine_trait_conformances(checked_provider)
                .iter()
                .any(|conformance| {
                    conformance.external_binding.is_none()
                        && psi_typed_trees::operator::resolve_satisfied_checked_operator(
                            &checked.typed,
                            checked_provider,
                            namespace.as_str(),
                            requirement.as_str(),
                        )
                        .is_some_and(|resolved| resolved.symbol == operator.symbol)
                })
        };
        if !satisfies_slot {
            return Err(psi_diagnostics::Diagnostic::error(format!(
                "selected boundary-operator ProviderPlan `{}` binds checked adapter `{machine_identity}`, but that machine does not satisfy exact slot `{slot}` with a checked body",
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

pub fn intrinsic_realization_matches_operator(
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
    if let Some(CompilerIntrinsicExecutionIdentity::PrimitiveFloatBinary { operation, format }) =
        primitive_float_binary_intrinsic_execution_identity(typed, operator)
    {
        return Some(format!("Float::{}.{}", operation.name(), format.name()));
    }
    let path = typed.operator_path_members(operator.name);
    let [namespace, requirement] = path else {
        return None;
    };
    let parameters = typed.operator_parameters(operator);
    let (operation, primitive, expected_result) = match namespace.as_str() {
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
    SelectedExternalRootProviderPlan::from_exact_plan((*plan).clone())
}

/// Resolve an external-root provider selection when the current artifact has
/// one. An absent plan remains an honest pending installation dependency;
/// multiple matching selections are still invalid.
pub fn optional_selected_external_root_provider_plan(
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
        [plan] => Ok(Some(SelectedExternalRootProviderPlan::from_exact_plan(
            (*plan).clone(),
        )?)),
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
            let ProviderBinding::CheckedAdapter {
                machine_identity, ..
            } = &row.binding
            else {
                return Err(omega_external_roots::ExternalRootDiagnostic(format!(
                    "selected external-root routed requirement `{}::{}` has no checked adapter fact to bind",
                    method.requirement_owner, method.name
                )));
            };
            let implementation =
                exact_checked_adapter(&checked.typed, plan, row).map_err(|diagnostic| {
                    omega_external_roots::ExternalRootDiagnostic(diagnostic.message)
                })?;
            let state = checked
                .typed
                .machine_states(implementation)
                .first()
                .ok_or_else(|| {
                    omega_external_roots::ExternalRootDiagnostic(format!(
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
                omega_external_roots::ExternalRootDiagnostic(format!(
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

fn provider_type_package_identity(
    typed: &TypedTrees,
    machine: &psi_typed_trees::machine::Machine,
) -> Option<psi_core::PackageKeyIdentity> {
    provider_type_symbol(typed, machine)
        .and_then(|symbol| typed.symbols.symbol_package_identity(symbol))
}

fn provider_type_symbol(
    typed: &TypedTrees,
    machine: &psi_typed_trees::machine::Machine,
) -> Option<psi_symbols::SymbolHandle> {
    let attached_data = machine.attached_data.as_ref()?;
    let mut owners = typed
        .data_definitions()
        .iter()
        .filter(|definition| definition.name == *attached_data);
    let owner = owners.next()?;
    owners.next().is_none().then_some(owner.symbol)
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
pub fn derive_satisfies_plans(
    typed: &TypedTrees,
    selected_target: Option<&str>,
) -> Vec<ProviderPlan> {
    derive_satisfies_plans_with_provenance(typed, selected_target)
        .into_iter()
        .map(|derived| derived.plan)
        .collect()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProviderSchemaDeclaration {
    BoundaryTrait(psi_symbols::SymbolHandle),
    BoundaryOperator(psi_symbols::SymbolHandle),
}

impl ProviderSchemaDeclaration {
    pub const fn symbol(self) -> psi_symbols::SymbolHandle {
        match self {
            Self::BoundaryTrait(symbol) | Self::BoundaryOperator(symbol) => symbol,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProviderPlanProvenance {
    pub schema: ProviderSchemaDeclaration,
    pub provider_type: Option<psi_symbols::SymbolHandle>,
    pub row_requirements: Vec<psi_symbols::SymbolHandle>,
    pub row_realizations: Vec<psi_symbols::SymbolHandle>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DerivedProviderPlan {
    pub plan: ProviderPlan,
    pub provenance: ProviderPlanProvenance,
}

pub fn derive_satisfies_plans_with_provenance(
    typed: &TypedTrees,
    selected_target: Option<&str>,
) -> Vec<DerivedProviderPlan> {
    let mut plans: Vec<DerivedProviderPlan> = Vec::new();
    // Target filtering has already admitted only unscoped and selected-target
    // machines into typed trees. Derive from their exact retained conformance
    // and supply identities; source syntax is no longer a binding authority.
    for machine in typed.machines() {
        let origin_package_identity = typed.symbols.symbol_package_identity(machine.symbol);
        let provider_type_package_identity = provider_type_package_identity(typed, machine);
        let provider_type_symbol = provider_type_symbol(typed, machine);
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
            let provider_type = machine
                .attached_data
                .as_ref()
                .map(|name| name.as_str().to_owned())
                .unwrap_or_default();
            let row_binding = match binding {
                None => ProviderBinding::CheckedAdapter {
                    machine_identity: typed
                        .normalized_machine_overload_identity(machine)
                        .map(|identity| identity.identity())
                        .unwrap_or_default(),
                    machine_package_identity: typed.symbols.symbol_package_identity(machine.symbol),
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
                clause.symbol,
                clause.requirement_symbol,
            );
            let requirement_symbol = clause.requirement_symbol;
            for (schema_declaration, schema_trait, schema) in provider_plan_schema_targets(
                typed,
                &provider_type,
                provider_type_symbol,
                clause.symbol,
                &semantic_requirement_identity,
            ) {
                let plan_name = satisfies_plan_name(&target, &schema_trait, &provider_type);
                let position = plans
                    .iter()
                    .position(|derived| {
                        derived.plan.name == plan_name
                            && derived.plan.provider_type_package_identity
                                == provider_type_package_identity
                            && derived.plan.origin_package_identity == origin_package_identity
                            && derived.provenance.schema == schema_declaration
                    })
                    .unwrap_or_else(|| {
                        plans.push(DerivedProviderPlan {
                            plan: ProviderPlan {
                                name: plan_name.clone(),
                                provider_type: provider_type.clone(),
                                provider_type_package_identity,
                                target: target.clone(),
                                schema,
                                rows: Vec::new(),
                                origin_package_identity,
                                origin_package: String::new(),
                            },
                            provenance: ProviderPlanProvenance {
                                schema: schema_declaration,
                                provider_type: provider_type_symbol,
                                row_requirements: Vec::new(),
                                row_realizations: Vec::new(),
                            },
                        });
                        plans.len() - 1
                    });
                debug_assert_eq!(plans[position].provenance.schema, schema_declaration);
                debug_assert_eq!(
                    plans[position].provenance.provider_type,
                    provider_type_symbol
                );
                plans[position].plan.rows.push(ProviderPlanRow {
                    method: requirement.as_str().to_owned(),
                    requirement_identity: requirement_identity.clone(),
                    binding: row_binding.clone(),
                });
                plans[position]
                    .provenance
                    .row_requirements
                    .push(requirement_symbol);
                plans[position]
                    .provenance
                    .row_realizations
                    .push(machine.symbol);
            }
        }
    }
    plans.extend(derive_boundary_operator_plans_with_provenance(
        typed,
        selected_target,
    ));
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
    provider_type_symbol: Option<psi_symbols::SymbolHandle>,
    satisfied_trait_symbol: psi_symbols::SymbolHandle,
    requirement_identity: &str,
) -> Vec<(ProviderSchemaDeclaration, String, ServiceSchema)> {
    let direct = typed
        .traits()
        .iter()
        .find(|definition| definition.is_boundary && definition.symbol == satisfied_trait_symbol);

    let mut refined = typed
        .conformances()
        .iter()
        .filter(|conformance| Some(conformance.carrier_symbol) == provider_type_symbol)
        .filter_map(|conformance| {
            let definition = typed.traits().iter().find(|definition| {
                definition.is_boundary && definition.symbol == conformance.trait_symbol
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
                .then(|| {
                    (
                        ProviderSchemaDeclaration::BoundaryTrait(definition.symbol),
                        definition.name.as_str().to_owned(),
                        schema,
                    )
                })
        })
        .collect::<Vec<_>>();
    refined.sort_by(|left, right| left.1.cmp(&right.1));
    refined.dedup_by(|left, right| left.0 == right.0);

    let has_descendant = refined.iter().any(|(schema, _, _)| {
        direct.is_some_and(|definition| schema.symbol() != definition.symbol)
    });
    if has_descendant {
        refined.retain(|(schema, _, _)| {
            direct.is_none_or(|definition| schema.symbol() != definition.symbol)
        });
    }
    if !refined.is_empty() {
        return refined;
    }

    direct
        .and_then(|definition| {
            let arguments = provider_boundary_arguments(typed, definition, provider_type);
            ServiceSchema::from_typed_instance(typed, definition, &arguments).map(|schema| {
                (
                    ProviderSchemaDeclaration::BoundaryTrait(definition.symbol),
                    definition.name.as_str().to_owned(),
                    schema,
                )
            })
        })
        .into_iter()
        .collect()
}

fn derive_boundary_operator_plans_with_provenance(
    typed: &TypedTrees,
    selected_target: Option<&str>,
) -> Vec<DerivedProviderPlan> {
    let mut plans = Vec::<DerivedProviderPlan>::new();
    for machine in typed.machines() {
        let origin_package_identity = typed.symbols.symbol_package_identity(machine.symbol);
        let provider_type_package_identity = provider_type_package_identity(typed, machine);
        let provider_type_symbol = provider_type_symbol(typed, machine);
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
                        machine_identity: typed
                            .normalized_machine_overload_identity(machine)
                            .map(|identity| identity.identity())
                            .unwrap_or_default(),
                        machine_package_identity: typed
                            .symbols
                            .symbol_package_identity(machine.symbol),
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
                .position(|derived| {
                    derived.plan.name == plan_name
                        && derived.plan.provider_type_package_identity
                            == provider_type_package_identity
                        && derived.plan.origin_package_identity == origin_package_identity
                        && derived.provenance.schema
                            == ProviderSchemaDeclaration::BoundaryOperator(operator.symbol)
                })
                .unwrap_or_else(|| {
                    plans.push(DerivedProviderPlan {
                        plan: ProviderPlan {
                            name: plan_name.clone(),
                            provider_type: provider_type.clone(),
                            provider_type_package_identity,
                            target: target.clone(),
                            schema: schema.clone(),
                            rows: Vec::new(),
                            origin_package_identity,
                            origin_package: String::new(),
                        },
                        provenance: ProviderPlanProvenance {
                            schema: ProviderSchemaDeclaration::BoundaryOperator(operator.symbol),
                            provider_type: provider_type_symbol,
                            row_requirements: Vec::new(),
                            row_realizations: Vec::new(),
                        },
                    });
                    plans.len() - 1
                });
            debug_assert_eq!(
                plans[position].provenance.schema,
                ProviderSchemaDeclaration::BoundaryOperator(operator.symbol)
            );
            debug_assert_eq!(
                plans[position].provenance.provider_type,
                provider_type_symbol
            );
            plans[position].plan.rows.push(ProviderPlanRow {
                method: "realize".to_owned(),
                requirement_identity: schema.methods[0].requirement_identity.clone(),
                binding,
            });
            plans[position]
                .provenance
                .row_requirements
                .push(operator.symbol);
            plans[position]
                .provenance
                .row_realizations
                .push(machine.symbol);
        }
    }
    plans
}

pub fn satisfied_requirement_identity(
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
    trait_symbol: psi_symbols::SymbolHandle,
    requirement_symbol: psi_symbols::SymbolHandle,
) -> String {
    let Some(definition) = typed
        .traits()
        .iter()
        .find(|definition| definition.symbol == trait_symbol)
    else {
        return String::new();
    };
    typed
        .trait_machine_signatures(definition)
        .iter()
        .find(|signature| signature.symbol == requirement_symbol)
        .map(|signature| {
            typed
                .normalized_trait_requirement_overload_identity(definition, signature)
                .identity()
        })
        .unwrap_or_default()
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
        ExternalBindingIdentity::Import { library, symbol } => {
            ProviderBinding::StringBackedImportBootstrap {
                library: library.clone(),
                symbol: symbol.clone(),
            }
        }
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
pub fn satisfies_plan_name(target: &str, trait_name: &str, provider_type: &str) -> String {
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

pub fn exact_checked_adapter<'typed>(
    typed: &'typed TypedTrees,
    plan: &ProviderPlan,
    row: &ProviderPlanRow,
) -> Result<&'typed psi_typed_trees::machine::Machine, psi_diagnostics::Diagnostic> {
    let ProviderBinding::CheckedAdapter {
        machine_identity,
        machine_package_identity,
    } = &row.binding
    else {
        return Err(psi_diagnostics::Diagnostic::error(format!(
            "ProviderPlan `{}` row `{}` is not a checked-adapter binding",
            plan.name, row.requirement_identity,
        )));
    };
    if machine_identity.is_empty() {
        return Err(psi_diagnostics::Diagnostic::error(format!(
            "checked adapter for ProviderPlan `{}` row `{}` has no complete machine identity",
            plan.name, row.requirement_identity,
        )));
    }
    if *machine_package_identity != plan.origin_package_identity {
        return Err(psi_diagnostics::Diagnostic::error(format!(
            "checked adapter `{machine_identity}` for ProviderPlan `{}` does not belong to the package realizing the plan",
            plan.name,
        )));
    }
    let identity_matches = typed
        .machines()
        .iter()
        .filter(|candidate| {
            typed
                .normalized_machine_overload_identity(candidate)
                .is_some_and(|identity| identity.identity() == *machine_identity)
        })
        .collect::<Vec<_>>();
    let matches = identity_matches
        .iter()
        .copied()
        .filter(|candidate| {
            typed.symbols.symbol_package_identity(candidate.symbol) == *machine_package_identity
        })
        .collect::<Vec<_>>();
    let adapter = match matches.as_slice() {
        [adapter] => *adapter,
        [] if identity_matches.is_empty() => {
            return Err(psi_diagnostics::Diagnostic::error(format!(
                "checked adapter `{machine_identity}` for `{}::{}` is absent from typed machines",
                plan.schema.trait_name, row.method,
            )));
        }
        [] => {
            return Err(psi_diagnostics::Diagnostic::error(format!(
                "checked adapter `{machine_identity}` for ProviderPlan `{}` does not belong to its retained package identity",
                plan.name,
            )));
        }
        _ => {
            return Err(psi_diagnostics::Diagnostic::error(format!(
                "checked adapter `{machine_identity}` for ProviderPlan `{}` row `{}` resolves to {} exact typed machines",
                plan.name,
                row.requirement_identity,
                matches.len(),
            )));
        }
    };
    if !adapter.symbol.is_valid() {
        return Err(psi_diagnostics::Diagnostic::error(format!(
            "checked adapter `{machine_identity}` for ProviderPlan `{}` has no exact typed machine symbol",
            plan.name,
        )));
    }
    let actual_package_identity = typed.symbols.symbol_package_identity(adapter.symbol);
    if actual_package_identity != *machine_package_identity {
        return Err(psi_diagnostics::Diagnostic::error(format!(
            "checked adapter `{machine_identity}` for ProviderPlan `{}` does not belong to its retained package identity",
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
) -> Result<Vec<String>, psi_diagnostics::Diagnostic> {
    let ProviderBinding::CheckedAdapter {
        machine_identity, ..
    } = &row.binding
    else {
        return Err(psi_diagnostics::Diagnostic::error(format!(
            "ProviderPlan `{}` row `{}` is not a checked-adapter binding",
            plan.name, row.requirement_identity,
        )));
    };
    let adapter = exact_checked_adapter(typed, plan, row)?;
    let summaries = inferred
        .machines
        .iter()
        .filter(|summary| summary.machine == adapter.symbol)
        .collect::<Vec<_>>();
    let [summary] = summaries.as_slice() else {
        return Err(psi_diagnostics::Diagnostic::error(format!(
            "checked adapter `{machine_identity}` resolves to {} exact synchronous-invocation inference summaries",
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
pub fn validate_provider_plan_candidates(
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
            let ProviderBinding::CheckedAdapter {
                machine_identity, ..
            } = &row.binding
            else {
                continue;
            };
            let method = match exact_schema_method_for_row(plan, row) {
                Ok(method) => method,
                Err(diagnostic) => {
                    diagnostics.push(diagnostic);
                    continue;
                }
            };
            let adapter = match exact_checked_adapter(typed, plan, row) {
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
                    "checked adapter `{machine_identity}` for `{}::{}` belongs to provider `{}`, not selected provider `{}`",
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
                    "checked adapter `{machine_identity}` for `{}::{}` does not name a checked body with an entry state",
                    plan.schema.trait_name, row.method,
                )));
                continue;
            }
            let has_exact_conformance =
                checked_adapter_has_exact_conformance(typed, adapter, plan, row);
            if !has_exact_conformance {
                diagnostics.push(psi_diagnostics::Diagnostic::error(format!(
                    "checked adapter `{machine_identity}` for `{}::{}` has no exact checked satisfies edge for requirement identity `{}`",
                    plan.schema.trait_name, row.method, row.requirement_identity,
                )));
                continue;
            }
            let service_ceiling = method.service_reach.as_slice();
            let invocation_ceiling = method.synchronous_invocations.as_slice();
            let hidden_invocations =
                match exact_checked_adapter_invocations(typed, &invocation_plan, plan, method, row)
                {
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
                    machine_identity,
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
                    machine_identity,
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
pub fn validate_selected_synchronous_invocation_cycles(
    typed: &TypedTrees,
    selected_plans: &[omega_effects::provider_plan::ProviderPlan],
) -> Result<(), Vec<psi_diagnostics::Diagnostic>> {
    let selected = exact_selected_synchronous_plans(selected_plans)?;
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
                ProviderBinding::CheckedAdapter { .. } => {
                    exact_checked_adapter_invocations(typed, &inferred, source, method, row)
                }
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
                let matching_targets = selected
                    .iter()
                    .enumerate()
                    .filter(|(_, target)| target.schema.trait_name == target_name)
                    .collect::<Vec<_>>();
                match matching_targets.as_slice() {
                    [] => {}
                    [(target_index, _)] if !edges[source_index].contains(target_index) => {
                        edges[source_index].push(*target_index);
                    }
                    [_] => {}
                    _ => diagnostics.push(psi_diagnostics::Diagnostic::error(format!(
                        "selected synchronous invocation `{target_name}` from `{}::{}` is ambiguous across {} package-qualified boundary slots",
                        source.schema.trait_name,
                        method.name,
                        matching_targets.len(),
                    ))),
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
    selected_plans: &'plans [ProviderPlan],
) -> Result<Vec<&'plans ProviderPlan>, Vec<psi_diagnostics::Diagnostic>> {
    let mut selected = Vec::new();
    let mut diagnostics = Vec::new();
    let mut seen_plans = Vec::new();
    for plan in selected_plans {
        if plan.name.is_empty() {
            diagnostics.push(psi_diagnostics::Diagnostic::error(
                "selected synchronous-invocation ProviderPlan name is empty",
            ));
            continue;
        }
        if seen_plans.contains(&plan) {
            diagnostics.push(psi_diagnostics::Diagnostic::error(format!(
                "selected synchronous-invocation ProviderPlan `{}` is listed more than once",
                plan.name,
            )));
            continue;
        }
        seen_plans.push(plan);
        selected.push(plan);
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
        let schema_identity = (
            plan.schema.trait_package_identity,
            plan.schema.trait_name.as_str(),
        );
        if seen_schemas.contains(&schema_identity) {
            diagnostics.push(psi_diagnostics::Diagnostic::error(format!(
                "selected synchronous-invocation schema `{}` is realized by more than one selected ProviderPlan",
                plan.schema.trait_name,
            )));
            continue;
        }
        seen_schemas.push(schema_identity);
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

type ProviderSelectionKey = (Option<psi_core::PackageKeyIdentity>, String);

fn provider_slot_key(plan: &omega_effects::provider_plan::ProviderPlan) -> ProviderSelectionKey {
    (
        plan.schema.trait_package_identity,
        plan.schema.trait_name.clone(),
    )
}

fn selected_subject_keys(selection: &crate::ProviderSelection) -> Vec<ProviderSelectionKey> {
    match &selection.subject {
        crate::ProviderSelectionSubject::BoundaryTrait(identity) => {
            vec![(identity.package, identity.canonical_path.clone())]
        }
        crate::ProviderSelectionSubject::BoundaryOperatorFamily(family) => family
            .coordinates()
            .iter()
            .map(|coordinate| (family.package, coordinate.requirement_identity.clone()))
            .collect(),
    }
}

fn selected_provider_key(selection: &crate::ProviderSelection) -> ProviderSelectionKey {
    (
        selection.provider_type.package,
        selection.provider_type.canonical_path.clone(),
    )
}

fn provider_plan_key(plan: &omega_effects::provider_plan::ProviderPlan) -> ProviderSelectionKey {
    (
        plan.provider_type_package_identity,
        plan.provider_type.clone(),
    )
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProviderSelectionProvenance {
    BuildOverride(Vec<crate::ProviderSelection>),
    TargetDefault(Vec<crate::ProviderSelection>),
    UniqueCoveringCandidate,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SelectedProviderPlanWithProvenance {
    pub derived: DerivedProviderPlan,
    pub selected_by: ProviderSelectionProvenance,
}

/// Closed compiler-owned execution child retained independently of the
/// authored realization-machine declaration.
///
/// This vocabulary is intentionally finite. A checked compiler intrinsic that
/// cannot be represented here is not package-reviewable yet.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CompilerNumericType {
    I8,
    I16,
    I32,
    I64,
    U8,
    U16,
    U32,
    U64,
    F32,
    F64,
}

impl CompilerNumericType {
    pub const fn from_primitive(primitive: psi_typed_trees::types::PrimitiveType) -> Option<Self> {
        use psi_typed_trees::types::PrimitiveType;

        match primitive {
            PrimitiveType::I8 => Some(Self::I8),
            PrimitiveType::I16 => Some(Self::I16),
            PrimitiveType::I32 => Some(Self::I32),
            PrimitiveType::I64 => Some(Self::I64),
            PrimitiveType::U8 => Some(Self::U8),
            PrimitiveType::U16 => Some(Self::U16),
            PrimitiveType::U32 => Some(Self::U32),
            PrimitiveType::U64 => Some(Self::U64),
            PrimitiveType::F32 => Some(Self::F32),
            PrimitiveType::F64 => Some(Self::F64),
            PrimitiveType::Bool | PrimitiveType::Addr => None,
        }
    }

    pub const fn name(self) -> &'static str {
        match self {
            Self::I8 => "i8",
            Self::I16 => "i16",
            Self::I32 => "i32",
            Self::I64 => "i64",
            Self::U8 => "u8",
            Self::U16 => "u16",
            Self::U32 => "u32",
            Self::U64 => "u64",
            Self::F32 => "f32",
            Self::F64 => "f64",
        }
    }

    pub const fn is_float(self) -> bool {
        matches!(self, Self::F32 | Self::F64)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CompilerIntrinsicExecutionIdentity {
    BuiltinFunction(psi_symbols::BuiltinFunction),
    PrimitiveFloatBinary {
        operation: CompilerPrimitiveFloatBinaryOperation,
        format: psi_numerics::literals::FloatFormat,
    },
    NamedFloatNegation(psi_numerics::literals::FloatFormat),
    NamedFloatConversion {
        source: CompilerNumericType,
        target: CompilerNumericType,
        domain: psi_numerics::arithmetic::ArithmeticDomain,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SelectedProviderReviewProvenance {
    pub plan: ProviderPlan,
    pub provider: ProviderPlanProvenance,
    pub selected_by: ProviderSelectionProvenance,
    /// Closed compiler-owned execution identity for each provider row.
    ///
    /// Selection initially leaves this empty because exact execution is not
    /// settled until after checking. The compiler must replace it with one
    /// row-aligned entry per plan row before publishing `CheckedCompilation`.
    /// `Some` is reserved for compiler-intrinsic rows whose selected
    /// execution has a closed identity; all other rows retain `None`.
    pub row_compiler_intrinsic_executions: Vec<Option<CompilerIntrinsicExecutionIdentity>>,
}

pub fn selected_provider_plan_facts_with_provenance(
    typed: &TypedTrees,
    mut selected: Vec<SelectedProviderPlanWithProvenance>,
) -> Result<
    (
        omega_effects::SelectedProviderPlanFacts,
        Vec<SelectedProviderReviewProvenance>,
    ),
    Vec<psi_diagnostics::Diagnostic>,
> {
    selected.sort_by(|left, right| {
        let left = &left.derived.plan;
        let right = &right.derived.plan;
        left.name
            .cmp(&right.name)
            .then_with(|| {
                left.origin_package_identity
                    .cmp(&right.origin_package_identity)
            })
            .then_with(|| {
                left.provider_type_package_identity
                    .cmp(&right.provider_type_package_identity)
            })
            .then_with(|| {
                left.schema
                    .trait_package_identity
                    .cmp(&right.schema.trait_package_identity)
            })
            .then_with(|| {
                left.identity_fingerprint()
                    .cmp(&right.identity_fingerprint())
            })
    });

    let mut diagnostics = Vec::new();
    for selected_plan in &selected {
        let plan = &selected_plan.derived.plan;
        let provenance = &selected_plan.derived.provenance;
        if provenance.row_realizations.len() != plan.rows.len() {
            diagnostics.push(psi_diagnostics::Diagnostic::error(format!(
                "selected provider plan `{}` has {} semantic rows but {} retained realization symbols",
                plan.name,
                plan.rows.len(),
                provenance.row_realizations.len(),
            )));
        }
        if provenance.row_requirements.len() != plan.rows.len() {
            diagnostics.push(psi_diagnostics::Diagnostic::error(format!(
                "selected provider plan `{}` has {} semantic rows but {} retained requirement symbols",
                plan.name,
                plan.rows.len(),
                provenance.row_requirements.len(),
            )));
        }
        let schema_symbol = provenance.schema.symbol();
        let schema_is_exact = match provenance.schema {
            ProviderSchemaDeclaration::BoundaryTrait(symbol) => typed
                .traits()
                .iter()
                .any(|definition| definition.symbol == symbol && definition.is_boundary),
            ProviderSchemaDeclaration::BoundaryOperator(symbol) => typed
                .operators()
                .iter()
                .any(|operator| operator.symbol == symbol && operator.is_boundary),
        };
        if !schema_is_exact {
            diagnostics.push(psi_diagnostics::Diagnostic::error(format!(
                "selected provider plan `{}` has no exact retained boundary schema symbol",
                plan.name,
            )));
        }
        match (plan.provider_type.is_empty(), provenance.provider_type) {
            (true, None) => {}
            (false, Some(symbol))
                if typed
                    .data_definitions()
                    .iter()
                    .any(|definition| definition.symbol == symbol) => {}
            _ => diagnostics.push(psi_diagnostics::Diagnostic::error(format!(
                "selected provider plan `{}` has no exact retained nominal provider declaration",
                plan.name,
            ))),
        }
        for realization in &provenance.row_realizations {
            if !typed
                .machines()
                .iter()
                .any(|machine| machine.symbol == *realization)
            {
                diagnostics.push(psi_diagnostics::Diagnostic::error(format!(
                    "selected provider plan `{}` has a row without its exact realizing machine",
                    plan.name,
                )));
            }
        }
        for (row, requirement) in plan.rows.iter().zip(&provenance.row_requirements) {
            let trait_requirement_is_exact = typed.traits().iter().any(|definition| {
                typed
                    .trait_machine_signatures(definition)
                    .iter()
                    .any(|signature| {
                        signature.symbol == *requirement
                            && typed
                                .normalized_trait_requirement_overload_identity(
                                    definition, signature,
                                )
                                .identity()
                                == row.requirement_identity
                    })
            });
            let boundary_operator_is_exact = typed.operators().iter().any(|operator| {
                operator.is_boundary
                    && operator.symbol == *requirement
                    && psi_typed_trees::operator::boundary_operator_requirement_identity(
                        typed, operator,
                    ) == row.requirement_identity
            });
            if !trait_requirement_is_exact && !boundary_operator_is_exact {
                diagnostics.push(psi_diagnostics::Diagnostic::error(format!(
                    "selected provider plan `{}` has a row without its exact requirement declaration",
                    plan.name,
                )));
            }
        }
        let declarations = match &selected_plan.selected_by {
            ProviderSelectionProvenance::BuildOverride(declarations)
            | ProviderSelectionProvenance::TargetDefault(declarations) => declarations,
            ProviderSelectionProvenance::UniqueCoveringCandidate => continue,
        };
        if declarations.is_empty() {
            diagnostics.push(psi_diagnostics::Diagnostic::error(format!(
                "selected provider plan `{}` has an authored selection origin without a declaration",
                plan.name,
            )));
        }
        for declaration in declarations {
            let selecting_source = typed
                .symbols
                .symbol_provenance_source_span(declaration.selecting_machine);
            if !declaration
                .subject
                .selects_schema(schema_symbol, &plan.schema.trait_name)
                || Some(declaration.provider_type.symbol) != provenance.provider_type
                || selecting_source.is_none_or(|source| {
                    source.source_id != declaration.source_span.source_id
                        || declaration.source_span.span.start >= declaration.source_span.span.end
                })
            {
                diagnostics.push(psi_diagnostics::Diagnostic::error(format!(
                    "selected provider plan `{}` has a selection declaration outside its exact schema, provider, or selecting-machine provenance",
                    plan.name,
                )));
            }
        }
    }
    if !diagnostics.is_empty() {
        return Err(diagnostics);
    }

    let plans = selected
        .iter()
        .map(|selected| selected.derived.plan.clone())
        .collect::<Vec<_>>();
    let facts = omega_effects::SelectedProviderPlanFacts::from_selected_plans(plans.clone())
        .map_err(|reason| vec![psi_diagnostics::Diagnostic::error(reason)])?;
    if facts.plans() != plans
        || plans.iter().any(|plan| {
            facts
                .plan_by_exact_evidence(plan.identity_fingerprint(), plan)
                .is_none()
        })
    {
        return Err(vec![psi_diagnostics::Diagnostic::error(
            "selected provider semantic facts do not retain the exact plans aligned with provenance",
        )]);
    }
    let provenance = selected
        .into_iter()
        .map(|selected| SelectedProviderReviewProvenance {
            plan: selected.derived.plan,
            provider: selected.derived.provenance,
            selected_by: selected.selected_by,
            row_compiler_intrinsic_executions: Vec::new(),
        })
        .collect();
    Ok((facts, provenance))
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct SelectedProviderPlanIndex {
    candidate: usize,
    selected_by: ProviderSelectionProvenance,
}

fn resolve_provider_selection_slots(
    slot_keys: &[ProviderSelectionKey],
    declarations: &[crate::ProviderSelection],
    owner: &str,
) -> (
    Vec<(ProviderSelectionKey, crate::ProviderSelection)>,
    Vec<psi_diagnostics::Diagnostic>,
) {
    let mut resolved = Vec::new();
    let mut diagnostics = Vec::new();
    for declaration in declarations {
        for slot_key in selected_subject_keys(declaration) {
            if slot_keys.contains(&slot_key) {
                resolved.push((slot_key, declaration.clone()));
            } else {
                let message = match &declaration.subject {
                    crate::ProviderSelectionSubject::BoundaryTrait(identity) => format!(
                        "{owner} selects provider `{}` for unknown boundary slot `{}`; the slot must exist in the loaded dependency closure",
                        declaration.provider_type.authored_path, identity.authored_path,
                    ),
                    crate::ProviderSelectionSubject::BoundaryOperatorFamily(_) => format!(
                        "{owner} selects provider `{}` for unknown boundary coordinate `{}` in subject `{}`; every selected coordinate must exist in the loaded dependency closure",
                        declaration.provider_type.authored_path,
                        slot_key.1,
                        declaration.subject.authored_path(),
                    ),
                };
                diagnostics.push(psi_diagnostics::Diagnostic::error(message));
            }
        }
    }
    (resolved, diagnostics)
}

/// PRV4c: select one fully covering provider type per applicable boundary
/// slot. An explicit build-root declaration wins over the selected target
/// package's ordinary default declaration. Without either, a unique covering
/// candidate supplies the declaration-era default. Rows are never selected
/// individually and partial candidates never combine.
pub fn select_provider_plans(
    plans: &[omega_effects::provider_plan::ProviderPlan],
    selected_target: omega_target::NativeTarget,
    defaults: &[crate::ProviderSelection],
    requested: &[crate::ProviderSelection],
) -> Result<Vec<ProviderPlan>, Vec<psi_diagnostics::Diagnostic>> {
    select_provider_plan_indices(plans, selected_target, defaults, requested).map(|selected| {
        selected
            .into_iter()
            .map(|selected| plans[selected.candidate].clone())
            .collect()
    })
}

pub fn select_provider_plans_with_provenance(
    derived: &[DerivedProviderPlan],
    selected_target: omega_target::NativeTarget,
    defaults: &[crate::ProviderSelection],
    requested: &[crate::ProviderSelection],
) -> Result<Vec<SelectedProviderPlanWithProvenance>, Vec<psi_diagnostics::Diagnostic>> {
    let plans = derived
        .iter()
        .map(|derived| derived.plan.clone())
        .collect::<Vec<_>>();
    select_provider_plan_indices(&plans, selected_target, defaults, requested).map(|selected| {
        selected
            .into_iter()
            .map(|selected| SelectedProviderPlanWithProvenance {
                derived: derived[selected.candidate].clone(),
                selected_by: selected.selected_by,
            })
            .collect()
    })
}

fn select_provider_plan_indices(
    plans: &[omega_effects::provider_plan::ProviderPlan],
    selected_target: omega_target::NativeTarget,
    defaults: &[crate::ProviderSelection],
    requested: &[crate::ProviderSelection],
) -> Result<Vec<SelectedProviderPlanIndex>, Vec<psi_diagnostics::Diagnostic>> {
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
    let mut slot_keys: Vec<ProviderSelectionKey> = plans
        .iter()
        .filter(|plan| !plan.schema.methods.is_empty())
        .map(provider_slot_key)
        .collect();
    slot_keys.sort_unstable();
    slot_keys.dedup();

    // Provider selections arrive after ordinary name resolution. Preserve
    // that exact nominal identity: readable paths are diagnostic material and
    // may never repair or approximate a package-qualified identity.
    let (resolved_requests, request_diagnostics) =
        resolve_provider_selection_slots(&slot_keys, requested, "build");
    diagnostics.extend(request_diagnostics);
    let (resolved_defaults, default_diagnostics) =
        resolve_provider_selection_slots(&slot_keys, defaults, "target package");
    diagnostics.extend(default_diagnostics);
    for slot_key in &slot_keys {
        let declarations = resolved_requests
            .iter()
            .filter(|(slot, _)| slot == slot_key)
            .map(|(_, declaration)| declaration)
            .collect::<Vec<_>>();
        if declarations.len() > 1 {
            let slot_name = &slot_key.1;
            diagnostics.push(psi_diagnostics::Diagnostic::error(format!(
                "build declares provider selection for slot `{slot_name}` more than once: {}",
                declarations
                    .iter()
                    .map(|declaration| format!(
                        "`{} -> {}`",
                        declaration.subject.authored_path(),
                        declaration.provider_type.authored_path,
                    ))
                    .collect::<Vec<_>>()
                    .join(", "),
            )));
        }
    }

    for slot_key in slot_keys {
        let slot_name = &slot_key.1;
        let explicit = resolved_requests
            .iter()
            .find(|(slot, _)| slot == &slot_key)
            .map(|(_, selection)| selection);
        let slot_defaults: Vec<_> = resolved_defaults
            .iter()
            .filter(|(slot, _)| slot == &slot_key)
            .map(|(_, selection)| selection)
            .collect();
        let candidates: Vec<(usize, &ProviderPlan)> = plans
            .iter()
            .enumerate()
            .filter(|(_, plan)| provider_slot_key(plan) == slot_key && applies(&plan.target))
            .collect();
        let covering: Vec<(usize, &ProviderPlan)> = candidates
            .iter()
            .copied()
            .filter(|(_, plan)| plan.covers_schema())
            .collect();

        let selected_declaration = if let Some(explicit) = explicit {
            // A slot-owner override intentionally replaces every target
            // default for this slot, including a default whose provider is
            // absent from the selected dependency closure.
            Some((
                "build",
                explicit,
                ProviderSelectionProvenance::BuildOverride(vec![explicit.clone()]),
            ))
        } else if let Some(first) = slot_defaults.first().copied() {
            let mut distinct_provider_types: Vec<ProviderSelectionKey> = slot_defaults
                .iter()
                .map(|selection| selected_provider_key(selection))
                .collect();
            distinct_provider_types.sort_unstable();
            distinct_provider_types.dedup();
            if distinct_provider_types.len() > 1 {
                diagnostics.push(psi_diagnostics::Diagnostic::error(format!(
                    "slot `{slot_name}` has conflicting target-package defaults: {} -- a target supplies at most one default provider type per slot",
                    distinct_provider_types
                        .iter()
                        .map(|(_, provider)| format!("`{provider}`"))
                        .collect::<Vec<_>>()
                        .join(", "),
                )));
                continue;
            }
            Some((
                "target package",
                first,
                ProviderSelectionProvenance::TargetDefault(
                    slot_defaults
                        .iter()
                        .map(|selection| (*selection).clone())
                        .collect(),
                ),
            ))
        } else {
            None
        };

        if let Some((owner, declaration, selected_by)) = selected_declaration {
            let selected_provider = selected_provider_key(declaration);
            let matching: Vec<(usize, &ProviderPlan)> = candidates
                .iter()
                .copied()
                .filter(|(_, plan)| provider_plan_key(plan) == selected_provider)
                .collect();
            match matching.as_slice() {
                [(candidate, plan)] if plan.covers_schema() => {
                    selected.push(SelectedProviderPlanIndex {
                        candidate: *candidate,
                        selected_by,
                    });
                }
                [(_, plan)] => diagnostics.push(psi_diagnostics::Diagnostic::error(format!(
                    "{owner} selects provider `{}` for slot `{slot_name}`, but candidate `{}` is partial ({}/{}) and cannot be selected",
                    declaration.provider_type.authored_path,
                    plan.name,
                    plan.rows.len(),
                    plan.schema.methods.len(),
                ))),
                [] => {
                    let wrong_target = plans.iter().any(|plan| {
                        provider_slot_key(plan) == slot_key
                            && provider_plan_key(plan) == selected_provider
                    });
                    diagnostics.push(psi_diagnostics::Diagnostic::error(format!(
                        "{owner} selects provider `{}` for slot `{slot_name}`, but no {}candidate exists in the loaded dependency closure",
                        declaration.provider_type.authored_path,
                        if wrong_target { "selected-target " } else { "" },
                    )));
                }
                _ => diagnostics.push(psi_diagnostics::Diagnostic::error(format!(
                    "{owner} selection `{}` for slot `{slot_name}` resolves to multiple provider candidates with the same exact identity",
                    declaration.provider_type.authored_path,
                ))),
            }
            continue;
        }

        match covering.as_slice() {
            [] => {}
            [(candidate, _)] => selected.push(SelectedProviderPlanIndex {
                candidate: *candidate,
                selected_by: ProviderSelectionProvenance::UniqueCoveringCandidate,
            }),
            many => {
                let count = if many.len() == 2 {
                    "two".to_owned()
                } else {
                    many.len().to_string()
                };
                diagnostics.push(psi_diagnostics::Diagnostic::error(format!(
                    "slot `{slot_name}` has {count} covering provider plans for the selected target: {} -- choose one in build.omg with `b.select_provider<{slot_name}, ProviderType>();`",
                    many.iter()
                        .map(|(_, plan)| format!("`{}` [{:016x}]", plan.name, plan.identity_fingerprint()))
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

#[cfg(test)]
#[path = "plans/tests.rs"]
mod tests;
