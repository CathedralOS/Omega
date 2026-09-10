//! Every retained case field participates in identity and independent replay.
use super::*;
use legalized_operations::LegalizedScalarTerminator;
use optimization_unit::ValueDefinitionSite;

#[test]
fn structural_case_legalization_rejects_changed_payload_home_edge_and_custody() {
    for native in [
        ::target::NativeTarget::linux_x64(),
        ::target::NativeTarget::linux_arm64(),
    ] {
        let (source, target, unit) = fixture(native);
        let legal = crate::legalize_target_operations(&target, &source, &unit).unwrap();
        let identity = legalized_operations::legalized_operation_plan_identity(legal.plan());
        for mutation in [
            "producer",
            "place",
            "type",
            "multiplicity",
            "layout",
            "case",
            "tag",
            "target",
            "edge",
            "roster",
            "field",
            "offset",
            "parameter value",
            "parameter type",
            "parameter block",
            "parameter position",
            "parameter node",
            "cleanup",
            "fuel",
            "effect",
            "ownership",
            "missing payload",
            "missing case",
        ] {
            let mut changed = legal.plan().clone();
            let LegalizedScalarTerminator::StructuralCase {
                source: subject,
                layout,
                cases,
                effect,
                ownership,
            } = &mut changed.scalar_functions[0].blocks[0].terminator
            else {
                panic!("case")
            };
            let legalized_operations::LegalizedStructuralCaseSource::OperationResult {
                operation: defining_operation,
                result,
            } = subject
            else {
                panic!("operation source");
            };
            match mutation {
                "producer" => *defining_operation = OperationId::new(99).unwrap(),
                "place" => result.place = PlaceId::new(99).unwrap(),
                "type" => {
                    result.structural_type = semantic_vocabulary::StructuralTypeId::new(99).unwrap()
                }
                "multiplicity" => {
                    result.multiplicity = terminal_psi::StructuralMultiplicity::Linear
                }
                "layout" => layout.payload_byte_offset = 0,
                "case" => cases[0].case = StructuralCaseId::new(99).unwrap(),
                "tag" => cases[0].case_tag = 1,
                "target" => cases[0].target = cases[1].target,
                "edge" => cases[0].edge = EdgeId::new(99).unwrap(),
                "roster" => cases.swap(0, 1),
                "field" => cases[1].payloads[0].field = StructuralFieldId::new(99).unwrap(),
                "offset" => cases[1].payloads[0].field_byte_offset = 0,
                "parameter value" => {
                    cases[1].payloads[0].parameter.value = ValueId::new(99).unwrap()
                }
                "parameter type" => {
                    cases[1].payloads[0].parameter.scalar_type = ScalarType::Integer(u32_type())
                }
                "parameter block" => {
                    cases[1].payloads[0].parameter.definition_site =
                        ValueDefinitionSite::BlockParameter {
                            block: BlockId::new(99).unwrap(),
                            position: 0,
                        }
                }
                "parameter position" => {
                    cases[1].payloads[0].parameter.definition_site =
                        ValueDefinitionSite::BlockParameter {
                            block: BlockId::new(30).unwrap(),
                            position: 1,
                        }
                }
                "parameter node" => {
                    cases[1].payloads[0].parameter.definition_site = ValueDefinitionSite::Node {
                        block: BlockId::new(30).unwrap(),
                        node: 0,
                    }
                }
                "cleanup" => cases[0].trivial_affine_discards.clear(),
                "fuel" => cases[0].fuel.clear(),
                "effect" => effect.output += 1,
                "ownership" => {
                    ownership.push(optimization_unit::OwnershipEvent::Cleanup(Vec::new()))
                }
                "missing payload" => cases[1].payloads.clear(),
                "missing case" => {
                    cases.pop();
                }
                _ => unreachable!(),
            }
            assert_ne!(
                legalized_operations::legalized_operation_plan_identity(&changed),
                identity,
                "identity omitted {mutation}"
            );
            assert!(
                crate::validate_legalized_operations(&target, &source, &unit, changed).is_err(),
                "accepted {mutation}"
            );
        }
    }
}
