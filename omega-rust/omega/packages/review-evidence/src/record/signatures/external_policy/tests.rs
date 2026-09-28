use crate::record::PackagePolicyExternalBinding;
use crate::record::PackagePolicyExternalCallableSignature;
use crate::record::PackagePolicyExternalExecutableSupply;
use crate::record::PackagePolicyExternalRequirement;
use crate::record::PackageReviewExternalBinding;
use crate::record::PackageReviewForeignLocator;
use crate::record::PackageReviewNominalIdentity;
use crate::record::{
    PackagePolicyCallableConformance, PackagePolicyMachineParameterContract,
    PackagePolicyTypeParameter, PackagePolicyTypeParameterKind, PackageReviewConformanceBound,
    PackageReviewDataProperties, PackageReviewEvaluatedBindingUsage, PackageReviewEvaluatedImport,
    PackageReviewEvaluatedSyscall, PackageReviewExternalCallableParameter,
    PackageReviewNominalOwner, PackageReviewOperatorCoordinate, PackageReviewTypeIdentity,
};
use semantic_vocabulary::PackageKeyIdentity;

mod recovery;

/// Receipt-bearing binding fixture paired with an already lossless signature;
/// this is not conversion from the lossy legacy executable-supply record.
#[derive(Clone)]
struct SupplyFixture {
    callable: PackageReviewNominalIdentity,
    signature: PackagePolicyExternalCallableSignature,
    requirement: PackagePolicyExternalRequirement,
    binding: PackageReviewExternalBinding,
}
impl SupplyFixture {
    fn policy(&self) -> PackagePolicyExternalExecutableSupply {
        PackagePolicyExternalExecutableSupply {
            callable: self.callable.clone(),
            signature: self.signature.clone(),
            requirement: self.requirement.clone(),
            binding: PackagePolicyExternalBinding::from(&self.binding),
        }
    }
    fn binding(&self) -> &PackageReviewExternalBinding {
        &self.binding
    }
}

fn static_parameter(kind: PackagePolicyTypeParameterKind) -> PackagePolicyTypeParameter {
    PackagePolicyTypeParameter {
        kind,
        bounds: PackageReviewDataProperties {
            multiplicity: language_semantics::Multiplicity::Affine,
            carry: None,
        },
    }
}

fn nominal(path: &str) -> PackageReviewNominalIdentity {
    PackageReviewNominalIdentity {
        owner: PackageReviewNominalOwner::Package(package(7)),
        path: path.to_owned(),
    }
}

fn package(value: u8) -> PackageKeyIdentity {
    PackageKeyIdentity::from_digest([value; 32]).expect("nonzero package identity")
}

fn value_type(canonical: &str) -> PackageReviewTypeIdentity {
    PackageReviewTypeIdentity {
        canonical: canonical.to_owned(),
    }
}

fn usage() -> PackageReviewEvaluatedBindingUsage {
    PackageReviewEvaluatedBindingUsage {
        usage_schema_version: 1,
        step_schedule_marker: 2,
        fuel_units: 3,
        fuel_ceiling: 4,
        build_log_bytes: 5,
        filesystem_operation_attempts: 6,
        peak_live_cells: 7,
        peak_live_text_bytes: 8,
        result_cells: 9,
        result_text_bytes: 10,
    }
}

fn import(locator: PackageReviewForeignLocator) -> PackageReviewEvaluatedImport {
    PackageReviewEvaluatedImport {
        target: target::TargetProfile::WindowsX64
            .identity()
            .as_str()
            .to_owned(),
        locator,
        locator_identity_digest: [1; 32],
        producer: nominal("Bindings::import"),
        producer_package: Some(package(7)),
        producer_callable_identity: "Bindings::import() -> Import".to_owned(),
        producer_closure_digest: [2; 32],
        evaluator_semantics_marker: 3,
        evaluation_usage: usage(),
        evaluation_digest: [4; 32],
        materializer_schema_version: 5,
        materialization_digest: [6; 32],
        receipt_locator_identity_digest: [7; 32],
        receipt_identity_digest: [8; 32],
    }
}

fn syscall() -> PackageReviewEvaluatedSyscall {
    PackageReviewEvaluatedSyscall {
        target: target::TargetProfile::LinuxX64
            .identity()
            .as_str()
            .to_owned(),
        number: 60,
        binding_identity_digest: [1; 32],
        producer: nominal("Bindings::exit"),
        producer_package: Some(package(7)),
        producer_callable_identity: "Bindings::exit() -> Syscall".to_owned(),
        producer_closure_digest: [2; 32],
        evaluator_semantics_marker: 3,
        evaluation_usage: usage(),
        evaluation_digest: [4; 32],
        materializer_schema_version: 5,
        materialization_digest: [6; 32],
        receipt_binding_identity_digest: [7; 32],
        receipt_identity_digest: [8; 32],
    }
}

fn locators() -> Vec<PackageReviewForeignLocator> {
    vec![
        PackageReviewForeignLocator::PeByName {
            library: b"kernel32.dll".to_vec(),
            export: b"ExitProcess".to_vec(),
        },
        PackageReviewForeignLocator::PeByOrdinal {
            library: b"kernel32.dll".to_vec(),
            ordinal: 19,
        },
        PackageReviewForeignLocator::ElfVersioned {
            object: b"libc.so.6".to_vec(),
            symbol: b"write".to_vec(),
            version: b"GLIBC_2.2.5".to_vec(),
        },
        PackageReviewForeignLocator::MachODylibSymbol {
            install_name: b"/usr/lib/libSystem.B.dylib".to_vec(),
            symbol: b"_write".to_vec(),
        },
    ]
}

fn supply(binding: PackageReviewExternalBinding) -> SupplyFixture {
    let signature = PackagePolicyExternalCallableSignature {
        lifetime_parameter_count: 1,
        static_parameters: vec![
            static_parameter(PackagePolicyTypeParameterKind::Type),
            static_parameter(PackagePolicyTypeParameterKind::Const(value_type("u64"))),
            static_parameter(PackagePolicyTypeParameterKind::Machine(
                PackagePolicyMachineParameterContract::Nominal {
                    trait_identity: nominal("Callable"),
                    requirement_identity: nominal("Callable::call"),
                },
            )),
        ],
        conformance_bounds: vec![PackageReviewConformanceBound {
            binder_ordinal: None,
            subject_parameter: 0,
            selected_conformance: None,
            selected_lifetime_arguments: Vec::new(),
            selected_arguments: Vec::new(),
            selected_subject: None,
            trait_identity: nominal("Copyable"),
            trait_lifetime_arguments: Vec::new(),
            arguments: Vec::new(),
        }],
        parameters: vec![PackageReviewExternalCallableParameter {
            type_identity: value_type("i32"),
            is_const: false,
            is_mutable: false,
            is_self: false,
        }],
        return_type: Some(value_type("unit")),
    };
    SupplyFixture {
        callable: nominal("Provider::invoke"),
        requirement: PackagePolicyExternalRequirement::TopLevelRequirement {
            identity: nominal("invoke"),
            signature: signature.clone(),
            alias: Some("chosen".to_owned()),
        },
        signature,
        binding,
    }
}

fn bytes(supply: &SupplyFixture) -> Vec<u8> {
    supply
        .policy()
        .canonical_bytes()
        .expect("bounded external-supply policy")
}

fn assert_changed(original: &SupplyFixture, changed: &SupplyFixture) {
    assert_ne!(original.policy(), changed.policy());
    assert_ne!(bytes(original), bytes(changed));
}

#[test]
fn every_foreign_locator_field_participates_in_policy_identity() {
    let rows = locators()
        .into_iter()
        .map(|locator| {
            supply(PackageReviewExternalBinding::NormalizedImport(import(
                locator,
            )))
        })
        .collect::<Vec<_>>();
    for (position, row) in rows.iter().enumerate() {
        for other in &rows[..position] {
            assert_changed(row, other);
        }
        let PackageReviewExternalBinding::NormalizedImport(original) = row.binding() else {
            panic!("normalized import fixture")
        };
        let mut changed_locators = Vec::new();
        let mut mutate = |mutation: fn(&mut PackageReviewForeignLocator)| {
            let mut locator = original.locator.clone();
            mutation(&mut locator);
            changed_locators.push(locator);
        };
        match &original.locator {
            PackageReviewForeignLocator::PeByName { .. } => {
                mutate(|locator| {
                    let PackageReviewForeignLocator::PeByName { library, .. } = locator else {
                        unreachable!()
                    };
                    library.push(b'x');
                });
                mutate(|locator| {
                    let PackageReviewForeignLocator::PeByName { export, .. } = locator else {
                        unreachable!()
                    };
                    export.push(b'x');
                });
            }
            PackageReviewForeignLocator::PeByOrdinal { .. } => {
                mutate(|locator| {
                    let PackageReviewForeignLocator::PeByOrdinal { library, .. } = locator else {
                        unreachable!()
                    };
                    library.push(b'x');
                });
                mutate(|locator| {
                    let PackageReviewForeignLocator::PeByOrdinal { ordinal, .. } = locator else {
                        unreachable!()
                    };
                    *ordinal += 1;
                });
            }
            PackageReviewForeignLocator::ElfVersioned { .. } => {
                mutate(|locator| {
                    let PackageReviewForeignLocator::ElfVersioned { object, .. } = locator else {
                        unreachable!()
                    };
                    object.push(b'x');
                });
                mutate(|locator| {
                    let PackageReviewForeignLocator::ElfVersioned { symbol, .. } = locator else {
                        unreachable!()
                    };
                    symbol.push(b'x');
                });
                mutate(|locator| {
                    let PackageReviewForeignLocator::ElfVersioned { version, .. } = locator else {
                        unreachable!()
                    };
                    version.push(b'x');
                });
            }
            PackageReviewForeignLocator::MachODylibSymbol { .. } => {
                mutate(|locator| {
                    let PackageReviewForeignLocator::MachODylibSymbol { install_name, .. } =
                        locator
                    else {
                        unreachable!()
                    };
                    install_name.push(b'x');
                });
                mutate(|locator| {
                    let PackageReviewForeignLocator::MachODylibSymbol { symbol, .. } = locator
                    else {
                        unreachable!()
                    };
                    symbol.push(b'x');
                });
            }
        }
        for locator in changed_locators {
            let mut changed = original.clone();
            changed.locator = locator;
            assert_changed(
                row,
                &supply(PackageReviewExternalBinding::NormalizedImport(changed)),
            );
        }
    }
}

#[test]
fn complete_callable_and_requirement_coordinates_change_policy() {
    let original = supply(PackageReviewExternalBinding::CompilerIntrinsic);
    let mutations: &[fn(&mut SupplyFixture)] = &[
        |row| row.callable.path.push_str("_other"),
        |row| row.callable.owner = PackageReviewNominalOwner::Package(package(8)),
        |row| row.signature.lifetime_parameter_count += 1,
        |row| {
            row.signature.static_parameters.pop();
        },
        |row| {
            row.signature.conformance_bounds[0]
                .trait_identity
                .path
                .push_str("_other")
        },
        |row| row.signature.parameters[0].type_identity = value_type("u32"),
        |row| row.signature.parameters[0].is_const = true,
        |row| row.signature.parameters[0].is_mutable = true,
        |row| row.signature.parameters[0].is_self = true,
        |row| row.signature.return_type = Some(value_type("u32")),
        |row| row.signature.return_type = None,
        |row| {
            let PackagePolicyExternalRequirement::TopLevelRequirement { identity, .. } =
                &mut row.requirement
            else {
                unreachable!()
            };
            identity.path.push_str("_other");
        },
        |row| {
            let PackagePolicyExternalRequirement::TopLevelRequirement { signature, .. } =
                &mut row.requirement
            else {
                unreachable!()
            };
            signature.return_type = Some(value_type("u32"));
        },
        |row| {
            let PackagePolicyExternalRequirement::TopLevelRequirement { alias, .. } =
                &mut row.requirement
            else {
                unreachable!()
            };
            *alias = None;
        },
    ];
    for mutation in mutations {
        let mut changed = original.clone();
        mutation(&mut changed);
        assert_changed(&original, &changed);
    }
    let mut trait_requirement = original.clone();
    trait_requirement.requirement =
        PackagePolicyExternalRequirement::Trait(PackagePolicyCallableConformance {
            trait_identity: nominal("Binding"),
            requirement_identity: nominal("invoke"),
            requirement_lifetime_partition: vec![0],
            trait_lifetime_arguments: vec![0],
            arguments: vec![value_type("u32")],
            alias: Some("chosen".to_owned()),
        });
    let mut operator_requirement = original.clone();
    operator_requirement.requirement = PackagePolicyExternalRequirement::Operator {
        coordinate: PackageReviewOperatorCoordinate {
            identity: nominal("invoke"),
            parameter_dispatch: "(i32)".to_owned(),
            result_dispatch: "unit".to_owned(),
        },
        alias: Some("chosen".to_owned()),
    };
    assert_changed(&original, &trait_requirement);
    assert_changed(&original, &operator_requirement);
    assert_changed(&trait_requirement, &operator_requirement);

    let trait_mutations: &[fn(&mut PackagePolicyCallableConformance)] = &[
        |row| row.trait_identity.path.push_str("_other"),
        |row| row.requirement_identity.path.push_str("_other"),
        |row| {
            row.requirement_lifetime_partition.push(0);
            row.trait_lifetime_arguments.push(0);
        },
        |row| row.arguments[0] = value_type("i64"),
        |row| row.alias = None,
    ];
    for mutation in trait_mutations {
        let mut changed = trait_requirement.clone();
        let PackagePolicyExternalRequirement::Trait(conformance) = &mut changed.requirement else {
            unreachable!()
        };
        mutation(conformance);
        assert_changed(&trait_requirement, &changed);
    }
    let operator_mutations: &[fn(&mut PackageReviewOperatorCoordinate)] = &[
        |row| row.identity.path.push_str("_other"),
        |row| row.parameter_dispatch.push_str("_other"),
        |row| row.result_dispatch.push_str("_other"),
    ];
    for mutation in operator_mutations {
        let mut changed = operator_requirement.clone();
        let PackagePolicyExternalRequirement::Operator { coordinate, .. } =
            &mut changed.requirement
        else {
            unreachable!()
        };
        mutation(coordinate);
        assert_changed(&operator_requirement, &changed);
    }
    let mut changed = operator_requirement.clone();
    let PackagePolicyExternalRequirement::Operator { alias, .. } = &mut changed.requirement else {
        unreachable!()
    };
    *alias = None;
    assert_changed(&operator_requirement, &changed);
}

#[test]
fn legacy_binding_fields_and_encoding_bound_are_preserved() {
    let pairs = [
        (
            PackageReviewExternalBinding::Syscall { number: 1 },
            PackageReviewExternalBinding::Syscall { number: 2 },
        ),
        (
            PackageReviewExternalBinding::VtableSlot { index: 1 },
            PackageReviewExternalBinding::VtableSlot { index: 2 },
        ),
        (
            PackageReviewExternalBinding::VtableField { field: "a".into() },
            PackageReviewExternalBinding::VtableField { field: "b".into() },
        ),
        (
            PackageReviewExternalBinding::TableFunction { field: "a".into() },
            PackageReviewExternalBinding::TableFunction { field: "b".into() },
        ),
    ];
    for (before, after) in pairs {
        assert_changed(&supply(before), &supply(after));
    }
    let oversized = supply(PackageReviewExternalBinding::VtableField {
        field: "x".repeat(4 * 1024 * 1024),
    });
    assert!(oversized.policy().canonical_bytes().is_err());
}
