//! Producer closure, stable source, evaluation and materialization
//! digests.

use crate::build_time_evaluation::{BuildTimeValue, CURRENT_EVALUATION_SEMANTICS, EvaluationUsage};
use crate::provider_planning::evaluated_via_bindings::MATERIALIZER_SCHEMA_VERSION;
use abstract_operations_to_target_operations::effects::provider_plan::{
    EvaluatedBindingEvaluationDigest, EvaluatedBindingMaterializationDigest,
    EvaluatedBindingProducerClosureDigest,
};
use diagnostics::Diagnostic;
use sha2::Digest;
use sha2::Sha256;
use source::{SourceFile, SourceOrigin, SourceSpan};
use std::path::Path;
use symbol_resolved_trees_to_typed_trees::typed_trees::TypedTrees;
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
                .machines()
                .iter()
                .find(|machine| machine.symbol == *symbol)
                .ok_or_else(|| "evaluated binding closure contains a missing machine".to_owned())?;
            let identity = typed
                .normalized_machine_overload_identity(machine)
                .map(|identity| identity.identity().to_owned())
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
    let mut hash = Sha256::new();
    hash.update(b"omega.evaluated-binding-producer-closure.sha256.v1\0");
    hash_u64(&mut hash, entries.len() as u64);
    for (identity, package, source) in entries {
        hash_field(&mut hash, identity.as_bytes());
        match package {
            Some(package) => {
                hash.update([1]);
                hash_field(&mut hash, &package.digest());
            }
            None => hash.update([0]),
        }
        hash_field(&mut hash, &source);
    }
    EvaluatedBindingProducerClosureDigest::from_bytes(hash.finalize().into())
}

fn stable_source_digest(source: &SourceFile) -> Result<[u8; 32], String> {
    if source.origin == SourceOrigin::Toolchain {
        return crate::package_compilation::toolchain_source_identity_digest(source).map_err(
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
    let mut hash = Sha256::new();
    hash.update(b"omega.evaluated-binding-user-source.sha256.v1\0");
    match source.package_identity {
        Some(package) => {
            hash.update([1]);
            hash_field(&mut hash, &package.digest());
        }
        None => hash.update([0]),
    }
    hash_field(&mut hash, relative.to_string_lossy().as_bytes());
    hash_field(&mut hash, source.source.as_bytes());
    Ok(hash.finalize().into())
}

pub(crate) fn evaluation_digest(
    target: TargetProfile,
    closure: EvaluatedBindingProducerClosureDigest,
    usage: EvaluationUsage,
    value: &BuildTimeValue,
) -> EvaluatedBindingEvaluationDigest {
    let mut hash = Sha256::new();
    hash.update(b"omega.evaluated-binding-evaluation.sha256.v1\0");
    hash_field(&mut hash, target.identity().as_str().as_bytes());
    hash_field(&mut hash, &closure.as_bytes());
    hash.update(CURRENT_EVALUATION_SEMANTICS.marker().to_le_bytes());
    encode_usage(&mut hash, usage);
    encode_value(&mut hash, value);
    EvaluatedBindingEvaluationDigest::from_bytes(hash.finalize().into())
        .expect("domain-separated SHA-256 is nonzero")
}

pub(crate) fn materialization_digest(
    target: TargetProfile,
    vocabulary: [u8; 32],
    widths: [u64; 3],
    value: &BuildTimeValue,
    locator: [u8; 32],
) -> EvaluatedBindingMaterializationDigest {
    let mut hash = Sha256::new();
    hash.update(b"omega.evaluated-binding-materialization.sha256.v1\0");
    hash.update(MATERIALIZER_SCHEMA_VERSION.to_le_bytes());
    hash_field(&mut hash, target.identity().as_str().as_bytes());
    hash_field(&mut hash, &vocabulary);
    for width in widths {
        hash_u64(&mut hash, width);
    }
    encode_value(&mut hash, value);
    hash_field(&mut hash, &locator);
    EvaluatedBindingMaterializationDigest::from_bytes(hash.finalize().into())
        .expect("domain-separated SHA-256 is nonzero")
}

fn encode_usage(hash: &mut Sha256, usage: EvaluationUsage) {
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

fn encode_value(hash: &mut Sha256, value: &BuildTimeValue) {
    match value {
        BuildTimeValue::Unit => hash.update([0]),
        BuildTimeValue::Int(value) => {
            hash.update([1]);
            hash.update(value.to_le_bytes());
        }
        BuildTimeValue::Bool(value) => hash.update([2, u8::from(*value)]),
        BuildTimeValue::Float(value) => {
            hash.update([3]);
            hash.update(value.to_bits().to_le_bytes());
        }
        BuildTimeValue::Text(bytes) => {
            hash.update([4]);
            hash_field(hash, bytes);
        }
        BuildTimeValue::Struct { type_name, fields } => {
            hash.update([5]);
            hash_field(hash, type_name.as_bytes());
            encode_fields(hash, fields);
        }
        BuildTimeValue::Case { variant, payload } => {
            hash.update([6]);
            hash_field(hash, variant.as_bytes());
            encode_fields(hash, payload);
        }
        BuildTimeValue::Array(elements) => {
            hash.update([7]);
            hash_u64(hash, elements.len() as u64);
            for element in elements {
                encode_value(hash, element);
            }
        }
    }
}

fn encode_fields(hash: &mut Sha256, fields: &[(String, BuildTimeValue)]) {
    hash_u64(hash, fields.len() as u64);
    for (name, value) in fields {
        hash_field(hash, name.as_bytes());
        encode_value(hash, value);
    }
}

fn hash_field(hash: &mut Sha256, bytes: &[u8]) {
    hash_u64(hash, bytes.len() as u64);
    hash.update(bytes);
}

fn hash_u64(hash: &mut Sha256, value: u64) {
    hash.update(value.to_le_bytes());
}

pub(crate) fn at(source_span: SourceSpan, message: impl Into<String>) -> Diagnostic {
    Diagnostic::error(message.into()).with_source_span(source_span)
}
