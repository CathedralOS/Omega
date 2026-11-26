// `omega.lock` framing: the shared length-delimited rows and sections, whose
// sections carry no trailing LF and whose framing faults all refuse as
// `InvalidFraming` apart from the byte and count limits.

use super::super::PackageLockError as Error;

pub(super) type Reader<'text> = crate::framing::Reader<'text, Error>;
pub(super) type Writer = crate::framing::Writer<Error>;

pub(super) static LOCK: crate::framing::Framing<Error> = crate::framing::Framing {
    errors: crate::framing::FramingErrors {
        byte_limit: Error::ByteLimitExceeded,
        allocation: Error::AllocationFailed,
        missing_line: Error::InvalidFraming,
        unexpected_line: Error::InvalidFraming,
        malformed_field: Error::InvalidFraming,
        unexpected_field: Error::InvalidFraming,
        noncanonical_count: Error::InvalidFraming,
        count_overflow: Error::InvalidFraming,
        count_limit: Error::CountLimitExceeded,
        section_length: Error::InvalidFraming,
        missing_section_terminator: Error::InvalidFraming,
        trailing_bytes: Error::InvalidFraming,
    },
    section_terminator: false,
};
