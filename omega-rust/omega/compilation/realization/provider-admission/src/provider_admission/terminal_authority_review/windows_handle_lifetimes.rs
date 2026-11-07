//! Native resource instances are separate from their copyable integer aliases.
//!
//! The canonical-flow reader supplies checked acquisition and call occurrences.
//! This ledger records their resource consequences; it does not discover calls,
//! classify a readable method name, accept a source qualification as provenance,
//! or turn an asserted digest into evidence. Close candidates remain provisional
//! until every continuation has established preservation and absence of later use.

use std::collections::{BTreeMap, BTreeSet};

use semantic_vocabulary::{BoundaryMachineId, InvocationSite, MachineId};

/// Exact static occurrence plus the substituted caller context. The context is
/// independent of the proof commitment, so changing a proof cannot rename use.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(super) struct NativeOccurrence {
    pub machine: MachineId,
    pub site: InvocationSite,
    pub context: [u8; 32],
    pub boundary: BoundaryMachineId,
}

/// A source-level scalar observation is a real operation, but has no foreign
/// boundary. Keep it distinct so checking a stale integer copy never needs an
/// invented native call identity.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(super) struct ScalarOccurrence {
    pub machine: MachineId,
    pub site: InvocationSite,
    pub context: [u8; 32],
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(super) enum HandleUseOccurrence {
    Native(NativeOccurrence),
    Scalar(ScalarOccurrence),
}

impl From<NativeOccurrence> for HandleUseOccurrence {
    fn from(occurrence: NativeOccurrence) -> Self {
        Self::Native(occurrence)
    }
}

/// Analysis-owned coordinates, never fabricated Terminal ValueId/PlaceId rows.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(super) struct HandleInstance(pub usize);

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
struct FileObject(usize);

#[derive(Clone, Debug, PartialEq, Eq)]
enum HandleOrigin {
    Acquired(NativeOccurrence),
    Duplicated {
        occurrence: NativeOccurrence,
        source: HandleInstance,
    },
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct Handle {
    object: FileObject,
    origin: HandleOrigin,
    release: Option<NativeOccurrence>,
    uses: BTreeSet<HandleUseOccurrence>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct FileObjectState {
    /// A disposition on one duplicate changes their shared file object. A
    /// pending deletion requested through another independently opened object
    /// does not set this flag and is not excluded from the environment.
    attached_deletion: bool,
    escaped: Option<NativeOccurrence>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) enum HandleLifetimeFailure {
    UnprovedOccurrence(HandleUseOccurrence),
    UnknownHandle {
        instance: HandleInstance,
        use_site: HandleUseOccurrence,
    },
    UseAfterRelease {
        acquisition: NativeOccurrence,
        release: NativeOccurrence,
        later_use: HandleUseOccurrence,
    },
    EscapedObject {
        acquisition: NativeOccurrence,
        use_site: HandleUseOccurrence,
    },
    AttachedDeletion {
        acquisition: NativeOccurrence,
        release: NativeOccurrence,
    },
    MissingRelease {
        acquisition: NativeOccurrence,
    },
    RepeatedAcquisition {
        instance: HandleInstance,
    },
}

/// One returning path's exact resource history. Control-flow joins must retain
/// every incoming ledger; choosing one branch is not a proof of one release.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(super) struct HandleLifetimes {
    handles: BTreeMap<HandleInstance, Handle>,
    objects: Vec<FileObjectState>,
}

impl HandleLifetimes {
    /// Called only on the successful, non-inheritable CreateFileW continuation
    /// after the flow reader binds the actual flags to this native occurrence.
    /// CreateFileW's result can name any supported file/device kind, including a
    /// console. No non-console claim is introduced by this acquisition.
    pub(super) fn acquire_file(
        &mut self,
        instance: HandleInstance,
        occurrence: NativeOccurrence,
        flags: u32,
    ) -> Result<(), HandleLifetimeFailure> {
        Self::require_occurrence(occurrence.into())?;
        if self.handles.contains_key(&instance) {
            return Err(HandleLifetimeFailure::RepeatedAcquisition { instance });
        }
        let object = FileObject(self.objects.len());
        self.objects.push(FileObjectState {
            attached_deletion: flags & 0x0400_0000 != 0,
            escaped: None,
        });
        self.handles.insert(
            instance,
            Handle {
                object,
                origin: HandleOrigin::Acquired(occurrence),
                release: None,
                uses: BTreeSet::new(),
            },
        );
        Ok(())
    }

    /// Same-process, same-access, non-inheritable duplication with no
    /// DUPLICATE_CLOSE_SOURCE. The canonical reader must establish those exact
    /// arguments and successful output before calling this transition.
    pub(super) fn duplicate_file(
        &mut self,
        source: HandleInstance,
        duplicate: HandleInstance,
        occurrence: NativeOccurrence,
    ) -> Result<(), HandleLifetimeFailure> {
        let object = self.require_live(source, occurrence.into())?;
        if self.handles.contains_key(&duplicate) {
            return Err(HandleLifetimeFailure::RepeatedAcquisition {
                instance: duplicate,
            });
        }
        self.handles
            .get_mut(&source)
            .unwrap()
            .uses
            .insert(occurrence.into());
        self.handles.insert(
            duplicate,
            Handle {
                object,
                origin: HandleOrigin::Duplicated { occurrence, source },
                release: None,
                uses: BTreeSet::new(),
            },
        );
        Ok(())
    }

    /// This transition requires an independently reviewed preserving native
    /// contract. Empty/query-only authority is not such a contract. The caller
    /// records every handle-taking call, including approved duplication.
    pub(super) fn preserve(
        &mut self,
        instance: HandleInstance,
        occurrence: NativeOccurrence,
    ) -> Result<(), HandleLifetimeFailure> {
        self.require_live(instance, occurrence.into())?;
        self.handles
            .get_mut(&instance)
            .unwrap()
            .uses
            .insert(occurrence.into());
        Ok(())
    }

    /// A comparison, copy or numeric derivation still observes this precise
    /// handle generation. Recording its real scalar site catches later use even
    /// when no second foreign call is made. Repeated loop observations of the
    /// same live input are a set of static obligations, not growing execution
    /// logs; acquisitions and retirement retain their separate transitions.
    pub(super) fn observe_scalar(
        &mut self,
        instance: HandleInstance,
        occurrence: ScalarOccurrence,
    ) -> Result<(), HandleLifetimeFailure> {
        let use_site = HandleUseOccurrence::Scalar(occurrence);
        self.require_live(instance, use_site)?;
        self.handles
            .get_mut(&instance)
            .unwrap()
            .uses
            .insert(use_site);
        Ok(())
    }

    /// A copied integer escaping checked custody invalidates the shared object
    /// assumption for all duplicates. The reader propagates this to all normal
    /// continuations; a later close cannot recover the lost private history.
    pub(super) fn escape(
        &mut self,
        instance: HandleInstance,
        occurrence: NativeOccurrence,
    ) -> Result<(), HandleLifetimeFailure> {
        self.preserve(instance, occurrence)?;
        let object = self.handles[&instance].object;
        self.objects[object.0].escaped = Some(occurrence);
        Ok(())
    }

    pub(super) fn attach_deletion(
        &mut self,
        instance: HandleInstance,
        occurrence: NativeOccurrence,
    ) -> Result<(), HandleLifetimeFailure> {
        self.preserve(instance, occurrence)?;
        let object = self.handles[&instance].object;
        self.objects[object.0].attached_deletion = true;
        Ok(())
    }

    /// Record one close attempt and retire the checked integer instance,
    /// regardless of the reported native status. This does not assert resource
    /// reclamation after a failed close. A copied numeric alias cannot be
    /// revived by a later acquisition reusing its OS number.
    pub(super) fn release_file(
        &mut self,
        instance: HandleInstance,
        occurrence: NativeOccurrence,
    ) -> Result<(), HandleLifetimeFailure> {
        let object = self.require_live(instance, occurrence.into())?;
        let acquisition = self.acquisition(instance, occurrence.into())?;
        if self.objects[object.0].attached_deletion {
            return Err(HandleLifetimeFailure::AttachedDeletion {
                acquisition,
                release: occurrence,
            });
        }
        let handle = self.handles.get_mut(&instance).unwrap();
        handle.uses.insert(occurrence.into());
        handle.release = Some(occurrence);
        Ok(())
    }

    /// The canonical reader retains the exact live-instance roster belonging to
    /// the checked caller continuation, including returned values and proven
    /// output storage. Every other acquired instance needs one release on this
    /// normal exit. This is not invoked for a fabricated empty path.
    pub(super) fn returning_path(
        &self,
        retained_by_caller: &[HandleInstance],
    ) -> Result<(), HandleLifetimeFailure> {
        for (instance, handle) in &self.handles {
            if let Some(escape) = self.objects[handle.object.0].escaped {
                return Err(HandleLifetimeFailure::EscapedObject {
                    acquisition: Self::origin_occurrence(&handle.origin),
                    use_site: escape.into(),
                });
            }
            if handle.release.is_none() && !retained_by_caller.contains(instance) {
                return Err(HandleLifetimeFailure::MissingRelease {
                    acquisition: Self::origin_occurrence(&handle.origin),
                });
            }
        }
        Ok(())
    }

    fn require_occurrence(occurrence: HandleUseOccurrence) -> Result<(), HandleLifetimeFailure> {
        let (context, site) = match occurrence {
            HandleUseOccurrence::Native(native) => (native.context, native.site),
            HandleUseOccurrence::Scalar(scalar) => (scalar.context, scalar.site),
        };
        if context == [0; 32] || site.operation().is_none() {
            return Err(HandleLifetimeFailure::UnprovedOccurrence(occurrence));
        }
        Ok(())
    }

    fn require_live(
        &self,
        instance: HandleInstance,
        occurrence: HandleUseOccurrence,
    ) -> Result<FileObject, HandleLifetimeFailure> {
        Self::require_occurrence(occurrence)?;
        let acquisition = self.acquisition(instance, occurrence)?;
        let handle = &self.handles[&instance];
        if let Some(release) = handle.release {
            return Err(HandleLifetimeFailure::UseAfterRelease {
                acquisition,
                release,
                later_use: occurrence,
            });
        }
        if self.objects[handle.object.0].escaped.is_some() {
            return Err(HandleLifetimeFailure::EscapedObject {
                acquisition,
                use_site: occurrence,
            });
        }
        Ok(handle.object)
    }

    fn acquisition(
        &self,
        instance: HandleInstance,
        occurrence: HandleUseOccurrence,
    ) -> Result<NativeOccurrence, HandleLifetimeFailure> {
        self.handles
            .get(&instance)
            .map(|handle| Self::origin_occurrence(&handle.origin))
            .ok_or(HandleLifetimeFailure::UnknownHandle {
                instance,
                use_site: occurrence,
            })
    }

    fn origin_occurrence(origin: &HandleOrigin) -> NativeOccurrence {
        match origin {
            HandleOrigin::Acquired(occurrence) | HandleOrigin::Duplicated { occurrence, .. } => {
                *occurrence
            }
        }
    }
}

#[cfg(test)]
mod transition_tests {
    use super::*;
    use semantic_vocabulary::OperationId;

    fn occurrence(operation: u64) -> NativeOccurrence {
        NativeOccurrence {
            machine: MachineId::new(1).unwrap(),
            site: InvocationSite::Operation(OperationId::new(operation).unwrap()),
            context: [1; 32],
            boundary: BoundaryMachineId::new(1).unwrap(),
        }
    }

    #[test]
    fn scalar_observation_after_release_and_escaped_alias_cannot_retire() {
        let source = HandleInstance(0);
        let alias = HandleInstance(1);
        let mut retired = HandleLifetimes::default();
        retired.acquire_file(source, occurrence(1), 0).unwrap();
        retired.release_file(source, occurrence(2)).unwrap();
        let before = retired.clone();
        let native = occurrence(3);
        assert!(matches!(
            retired.observe_scalar(
                source,
                ScalarOccurrence {
                    machine: native.machine,
                    site: native.site,
                    context: native.context,
                }
            ),
            Err(HandleLifetimeFailure::UseAfterRelease {
                later_use: HandleUseOccurrence::Scalar(_),
                ..
            })
        ));
        assert_eq!(retired, before);

        let mut escaped = HandleLifetimes::default();
        escaped.acquire_file(source, occurrence(4), 0).unwrap();
        escaped
            .duplicate_file(source, alias, occurrence(5))
            .unwrap();
        escaped.escape(alias, occurrence(6)).unwrap();
        assert!(matches!(
            escaped.returning_path(&[source, alias]),
            Err(HandleLifetimeFailure::EscapedObject { .. })
        ));
        assert!(matches!(
            escaped.release_file(source, occurrence(7)),
            Err(HandleLifetimeFailure::EscapedObject { .. })
        ));
    }

    #[test]
    fn refused_duplicate_and_delete_release_leave_the_history_unchanged() {
        let mut aliases = HandleLifetimes::default();
        let source = HandleInstance(0);
        aliases.acquire_file(source, occurrence(1), 0).unwrap();
        let before = aliases.clone();
        assert!(matches!(
            aliases.duplicate_file(source, source, occurrence(2)),
            Err(HandleLifetimeFailure::RepeatedAcquisition { .. })
        ));
        assert_eq!(aliases, before);

        let mut deletion = HandleLifetimes::default();
        deletion
            .acquire_file(source, occurrence(3), 0x0400_0000)
            .unwrap();
        let before = deletion.clone();
        assert!(matches!(
            deletion.release_file(source, occurrence(4)),
            Err(HandleLifetimeFailure::AttachedDeletion { .. })
        ));
        assert_eq!(deletion, before);
    }
}
