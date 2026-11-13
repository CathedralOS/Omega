// Framed policy-requirement and `named-callable` coordinate comparison for
// `PackageReviewOperatorCoordinate`.
//
// `matches_policy_requirement` compares framed semantic coordinates without
// allocating during recovery: a named boundary operator's policy requirement is
// the framed `boundary-operator-policy` coordinate, while a top-level `boundary
// requirement` is selected by its normalized `named-callable` overload
// (`named-callable(path(..),parameters(..),result-dispatch(..))` with `\`, `(`,
// `)`, and `,` escaped inside each atom). `write_qualified_path` qualifies the
// declaration path under its owner namespace (`toolchain-owner` without source
// digest, or `package-owner(32:<hex>)`).

use super::PackageReviewOperatorCoordinate;
#[cfg(test)]
use crate::record::PackageReviewNominalIdentity;
use crate::record::PackageReviewNominalOwner;
use std::fmt::Write;

impl PackageReviewOperatorCoordinate {
    #[cfg(test)]
    pub(crate) fn policy_requirement_identity(&self) -> PackageReviewNominalIdentity {
        let mut path = String::new();
        let _ = self.write_policy_requirement(&mut path);
        PackageReviewNominalIdentity {
            owner: self.identity.owner,
            path,
        }
    }

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

    fn write_named_callable_requirement(&self, writer: &mut impl Write) -> std::fmt::Result {
        writer.write_str("named-callable(")?;
        write_atom(writer, "path", |value| self.write_qualified_path(value))?;
        writer.write_char(',')?;
        write_atom(writer, "parameters", |value| {
            value.write_str(&self.parameter_dispatch)
        })?;
        writer.write_char(',')?;
        write_atom(writer, "result-dispatch", |value| {
            value.write_str(&self.result_dispatch)
        })?;
        writer.write_char(')')
    }

    fn write_qualified_path(&self, writer: &mut impl Write) -> std::fmt::Result {
        writer.write_str("nominal(")?;
        match self.identity.owner {
            PackageReviewNominalOwner::Package(package) => {
                let digest = package.digest();
                write!(writer, "package-owner({}:", digest.len())?;
                for byte in digest {
                    write!(writer, "{byte:02x}")?;
                }
                writer.write_char(')')?;
            }
            PackageReviewNominalOwner::ToolchainSource(_) => writer.write_str("toolchain-owner")?,
            PackageReviewNominalOwner::Unresolved => writer.write_str("unresolved-owner")?,
        }
        writer.write_char(',')?;
        write_atom(writer, "path", |value| value.write_str(&self.identity.path))?;
        writer.write_char(')')
    }

    fn write_policy_requirement(&self, writer: &mut impl Write) -> std::fmt::Result {
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

fn write_atom<W: Write>(
    writer: &mut W,
    tag: &str,
    value: impl FnOnce(&mut EscapingWriter<'_, W>) -> std::fmt::Result,
) -> std::fmt::Result {
    writer.write_str(tag)?;
    writer.write_char('(')?;
    value(&mut EscapingWriter(writer))?;
    writer.write_char(')')
}

struct EscapingWriter<'writer, W>(&'writer mut W);

impl<W: Write> Write for EscapingWriter<'_, W> {
    fn write_str(&mut self, value: &str) -> std::fmt::Result {
        for character in value.chars() {
            if matches!(character, '\\' | '(' | ')' | ',') {
                self.0.write_char('\\')?;
            }
            self.0.write_char(character)?;
        }
        Ok(())
    }
}

struct MatchingWriter<'text> {
    remaining: &'text str,
}

impl Write for MatchingWriter<'_> {
    fn write_str(&mut self, value: &str) -> std::fmt::Result {
        self.remaining = self.remaining.strip_prefix(value).ok_or(std::fmt::Error)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::record::PackageReviewToolchainSourceIdentity;
    use semantic_vocabulary::PackageKeyIdentity;

    const FLOAT_ADD: &str = r"named-callable(path(nominal\(toolchain-owner\,path\(Float::add\)\)),parameters(parameter\(mutable\(no\)\,const\(no\)\,named\(name\(f64\)\)\)\,parameter\(mutable\(no\)\,const\(no\)\,named\(name\(f64\)\)\)),result-dispatch())";
    const FLOAT_ADD_PARAMETERS: &str = "parameter(mutable(no),const(no),named(name(f64))),parameter(mutable(no),const(no),named(name(f64)))";
    const CLOCK_SLEEP: &str = r"named-callable(path(nominal\(package-owner\(32:a0fc4b8bb77397bb6cba1ab5d7153b87d64b2536369880f681fe4c78c70d6144\)\,path\(Clock::sleep\)\)),parameters(parameter\(mutable\(no\)\,const\(no\)\,named\(name\(u32\)\)\)),result-dispatch())";
    const CLOCK_SLEEP_PARAMETERS: &str = "parameter(mutable(no),const(no),named(name(u32)))";
    const STANDARD_LIBRARY_DIGEST: &str =
        "a0fc4b8bb77397bb6cba1ab5d7153b87d64b2536369880f681fe4c78c70d6144";

    fn digest(hex: &str) -> [u8; 32] {
        let mut bytes = [0; 32];
        for (index, byte) in bytes.iter_mut().enumerate() {
            *byte = u8::from_str_radix(&hex[index * 2..index * 2 + 2], 16).unwrap();
        }
        bytes
    }

    fn coordinate(
        owner: PackageReviewNominalOwner,
        path: &str,
        parameters: &str,
    ) -> PackageReviewOperatorCoordinate {
        PackageReviewOperatorCoordinate {
            identity: PackageReviewNominalIdentity {
                owner,
                path: path.to_owned(),
            },
            parameter_dispatch: parameters.to_owned(),
            result_dispatch: String::new(),
        }
    }

    fn toolchain() -> PackageReviewNominalOwner {
        PackageReviewNominalOwner::ToolchainSource(PackageReviewToolchainSourceIdentity {
            digest: [7; 32],
        })
    }

    fn package(hex: &str) -> PackageReviewNominalOwner {
        PackageReviewNominalOwner::Package(PackageKeyIdentity::from_digest(digest(hex)).unwrap())
    }

    #[test]
    fn toolchain_requirement_matches_its_owner_qualified_callable_path() {
        let add = coordinate(toolchain(), "Float::add", FLOAT_ADD_PARAMETERS);
        assert!(add.matches_policy_requirement(FLOAT_ADD));
        let other_source = coordinate(
            PackageReviewNominalOwner::ToolchainSource(PackageReviewToolchainSourceIdentity {
                digest: [9; 32],
            }),
            "Float::add",
            FLOAT_ADD_PARAMETERS,
        );
        assert!(other_source.matches_policy_requirement(FLOAT_ADD));
    }

    #[test]
    fn package_requirement_matches_its_owner_qualified_callable_path() {
        let sleep = coordinate(
            package(STANDARD_LIBRARY_DIGEST),
            "Clock::sleep",
            CLOCK_SLEEP_PARAMETERS,
        );
        assert!(sleep.matches_policy_requirement(CLOCK_SLEEP));
    }

    #[test]
    fn requirement_rejects_another_owner_namespace_path_or_dispatch() {
        let add = coordinate(toolchain(), "Float::add", FLOAT_ADD_PARAMETERS);
        let sleep = coordinate(
            package(STANDARD_LIBRARY_DIGEST),
            "Clock::sleep",
            CLOCK_SLEEP_PARAMETERS,
        );
        assert!(
            !coordinate(
                package(STANDARD_LIBRARY_DIGEST),
                "Float::add",
                FLOAT_ADD_PARAMETERS
            )
            .matches_policy_requirement(FLOAT_ADD)
        );
        assert!(
            !coordinate(toolchain(), "Clock::sleep", CLOCK_SLEEP_PARAMETERS)
                .matches_policy_requirement(CLOCK_SLEEP)
        );
        assert!(
            !coordinate(
                PackageReviewNominalOwner::Unresolved,
                "Float::add",
                FLOAT_ADD_PARAMETERS
            )
            .matches_policy_requirement(FLOAT_ADD)
        );
        let mut other = digest(STANDARD_LIBRARY_DIGEST);
        other[31] ^= 1;
        let other_hex = other
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>();
        assert!(
            !coordinate(package(&other_hex), "Clock::sleep", CLOCK_SLEEP_PARAMETERS)
                .matches_policy_requirement(CLOCK_SLEEP)
        );
        let unqualified = FLOAT_ADD.replace(
            r"nominal\(toolchain-owner\,path\(Float::add\)\)",
            "Float::add",
        );
        assert_ne!(unqualified, FLOAT_ADD);
        assert!(!add.matches_policy_requirement(&unqualified));
        assert!(
            !coordinate(toolchain(), "Float::subtract", FLOAT_ADD_PARAMETERS)
                .matches_policy_requirement(FLOAT_ADD)
        );
        assert!(
            !coordinate(toolchain(), "Float::add", CLOCK_SLEEP_PARAMETERS)
                .matches_policy_requirement(FLOAT_ADD)
        );
        let mut dispatched = add.clone();
        dispatched.result_dispatch = "named(name(f64))".to_owned();
        assert!(!dispatched.matches_policy_requirement(FLOAT_ADD));
        assert!(!sleep.matches_policy_requirement(FLOAT_ADD));
    }

    #[test]
    fn named_operator_policy_requirement_keeps_its_framed_coordinate() {
        let operator = coordinate(
            toolchain(),
            "F64::square_root",
            "operands(named(name(f64)))",
        );
        let identity = operator.policy_requirement_identity();
        assert!(operator.matches_policy_requirement(&identity.path));
        assert!(
            !coordinate(toolchain(), "F64::absolute", "operands(named(name(f64)))")
                .matches_policy_requirement(&identity.path)
        );
    }
}
