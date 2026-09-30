//! Borrowed service receipts rejoin authored parameters, not erased ABI shapes.

use super::{lower_unit_parameters, validate_direct_unit_parameter_custody};
use checked_trees::{CheckedStructuralAccess, CheckedTrees, CheckedUnitStructuralParameterPlan};
use language_semantics::Multiplicity;
use source::{SourceMap, SourceOrigin};
use std::path::PathBuf;

fn fixture(access: &str, composed: bool) -> CheckedTrees {
    let body = if composed {
        format!(
            "transition enabled {{ true -> output(host, value) false -> done(host) }}
             state output(host: {access}Binding<Host>, value: u64) {{ host.mark(value); }}
             state done(host: {access}Binding<Host>) {{}}"
        )
    } else {
        "host.mark(value);".into()
    };
    let program = format!(
        "pub boundary trait Host {{ machine mark(value: u64); }}
         pub boundary trait Other {{ machine mark(value: u64); }}
         machine report(host: {access}Binding<Host>, value: u64, enabled: bool) reaches Host {{ {body} }}"
    );
    let binding = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../../../../source/library/core/binding.omg"
    ));
    let mut sources = SourceMap::default();
    let binding_source = sources
        .add_with_metadata(
            PathBuf::from("source/library/core/binding.omg"),
            binding.into(),
            PathBuf::from("source/library/core"),
            None,
            SourceOrigin::Toolchain,
        )
        .source_id;
    let program_source = sources
        .add(PathBuf::from("tests/main.omg"), program.clone())
        .source_id;
    let mut typed = crate::front_end::typed_program_from_source_map(
        sources,
        &[(binding_source, binding), (program_source, &program)],
    );
    let authorizations = typed
        .traits()
        .iter()
        .filter(|row| row.is_boundary)
        .map(
            |row| typed_trees::typed_trees::FusedServiceErasureAuthorization {
                requirement: row.symbol,
            },
        )
        .collect();
    typed
        .bind_fused_service_erasures(authorizations)
        .expect("selected Fused fixtures");
    typed_trees_to_checked_trees::lower_typed_trees(
        typed,
        &typed_trees_to_checked_trees::CheckingRequest::settled(),
    )
    .expect("borrowed Binding source checks")
}

fn parameter(
    checked: &mut CheckedTrees,
    composed: bool,
) -> &mut CheckedUnitStructuralParameterPlan {
    let report = checked
        .machines()
        .iter()
        .find(|row| row.name.as_str() == "report")
        .unwrap()
        .symbol;
    let plans = &mut checked.facts.flow.terminal_unit_effects;
    if composed {
        &mut plans
            .composed_machines
            .iter_mut()
            .find(|row| row.machine == report)
            .expect("composed report")
            .states[0]
            .structural_parameters[0]
    } else {
        &mut plans
            .machines
            .iter_mut()
            .find(|row| row.machine == report)
            .expect("direct report")
            .structural_parameters[0]
    }
}

#[test]
fn borrowed_service_parameters_rejoin_direct_and_composed_source_custody() {
    for (access, expected) in [
        ("&", CheckedStructuralAccess::SharedBorrow),
        ("&mut ", CheckedStructuralAccess::MutableBorrow),
    ] {
        for composed in [false, true] {
            let mut checked = fixture(access, composed);
            validate_direct_unit_parameter_custody(&checked).expect("exact source receipts");
            let parameter = parameter(&mut checked, composed);
            assert_eq!(parameter.access, expected);
            assert_eq!(parameter.multiplicity, Multiplicity::Unrestricted);
            let type_id = semantic_vocabulary::StructuralTypeId::new(1).unwrap();
            let types = vec![(parameter.type_identity.clone(), type_id)];
            let lowered =
                lower_unit_parameters(std::slice::from_ref(parameter), &types, &[], &[], &mut 1)
                    .expect("borrowed receipt keeps its ordinary reference declaration");
            assert_eq!(lowered[0].access, super::terminal_access(expected));
            assert_eq!(
                lowered[0].multiplicity,
                terminal_psi::StructuralMultiplicity::Unrestricted
            );
        }
    }
}

#[test]
fn borrowed_service_parameters_reject_missing_or_substituted_custody() {
    for composed in [false, true] {
        for mutation in 0..8 {
            let mut checked = fixture("&mut ", composed);
            let other_requirement = checked
                .traits()
                .iter()
                .find(|row| row.name.as_str() == "Other")
                .unwrap()
                .symbol;
            let parameter = parameter(&mut checked, composed);
            match mutation {
                0 => parameter.fused_service_erasure = None,
                1 => parameter.access = CheckedStructuralAccess::SharedBorrow,
                2 => parameter.multiplicity = Multiplicity::Affine,
                3 => {
                    parameter
                        .fused_service_erasure
                        .as_mut()
                        .unwrap()
                        .requirement = other_requirement
                }
                4 => {
                    parameter
                        .fused_service_erasure
                        .as_mut()
                        .unwrap()
                        .carrier_type_identity = parameter.type_identity.clone()
                }
                5 => {
                    parameter
                        .fused_service_erasure
                        .as_mut()
                        .unwrap()
                        .source_parameter = other_requirement
                }
                6 => parameter.position = u32::MAX,
                7 => parameter.type_identity.push_str("::substituted"),
                _ => unreachable!(),
            }
            assert!(
                validate_direct_unit_parameter_custody(&checked).is_err(),
                "composed={composed} mutation={mutation}"
            );
        }
    }
}
