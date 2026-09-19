//! The transactional-start custody boundary: the moved arguments and storage
//! authority presented to one task start, and the rejection that conserves
//! them.
//!
//! `start<M>`/`try_start<M>` is an ownership transaction. The moved argument
//! bundle and any supplied stack lease move into the provider call together;
//! a rejection returns every one of them to the caller — no linear value
//! disappears in a failed provider call. The contract also permits returning
//! proof that custody transferred to another named authorized owner; no
//! producer of that alternative exists yet, so the only rejection carrier is
//! the conserving return below.

use crate::stack_leases::StackLease;
use crate::{ActivationInstanceId, TaskArgumentCustodyId, TaskArgumentLayout, TaskPlanDiagnostic};

/// Moved task-start arguments presented to the provider.
///
/// Linear carrier mirroring the source-level move into the callee: the
/// bundle is marshalled under the activation plan's exact argument layout —
/// each source argument's bytes packed at its canonical field offset — and
/// `custody` is the single-use identity this presentation is known by. The
/// marshalled image is what the provider writes into the activation's
/// argument area; on the rejected path the same image returns to the caller
/// byte-exact, so no linear payload disappears in a failed provider call.
#[derive(Debug, PartialEq, Eq)]
pub struct MovedTaskArguments {
    layout: TaskArgumentLayout,
    custody: TaskArgumentCustodyId,
    image: Box<[u8]>,
}

impl MovedTaskArguments {
    /// Marshal `arguments` — one byte string per layout field, in source
    /// parameter order — under `layout`, minting the single-use custody
    /// identity this presentation is known by.
    ///
    /// The layout must be canonical (the same shape activation validation
    /// seals) and every supplied argument must exactly fill its field
    /// extent: neither truncation nor a partial bundle crosses the
    /// boundary. Interior and trailing padding is zeroed so the image is a
    /// deterministic function of the layout and the argument bytes.
    pub fn marshal(
        layout: &TaskArgumentLayout,
        arguments: &[&[u8]],
        custody: TaskArgumentCustodyId,
    ) -> Result<Self, TaskPlanDiagnostic> {
        if !layout.is_canonical() {
            return Err(TaskPlanDiagnostic(
                "moved task arguments cannot marshal under a non-canonical argument layout".into(),
            ));
        }
        if arguments.len() != layout.fields.len() {
            return Err(TaskPlanDiagnostic(format!(
                "moved task arguments supply {} value(s) under a {}-field argument layout",
                arguments.len(),
                layout.fields.len()
            )));
        }
        for (index, (field, argument)) in layout.fields.iter().zip(arguments.iter()).enumerate() {
            if argument.len() as u64 != field.bytes {
                return Err(TaskPlanDiagnostic(format!(
                    "moved task argument {index} supplies {} byte(s) under a {}-byte layout field",
                    argument.len(),
                    field.bytes
                )));
            }
        }
        let image_bytes = usize::try_from(layout.bytes).map_err(|_| {
            TaskPlanDiagnostic(
                "task argument marshalling image extent does not fit this host".into(),
            )
        })?;
        let mut image = vec![0u8; image_bytes];
        for (field, argument) in layout.fields.iter().zip(arguments.iter()) {
            let offset = usize::try_from(field.offset).map_err(|_| {
                TaskPlanDiagnostic("task argument field offset does not fit this host".into())
            })?;
            let end = offset.checked_add(argument.len()).ok_or_else(|| {
                TaskPlanDiagnostic(
                    "task argument field extent overflows the marshalling image".into(),
                )
            })?;
            let destination = image.get_mut(offset..end).ok_or_else(|| {
                TaskPlanDiagnostic(
                    "task argument field extent lies outside the marshalling image".into(),
                )
            })?;
            destination.copy_from_slice(argument);
        }
        Ok(Self {
            layout: layout.clone(),
            custody,
            image: image.into_boxed_slice(),
        })
    }

    /// The exact layout this bundle was marshalled under.
    pub const fn layout(&self) -> &TaskArgumentLayout {
        &self.layout
    }

    /// Single-use custody identity of this moved bundle.
    pub const fn custody(&self) -> TaskArgumentCustodyId {
        self.custody
    }

    /// The packed marshalling image: `layout().bytes` long, each argument at
    /// its canonical field offset with zeroed padding. This is the byte
    /// array the provider writes into the activation's argument area.
    pub fn image(&self) -> &[u8] {
        &self.image
    }

    /// Number of arguments the bundle carries.
    pub fn argument_count(&self) -> usize {
        self.layout.fields.len()
    }

    /// The marshalled bytes of argument `index`, or `None` when out of
    /// range.
    pub fn argument(&self, index: usize) -> Option<&[u8]> {
        let field = self.layout.fields.get(index)?;
        let offset = usize::try_from(field.offset).ok()?;
        let bytes = usize::try_from(field.bytes).ok()?;
        self.image.get(offset..offset.checked_add(bytes)?)
    }
}

/// The activation-storage custody presented with one task start.
#[derive(Debug, PartialEq, Eq)]
pub enum TaskStartStorage {
    /// A nonmoving stack lease established against this activation's plan.
    Persistent(StackLease),
    /// The provider reports the activation completed during start and
    /// retained no persistent activation storage. Its lifecycle claim still
    /// requires settlement.
    InlineCompletion,
}

/// A rejected transactional task start.
///
/// Every moved argument and the supplied storage custody return to the
/// caller; nothing linear stays behind in a failed provider call. The
/// retained activation identity and diagnostic explain why admission failed.
#[derive(Debug)]
pub struct TaskStartRejection {
    activation: ActivationInstanceId,
    arguments: MovedTaskArguments,
    storage: TaskStartStorage,
    diagnostic: TaskPlanDiagnostic,
}

impl TaskStartRejection {
    pub(crate) const fn new(
        activation: ActivationInstanceId,
        arguments: MovedTaskArguments,
        storage: TaskStartStorage,
        diagnostic: TaskPlanDiagnostic,
    ) -> Self {
        Self {
            activation,
            arguments,
            storage,
            diagnostic,
        }
    }

    /// Why the provider call refused custody.
    pub const fn diagnostic(&self) -> &TaskPlanDiagnostic {
        &self.diagnostic
    }

    /// The activation identity the rejected start was aimed at.
    pub const fn activation(&self) -> ActivationInstanceId {
        self.activation
    }

    /// Consume the rejection and return the conserved custody: every moved
    /// argument and the supplied storage reservation/lease.
    pub fn into_custody(self) -> (MovedTaskArguments, TaskStartStorage) {
        (self.arguments, self.storage)
    }
}
