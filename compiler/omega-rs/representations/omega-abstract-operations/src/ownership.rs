use omega_control_flow::StateKey;
use psi_arena::{Arena, Handle, HandleSpan};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct AbstractPermissionEvent {
    /// Canonical identity in the control-flow permission arena. Instruction
    /// selection runs before the abstract semantic root is assembled, so this
    /// is the stable join key between selected code and the lowered event.
    pub source_event_index: u32,
    pub source_key: StateKey,
    pub source: psi_language_semantics::PermissionEventSource,
    pub kind: psi_language_semantics::PermissionEventKind,
    pub multiplicity: psi_language_semantics::Multiplicity,
    pub access: psi_language_semantics::PermissionAccess,
    pub claim_identity: psi_language_semantics::PermissionClaimIdentity,
    pub provenance: psi_language_semantics::PermissionProvenance,
    pub root: psi_facts::PlaceRoot,
    pub segments: HandleSpan<psi_facts::PlaceSegment>,
    pub obligation_live: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CheckedNoCodePermissionReason {
    /// The source contains an explicit terminal consume, but its checked body
    /// needs no machine instruction. This is a semantic backend action, not a
    /// proof inferred from an accidentally empty selection site.
    ExplicitZeroCodeConsume,
    /// The canonical permission event carries no live runtime debt, so
    /// ownership lowering has nothing to transfer or clean up.
    ElidedNoDebt,
    /// Ordinary affine discard has no cleanup body and carries no live debt.
    TrivialAffineDrop,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PermissionRealizationCandidateKind {
    SelectedInstruction {
        instruction_index: u32,
    },
    CheckedNoCode {
        reason: CheckedNoCodePermissionReason,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PermissionRealizationCandidate {
    pub source_event_index: u32,
    pub kind: PermissionRealizationCandidateKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AbstractPermissionRealizationKind {
    SelectedInstructions {
        instruction_indices: HandleSpan<u32>,
    },
    CheckedNoCode {
        reason: CheckedNoCodePermissionReason,
    },
}

impl Default for AbstractPermissionRealizationKind {
    fn default() -> Self {
        Self::CheckedNoCode {
            reason: CheckedNoCodePermissionReason::ExplicitZeroCodeConsume,
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct AbstractPermissionRealization {
    pub event: Handle<AbstractPermissionEvent>,
    pub kind: AbstractPermissionRealizationKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PermissionRealizationError {
    DuplicateSourceEvent { source_event_index: u32 },
    ForeignCandidate { source_event_index: u32 },
    MissingCandidate { source_event_index: u32 },
    InvalidInstruction { instruction_index: u32 },
    ConflictingNoCode { source_event_index: u32 },
    InvalidNoCodeProof { source_event_index: u32 },
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct AbstractOwnershipSummary {
    pub segments: Arena<psi_facts::PlaceSegment>,
    pub permissions: Arena<AbstractPermissionEvent>,
    pub realization_instruction_indices: Arena<u32>,
    pub realizations: Arena<AbstractPermissionRealization>,
}

impl AbstractOwnershipSummary {
    pub fn with_capacity(segment_capacity: usize, permission_capacity: usize) -> Self {
        Self {
            segments: Arena::with_capacity(segment_capacity),
            permissions: Arena::with_capacity(permission_capacity),
            realization_instruction_indices: Arena::new(),
            realizations: Arena::with_capacity(permission_capacity),
        }
    }
}

impl AbstractOwnershipSummary {
    /// Normalize selection-time candidates into exactly one realization per
    /// canonical permission event. Installation is atomic and fail-closed:
    /// incomplete, foreign, or malformed candidates leave the published
    /// realization ledger empty.
    pub fn install_permission_realization_candidates(
        &mut self,
        candidates: &[PermissionRealizationCandidate],
        instruction_count: usize,
    ) -> Result<(), PermissionRealizationError> {
        self.realizations.clear();
        self.realization_instruction_indices.clear();

        let mut events_by_source = BTreeMap::new();
        for (event_handle, event) in self.permissions.iter() {
            if events_by_source
                .insert(event.source_event_index, (event_handle, event))
                .is_some()
            {
                return Err(PermissionRealizationError::DuplicateSourceEvent {
                    source_event_index: event.source_event_index,
                });
            }
        }

        let mut candidates_by_source: BTreeMap<
            u32,
            (BTreeSet<u32>, Option<CheckedNoCodePermissionReason>),
        > = BTreeMap::new();
        for candidate in candidates {
            if !events_by_source.contains_key(&candidate.source_event_index) {
                return Err(PermissionRealizationError::ForeignCandidate {
                    source_event_index: candidate.source_event_index,
                });
            }
            let group = candidates_by_source
                .entry(candidate.source_event_index)
                .or_default();
            match candidate.kind {
                PermissionRealizationCandidateKind::SelectedInstruction { instruction_index } => {
                    if instruction_index == 0
                        || usize::try_from(instruction_index)
                            .ok()
                            .is_none_or(|index| index > instruction_count)
                    {
                        return Err(PermissionRealizationError::InvalidInstruction {
                            instruction_index,
                        });
                    }
                    group.0.insert(instruction_index);
                }
                PermissionRealizationCandidateKind::CheckedNoCode { reason } => {
                    if group.1.replace(reason).is_some_and(|prior| prior != reason) {
                        return Err(PermissionRealizationError::ConflictingNoCode {
                            source_event_index: candidate.source_event_index,
                        });
                    }
                }
            }
        }

        let mut normalized_indices = Arena::with_capacity(candidates.len());
        let mut normalized_realizations = Arena::with_capacity(self.permissions.len());
        for (event_handle, event) in self.permissions.iter() {
            let Some((instruction_indices, no_code_reason)) =
                candidates_by_source.remove(&event.source_event_index)
            else {
                return Err(PermissionRealizationError::MissingCandidate {
                    source_event_index: event.source_event_index,
                });
            };

            let kind = if !instruction_indices.is_empty() {
                AbstractPermissionRealizationKind::SelectedInstructions {
                    instruction_indices: normalized_indices.insert_many(instruction_indices),
                }
            } else {
                let Some(reason) = no_code_reason else {
                    return Err(PermissionRealizationError::MissingCandidate {
                        source_event_index: event.source_event_index,
                    });
                };
                let valid = match reason {
                    CheckedNoCodePermissionReason::ExplicitZeroCodeConsume => {
                        matches!(
                            event.kind,
                            psi_language_semantics::PermissionEventKind::Consume
                        ) && event.access == psi_language_semantics::PermissionAccess::Owned
                            && event.obligation_live
                    }
                    CheckedNoCodePermissionReason::ElidedNoDebt => !event.obligation_live,
                    CheckedNoCodePermissionReason::TrivialAffineDrop => {
                        matches!(
                            event.kind,
                            psi_language_semantics::PermissionEventKind::AffineDrop
                        ) && !event.obligation_live
                    }
                };
                if !valid {
                    return Err(PermissionRealizationError::InvalidNoCodeProof {
                        source_event_index: event.source_event_index,
                    });
                }
                AbstractPermissionRealizationKind::CheckedNoCode { reason }
            };

            normalized_realizations.insert(AbstractPermissionRealization {
                event: event_handle,
                kind,
            });
        }

        self.realization_instruction_indices = normalized_indices;
        self.realizations = normalized_realizations;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn event(source_event_index: u32) -> AbstractPermissionEvent {
        AbstractPermissionEvent {
            source_event_index,
            ..AbstractPermissionEvent::default()
        }
    }

    #[test]
    fn realization_candidates_normalize_per_event() {
        let mut summary = AbstractOwnershipSummary::default();
        summary.permissions.insert(event(7));
        let drop = summary.permissions.insert(AbstractPermissionEvent {
            source_event_index: 9,
            kind: psi_language_semantics::PermissionEventKind::AffineDrop,
            obligation_live: false,
            ..AbstractPermissionEvent::default()
        });

        summary
            .install_permission_realization_candidates(
                &[
                    PermissionRealizationCandidate {
                        source_event_index: 7,
                        kind: PermissionRealizationCandidateKind::SelectedInstruction {
                            instruction_index: 3,
                        },
                    },
                    PermissionRealizationCandidate {
                        source_event_index: 7,
                        kind: PermissionRealizationCandidateKind::SelectedInstruction {
                            instruction_index: 1,
                        },
                    },
                    PermissionRealizationCandidate {
                        source_event_index: 7,
                        kind: PermissionRealizationCandidateKind::SelectedInstruction {
                            instruction_index: 3,
                        },
                    },
                    PermissionRealizationCandidate {
                        source_event_index: 9,
                        kind: PermissionRealizationCandidateKind::CheckedNoCode {
                            reason: CheckedNoCodePermissionReason::TrivialAffineDrop,
                        },
                    },
                ],
                3,
            )
            .expect("complete candidates should normalize");

        assert_eq!(summary.realizations.len(), 2);
        let first = summary.realizations.iter().next().unwrap().1;
        let AbstractPermissionRealizationKind::SelectedInstructions {
            instruction_indices,
        } = first.kind
        else {
            panic!("first event should select instructions");
        };
        assert_eq!(
            summary
                .realization_instruction_indices
                .span_or_empty(instruction_indices),
            &[1, 3]
        );
        assert_eq!(summary.realizations.iter().nth(1).unwrap().1.event, drop);
    }

    #[test]
    fn incomplete_candidates_leave_no_published_ledger() {
        let mut summary = AbstractOwnershipSummary::default();
        summary.permissions.insert(event(1));
        summary.permissions.insert(event(2));

        assert_eq!(
            summary.install_permission_realization_candidates(
                &[PermissionRealizationCandidate {
                    source_event_index: 1,
                    kind: PermissionRealizationCandidateKind::SelectedInstruction {
                        instruction_index: 1,
                    },
                }],
                1,
            ),
            Err(PermissionRealizationError::MissingCandidate {
                source_event_index: 2,
            })
        );
        assert!(summary.realizations.is_empty());
        assert!(summary.realization_instruction_indices.is_empty());
    }

    #[test]
    fn no_code_proofs_reject_live_debt() {
        let mut summary = AbstractOwnershipSummary::default();
        summary.permissions.insert(AbstractPermissionEvent {
            source_event_index: 5,
            obligation_live: true,
            ..AbstractPermissionEvent::default()
        });

        assert_eq!(
            summary.install_permission_realization_candidates(
                &[PermissionRealizationCandidate {
                    source_event_index: 5,
                    kind: PermissionRealizationCandidateKind::CheckedNoCode {
                        reason: CheckedNoCodePermissionReason::ElidedNoDebt,
                    },
                }],
                0,
            ),
            Err(PermissionRealizationError::InvalidNoCodeProof {
                source_event_index: 5,
            })
        );
        assert!(summary.realizations.is_empty());
    }

    #[test]
    fn explicit_zero_code_action_accepts_only_live_owned_consumes() {
        let mut summary = AbstractOwnershipSummary::default();
        summary.permissions.insert(AbstractPermissionEvent {
            source_event_index: 5,
            kind: psi_language_semantics::PermissionEventKind::Establish,
            access: psi_language_semantics::PermissionAccess::Owned,
            obligation_live: true,
            ..AbstractPermissionEvent::default()
        });

        let candidate = PermissionRealizationCandidate {
            source_event_index: 5,
            kind: PermissionRealizationCandidateKind::CheckedNoCode {
                reason: CheckedNoCodePermissionReason::ExplicitZeroCodeConsume,
            },
        };
        assert_eq!(
            summary.install_permission_realization_candidates(&[candidate], 0),
            Err(PermissionRealizationError::InvalidNoCodeProof {
                source_event_index: 5,
            })
        );
        assert!(summary.realizations.is_empty());

        summary.permissions.clear();
        summary.permissions.insert(AbstractPermissionEvent {
            source_event_index: 5,
            kind: psi_language_semantics::PermissionEventKind::Consume,
            access: psi_language_semantics::PermissionAccess::Owned,
            obligation_live: true,
            ..AbstractPermissionEvent::default()
        });
        summary
            .install_permission_realization_candidates(&[candidate], 0)
            .expect("an explicit terminal consume may be a zero-code backend action");
        assert_eq!(summary.realizations.len(), 1);
    }
}
