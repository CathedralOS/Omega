// Canonical wire encoding, decoding, and content digesting (`OMG-NATIVE-OUTPUTS\x04`)
// for the native declarations, foreign/syscall locators, and `NativeBuildEvaluation`
// carried inside a `PackageBuildHandoff`. Decoding reconstructs inert anchors
// without granting source visibility, host backing, or native execution authority.

use super::{NativeBuildEvaluation, PackageBuildHandoff};
use crate::build_execution::build_record_codec;
use build_time_evaluation::native_bindings::CapturedNativeDeclaration;
use filesystem_authority::observation_codec::{ObservationCursor, push_framed};
use language_semantics::declaration_selection::{
    CapturedNativeIntent, NativeBindingProducer, NativeBindingSource, ProductTarget,
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

fn parameter_description(output: &mut Vec<u8>, value: &semantic_vocabulary::ParameterDescription) {
    for bytes in [
        &value.requirement_identity,
        &value.requirement_application,
        &value.parameter_identity,
        &value.qualified_type,
    ] {
        push_framed(output, bytes);
    }
    number(output, value.declaration_position);
    push_framed(output, &value.source_revision);
}

fn read_parameter_description(
    cursor: &mut ObservationCursor<'_>,
) -> Option<semantic_vocabulary::ParameterDescription> {
    Some(semantic_vocabulary::ParameterDescription {
        requirement_identity: cursor.framed()?.to_vec(),
        requirement_application: cursor.framed()?.to_vec(),
        parameter_identity: cursor.framed()?.to_vec(),
        qualified_type: cursor.framed()?.to_vec(),
        declaration_position: cursor.u64()?,
        source_revision: cursor.framed()?.to_vec(),
    })
}

fn field_description(output: &mut Vec<u8>, value: &semantic_vocabulary::FieldInfoDescription) {
    for bytes in [
        &value.member_identity,
        &value.case_identity,
        &value.owner_identity,
        &value.owner_application,
        &value.name,
        &value.qualified_type,
    ] {
        push_framed(output, bytes);
    }
    number(output, value.declaration_position);
    output.push(u8::from(value.has_stable_number));
    output.push(u8::from(value.is_erased));
    number(output, value.stable_number);
    push_framed(output, &value.source_revision);
}

fn read_boolean(cursor: &mut ObservationCursor<'_>) -> Option<bool> {
    match cursor.u8()? {
        0 => Some(false),
        1 => Some(true),
        _ => None,
    }
}

fn read_field_description(
    cursor: &mut ObservationCursor<'_>,
) -> Option<semantic_vocabulary::FieldInfoDescription> {
    Some(semantic_vocabulary::FieldInfoDescription {
        member_identity: cursor.framed()?.to_vec(),
        case_identity: cursor.framed()?.to_vec(),
        owner_identity: cursor.framed()?.to_vec(),
        owner_application: cursor.framed()?.to_vec(),
        name: cursor.framed()?.to_vec(),
        qualified_type: cursor.framed()?.to_vec(),
        declaration_position: cursor.u64()?,
        has_stable_number: read_boolean(cursor)?,
        is_erased: read_boolean(cursor)?,
        stable_number: cursor.u64()?,
        source_revision: cursor.framed()?.to_vec(),
    })
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
            NativeBindingLocator::FunctionField { table, field } => {
                output.push(5);
                parameter_description(output, table);
                field_description(output, field);
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
    let mut output = b"OMG-NATIVE-OUTPUTS\x04".to_vec();
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
        }
    }
    output
}

pub fn decode(bytes: &[u8], handoff: PackageBuildHandoff) -> Option<PackageBuildHandoff> {
    let mut cursor = ObservationCursor::new(bytes);
    let magic = b"OMG-NATIVE-OUTPUTS\x04";
    if cursor.bytes(magic.len())? != magic {
        return None;
    }
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
            5 => NativeBindingLocator::FunctionField {
                table: read_parameter_description(&mut cursor)?,
                field: read_field_description(&mut cursor)?,
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

#[cfg(test)]
mod tests {
    use super::*;
    use evaluation_accounting::{
        BuildEvaluationSponsor, BuildEvaluationSponsorLimits, EvaluationMeasurementIdentity,
        TerminalStorageScheduleIdentity,
    };
    use semantic_vocabulary::{FieldInfoDescription, ParameterDescription, WorkScheduleIdentity};

    fn selectors() -> NativeBindingLocator {
        NativeBindingLocator::FunctionField {
            table: ParameterDescription {
                requirement_identity: b"exact requirement".to_vec(),
                requirement_application: b"invoke<'firmware>(u64,&'firmware Table)->Status"
                    .to_vec(),
                parameter_identity: b"exact formal".to_vec(),
                qualified_type: b"&'firmware Table".to_vec(),
                declaration_position: 1,
                source_revision: b"exact requirement revision".to_vec(),
            },
            field: FieldInfoDescription {
                member_identity: b"exact function field".to_vec(),
                case_identity: Vec::new(),
                owner_identity: b"exact table".to_vec(),
                owner_application: b"exact table application".to_vec(),
                name: b"function".to_vec(),
                qualified_type: b"addr".to_vec(),
                declaration_position: 4,
                has_stable_number: true,
                is_erased: false,
                stable_number: 19,
                source_revision: b"exact table revision".to_vec(),
            },
        }
    }

    fn handoff(locator: NativeBindingLocator) -> PackageBuildHandoff {
        // This is an inert wire candidate. Receiving source admission must
        // still authenticate its declaration, selectors and evaluation.
        let package = PackageKeyIdentity::from_digest([7; 32]).unwrap();
        let mut sources = source::SourceMap::default();
        let text = "public boundary trait Raw { machine invoke() reaches none crashes none; }";
        let source = sources
            .add_checked_instance(
                "/package/main.omg".into(),
                text,
                "/package".into(),
                Some(package),
                source::SourceOrigin::User,
                source::SourceResolutionStratum::Base,
                source::DependencyScope::Product,
            )
            .source_id;
        let requirement = symbols::RetainedDeclaration::from_retained_parts(
            sources.get(source).unwrap().clone(),
            source::Span::new(28, 34),
            symbols::SymbolKind::State,
        )
        .unwrap();
        let declaration = CapturedNativeDeclaration {
            application: None,
            source: NativeBindingSource {
                producer: NativeBindingProducer {
                    package,
                    declaration_path: "build".into(),
                    callable_identity: "build(&mut Build)".into(),
                },
                requirement_path: "Raw::invoke".into(),
                source_occurrence_digest: [11; 32],
                resolution_stratum: source::SourceResolutionStratum::Base,
            },
            requirement,
            requirement_identity: "Raw::invoke()".into(),
            dependency_alias: None,
        };
        let measurement = EvaluationMeasurementIdentity::TerminalPsi {
            work_schedule: WorkScheduleIdentity::new(1).unwrap(),
            storage_schedule: TerminalStorageScheduleIdentity::CURRENT,
        };
        let limits = BuildEvaluationSponsorLimits::new(100, 100, 8, 8, 100, 100, 100, 100).unwrap();
        let sponsor = BuildEvaluationSponsor::new_with_measurement(limits, measurement).unwrap();
        let usage = *sponsor
            .begin_invocation(measurement, 100)
            .unwrap()
            .snapshot()
            .unwrap()
            .record();
        let evaluation = Arc::new(NativeBuildEvaluation {
            terminal_code: terminal_psi::TerminalPsiIdentity {
                vocabulary_marker: terminal_psi::VocabularyMarker::CURRENT,
                program_fingerprint: terminal_psi::SemanticFingerprint::from_bytes([13; 32]),
            },
            usage,
            observation_digest: [17; 32],
        });
        let closure = crate::PackageDependencyClosure::from_canonical_parts(
            package,
            crate::BuildDeclarationKind::Package,
            vec![package],
            Vec::new(),
        )
        .unwrap();
        PackageBuildHandoff::from_checked(
            package,
            build_declarations::DependencyPurpose::Product,
            target::TargetProfile::LinuxX64,
            None,
            closure,
            crate::PackageSourceConsumptionCommitment::from_retained_digest([23; 32]),
            Vec::new(),
        )
        .with_native_binding_outputs(
            vec![declaration],
            vec![CapturedNativeIntent {
                declaration: 0,
                target: ProductTarget::LinuxX86_64,
                locator,
            }],
            Some(evaluation),
        )
        .unwrap()
    }

    #[test]
    fn native_output_choices_round_trip_without_changing_selector_bytes() {
        for locator in [
            NativeBindingLocator::Import(ForeignLocatorCandidate::PeByName {
                library: b"library".to_vec(),
                export: b"symbol".to_vec(),
            }),
            NativeBindingLocator::Import(ForeignLocatorCandidate::PeByOrdinal {
                library: b"library".to_vec(),
                ordinal: 19,
            }),
            NativeBindingLocator::Import(ForeignLocatorCandidate::ElfVersioned {
                object: b"object".to_vec(),
                symbol: b"symbol".to_vec(),
                version: b"version".to_vec(),
            }),
            NativeBindingLocator::Import(ForeignLocatorCandidate::MachODylibSymbol {
                install_name: b"dylib".to_vec(),
                symbol: b"symbol".to_vec(),
            }),
            NativeBindingLocator::Syscall { number: 77 },
            selectors(),
        ] {
            let original = handoff(locator);
            let bytes = encode(&original);
            assert!(bytes.starts_with(b"OMG-NATIVE-OUTPUTS\x04"));
            let recovered = decode(&bytes, original.clone()).unwrap();
            assert_eq!(recovered, original);
            assert_eq!(encode(&recovered), bytes);
            assert_eq!(digest(&recovered), digest(&original));
        }
    }

    #[test]
    fn table_output_refuses_invalid_booleans_truncation_and_other_versions() {
        let original = handoff(selectors());
        let bytes = encode(&original);
        for length in 0..bytes.len() {
            assert!(
                decode(&bytes[..length], original.clone()).is_none(),
                "truncated length {length}"
            );
        }
        for version in [3, 5] {
            let mut changed = bytes.clone();
            changed[b"OMG-NATIVE-OUTPUTS".len()] = version;
            assert!(decode(&changed, original.clone()).is_none());
        }
        let NativeBindingLocator::FunctionField { field, .. } = selectors() else {
            unreachable!()
        };
        let mut encoded_field = Vec::new();
        field_description(&mut encoded_field, &field);
        let offsets = bytes
            .windows(encoded_field.len())
            .enumerate()
            .filter_map(|(index, chunk)| (chunk == encoded_field).then_some(index))
            .collect::<Vec<_>>();
        let [offset] = offsets.as_slice() else {
            panic!("one exact encoded field")
        };
        let mut prefix = Vec::new();
        for value in [
            &field.member_identity,
            &field.case_identity,
            &field.owner_identity,
            &field.owner_application,
            &field.name,
            &field.qualified_type,
        ] {
            push_framed(&mut prefix, value);
        }
        number(&mut prefix, field.declaration_position);
        for boolean in 0..2 {
            let mut changed = bytes.clone();
            changed[offset + prefix.len() + boolean] = 2;
            assert!(decode(&changed, original.clone()).is_none());
        }
        let mut changed = selectors();
        let NativeBindingLocator::FunctionField { field, .. } = &mut changed else {
            unreachable!()
        };
        field.member_identity.push(0);
        let changed = handoff(changed);
        assert_ne!(encode(&changed), bytes);
        assert_ne!(digest(&changed), digest(&original));
    }
}
