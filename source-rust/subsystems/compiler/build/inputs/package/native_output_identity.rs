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
