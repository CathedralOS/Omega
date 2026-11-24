use crate::{AccessPermissions, AtomicPermissions, ExternalRead};
use record_storage::{Record, RecordError, RecordReader, RecordWriter};

/// Operations a policy requests on a field. The storage owner separately
/// checks whether the backing can provide them at that field's geometry.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ObservationDemand {
    Stable {
        read: bool,
        take: bool,
        write: bool,
        swap: bool,
    },
    External {
        read: ExternalRead,
        write: bool,
    },
    Atomic(AtomicPermissions),
}

impl ObservationDemand {
    pub fn any(self) -> bool {
        match self {
            Self::Stable {
                read,
                take,
                write,
                swap,
            } => read || take || write || swap,
            Self::External { read, write } => read != ExternalRead::None || write,
            Self::Atomic(ops) => ops.any(),
        }
    }

    /// Snapshots and Psi artifacts previously stored a family tag followed
    /// by the same permission flags. A fourth tag carries Stable swap while
    /// keeping the original encodings for existing policies.
    pub fn stored_permissions(self) -> (u8, AccessPermissions) {
        let mut flags = AccessPermissions::default();
        let tag = match self {
            Self::Stable {
                read,
                take,
                write,
                swap,
            } => {
                flags.read = read;
                flags.take = take;
                flags.write = write;
                if swap { 3 } else { 0 }
            }
            Self::External { read, write } => {
                flags.read = read == ExternalRead::Read;
                flags.take = read == ExternalRead::Take;
                flags.write = write;
                1
            }
            Self::Atomic(ops) => {
                flags.atomic = ops;
                2
            }
        };
        (tag, flags)
    }

    pub fn from_stored_permissions(tag: u8, flags: AccessPermissions) -> Option<Self> {
        match tag {
            0 | 3 if !flags.atomic.any() => Some(Self::Stable {
                read: flags.read,
                take: flags.take,
                write: flags.write,
                swap: tag == 3,
            }),
            1 if !flags.atomic.any() && !(flags.read && flags.take) => Some(Self::External {
                read: if flags.read {
                    ExternalRead::Read
                } else if flags.take {
                    ExternalRead::Take
                } else {
                    ExternalRead::None
                },
                write: flags.write,
            }),
            2 if !(flags.read || flags.take || flags.write) => Some(Self::Atomic(flags.atomic)),
            _ => None,
        }
    }
}

// A plan can intersect multiple rules for one field. Keep the canonical
// ordering of the original permission flags when normalizing restored rows.
impl Ord for ObservationDemand {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.stored_permissions().cmp(&other.stored_permissions())
    }
}

impl PartialOrd for ObservationDemand {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Record for ObservationDemand {
    fn write(&self, output: &mut RecordWriter) -> Result<(), RecordError> {
        let (tag, flags) = self.stored_permissions();
        u32::from(tag).write(output)?;
        flags.write(output)
    }

    fn read<Context>(input: &mut RecordReader<'_, Context>) -> Result<Self, RecordError> {
        let tag = u8::try_from(u32::read(input)?)
            .map_err(|_| input.error("unknown field observation family"))?;
        Self::from_stored_permissions(tag, AccessPermissions::read(input)?)
            .ok_or_else(|| input.error("operations do not belong to the field observation family"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{AccessExposure, ObservationModel};
    use record_storage::RecordLimits;

    #[test]
    fn stored_domain_observations_preserve_existing_snapshots() {
        let original = Some((
            ObservationModel::External,
            AccessPermissions {
                take: true,
                write: true,
                ..Default::default()
            },
            AccessExposure::Exported,
        ));
        let mut writer = RecordWriter::new(RecordLimits::default());
        original.write(&mut writer).unwrap();
        let bytes = writer.finish();
        let mut reader = RecordReader::<()>::new(&bytes, RecordLimits::default()).unwrap();
        let restored = Option::<(ObservationDemand, AccessExposure)>::read(&mut reader).unwrap();
        reader.finish().unwrap();
        assert_eq!(
            restored,
            Some((
                ObservationDemand::External {
                    read: ExternalRead::Take,
                    write: true
                },
                AccessExposure::Exported
            ))
        );

        let mut writer = RecordWriter::new(RecordLimits::default());
        restored.write(&mut writer).unwrap();
        assert_eq!(writer.finish(), bytes);
        let external = |read| ObservationDemand::External { read, write: false };
        assert!(external(ExternalRead::Take) < external(ExternalRead::Read));
    }
}
