//! Retained descriptor and conformance applications for dynamic calls.

use semantic_vocabulary::{MachineId, OperationId};
use terminal_psi::{
    ClosedConformanceApplication, TerminalDynamicConformanceSelection,
    TerminalDynamicDescriptorArgument, TerminalDynamicDescriptorParameter,
    TerminalIndirectDynamicDispatch, TerminalParameterDynamicDispatch,
    TerminalReboundDynamicDescriptor, TerminalStoredDynamicDescriptor,
    TerminalStoredDynamicDispatch,
};

/// Descriptor custody of one dynamic call.
///
/// The three routes are different ownership facts, not spellings of one
/// "descriptor" case. A rebound descriptor is caller-local and carries its
/// initializer and latest selections; a stored descriptor is reloaded from an
/// aggregate field established by a separate operation; a parameter descriptor
/// is a runtime input whose table the caller cannot see. Consumers that
/// differ by custody match on the payload; only byte-identical treatment may
/// share an arm.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AbstractDynamicDispatch {
    Rebound(AbstractReboundDynamicDispatch),
    Stored(AbstractStoredDynamicDispatch),
    Parameter(AbstractParameterDynamicDispatch),
}

impl AbstractDynamicDispatch {
    /// The realization machine this call site names. Rebound and stored
    /// custody retain the selected table row; a parameter descriptor's
    /// realization is chosen by its incoming table at run time.
    pub fn realization(&self) -> Option<MachineId> {
        match self {
            Self::Rebound(dispatch) => Some(dispatch.dispatch.realization),
            Self::Stored(dispatch) => Some(dispatch.dispatch.realization),
            Self::Parameter(_) => None,
        }
    }
}

/// Exact target-neutral custody for one rebound dynamic invocation.
///
/// The two selections retain the initializer and latest runtime source. The
/// descriptor retains their version relation. `initial_application` preserves
/// the initializer's exact conformance even when the rebound changes it;
/// `application` is the latest complete canonical private-table map, while
/// `dispatch` identifies the sole row permitted at this call site. Realization
/// machines are table content, not statically addressed call targets.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AbstractReboundDynamicDispatch {
    pub initial: TerminalDynamicConformanceSelection,
    pub rebound: TerminalDynamicConformanceSelection,
    pub descriptor: TerminalReboundDynamicDescriptor,
    pub initial_application: ClosedConformanceApplication,
    pub application: ClosedConformanceApplication,
    pub dispatch: TerminalIndirectDynamicDispatch,
}

/// Exact target-neutral custody for one descriptor stored in an aggregate
/// field. Establishment and dispatch remain separate operations, but both
/// retain this same source selection and closed application instead of
/// reconstructing either from a descriptor ordinal.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AbstractStoredDynamicDescriptor {
    pub selection: TerminalDynamicConformanceSelection,
    pub descriptor: TerminalStoredDynamicDescriptor,
    pub application: ClosedConformanceApplication,
}

impl AbstractStoredDynamicDescriptor {
    pub fn has_complete_custody(&self, owner: MachineId, operation: OperationId) -> bool {
        self.descriptor.owner == owner
            && self.descriptor.establishment_operation == operation
            && self.selection.owner == owner
            && self.selection.ordinal == self.descriptor.selection_ordinal
            && self.application.owner == owner
            && !self.descriptor.aggregate_type_identity.is_empty()
            && !self.descriptor.field_identity.is_empty()
            && self.selection.conformance_application_report_fingerprint
                == self.application.report_fingerprint
            && self.selection.conformance_application_commitment == self.application.commitment
            && self.application.report_fingerprint
                == terminal_psi::closed_conformance_application_report_fingerprint(
                    &self.application,
                )
            && self.application.commitment
                == terminal_psi::closed_conformance_application_commitment(&self.application)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AbstractStoredDynamicDispatch {
    pub stored: AbstractStoredDynamicDescriptor,
    pub dispatch: TerminalStoredDynamicDispatch,
}

impl AbstractStoredDynamicDispatch {
    pub fn has_complete_custody(&self, owner: MachineId, operation: OperationId) -> bool {
        self.stored
            .has_complete_custody(owner, self.stored.descriptor.establishment_operation)
            && self.dispatch.owner == owner
            && self.dispatch.operation == operation
            && self.dispatch.descriptor_ordinal == self.stored.descriptor.ordinal
            && self
                .stored
                .application
                .rows
                .iter()
                .filter(|row| {
                    row.declaring_trait_identity == self.dispatch.declaring_trait_identity
                        && row.public_requirement_identity
                            == self.dispatch.public_requirement_identity
                        && row.family_tuple == self.dispatch.family_tuple
                        && row.requirement_identity == self.dispatch.requirement_identity
                        && row.realization_identity == self.dispatch.realization_identity
                        && row.realization_callable_identity.as_deref()
                            == Some(self.dispatch.realization_callable_identity.as_str())
                })
                .count()
                == 1
            && self
                .stored
                .application
                .realization_callables
                .iter()
                .filter(|callable| {
                    callable.source_callable_identity == self.dispatch.realization_callable_identity
                        && callable.machine == self.dispatch.realization
                })
                .count()
                == 1
    }
}

/// One target-neutral dynamic descriptor argument after the Terminal catalog
/// has been independently rejoined. The target parameter is retained beside
/// the exact source custody so later ABI selection never has to recover either
/// side from owner-local ordinals alone.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AbstractDynamicDescriptorArgument {
    pub argument: TerminalDynamicDescriptorArgument,
    pub target: TerminalDynamicDescriptorParameter,
    pub source: AbstractDynamicDescriptorSource,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AbstractDynamicDescriptorSource {
    Selection {
        selection: TerminalDynamicConformanceSelection,
        application: ClosedConformanceApplication,
    },
    Rebound {
        initial: TerminalDynamicConformanceSelection,
        rebound: TerminalDynamicConformanceSelection,
        descriptor: TerminalReboundDynamicDescriptor,
        initial_application: ClosedConformanceApplication,
        application: ClosedConformanceApplication,
    },
    Parameter(TerminalDynamicDescriptorParameter),
}

impl AbstractDynamicDescriptorArgument {
    /// Replay one caller-to-callee existential interface join without treating
    /// either repeated ordinal as authority.
    pub fn has_complete_custody(
        &self,
        caller: MachineId,
        operation: OperationId,
        callee: MachineId,
    ) -> bool {
        if self.argument.owner != caller
            || self.argument.operation != operation
            || self.argument.parameter_ordinal != self.target.ordinal
            || self.target.owner != callee
        {
            return false;
        }
        let interfaces_match = |source: &TerminalDynamicDescriptorParameter| {
            source.trait_identity == self.target.trait_identity
                && source.access == self.target.access
                && source.requirements == self.target.requirements
        };
        match (&self.argument.source, &self.source) {
            (
                terminal_psi::TerminalDynamicDescriptorSource::Selection { ordinal },
                AbstractDynamicDescriptorSource::Selection {
                    selection,
                    application,
                },
            ) => {
                selection.owner == caller
                    && selection.ordinal == *ordinal
                    && application.owner == caller
                    && selection.conformance_application_report_fingerprint
                        == application.report_fingerprint
                    && selection.conformance_application_commitment == application.commitment
                    && application.report_fingerprint != 0
                    && !application.commitment.is_zero()
                    && application.report_fingerprint
                        == terminal_psi::closed_conformance_application_report_fingerprint(
                            application,
                        )
                    && application.commitment
                        == terminal_psi::closed_conformance_application_commitment(application)
                    && selection.source.access == self.target.access
                    && application.trait_identity == self.target.trait_identity
                    && application.rows.len() == self.target.requirements.len()
                    && application.rows.iter().zip(&self.target.requirements).all(
                        |(row, requirement)| {
                            row.declaring_trait_identity == requirement.declaring_trait_identity
                                && row.public_requirement_identity
                                    == requirement.public_requirement_identity
                                && row.family_tuple == requirement.family_tuple
                                && row
                                    .realization_callable_identity
                                    .as_ref()
                                    .and_then(|identity| {
                                        application.realization_callables.iter().find(|callable| {
                                            callable.source_callable_identity == *identity
                                        })
                                    })
                                    .is_some_and(|callable| callable.result == requirement.result)
                        },
                    )
            }
            (
                terminal_psi::TerminalDynamicDescriptorSource::Parameter { ordinal },
                AbstractDynamicDescriptorSource::Parameter(source),
            ) => source.owner == caller && source.ordinal == *ordinal && interfaces_match(source),
            (
                terminal_psi::TerminalDynamicDescriptorSource::ReboundDescriptor { ordinal },
                AbstractDynamicDescriptorSource::Rebound {
                    initial,
                    rebound,
                    descriptor,
                    initial_application,
                    application,
                },
            ) => {
                descriptor.owner == caller
                    && descriptor.ordinal == *ordinal
                    && initial.owner == caller
                    && rebound.owner == caller
                    && descriptor.initial_selection_ordinal == initial.ordinal
                    && descriptor.rebound_selection_ordinal == rebound.ordinal
                    && initial.conformance_application_report_fingerprint
                        == initial_application.report_fingerprint
                    && rebound.conformance_application_report_fingerprint
                        == application.report_fingerprint
                    && initial.conformance_application_commitment == initial_application.commitment
                    && rebound.conformance_application_commitment == application.commitment
                    && initial_application.owner == caller
                    && application.owner == caller
                    && initial_application.report_fingerprint != 0
                    && !initial_application.commitment.is_zero()
                    && initial_application.report_fingerprint
                        == terminal_psi::closed_conformance_application_report_fingerprint(
                            initial_application,
                        )
                    && initial_application.commitment
                        == terminal_psi::closed_conformance_application_commitment(
                            initial_application,
                        )
                    && application.report_fingerprint != 0
                    && !application.commitment.is_zero()
                    && application.report_fingerprint
                        == terminal_psi::closed_conformance_application_report_fingerprint(
                            application,
                        )
                    && application.commitment
                        == terminal_psi::closed_conformance_application_commitment(application)
                    && initial.source.access == self.target.access
                    && rebound.source.access == self.target.access
                    && initial_application.trait_identity == application.trait_identity
                    && initial_application.trait_lifetime_arguments
                        == application.trait_lifetime_arguments
                    && initial_application.trait_arguments == application.trait_arguments
                    && initial_application.telescope == application.telescope
                    && initial_application.rows.len() == application.rows.len()
                    && initial_application.rows.iter().zip(&application.rows).all(
                        |(initial, rebound)| {
                            initial.declaring_trait_identity == rebound.declaring_trait_identity
                                && initial.public_requirement_identity
                                    == rebound.public_requirement_identity
                                && initial.family_tuple == rebound.family_tuple
                                && initial.requirement_identity == rebound.requirement_identity
                        },
                    )
                    && application.trait_identity == self.target.trait_identity
                    && application.rows.len() == self.target.requirements.len()
                    && application.rows.iter().zip(&self.target.requirements).all(
                        |(row, requirement)| {
                            row.declaring_trait_identity == requirement.declaring_trait_identity
                                && row.public_requirement_identity
                                    == requirement.public_requirement_identity
                                && row.family_tuple == requirement.family_tuple
                                && row
                                    .realization_callable_identity
                                    .as_ref()
                                    .and_then(|identity| {
                                        application.realization_callables.iter().find(|callable| {
                                            callable.source_callable_identity == *identity
                                        })
                                    })
                                    .is_some_and(|callable| callable.result == requirement.result)
                        },
                    )
            }
            _ => false,
        }
    }
}

/// One call through a descriptor received by the current function.
/// The closed parameter interface supplies the public slot and result shape;
/// the concrete table and instance remain runtime inputs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AbstractParameterDynamicDispatch {
    pub parameter: TerminalDynamicDescriptorParameter,
    pub dispatch: TerminalParameterDynamicDispatch,
}

impl AbstractParameterDynamicDispatch {
    /// Replay the caller-side join between the dispatch row and the exact
    /// descriptor parameter it consumes: same owner, the call operation that
    /// produced the dispatch, the named parameter ordinal, and exactly one
    /// interface requirement carrying the selected slot. The borrowed access
    /// stays part of the join so an owned or fabricated descriptor row cannot
    /// stand in for the parameter ABI.
    pub fn has_complete_custody(&self, owner: MachineId, operation: OperationId) -> bool {
        self.parameter.owner == owner
            && self.dispatch.owner == owner
            && self.dispatch.operation == operation
            && self.dispatch.parameter_ordinal == self.parameter.ordinal
            && matches!(
                self.parameter.access,
                terminal_psi::StructuralAccess::SharedBorrow
                    | terminal_psi::StructuralAccess::MutableBorrow
            )
            && self
                .parameter
                .requirements
                .iter()
                .filter(|requirement| requirement.slot == self.dispatch.requirement_slot)
                .count()
                == 1
    }
}

impl AbstractReboundDynamicDispatch {
    /// Replay the complete selected-table join without trusting the repeated
    /// call-site row or compact report coordinate as authority.
    pub fn has_complete_application_custody(
        &self,
        owner: MachineId,
        operation: OperationId,
    ) -> bool {
        self.initial.owner == owner
            && self.rebound.owner == owner
            && self.descriptor.owner == owner
            && self.initial_application.owner == owner
            && self.application.owner == owner
            && self.dispatch.owner == owner
            && self.descriptor.initial_selection_ordinal == self.initial.ordinal
            && self.descriptor.rebound_selection_ordinal == self.rebound.ordinal
            && self.dispatch.operation == operation
            && self.dispatch.descriptor_ordinal == self.descriptor.ordinal
            && self.initial_application.report_fingerprint != 0
            && !self.initial_application.commitment.is_zero()
            && self.initial_application.report_fingerprint
                == terminal_psi::closed_conformance_application_report_fingerprint(
                    &self.initial_application,
                )
            && self.initial_application.commitment
                == terminal_psi::closed_conformance_application_commitment(
                    &self.initial_application,
                )
            && self.application.report_fingerprint != 0
            && !self.application.commitment.is_zero()
            && self.application.report_fingerprint
                == terminal_psi::closed_conformance_application_report_fingerprint(
                    &self.application,
                )
            && self.application.commitment
                == terminal_psi::closed_conformance_application_commitment(&self.application)
            && self.initial.conformance_application_report_fingerprint
                == self.initial_application.report_fingerprint
            && self.initial.conformance_application_commitment
                == self.initial_application.commitment
            && self.rebound.conformance_application_report_fingerprint
                == self.application.report_fingerprint
            && self.rebound.conformance_application_commitment == self.application.commitment
            && self.initial_application.trait_identity == self.application.trait_identity
            && self.initial_application.trait_lifetime_arguments
                == self.application.trait_lifetime_arguments
            && self.initial_application.trait_arguments == self.application.trait_arguments
            && self.initial_application.telescope == self.application.telescope
            && self.initial_application.rows.len() == self.application.rows.len()
            && self
                .initial_application
                .rows
                .iter()
                .zip(&self.application.rows)
                .all(|(initial, rebound)| {
                    initial.declaring_trait_identity == rebound.declaring_trait_identity
                        && initial.public_requirement_identity
                            == rebound.public_requirement_identity
                        && initial.family_tuple == rebound.family_tuple
                        && initial.requirement_identity == rebound.requirement_identity
                })
            && self
                .application
                .rows
                .iter()
                .filter(|row| {
                    row.declaring_trait_identity == self.dispatch.declaring_trait_identity
                        && row.public_requirement_identity
                            == self.dispatch.public_requirement_identity
                        && row.family_tuple == self.dispatch.family_tuple
                        && row.requirement_identity == self.dispatch.requirement_identity
                        && row.realization_identity == self.dispatch.realization_identity
                        && row.realization_callable_identity.as_deref()
                            == Some(self.dispatch.realization_callable_identity.as_str())
                })
                .count()
                == 1
            && self
                .application
                .realization_callables
                .iter()
                .filter(|callable| {
                    callable.source_callable_identity == self.dispatch.realization_callable_identity
                        && callable.machine == self.dispatch.realization
                })
                .count()
                == 1
    }
}
