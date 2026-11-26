//! The native part of a package handoff binds its selected declarations, locators and Build execution.

use super::PackageBuildHandoff;
use filesystem_authority::observation_codec::push_framed;
use semantic_vocabulary::{ForeignLocatorCandidate, NativeBindingLocator};

fn number(output: &mut Vec<u8>, value: u64) {
    output.extend_from_slice(&value.to_le_bytes());
}
fn string(output: &mut Vec<u8>, value: &str) {
    push_framed(output, value.as_bytes());
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
            NativeBindingLocator::FunctionField(binding) => {
                output.push(5);
                output.extend_from_slice(&binding.identity_digest());
            }
            NativeBindingLocator::Syscall { number: syscall } => {
                output.push(4);
                number(output, *syscall);
            }
        }
    }
}

pub(super) fn digest(bundle: &PackageBuildHandoff) -> [u8; 32] {
    let mut digest =
        crate::digest_encoder::DomainDigestEncoder::new(b"omega-package-native-outputs-v4\0");
    let anchors = bundle
        .native_declarations()
        .iter()
        .map(|declaration| &declaration.requirement)
        .collect::<Vec<_>>();
    digest.raw(&super::provider_identity::retained_declarations_digest(
        &anchors,
    ));
    let applications = bundle
        .native_declarations()
        .iter()
        .map(|declaration| declaration.application.as_ref())
        .collect::<Vec<_>>();
    digest.raw(&super::provider_identity::product_applications_digest(
        &applications,
    ));
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

#[cfg(test)]
mod tests {
    use super::*;
    use build_declarations::{BuildDeclarationKind, DependencyPurpose};
    use evaluation_receipts::BuildExecutionHistory;
    use evaluation_receipts::NativeBuildEvaluation;
    use semantic_vocabulary::PackageKeyIdentity;
    use semantic_vocabulary::{MachineId, OperationId};
    use std::sync::Arc;

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
    fn single_program_receipts_keep_their_package_lock_bytes() {
        let receipt = single_stage_receipt();
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
