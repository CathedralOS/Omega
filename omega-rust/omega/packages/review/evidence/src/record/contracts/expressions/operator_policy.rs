use super::{PackageReviewNominalIdentity, PackageReviewOperatorCoordinate};

impl PackageReviewOperatorCoordinate {
    pub(crate) fn policy_requirement_identity(&self) -> PackageReviewNominalIdentity {
        let mut path = String::new();
        let _ = self.write_policy_requirement(&mut path);
        PackageReviewNominalIdentity {
            owner: self.identity.owner,
            path,
        }
    }

    /// Compare framed semantic coordinates without allocating during recovery.
    pub(crate) fn matches_policy_requirement(&self, expected: &str) -> bool {
        let mut writer = MatchingWriter {
            remaining: expected,
        };
        self.write_policy_requirement(&mut writer).is_ok() && writer.remaining.is_empty()
    }

    fn write_policy_requirement(&self, writer: &mut impl std::fmt::Write) -> std::fmt::Result {
        crate::record::write_framed_identity(
            writer,
            "boundary-operator-policy",
            [
                self.identity.path.as_str(),
                self.parameter_dispatch.as_str(),
                self.result_dispatch.as_str(),
            ],
        )
    }
}

struct MatchingWriter<'text> {
    remaining: &'text str,
}

impl std::fmt::Write for MatchingWriter<'_> {
    fn write_str(&mut self, value: &str) -> std::fmt::Result {
        self.remaining = self.remaining.strip_prefix(value).ok_or(std::fmt::Error)?;
        Ok(())
    }
}
