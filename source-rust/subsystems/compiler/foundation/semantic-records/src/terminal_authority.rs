//! Canonical sets of physical terminal-authority classes and their filesystem facets.
use crate::TerminalAuthorityClass;
use record_storage::{Record, RecordError, RecordReader, RecordWriter};
use std::collections::BTreeSet;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum PortableFilesystemAuthorityFacet {
    ContentRead,
    ContentWrite,
    MetadataQuery,
    DirectoryEnumeration,
    NamespaceMutation,
    MetadataMutation,
}

impl PortableFilesystemAuthorityFacet {
    pub const ALL: [Self; 6] = [
        Self::ContentRead,
        Self::ContentWrite,
        Self::MetadataQuery,
        Self::DirectoryEnumeration,
        Self::NamespaceMutation,
        Self::MetadataMutation,
    ];

    pub const fn terminal_authority_class(self) -> TerminalAuthorityClass {
        match self {
            Self::ContentRead => TerminalAuthorityClass::FilesystemContentRead,
            Self::ContentWrite => TerminalAuthorityClass::FilesystemContentWrite,
            Self::MetadataQuery => TerminalAuthorityClass::FilesystemMetadataQuery,
            Self::DirectoryEnumeration => TerminalAuthorityClass::DirectoryEnumeration,
            Self::NamespaceMutation => TerminalAuthorityClass::FilesystemNamespaceMutation,
            Self::MetadataMutation => TerminalAuthorityClass::FilesystemMetadataMutation,
        }
    }
}

impl From<PortableFilesystemAuthorityFacet> for TerminalAuthorityClass {
    fn from(facet: PortableFilesystemAuthorityFacet) -> Self {
        facet.terminal_authority_class()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TerminalAuthorityDisposition {
    classes: Vec<TerminalAuthorityClass>,
}

impl Record for TerminalAuthorityDisposition {
    fn write(&self, output: &mut RecordWriter) -> Result<(), RecordError> {
        output.record(|output| self.classes.write(output))
    }

    fn read<Context>(input: &mut RecordReader<'_, Context>) -> Result<Self, RecordError> {
        input.record(|input| {
            let classes = Vec::<TerminalAuthorityClass>::read(input)?;
            Self::try_from_canonical_classes(classes)
                .map_err(|_| input.error("authority classes are not strictly ordered"))
        })
    }
}

impl TerminalAuthorityDisposition {
    pub fn try_from_canonical_classes(
        classes: Vec<TerminalAuthorityClass>,
    ) -> Result<Self, Vec<TerminalAuthorityClass>> {
        if classes.windows(2).any(|pair| pair[0] >= pair[1]) {
            Err(classes)
        } else {
            Ok(Self { classes })
        }
    }

    pub fn from_classes(classes: impl IntoIterator<Item = TerminalAuthorityClass>) -> Self {
        let classes = classes
            .into_iter()
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect();
        Self { classes }
    }

    pub fn from_filesystem_facets(
        facets: impl IntoIterator<Item = PortableFilesystemAuthorityFacet>,
    ) -> Self {
        Self::from_classes(facets.into_iter().map(TerminalAuthorityClass::from))
    }

    pub fn classes(&self) -> &[TerminalAuthorityClass] {
        &self.classes
    }

    pub fn contains_all(&self, other: &Self) -> bool {
        self.first_missing_class(other).is_none()
    }

    pub fn first_missing_class(&self, other: &Self) -> Option<TerminalAuthorityClass> {
        other
            .classes
            .iter()
            .find(|class| self.classes.binary_search(class).is_err())
            .copied()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use record_storage::{RecordLimits, decode, encode};

    #[test]
    fn stored_dispositions_require_the_canonical_class_order() {
        // The stored record contains one vector of class tags.
        #[derive(Record)]
        struct StoredDisposition {
            classes: Vec<TerminalAuthorityClass>,
        }

        use TerminalAuthorityClass::{MachineControl, PortIo};
        let bytes =
            |classes| encode(&StoredDisposition { classes }, RecordLimits::default()).unwrap();
        for classes in [vec![], vec![MachineControl, PortIo]] {
            let value = TerminalAuthorityDisposition::from_classes(classes.clone());
            let stored = bytes(classes);
            assert_eq!(encode(&value, RecordLimits::default()).unwrap(), stored);
            assert_eq!(
                decode::<TerminalAuthorityDisposition>(&stored, RecordLimits::default()).unwrap(),
                value
            );
        }
        for classes in [vec![PortIo, MachineControl], vec![PortIo, PortIo]] {
            assert!(
                decode::<TerminalAuthorityDisposition>(&bytes(classes), RecordLimits::default())
                    .is_err()
            );
        }
    }
}
