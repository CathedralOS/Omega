//! Framed native output data for the complete package handoff. Decoding never
//! grants source visibility, host backing, or native execution authority.
use super::{NativeBuildEvaluation, PackageBuildHandoff};
use crate::build_execution::build_record_codec;
use filesystem_authority::observation_codec::{ObservationCursor, push_framed};
use language_semantics::declaration_selection::{
    CapturedNativeDeclaration, CapturedNativeIntent, NativeBindingProducer, NativeBindingSource,
    ProductTarget,
};
use semantic_vocabulary::{ForeignLocatorCandidate, NativeBindingLocator, PackageKeyIdentity};
use std::sync::Arc;

fn number(output: &mut Vec<u8>, value: u64) {
    output.extend_from_slice(&value.to_le_bytes());
}
fn string(output: &mut Vec<u8>, value: &str) {
    push_framed(output, value.as_bytes());
}
fn read_string(cursor: &mut ObservationCursor<'_>) -> Option<String> {
    std::str::from_utf8(cursor.framed()?)
        .ok()
        .map(str::to_owned)
}
fn count(cursor: &mut ObservationCursor<'_>) -> Option<usize> {
    let length = cursor.u64()?;
    (length <= (1 << 20)).then_some(usize::try_from(length).ok()?)
}

fn choices(bundle: &PackageBuildHandoff, output: &mut Vec<u8>) {
    number(output, bundle.native_declarations().len() as u64);
    for declaration in bundle.native_declarations() {
        output.extend_from_slice(&declaration.source.producer.package.digest());
        string(output, &declaration.source.producer.declaration_path);
        string(output, &declaration.source.producer.callable_identity);
        string(output, &declaration.source.requirement_path);
        output.extend_from_slice(&declaration.source.source_occurrence_digest);
        output.push(match declaration.source.resolution_stratum {
            source::SourceResolutionStratum::Base => 0,
            source::SourceResolutionStratum::CurrentActivationExtension => 1,
            source::SourceResolutionStratum::DependencyActivationExtension => 2,
        });
        string(output, &declaration.requirement_identity);
        match &declaration.dependency_alias {
            None => output.push(0),
            Some(alias) => {
                output.push(1);
                string(output, alias);
            }
        }
    }
    number(output, bundle.native_intents().len() as u64);
    for intent in bundle.native_intents() {
        number(output, intent.declaration as u64);
        number(output, intent.target.code());
        match &intent.locator {
            NativeBindingLocator::Import(ForeignLocatorCandidate::PeByName { library, export }) => {
                output.push(0);
                push_framed(output, library);
                push_framed(output, export);
            }
            NativeBindingLocator::Import(ForeignLocatorCandidate::PeByOrdinal {
                library,
                ordinal,
            }) => {
                output.push(1);
                push_framed(output, library);
                number(output, *ordinal as u64);
            }
            NativeBindingLocator::Import(ForeignLocatorCandidate::ElfVersioned {
                object,
                symbol,
                version,
            }) => {
                output.push(2);
                push_framed(output, object);
                push_framed(output, symbol);
                push_framed(output, version);
            }
            NativeBindingLocator::Import(ForeignLocatorCandidate::MachODylibSymbol {
                install_name,
                symbol,
            }) => {
                output.push(3);
                push_framed(output, install_name);
                push_framed(output, symbol);
            }
            NativeBindingLocator::Syscall { number: syscall } => {
                output.push(4);
                number(output, *syscall);
            }
        }
    }
}

pub fn digest(bundle: &PackageBuildHandoff) -> [u8; 32] {
    let mut digest =
        crate::digest_encoder::DomainDigestEncoder::new(b"omega-package-native-outputs-v2\0");
    let anchors = bundle
        .native_declarations()
        .iter()
        .map(|declaration| &declaration.requirement)
        .collect::<Vec<_>>();
    digest.raw(&super::provider_selection_codec::retained_declarations_digest(&anchors));
    let mut bytes = Vec::new();
    choices(bundle, &mut bytes);
    digest.bytes(&bytes);
    match bundle.native_evaluation() {
        None => digest.byte(0),
        Some(evaluation) => {
            digest.byte(1);
            digest.raw(&evaluation.identity_digest());
        }
    }
    digest.finish()
}

pub fn encode(bundle: &PackageBuildHandoff) -> Vec<u8> {
    let mut output = b"OMG-NATIVE-OUTPUTS\x02".to_vec();
    let anchors = bundle
        .native_declarations()
        .iter()
        .map(|declaration| &declaration.requirement)
        .collect::<Vec<_>>();
    push_framed(
        &mut output,
        &super::provider_selection_codec::encode_retained_declarations(&anchors),
    );
    choices(bundle, &mut output);
    match bundle.native_evaluation() {
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

pub fn decode(bytes: &[u8], handoff: PackageBuildHandoff) -> Option<PackageBuildHandoff> {
    let mut cursor = ObservationCursor::new(bytes);
    let magic = b"OMG-NATIVE-OUTPUTS\x02";
    if cursor.bytes(magic.len())? != magic {
        return None;
    }
    let anchors = super::provider_selection_codec::decode_retained_declarations(cursor.framed()?)?;
    let declared_count = count(&mut cursor)?;
    if declared_count != anchors.len() {
        return None;
    }
    let mut declarations = Vec::new();
    for requirement in anchors {
        let package = PackageKeyIdentity::from_digest(cursor.bytes(32)?.try_into().ok()?)?;
        let declaration_path = read_string(&mut cursor)?;
        let callable_identity = read_string(&mut cursor)?;
        let requirement_path = read_string(&mut cursor)?;
        let source_occurrence_digest = cursor.bytes(32)?.try_into().ok()?;
        let resolution_stratum = match cursor.u8()? {
            0 => source::SourceResolutionStratum::Base,
            1 => source::SourceResolutionStratum::CurrentActivationExtension,
            2 => source::SourceResolutionStratum::DependencyActivationExtension,
            _ => return None,
        };
        let requirement_identity = read_string(&mut cursor)?;
        let dependency_alias = match cursor.u8()? {
            0 => None,
            1 => Some(read_string(&mut cursor)?),
            _ => return None,
        };
        declarations.push(CapturedNativeDeclaration {
            source: NativeBindingSource {
                producer: NativeBindingProducer {
                    package,
                    declaration_path,
                    callable_identity,
                },
                requirement_path,
                source_occurrence_digest,
                resolution_stratum,
            },
            requirement,
            requirement_identity,
            dependency_alias,
        });
    }
    let mut intents = Vec::new();
    for _ in 0..count(&mut cursor)? {
        let declaration = u32::try_from(cursor.u64()?).ok()?;
        let target = ProductTarget::from_code(cursor.u64()?)?;
        let locator = match cursor.u8()? {
            0 => NativeBindingLocator::Import(ForeignLocatorCandidate::PeByName {
                library: cursor.framed()?.to_vec(),
                export: cursor.framed()?.to_vec(),
            }),
            1 => NativeBindingLocator::Import(ForeignLocatorCandidate::PeByOrdinal {
                library: cursor.framed()?.to_vec(),
                ordinal: u16::try_from(cursor.u64()?).ok()?,
            }),
            2 => NativeBindingLocator::Import(ForeignLocatorCandidate::ElfVersioned {
                object: cursor.framed()?.to_vec(),
                symbol: cursor.framed()?.to_vec(),
                version: cursor.framed()?.to_vec(),
            }),
            3 => NativeBindingLocator::Import(ForeignLocatorCandidate::MachODylibSymbol {
                install_name: cursor.framed()?.to_vec(),
                symbol: cursor.framed()?.to_vec(),
            }),
            4 => NativeBindingLocator::Syscall {
                number: cursor.u64()?,
            },
            _ => return None,
        };
        intents.push(CapturedNativeIntent {
            declaration,
            target,
            locator,
        });
    }
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
    handoff
        .with_native_binding_outputs(declarations, intents, evaluation)
        .ok()
}
