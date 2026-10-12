//! Source-free indexed application admission controls; no compiler input.

use super::*;
use crate::{decode_module, encode_module};
use semantic_vocabulary::MachineId;
use terminal_psi::{
    BoundaryApplication, BoundaryApplicationArgument, BoundaryApplicationCandidate,
    BoundaryApplicationLifetimeBinding, BoundaryDeclarationArgumentKind, BoundaryLifetimeArgument,
    BoundaryLifetimeParameter, BoundaryOperatorRequirement, BoundaryTypeIdentity,
    CheckedMachineDeclaration, CheckedMachineSpecialization, NominalDeclarationIdentity,
};

fn identity_table() -> TerminalIdentityTable {
    let mut identities = TerminalIdentityTable::from(vec![
        String::new(),
        "owner::requirement".into(),
        "owner::selection".into(),
        "owner::body".into(),
        "owner::body::entry".into(),
        "u64".into(),
    ])
    .into_builder();
    for declaration in 1..=5 {
        identities
            .retain_lifetime_declaration(
                TerminalIdentityIndex(declaration),
                vec![terminal_psi::TerminalLifetimeFormalRole::Captured],
            )
            .unwrap();
    }
    identities.build()
}

fn lifetime(declaration: TerminalIdentityIndex) -> BoundaryLifetimeParameter {
    BoundaryLifetimeParameter {
        declaration: NominalDeclarationIdentity::new(declaration),
        parameter_ordinal: 0,
    }
}

// Each leaf isolates one referenced role. Wrapping it below challenges the
// same roles beneath a genuine declaration application, including lifetimes.
fn applications_with_reference(identity: TerminalIdentityIndex) -> Vec<BoundaryApplication> {
    let mut applications = vec![
        BoundaryApplication::Exact {
            arguments: vec![BoundaryApplicationArgument::type_argument(
                0,
                BoundaryTypeIdentity::new(identity),
            )],
            lifetimes: Vec::new(),
        },
        BoundaryApplication::Exact {
            arguments: vec![BoundaryApplicationArgument::Const {
                binder_ordinal: 0,
                declared_carrier: BoundaryTypeIdentity::new(identity),
                value_type: "u64".into(),
                value_encoding: "integer(u64,7)".into(),
            }],
            lifetimes: Vec::new(),
        },
        BoundaryApplication::Exact {
            arguments: vec![BoundaryApplicationArgument::Value {
                binder_ordinal: 0,
                declared_carrier: BoundaryTypeIdentity::new(identity),
                value_type: "u64".into(),
                value_encoding: "integer(u64,7)".into(),
            }],
            lifetimes: Vec::new(),
        },
    ];
    for kind in [
        BoundaryDeclarationArgumentKind::Machine,
        BoundaryDeclarationArgumentKind::Conformance,
        BoundaryDeclarationArgumentKind::Proposition,
    ] {
        applications.push(BoundaryApplication::Exact {
            arguments: vec![BoundaryApplicationArgument::Declaration {
                binder_ordinal: 0,
                kind,
                declaration: NominalDeclarationIdentity::new(identity),
                application: BoundaryApplication::Empty,
            }],
            lifetimes: Vec::new(),
        });
    }
    for parameter in [true, false] {
        applications.push(BoundaryApplication::Exact {
            arguments: Vec::new(),
            lifetimes: vec![BoundaryApplicationLifetimeBinding {
                binder_ordinal: 0,
                parameter: lifetime(if parameter {
                    identity
                } else {
                    TerminalIdentityIndex(1)
                }),
                argument: BoundaryLifetimeArgument::Named(lifetime(if parameter {
                    TerminalIdentityIndex(2)
                } else {
                    identity
                })),
            }],
        });
    }
    applications
}

fn nested(application: BoundaryApplication) -> BoundaryApplication {
    BoundaryApplication::Exact {
        arguments: vec![BoundaryApplicationArgument::Declaration {
            binder_ordinal: 0,
            kind: BoundaryDeclarationArgumentKind::Machine,
            declaration: NominalDeclarationIdentity::new(TerminalIdentityIndex(1)),
            application,
        }],
        lifetimes: Vec::new(),
    }
}

#[test]
fn application_table_admission_preserves_all_categories_and_nested_lifetime_subjects() {
    let identities = identity_table();
    for (ordinal, application) in applications_with_reference(TerminalIdentityIndex(5))
        .into_iter()
        .enumerate()
    {
        assert_eq!(application.validate_in_identity_table(&identities), Ok(()));
        assert_eq!(
            application.validate_operator_application().is_ok(),
            ordinal < 2
        );
        let nested = nested(application);
        assert_eq!(nested.validate_in_identity_table(&identities), Ok(()));
        assert!(nested.validate_operator_application().is_err());
    }
}

#[test]
fn application_table_admission_rejects_absent_missing_and_empty_references() {
    let identities = identity_table();
    let mut empty_entries = identities.entries().to_vec();
    empty_entries[5].clear();
    let empty_identity = TerminalIdentityTable::from(empty_entries);
    for (identity, table) in [
        (TerminalIdentityIndex::ABSENT, &identities),
        (TerminalIdentityIndex(999), &identities),
        (TerminalIdentityIndex(5), &empty_identity),
    ] {
        for application in applications_with_reference(identity) {
            assert!(application.validate_in_identity_table(table).is_err());
            assert!(
                nested(application)
                    .validate_in_identity_table(table)
                    .is_err()
            );
        }
    }
}

fn candidate_module() -> TerminalModule {
    let mut module = TerminalModule::for_entry(MachineId::new(1).unwrap());
    module.identities = identity_table();
    module
        .boundary_application_candidates
        .push(BoundaryApplicationCandidate {
            requirement: BoundaryOperatorRequirement::new(
                NominalDeclarationIdentity::new(TerminalIdentityIndex(1)),
                "operator::compare".into(),
                None,
            )
            .unwrap(),
            application: BoundaryApplication::exact(vec![
                BoundaryApplicationArgument::type_argument(
                    0,
                    BoundaryTypeIdentity::new(TerminalIdentityIndex(5)),
                ),
            ])
            .unwrap(),
            selection_identity: TerminalIdentityIndex(2),
            selection_package: None,
            target: None,
            body: CheckedMachineDeclaration {
                machine: NominalDeclarationIdentity::new(TerminalIdentityIndex(3)),
                state: NominalDeclarationIdentity::new(TerminalIdentityIndex(4)),
                contract: [1; 32],
                specialization: None,
            },
        });
    module
}

#[test]
fn decoded_candidate_refuses_out_of_table_type_and_constant_carriers() {
    let module = candidate_module();
    let bytes = encode_module(&module).unwrap();
    assert_eq!(decode_module(&bytes).unwrap(), module);
    for application in [
        BoundaryApplication::exact(vec![BoundaryApplicationArgument::type_argument(
            0,
            BoundaryTypeIdentity::new(TerminalIdentityIndex(999)),
        )])
        .unwrap(),
        BoundaryApplication::exact(vec![
            BoundaryApplicationArgument::const_argument(
                0,
                BoundaryTypeIdentity::new(TerminalIdentityIndex(999)),
                "u64".into(),
                "integer(u64,7)".into(),
            )
            .unwrap(),
        ])
        .unwrap(),
    ] {
        let mut malformed = module.clone();
        malformed.boundary_application_candidates[0].application = application;
        // The trusted producer encoder keeps its existing verification surface.
        // External decoding must refuse the unresolved reference it receives.
        let bytes = encode_module(&malformed).unwrap();
        assert_eq!(
            decode_module(&bytes),
            Err(CodecError::MalformedStructuralFoundation(
                "boundary application identity is not in its issuing table",
            ))
        );
    }
}

#[test]
fn decoded_candidate_refuses_out_of_table_requirement_selection_and_body_references() {
    for role in 0..5 {
        let mut module = candidate_module();
        let candidate = &mut module.boundary_application_candidates[0];
        let missing = NominalDeclarationIdentity::new(TerminalIdentityIndex(999));
        match role {
            0 => {
                candidate.requirement =
                    BoundaryOperatorRequirement::new(missing, "operator::compare".into(), None)
                        .unwrap()
            }
            1 => candidate.selection_identity = TerminalIdentityIndex(999),
            2 => candidate.body.machine = missing,
            3 => candidate.body.state = missing,
            4 => {
                candidate.body.specialization = Some(CheckedMachineSpecialization {
                    template: missing,
                    commitment: [2; 32],
                })
            }
            _ => unreachable!(),
        }
        let bytes = encode_module(&module).unwrap();
        assert_eq!(
            decode_module(&bytes),
            Err(CodecError::MalformedStructuralFoundation(
                "boundary application declaration is not in its issuing table",
            ))
        );
    }
}

#[test]
fn demand_reference_admission_uses_the_same_module_table() {
    let mut module = TerminalModule::for_entry(MachineId::new(1).unwrap());
    module.identities = identity_table();
    for identity in [TerminalIdentityIndex(5), TerminalIdentityIndex(999)] {
        module.boundary_application_demands =
            vec![terminal_psi::TerminalBoundaryApplicationDemand::new(
                semantic_vocabulary::OperationId::new(1).unwrap(),
                BoundaryOperatorRequirement::new(
                    NominalDeclarationIdentity::new(TerminalIdentityIndex(1)),
                    "operator::compare".into(),
                    None,
                )
                .unwrap(),
                BoundaryApplication::exact(vec![BoundaryApplicationArgument::type_argument(
                    0,
                    BoundaryTypeIdentity::new(identity),
                )])
                .unwrap(),
            )];
        assert_eq!(
            validate_boundary_application_identities(&module).is_ok(),
            identity.0 == 5
        );
    }
}

#[test]
fn table_admission_preserves_original_order_and_depth_validation() {
    let identities = identity_table();
    let reordered = BoundaryApplication::Exact {
        arguments: vec![BoundaryApplicationArgument::type_argument(
            1,
            BoundaryTypeIdentity::new(TerminalIdentityIndex(5)),
        )],
        lifetimes: Vec::new(),
    };
    assert_eq!(
        reordered.validate_in_identity_table(&identities),
        Err("boundary application arguments are not in exact binder order")
    );
    let mut application = BoundaryApplication::Empty;
    for _ in 0..=terminal_psi::MAX_BOUNDARY_APPLICATION_DEPTH {
        application = nested(application);
    }
    assert_eq!(
        application.validate_in_identity_table(&identities),
        Err("boundary application nesting exceeds the supported depth")
    );
}
