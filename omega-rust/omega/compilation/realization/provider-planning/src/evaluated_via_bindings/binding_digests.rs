//! Producer closure, stable source, evaluation and materialization
//! digests.

use crate::evaluated_via_bindings::MATERIALIZER_SCHEMA_VERSION;
use build_time_evaluation::{BuildTimeValue, CURRENT_EVALUATION_SEMANTICS, EvaluationUsage};
use checked_trees::typed_trees::TypedTrees;
use diagnostics::Diagnostic;
use effects::provider_plan::{
    EvaluatedBindingEvaluationDigest, EvaluatedBindingMaterializationDigest,
    EvaluatedBindingProducerClosureDigest,
};
use source::{SourceFile, SourceOrigin, SourceSpan};
use std::path::Path;
use symbols::SymbolHandle;
use target::TargetProfile;

pub(crate) fn producer_closure_digest(
    typed: &TypedTrees,
    closure: &[SymbolHandle],
) -> Result<EvaluatedBindingProducerClosureDigest, String> {
    let mut entries = closure
        .iter()
        .map(|symbol| {
            let machine = typed
                .machine(*symbol)
                .ok_or_else(|| "evaluated binding closure contains a missing machine".to_owned())?;
            let identity = typed
                .machine_callable_identity_text(machine)
                .ok_or_else(|| {
                    "evaluated binding closure machine has no canonical callable identity"
                        .to_owned()
                })?;
            let span = typed
                .symbols
                .symbol_provenance_source_span(*symbol)
                .ok_or_else(|| {
                    format!(
                        "evaluated binding closure machine `{}` has no source custody",
                        machine.name
                    )
                })?;
            let source = typed.symbols.source_file(span).ok_or_else(|| {
                format!(
                    "evaluated binding closure machine `{}` has no source file",
                    machine.name
                )
            })?;
            let source_digest = stable_source_digest(source)?;
            Ok((
                identity,
                typed.symbols.symbol_package_identity(*symbol),
                source_digest,
            ))
        })
        .collect::<Result<Vec<_>, String>>()?;
    entries.sort_by(|left, right| left.0.cmp(&right.0).then_with(|| left.2.cmp(&right.2)));
    let mut hash = crate::digest_encoder::DomainDigestEncoder::new(
        b"omega.evaluated-binding-producer-closure.sha256.v1\0",
    );
    hash_u64(&mut hash, entries.len() as u64);
    for (identity, package, source) in entries {
        hash.bytes(identity.as_bytes());
        match package {
            Some(package) => {
                hash.byte(1);
                hash_field(&mut hash, &package.digest());
            }
            None => hash.byte(0),
        }
        hash_field(&mut hash, &source);
    }
    EvaluatedBindingProducerClosureDigest::from_bytes(hash.finish())
}

fn stable_source_digest(source: &SourceFile) -> Result<[u8; 32], String> {
    if source.origin == SourceOrigin::Toolchain {
        return package_compilation::toolchain_source_identity_digest(source).map_err(
            |diagnostics| {
                diagnostics
                    .into_iter()
                    .map(|diagnostic| diagnostic.to_string())
                    .collect::<Vec<_>>()
                    .join("; ")
            },
        );
    }
    let relative = match source.path.strip_prefix(&source.package_root) {
        Ok(relative) => relative,
        Err(_) if source.package_identity.is_none() => {
            Path::new(source.path.file_name().unwrap_or_default())
        }
        Err(_) => {
            return Err(format!(
                "source `{}` is outside its retained package root",
                source.path.display()
            ));
        }
    };
    let mut hash = crate::digest_encoder::DomainDigestEncoder::new(
        b"omega.evaluated-binding-user-source.sha256.v1\0",
    );
    match source.package_identity {
        Some(package) => {
            hash.byte(1);
            hash_field(&mut hash, &package.digest());
        }
        None => hash.byte(0),
    }
    hash.bytes(relative.to_string_lossy().as_bytes());
    hash.bytes(source.source.as_bytes());
    Ok(hash.finish())
}

pub(crate) fn evaluation_digest(
    target: TargetProfile,
    closure: EvaluatedBindingProducerClosureDigest,
    usage: EvaluationUsage,
    value: &BuildTimeValue,
) -> EvaluatedBindingEvaluationDigest {
    let mut hash = crate::digest_encoder::DomainDigestEncoder::new(
        b"omega.evaluated-binding-evaluation.sha256.v1\0",
    );
    hash.bytes(target.identity().as_str().as_bytes());
    hash_field(&mut hash, &closure.as_bytes());
    hash.raw(&CURRENT_EVALUATION_SEMANTICS.marker().to_le_bytes());
    encode_usage(&mut hash, usage);
    encode_value(&mut hash, value);
    EvaluatedBindingEvaluationDigest::from_bytes(hash.finish())
        .expect("domain-separated SHA-256 is nonzero")
}

pub(crate) fn materialization_digest(
    target: TargetProfile,
    vocabulary: [u8; 32],
    widths: [u64; 3],
    value: &BuildTimeValue,
    locator: [u8; 32],
) -> EvaluatedBindingMaterializationDigest {
    let mut hash = crate::digest_encoder::DomainDigestEncoder::new(
        b"omega.evaluated-binding-materialization.sha256.v1\0",
    );
    hash.raw(&MATERIALIZER_SCHEMA_VERSION.to_le_bytes());
    hash.bytes(target.identity().as_str().as_bytes());
    hash_field(&mut hash, &vocabulary);
    for width in widths {
        hash.u64(width);
    }
    encode_value(&mut hash, value);
    hash_field(&mut hash, &locator);
    EvaluatedBindingMaterializationDigest::from_bytes(hash.finish())
        .expect("domain-separated SHA-256 is nonzero")
}

fn encode_usage(hash: &mut crate::digest_encoder::DomainDigestEncoder, usage: EvaluationUsage) {
    for value in [
        u64::from(usage.schema().schema_version()),
        u64::from(usage.schedule().marker()),
        usage.fuel_units(),
        usage.fuel_ceiling(),
        usage.build_log_bytes(),
        usage.filesystem_operation_attempts(),
        usage.peak_live_cells(),
        usage.peak_live_text_bytes(),
        usage.result_cells(),
        usage.result_text_bytes(),
    ] {
        hash_u64(hash, value);
    }
}

fn encode_value(hash: &mut crate::digest_encoder::DomainDigestEncoder, value: &BuildTimeValue) {
    match value {
        BuildTimeValue::Unit => hash.byte(0),
        BuildTimeValue::Int(value) => {
            hash.byte(1);
            hash.raw(&value.to_le_bytes());
        }
        BuildTimeValue::Bool(value) => {
            hash.byte(2);
            hash.byte(u8::from(*value))
        }
        BuildTimeValue::Float(value) => {
            hash.byte(3);
            hash.raw(&value.to_bits().to_le_bytes());
        }
        BuildTimeValue::UndeterminedNaN(format) => {
            // Commit the semantic input without inventing a runtime payload.
            // The separate materialization judgment still requires bits.
            hash.byte(8);
            hash_field(hash, format.name().as_bytes());
        }
        BuildTimeValue::Text(bytes) => {
            hash.byte(4);
            hash_field(hash, bytes);
        }
        BuildTimeValue::Struct { type_name, fields } => {
            hash.byte(5);
            hash_field(hash, type_name.as_bytes());
            encode_fields(hash, fields);
        }
        BuildTimeValue::Case { variant, payload } => {
            hash.byte(6);
            hash_field(hash, variant.as_bytes());
            encode_fields(hash, payload);
        }
        BuildTimeValue::Array(elements) => {
            hash.byte(7);
            hash_u64(hash, elements.len() as u64);
            for element in elements {
                encode_value(hash, element);
            }
        }
    }
}

fn encode_fields(
    hash: &mut crate::digest_encoder::DomainDigestEncoder,
    fields: &[(String, BuildTimeValue)],
) {
    hash_u64(hash, fields.len() as u64);
    for (name, value) in fields {
        hash_field(hash, name.as_bytes());
        encode_value(hash, value);
    }
}

fn hash_field(hash: &mut crate::digest_encoder::DomainDigestEncoder, bytes: &[u8]) {
    hash_u64(hash, bytes.len() as u64);
    hash.raw(&bytes);
}

fn hash_u64(hash: &mut crate::digest_encoder::DomainDigestEncoder, value: u64) {
    hash.raw(&value.to_le_bytes());
}

pub(crate) fn at(source_span: SourceSpan, message: impl Into<String>) -> Diagnostic {
    Diagnostic::error(message.into()).with_source_span(source_span)
}
