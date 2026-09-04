//! Exact named-const values used by reviewed contract calls.

use omega_compiler::CheckedCompilation;
use psi_diagnostics::Diagnostic;
use psi_language_semantics::const_value::{CanonicalConstValue, DecodedCanonicalConstValue};

use crate::capture::semantics::types::review_type_identity_with_binders;
use crate::record::PackageReviewContractStaticArgument;

pub(super) fn project_named_const_static_argument(
    compilation: &CheckedCompilation,
    subject_kind: &str,
    subject_name: &str,
    declaration: &psi_typed_trees::constant::ConstDeclaration,
) -> Result<PackageReviewContractStaticArgument, Vec<Diagnostic>> {
    let rejected = |reason: &str| {
        vec![Diagnostic::error(format!(
            "reviewed {subject_kind} `{subject_name}` uses a named const static argument {reason}",
        ))]
    };
    let Some(encoding) = declaration.canonical_value_encoding.as_deref() else {
        return Err(rejected("without an admitted canonical public value"));
    };
    let Some(decoded) = decode_canonical_const_value(encoding) else {
        return Err(rejected("with a malformed canonical value encoding"));
    };
    psi_validation::validate_exact_const_value_encoding(
        &compilation.typed,
        declaration.declared_type,
        encoding,
    )
    .map_err(|reason| {
        rejected(&format!(
            "whose canonical value does not replay against its exact declared carrier: {reason}"
        ))
    })?;
    match decoded {
        DecodedCanonicalConstValue::Integer { type_name, value }
            if canonical_integer_type_name(type_name.as_str()) =>
        {
            return Ok(PackageReviewContractStaticArgument::ConstInteger(
                value.to_string(),
            ));
        }
        DecodedCanonicalConstValue::Boolean(value) => {
            return Ok(PackageReviewContractStaticArgument::ConstBoolean(value));
        }
        DecodedCanonicalConstValue::Array { .. }
        | DecodedCanonicalConstValue::Record { .. }
        | DecodedCanonicalConstValue::Variant { .. } => {}
        DecodedCanonicalConstValue::Integer { .. } => {
            return Err(rejected(
                "outside the exact scalar carrier admitted by package review",
            ));
        }
    }
    Ok(PackageReviewContractStaticArgument::ConstStructured {
        declared_type: review_type_identity_with_binders(
            compilation,
            declaration.declared_type,
            &[],
        )?,
        canonical_value_encoding: encoding.to_owned(),
    })
}

fn decode_canonical_const_value(encoding: &str) -> Option<DecodedCanonicalConstValue> {
    CanonicalConstValue::new("", encoding, "").decode_encoding()
}

fn canonical_integer_type_name(type_name: &str) -> bool {
    matches!(
        type_name,
        "i8" | "i16" | "i32" | "i64" | "u8" | "u16" | "u32" | "u64" | "addr"
    )
}
