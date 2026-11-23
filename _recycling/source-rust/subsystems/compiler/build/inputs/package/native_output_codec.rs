// Canonical wire encoding, decoding, and content digesting
// for the native declarations, foreign/syscall locators, and `NativeBuildEvaluation`
// carried inside a `PackageBuildHandoff`. Decoding reconstructs inert anchors
// without granting source visibility, host backing, or native execution authority.

use super::{NativeBuildEvaluation, PackageBuildHandoff};
use crate::build_execution::build_record_codec;
use filesystem_authority::observation_codec::{ObservationCursor, push_framed};
use retained_product_selections::CapturedNativeDeclaration;
use semantic_vocabulary::source_selections::NativeBindingProducer;
use semantic_vocabulary::source_selections::ProductTarget;
use semantic_vocabulary::{ForeignLocatorCandidate, NativeBindingLocator, PackageKeyIdentity};
use source_declarations::declaration_selection::CapturedNativeIntent;
use source_declarations::declaration_selection::NativeBindingSource;
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
            source_admission::SourceResolutionStratum::Base => 0,
            source_admission::SourceResolutionStratum::CurrentActivationExtension => 1,
            source_admission::SourceResolutionStratum::DependencyActivationExtension => 2,
        });
        string(output, &declaration.requirement_identity);
        output.extend_from_slice(&declaration.service_declaration);
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
        crate::digest_encoder::DomainDigestEncoder::new(b"omega-package-native-outputs-v4\0");
    let anchors = bundle
        .native_declarations()
        .iter()
        .map(|declaration| &declaration.requirement)
        .collect::<Vec<_>>();
    digest.raw(&super::provider_selection_codec::retained_declarations_digest(&anchors));
    let applications = bundle
        .native_declarations()
        .iter()
        .map(|declaration| declaration.application.as_ref())
        .collect::<Vec<_>>();
    digest.raw(&super::provider_selection_codec::product_applications_digest(&applications));
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
    let mut output = b"OMG-NATIVE-OUTPUTS".to_vec();
    let staged = bundle
        .native_evaluation()
        .is_some_and(|evaluation| !evaluation.execution_history.steps().is_empty());
    output.push(if staged { 5 } else { 4 });
    let anchors = bundle
        .native_declarations()
        .iter()
        .map(|declaration| &declaration.requirement)
        .collect::<Vec<_>>();
    push_framed(
        &mut output,
        &super::provider_selection_codec::encode_retained_declarations(&anchors),
    );
    let applications = bundle
        .native_declarations()
        .iter()
        .map(|declaration| declaration.application.as_ref())
        .collect::<Vec<_>>();
    push_framed(
        &mut output,
        &super::provider_selection_codec::encode_product_applications(&applications),
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
            if staged {
                let frame = output.len();
                number(&mut output, 0);
                evaluation
                    .execution_history
                    .write_bytes(|bytes| output.extend_from_slice(bytes));
                let length = (output.len() - frame - 8) as u64;
                output[frame..frame + 8].copy_from_slice(&length.to_le_bytes());
            }
        }
    }
    output
}

pub fn decode(bytes: &[u8], handoff: PackageBuildHandoff) -> Option<PackageBuildHandoff> {
    let mut cursor = ObservationCursor::new(bytes);
    let magic = b"OMG-NATIVE-OUTPUTS";
    if cursor.bytes(magic.len())? != magic {
        return None;
    }
    let staged = match cursor.u8()? {
        4 => false,
        5 => true,
        _ => return None,
    };
    let anchors = super::provider_selection_codec::decode_retained_declarations(cursor.framed()?)?;
    let applications =
        super::provider_selection_codec::decode_product_applications(cursor.framed()?)?;
    let declared_count = count(&mut cursor)?;
    if declared_count != anchors.len() || declared_count != applications.len() {
        return None;
    }
    let mut declarations = Vec::new();
    for (requirement, application) in anchors.into_iter().zip(applications) {
        let package = PackageKeyIdentity::from_digest(cursor.bytes(32)?.try_into().ok()?)?;
        let declaration_path = read_string(&mut cursor)?;
        let callable_identity = read_string(&mut cursor)?;
        let requirement_path = read_string(&mut cursor)?;
        let source_occurrence_digest = cursor.bytes(32)?.try_into().ok()?;
        let resolution_stratum = match cursor.u8()? {
            0 => source_admission::SourceResolutionStratum::Base,
            1 => source_admission::SourceResolutionStratum::CurrentActivationExtension,
            2 => source_admission::SourceResolutionStratum::DependencyActivationExtension,
            _ => return None,
        };
        let requirement_identity = read_string(&mut cursor)?;
        let service_declaration = cursor.bytes(32)?.try_into().ok()?;
        let dependency_alias = match cursor.u8()? {
            0 => None,
            1 => Some(read_string(&mut cursor)?),
            _ => return None,
        };
        declarations.push(CapturedNativeDeclaration {
            application,
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
            service_declaration,
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
        0 if !staged => None,
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
            let history = if staged {
                let history =
                    evaluation_receipts::BuildExecutionHistory::from_bytes(cursor.framed()?)?;
                if history.steps().is_empty() {
                    return None;
                }
                history
            } else {
                evaluation_receipts::BuildExecutionHistory::default()
            };
            Some(Arc::new(
                NativeBuildEvaluation::from_execution(
                    terminal_psi::TerminalPsiIdentity {
                        vocabulary_marker,
                        program_fingerprint,
                    },
                    Arc::new(history),
                    usage,
                    observation_digest,
                )
                .ok()?,
            ))
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

#[cfg(test)]
mod tests {
    use super::*;
    use build_declarations::{BuildDeclarationKind, DependencyPurpose};
    use evaluation_receipts::BuildExecutionHistory;
    use semantic_vocabulary::{MachineId, OperationId};

    fn program(byte: u8) -> terminal_psi::TerminalPsiIdentity {
        terminal_psi::TerminalPsiIdentity {
            vocabulary_marker: terminal_psi::VocabularyMarker::CURRENT,
            program_fingerprint: terminal_psi::SemanticFingerprint::from_bytes([byte; 32]),
        }
    }

    fn single_stage_receipt() -> NativeBuildEvaluation {
        let measurement = evaluation_contracts::EvaluationMeasurementIdentity::current();
        let sponsor = evaluation_accounting::BuildEvaluationSponsor::new_with_measurement(
            evaluation_contracts::BuildEvaluationSponsorLimits::new(
                100, 100, 100, 100, 100, 100, 100, 100,
            )
            .unwrap(),
            measurement,
        )
        .unwrap();
        let usage = *sponsor
            .begin_invocation(measurement, 100)
            .unwrap()
            .snapshot()
            .unwrap()
            .record();
        NativeBuildEvaluation::from_observation_digest(program(9), usage, [8; 32]).unwrap()
    }

    fn handoff() -> PackageBuildHandoff {
        let root = PackageKeyIdentity::from_digest([1; 32]).unwrap();
        let closure = crate::PackageDependencyClosure::from_canonical_parts(
            root,
            BuildDeclarationKind::Package,
            vec![root],
            vec![],
        )
        .unwrap();
        PackageBuildHandoff::from_checked(
            root,
            DependencyPurpose::Build,
            target::TargetProfile::LinuxArm64,
            None,
            closure,
            crate::PackageSourceConsumptionCommitment::from_retained_digest([2; 32]),
            Vec::new(),
        )
    }

    #[test]
    fn original_single_program_receipts_round_trip_without_changing_their_bytes() {
        let receipt = single_stage_receipt();
        let plain = handoff();
        let mut old = encode(&plain);
        assert!(old.starts_with(b"OMG-NATIVE-OUTPUTS\x04"));
        assert_eq!(old.pop(), Some(0));
        old.push(1);
        old.extend_from_slice(&receipt.terminal_code.vocabulary_marker.get().to_le_bytes());
        old.extend_from_slice(receipt.terminal_code.program_fingerprint.as_bytes());
        let mut usage = Vec::new();
        build_record_codec::encode_build_evaluation_usage(&receipt.usage, &mut usage);
        push_framed(&mut old, &usage);
        old.extend_from_slice(&receipt.observation_digest);
        let decoded = decode(&old, plain).unwrap();
        assert_eq!(decoded.native_evaluation().unwrap().as_ref(), &receipt);
        assert_eq!(encode(&decoded), old);

        // Package locks used the original field sequence with the same receipt.
        let old_record = record_storage::encode(
            &(
                receipt.terminal_code,
                receipt.usage,
                receipt.observation_digest,
            ),
            Default::default(),
        )
        .unwrap();
        let recovered: NativeBuildEvaluation =
            record_storage::decode(&old_record, Default::default()).unwrap();
        assert_eq!(recovered, receipt);
        assert_eq!(
            record_storage::encode(&recovered, Default::default()).unwrap(),
            old_record
        );
    }

    #[test]
    fn preparing_a_product_binds_the_child_build_without_selecting_a_product_handoff() {
        let single = single_stage_receipt();
        let receipt = |child| {
            let mut history = BuildExecutionHistory::default();
            history.prepared(
                source_provenance::DependencyScope::Product,
                0,
                PackageKeyIdentity::from_digest([1; 32]).unwrap(),
                [2; 32],
                child,
                None,
            );
            NativeBuildEvaluation::from_execution(
                single.terminal_code,
                Arc::new(history),
                single.usage,
                single.observation_digest,
            )
            .unwrap()
        };
        let actual = receipt(Some([3; 32]));
        for other in [receipt(Some([4; 32])), receipt(None)] {
            assert_ne!(actual.identity_digest(), other.identity_digest());
        }
        let bundle = handoff()
            .with_native_binding_outputs(vec![], vec![], Some(Arc::new(actual)))
            .unwrap();
        assert_eq!(decode(&encode(&bundle), handoff()).unwrap(), bundle);
    }

    #[test]
    fn equal_final_code_distinguishes_the_stages_calls_and_dependency_products_that_ran() {
        let plain = single_stage_receipt();
        let receipt = |earlier, caller, dependency, sources, generated, reverse| {
            let mut history = BuildExecutionHistory::default();
            let site = |operation| {
                (
                    MachineId::new(1).unwrap(),
                    OperationId::new(operation).unwrap(),
                )
            };
            let prepare = |history: &mut BuildExecutionHistory, ordinal, package| {
                history.prepared(
                    source_provenance::DependencyScope::Build,
                    ordinal,
                    PackageKeyIdentity::from_digest([package; 32]).unwrap(),
                    [sources; 32],
                    Some([3; 32]),
                    Some([generated; 32]),
                );
            };
            if reverse {
                prepare(&mut history, 1, 7);
            }
            prepare(&mut history, 0, dependency);
            if !reverse {
                prepare(&mut history, 1, 7);
            }
            history
                .resumed(
                    program(earlier),
                    program(5),
                    [(site(caller), site(4)), (site(6), site(8))],
                )
                .unwrap();
            history
                .resumed(program(5), plain.terminal_code, [(site(10), site(11))])
                .unwrap();
            NativeBuildEvaluation::from_execution(
                plain.terminal_code,
                Arc::new(history),
                plain.usage,
                plain.observation_digest,
            )
            .unwrap()
        };
        let actual = receipt(1, 2, 3, 4, 5, false);
        for other in [
            receipt(2, 2, 3, 4, 5, false),
            receipt(1, 3, 3, 4, 5, false),
            receipt(1, 2, 4, 4, 5, false),
            receipt(1, 2, 3, 5, 5, false),
            receipt(1, 2, 3, 4, 6, false),
            receipt(1, 2, 3, 4, 5, true),
            plain.clone(),
        ] {
            assert_ne!(actual.identity_digest(), other.identity_digest());
        }
        let bundle = handoff()
            .with_native_binding_outputs(vec![], vec![], Some(Arc::new(actual.clone())))
            .unwrap();
        let bytes = encode(&bundle);
        assert!(bytes.starts_with(b"OMG-NATIVE-OUTPUTS\x05"));
        assert_eq!(decode(&bytes, handoff()).unwrap(), bundle);
        assert!(decode(&bytes[..bytes.len() - 1], handoff()).is_none());
        let stored = record_storage::encode(&actual, Default::default()).unwrap();
        assert_eq!(
            record_storage::decode::<NativeBuildEvaluation>(&stored, Default::default()).unwrap(),
            actual
        );
        let mut wrong_program = actual.clone();
        wrong_program.terminal_code = program(3);
        assert!(
            handoff()
                .with_native_binding_outputs(vec![], vec![], Some(Arc::new(wrong_program)))
                .is_err()
        );
        assert!(
            NativeBuildEvaluation::from_execution(
                program(3),
                Arc::clone(&actual.execution_history),
                plain.usage,
                plain.observation_digest
            )
            .is_err()
        );
    }
}
