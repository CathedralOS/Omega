#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct PackageName(String);

impl PackageName {
    pub fn parse(value: impl Into<String>) -> Result<Self, String> {
        let value = value.into();
        if is_kebab_case(&value) {
            Ok(Self(value))
        } else {
            Err(format!(
                "package identity `{value}` must start with a lowercase letter and use kebab-case lowercase words"
            ))
        }
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

fn is_kebab_case(value: &str) -> bool {
    if !value.as_bytes().first().is_some_and(u8::is_ascii_lowercase) || value.ends_with('-') {
        return false;
    }

    let mut previous_separator = false;
    for byte in value.bytes() {
        if byte == b'-' {
            if previous_separator {
                return false;
            }
            previous_separator = true;
            continue;
        }
        previous_separator = false;
        if !byte.is_ascii_lowercase() && !byte.is_ascii_digit() {
            return false;
        }
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn package_names_require_canonical_kebab_case() {
        assert!(PackageName::parse("arithmetic-kernels").is_ok());
        assert!(PackageName::parse("sha256").is_ok());
        assert!(PackageName::parse("codec-2").is_ok());
        for invalid in [
            "",
            "Arithmetic-kernels",
            "arithmetic_kernels",
            "-arithmetic",
            "arithmetic-",
            "arithmetic--kernels",
            "arithmetic.kernels",
            "123-tools",
        ] {
            assert!(PackageName::parse(invalid).is_err(), "accepted {invalid:?}");
        }
    }
}
