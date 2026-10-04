use super::super::PackageReviewNominalIdentity;
use super::PackageReviewOperatorCoordinate;

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
    ///
    /// A named boundary operator's policy requirement is the framed
    /// `boundary-operator-policy` coordinate; a top-level `boundary
    /// requirement` is selected by its normalized named-callable overload,
    /// whose path, parameters and result dispatch are the same three
    /// coordinate parts under the `named-callable` framing typed Psi writes.
    pub(crate) fn matches_policy_requirement(&self, expected: &str) -> bool {
        let mut writer = MatchingWriter {
            remaining: expected,
        };
        if self.write_policy_requirement(&mut writer).is_ok() && writer.remaining.is_empty() {
            return true;
        }
        let mut writer = MatchingWriter {
            remaining: expected,
        };
        self.write_named_callable_requirement(&mut writer).is_ok() && writer.remaining.is_empty()
    }

    /// The exact text `NormalizedNamedCallableIdentity::identity` produces for
    /// this coordinate's parts: `named-callable(path(..),parameters(..),
    /// result-dispatch(..))` with `\`, `(`, `)` and `,` escaped inside each atom.
    fn write_named_callable_requirement(
        &self,
        writer: &mut impl std::fmt::Write,
    ) -> std::fmt::Result {
        writer.write_str("named-callable(")?;
        for (index, (tag, value)) in [
            ("path", self.identity.path.as_str()),
            ("parameters", self.parameter_dispatch.as_str()),
            ("result-dispatch", self.result_dispatch.as_str()),
        ]
        .into_iter()
        .enumerate()
        {
            if index > 0 {
                writer.write_char(',')?;
            }
            writer.write_str(tag)?;
            writer.write_char('(')?;
            for character in value.chars() {
                if matches!(character, '\\' | '(' | ')' | ',') {
                    writer.write_char('\\')?;
                }
                writer.write_char(character)?;
            }
            writer.write_char(')')?;
        }
        writer.write_char(')')
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
