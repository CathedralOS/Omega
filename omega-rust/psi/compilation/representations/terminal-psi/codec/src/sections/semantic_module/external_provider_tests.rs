use super::{
    module_wire::encode_raw, numeric_intrinsic_tests, target_family_tests::identity_table,
};
use crate::{decode_module, encode_module, semantic_fingerprint};
use language_semantics::{ExternalBindingIdentity, numeric_intrinsics::NumericIntrinsic};
use terminal_psi::*;

fn module() -> TerminalModule {
    let mut module = numeric_intrinsic_tests::module(
        NumericIntrinsic::from_canonical_encoding([3, 0, 0, 0]).unwrap(),
    );
    module.identities = identity_table(8);
    module.external_providers = vec![ExternalProviderConformance {
        requirement: NominalDeclarationIdentity::new(TerminalIdentityIndex(3)),
        requirement_identity: TerminalIdentityIndex(3),
        requirement_package: None,
        numeric_intrinsic: module.boundary_machines[0].numeric_intrinsic,
        target: Some("macos_arm64".into()),
        realization: CheckedMachineDeclaration {
            machine: NominalDeclarationIdentity::new(TerminalIdentityIndex(4)),
            state: NominalDeclarationIdentity::new(TerminalIdentityIndex(5)),
            semantic_declaration: TerminalIdentityIndex(6),
            parameter_types: vec![TerminalIdentityIndex(7)],
            callable: TerminalIdentityIndex(8),
            result_type: TerminalIdentityIndex(7),
            contract: [17; 32],
            specialization: None,
        },
        realization_package: semantic_vocabulary::PackageKeyIdentity::from_digest([1; 32]),
        provider: Some(ExternalProviderOwner {
            declaration: NominalDeclarationIdentity::new(TerminalIdentityIndex(6)),
            identity: TerminalIdentityIndex(6),
            package: semantic_vocabulary::PackageKeyIdentity::from_digest([2; 32]),
        }),
        binding: ExternalBindingIdentity::CompilerIntrinsic,
    }];
    module
}

#[test]
fn external_provider_satisfaction_round_trips_all_mechanisms_without_source() {
    for binding in [
        ExternalBindingIdentity::CompilerIntrinsic,
        ExternalBindingIdentity::Syscall { number: 14 },
        ExternalBindingIdentity::VtableSlot { index: 9 },
        ExternalBindingIdentity::VtableField {
            field: "read".into(),
        },
        ExternalBindingIdentity::TableFunction {
            field: "allocate".into(),
        },
    ] {
        let mut module = module();
        module.external_providers[0].binding = binding;
        let bytes = encode_module(&module).unwrap();
        let decoded = decode_module(&bytes).unwrap();
        assert_eq!(decoded, module);
        assert_eq!(encode_module(&decoded).unwrap(), bytes);
        let mut missing = module.clone();
        missing.external_providers.clear();
        assert_ne!(
            semantic_fingerprint(&module).unwrap(),
            semantic_fingerprint(&missing).unwrap()
        );
    }
}

#[test]
fn external_provider_satisfaction_survives_without_an_executable_boundary_call() {
    let mut module = module();
    module.boundary_machines.clear();
    let bytes = encode_module(&module).unwrap();
    let decoded = decode_module(&bytes).unwrap();
    assert_eq!(decoded.external_providers, module.external_providers);
    assert!(decoded.boundary_machines.is_empty());
    assert_eq!(encode_module(&decoded).unwrap(), bytes);
}

#[test]
fn external_provider_satisfaction_rejects_missing_coordinates_and_duplicate_edges() {
    for mutation in 0..6 {
        let mut module = module();
        let candidate = &mut module.external_providers[0];
        match mutation {
            0 => candidate.realization.callable = TerminalIdentityIndex(99),
            1 => candidate
                .realization
                .parameter_types
                .push(TerminalIdentityIndex(99)),
            2 => candidate.provider.as_mut().unwrap().identity = TerminalIdentityIndex(99),
            3 => {
                candidate.binding = ExternalBindingIdentity::TableFunction {
                    field: String::new(),
                }
            }
            4 => {
                let duplicate = candidate.clone();
                module.external_providers.push(duplicate);
            }
            5 => candidate.target = Some(String::new()),
            _ => unreachable!(),
        }
        let bytes = encode_raw(&module).unwrap();
        let error = decode_module(&bytes).unwrap_err().to_string();
        assert!(
            error.contains("external provider") || error.contains("numeric intrinsic"),
            "{error}"
        );
    }
}

#[test]
fn external_provider_satisfaction_binds_declaration_package_owner_and_mechanism() {
    let original = module();
    let fingerprint = semantic_fingerprint(&original).unwrap();
    for mutation in 0..5 {
        let mut changed = original.clone();
        let candidate = &mut changed.external_providers[0];
        match mutation {
            0 => candidate.realization.contract = [18; 32],
            1 => {
                candidate.realization_package =
                    semantic_vocabulary::PackageKeyIdentity::from_digest([3; 32])
            }
            2 => {
                candidate.provider.as_mut().unwrap().package =
                    semantic_vocabulary::PackageKeyIdentity::from_digest([4; 32])
            }
            3 => candidate.provider = None,
            4 => candidate.binding = ExternalBindingIdentity::Syscall { number: 14 },
            _ => unreachable!(),
        }
        assert_ne!(semantic_fingerprint(&changed).unwrap(), fingerprint);
    }
}
