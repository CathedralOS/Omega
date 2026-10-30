//! Shared restriction vocabulary. Naming a class or crash cause is not evidence
//! that a selected executable exercises it; classification remains with Omega.

/// Semantic crash outcomes shared by authored policy and Terminal control flow.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum CrashCause {
    Trap,
    Abort,
}

/// Mints a closed compiler vocabulary from one table.
///
/// Each of these vocabularies exists in several spellings at once: the `enum`,
/// its `ALL` roster, the authored spelling of every variant and the tag its
/// encoded form uses. The rows in each invocation are the one place those are
/// written; the enum, its canonical-order roster and its tags are generated
/// from them, so a tag can no longer drift from the declaration it belongs to.
macro_rules! tagged_vocabulary {
    // Vocabulary whose tag callers read through its own method.
    ($documentation:literal, $name:ident, $tag_method:ident { $($variant:ident = $tag:literal),+ $(,)? }) => {
        #[doc = $documentation]
        #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
        pub enum $name {
            $($variant,)+
        }

        impl $name {
            /// Every variant, in canonical (declaration) order.
            pub const ALL: [Self; [$(Self::$variant),+].len()] = [$(Self::$variant),+];

            /// This variant's canonical tag.
            pub const fn $tag_method(self) -> u8 {
                match self {
                    $(Self::$variant => $tag,)+
                }
            }
        }
    };
}

tagged_vocabulary!(
    "D45's closed physical terminal-authority vocabulary. Declaration order is the canonical encoded order; additions require a target-policy migration.",
    TerminalAuthorityClass,
    canonical_tag
    {
        FilesystemContentRead = 0,
        FilesystemContentWrite = 1,
        FilesystemMetadataQuery = 2,
        DirectoryEnumeration = 3,
        FilesystemNamespaceMutation = 4,
        FilesystemMetadataMutation = 5,
        ProcessOutput = 6,
        ProcessTermination = 7,
        MachineControl = 8,
        PortIo = 9,
        InterruptControl = 10,
        InterruptEntry = 11,
        RootMemoryAccess = 12,
        ProcessInput = 13,
    }
);

impl compiler_storage::Record for TerminalAuthorityClass {
    fn write(
        &self,
        output: &mut compiler_storage::RecordWriter,
    ) -> Result<(), compiler_storage::RecordError> {
        compiler_storage::Record::write(&self.canonical_tag(), output)
    }
    fn read<Context>(
        input: &mut compiler_storage::RecordReader<'_, Context>,
    ) -> Result<Self, compiler_storage::RecordError> {
        let tag = <u8 as compiler_storage::Record>::read(input)?;
        Self::ALL
            .into_iter()
            .find(|class| class.canonical_tag() == tag)
            .ok_or_else(|| input.error("unknown retained terminal authority class"))
    }
}

#[cfg(test)]
mod tests {
    use super::TerminalAuthorityClass;
    use compiler_storage::{RecordLimits, decode, encode};

    #[test]
    fn physical_policy_tags_preserve_retained_encoding() {
        use TerminalAuthorityClass::*;
        let expected = [
            FilesystemContentRead,
            FilesystemContentWrite,
            FilesystemMetadataQuery,
            DirectoryEnumeration,
            FilesystemNamespaceMutation,
            FilesystemMetadataMutation,
            ProcessOutput,
            ProcessTermination,
            MachineControl,
            PortIo,
            InterruptControl,
            InterruptEntry,
            RootMemoryAccess,
            ProcessInput,
        ];
        assert_eq!(TerminalAuthorityClass::ALL, expected);
        for (tag, class) in expected.into_iter().enumerate() {
            let bytes = [u8::try_from(tag).expect("14 canonical tags")];
            assert_eq!(class.canonical_tag(), bytes[0]);
            assert_eq!(encode(&class, RecordLimits::default()).unwrap(), bytes);
            assert_eq!(
                decode::<TerminalAuthorityClass>(&bytes, RecordLimits::default()).unwrap(),
                class,
            );
        }
        for invalid_tag in [14, 255] {
            assert!(
                decode::<TerminalAuthorityClass>(&[invalid_tag], RecordLimits::default(),).is_err()
            );
        }
    }
}
