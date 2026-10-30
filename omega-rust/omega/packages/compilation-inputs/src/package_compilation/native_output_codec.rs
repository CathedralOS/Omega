//! Complete checked source intent and original Build observation in one facet.
//! The compiler-private frame preserves immutable graphs/maps; semantic identity
//! uses source occurrence commitments and exact captured actual identities.
use super::{NativeBuildEvaluation, PackageBuildHandoff, native_bindings::NativeBuildOutputs};
use crate::build_execution::build_record_codec;
use filesystem_authority::observation_codec::{ObservationCursor, push_framed};
use language_semantics::declaration_selection::{CapturedNativeDeclaration, CapturedNativeIntent};
use std::sync::Arc;

fn owned<T: compiler_storage::Record>(value: &T) -> Vec<u8> {
    compiler_storage::encode(
        value,
        compiler_storage::RecordLimits {
            bytes: usize::MAX,
            allocation: usize::MAX,
            elements: usize::MAX,
            depth: usize::MAX,
        },
    )
    .expect("owned native graph encoding has no external resource limits")
}

pub(super) fn encode_parts(outputs: &NativeBuildOutputs) -> Vec<u8> {
    let mut output = b"OMGNTI08".to_vec();
    push_framed(
        &mut output,
        &owned(&(outputs.declarations.clone(), outputs.intents.clone())),
    );
    match outputs.evaluation.as_ref() {
        None => output.push(0),
        Some(evaluation) => {
            output.push(1);
            output.extend_from_slice(
                &evaluation
                    .terminal_code
                    .vocabulary_marker
                    .get()
                    .to_le_bytes(),
            );
            output.extend_from_slice(evaluation.terminal_code.program_fingerprint.as_bytes());
            let mut usage = Vec::new();
            build_record_codec::encode_build_evaluation_usage(&evaluation.usage, &mut usage);
            push_framed(&mut output, &usage);
            output.extend_from_slice(&evaluation.observation_digest);
        }
    }
    output
}

pub(super) fn decode_parts(bytes: &[u8]) -> Option<NativeBuildOutputs> {
    let mut cursor = ObservationCursor::new(bytes);
    if cursor.bytes(8)? != b"OMGNTI08" {
        return None;
    }
    let (declarations, intents): (Vec<CapturedNativeDeclaration>, Vec<CapturedNativeIntent>) =
        compiler_storage::decode(cursor.framed()?, compiler_storage::RecordLimits::default())
            .ok()?;
    let evaluation = match cursor.u8()? {
        0 => None,
        1 => {
            let vocabulary_marker = terminal_psi::VocabularyMarker::new(cursor.u16()?)?;
            let program_fingerprint =
                terminal_psi::SemanticFingerprint::from_bytes(cursor.bytes(32)?.try_into().ok()?);
            let mut usage_cursor = ObservationCursor::new(cursor.framed()?);
            let usage = build_record_codec::recover_build_evaluation_usage(&mut usage_cursor)?;
            if !usage_cursor.exhausted() {
                return None;
            }
            let observation_digest = cursor.bytes(32)?.try_into().ok()?;
            Some(Arc::new(NativeBuildEvaluation {
                terminal_code: terminal_psi::TerminalPsiIdentity {
                    vocabulary_marker,
                    program_fingerprint,
                },
                usage,
                observation_digest,
            }))
        }
        _ => return None,
    };

    if !cursor.exhausted() {
        return None;
    }
    Some(NativeBuildOutputs {
        declarations,
        intents,
        evaluation,
    })
}

pub fn encode(bundle: &PackageBuildHandoff) -> Vec<u8> {
    encode_parts(&NativeBuildOutputs {
        declarations: bundle.native_declarations().to_vec(),
        intents: bundle.native_intents().to_vec(),
        evaluation: bundle.native_evaluation().cloned(),
    })
}
pub fn decode(bytes: &[u8], bundle: PackageBuildHandoff) -> Option<PackageBuildHandoff> {
    let outputs = decode_parts(bytes)?;
    bundle
        .with_native_binding_outputs(outputs.declarations, outputs.intents, outputs.evaluation)
        .ok()
}

pub fn digest(bundle: &PackageBuildHandoff) -> [u8; 32] {
    let mut digest =
        crate::digest_encoder::DomainDigestEncoder::new(b"omega-package-native-outputs-v12\0");
    digest.u64(bundle.native_declarations().len() as u64);
    for declaration in bundle.native_declarations() {
        digest.raw(&declaration.producer.package.digest());
        digest.bytes(declaration.producer.declaration_path.as_bytes());
        digest.bytes(declaration.producer.callable_identity.as_bytes());
        digest.raw(&declaration.source_occurrence_digest);
    }
    digest.u64(bundle.native_intents().len() as u64);
    for intent in bundle.native_intents() {
        digest.u64(u64::from(intent.declaration));
        digest.u64(intent.target.code());
        digest.bytes(&owned(&intent.intent));
        match &intent.static_environment {
            None => digest.byte(0),
            Some(environment) => {
                digest.byte(1);
                digest.bytes(&environment.identity);
            }
        }
    }
    match bundle.native_evaluation() {
        None => digest.byte(0),
        Some(evaluation) => {
            digest.byte(1);
            digest.raw(&evaluation.identity_digest());
        }
    }
    digest.finish()
}
