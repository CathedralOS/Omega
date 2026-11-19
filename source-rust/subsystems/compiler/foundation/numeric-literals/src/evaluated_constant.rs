//! A constant result retained by source program representations.

use crate::FloatFormat;

#[derive(Debug, Clone, PartialEq, Eq, record_storage::Record)]
pub enum EvaluatedConstant {
    Encoded(String), // An existing declaration encoding; index consumers still validate eligibility.
    UndeterminedNaN(FloatFormat), // Format and NaN meaning are known, but no observable payload bits are fixed.
}

impl EvaluatedConstant {
    pub fn encoding(&self) -> Option<&str> {
        match self {
            Self::Encoded(encoding) => Some(encoding),
            Self::UndeterminedNaN(_) => None,
        }
    }
}

impl Default for EvaluatedConstant {
    fn default() -> Self {
        Self::Encoded(String::new())
    }
}

impl From<String> for EvaluatedConstant {
    fn from(encoding: String) -> Self {
        Self::Encoded(encoding)
    }
}

#[cfg(test)]
mod tests {
    use super::{EvaluatedConstant, FloatFormat};

    #[test]
    fn semantic_nan_never_supplies_an_encoding() {
        for format in [FloatFormat::F32, FloatFormat::F64] {
            assert_eq!(EvaluatedConstant::UndeterminedNaN(format).encoding(), None);
        }
        let encoded = EvaluatedConstant::from("boolean4:true".to_owned());
        assert_eq!(encoded.encoding(), Some("boolean4:true"));
    }
}
