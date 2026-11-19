use semantic_vocabulary::{StructuralDomainId, StructuralTypeId};
use terminal_psi::{
    BoundaryMachineDeclaration, BoundaryMachineResult, NominalDeclarationIdentity,
    StructuralDomainDeclaration, StructuralEstablishmentRoute, TerminalModule,
};

pub(super) type Memberships = Vec<(StructuralDomainId, StructuralDomainId)>;
pub(super) type InputMemberships = Vec<Memberships>;

pub(super) fn admit_input_memberships(
    receiver: &TerminalModule,
    required: &BoundaryMachineDeclaration,
    provider: &TerminalModule,
    supplied: &BoundaryMachineDeclaration,
) -> Result<InputMemberships, String> {
    if required.structural_parameters.len() != supplied.structural_parameters.len() {
        return Err("component input membership arities differ".into());
    }
    required
        .structural_parameters
        .iter()
        .zip(&supplied.structural_parameters)
        .map(|(left, right)| {
            admit_memberships(
                receiver,
                left.structural_type,
                &left.qualifications,
                provider,
                right.structural_type,
                &right.qualifications,
            )
            .map_err(|reason| format!("component input {reason}"))
        })
        .collect()
}

pub(super) fn admit_result_memberships(
    receiver: &TerminalModule,
    required: &BoundaryMachineDeclaration,
    provider: &TerminalModule,
    supplied: &BoundaryMachineDeclaration,
) -> Result<Memberships, String> {
    match (&supplied.result, &required.result) {
        (BoundaryMachineResult::Unit, BoundaryMachineResult::Unit) => Ok(Vec::new()),
        (BoundaryMachineResult::Scalar(left), BoundaryMachineResult::Scalar(right))
            if left == right =>
        {
            Ok(Vec::new())
        }
        (BoundaryMachineResult::Structural(left), BoundaryMachineResult::Structural(right)) => {
            if !left.projected_qualifications.is_empty()
                || !right.projected_qualifications.is_empty()
            {
                return Err("component result requires projected membership transfer".into());
            }
            admit_memberships(
                provider,
                left.structural_type,
                &left.qualifications,
                receiver,
                right.structural_type,
                &right.qualifications,
            )
            .map_err(|reason| format!("component result {reason}"))
        }
        _ => Err("component result kinds or scalar contracts differ".into()),
    }
}

fn admit_memberships(
    source_module: &TerminalModule,
    source_type: StructuralTypeId,
    source_required: &[StructuralDomainId],
    target_module: &TerminalModule,
    target_type: StructuralTypeId,
    target_required: &[StructuralDomainId],
) -> Result<Memberships, String> {
    let mut bindings = Vec::new();
    for source in source_module
        .structural_domains
        .iter()
        .filter(|row| row.carrier == source_type)
    {
        let mut matches = target_module.structural_domains.iter().filter(|row| {
            row.carrier == target_type && same_domain(source_module, source, target_module, row)
        });
        if let Some(destination) = matches.next() {
            if matches.next().is_some() {
                return Err("has ambiguous membership theories".into());
            }
            bindings.push((source.id, destination.id));
        }
    }
    let mut mapped = source_required
        .iter()
        .map(|source| {
            bindings
                .iter()
                .find(|(domain, _)| domain == source)
                .map(|(_, domain)| *domain)
                .ok_or_else(|| "membership theories differ".to_owned())
        })
        .collect::<Result<Vec<_>, _>>()?;
    mapped.sort();
    if mapped != target_required {
        return Err("membership requirements differ".into());
    }
    Ok(bindings)
}

fn same_domain(
    left: &TerminalModule,
    source: &StructuralDomainDeclaration,
    right: &TerminalModule,
    target: &StructuralDomainDeclaration,
) -> bool {
    if source
        .required_domain_closure(&left.structural_domains)
        .is_none()
        || target
            .required_domain_closure(&right.structural_domains)
            .is_none()
    {
        return false;
    }
    let mut pending = vec![(source, target)];
    let mut paired = Vec::new();
    while let Some((source, target)) = pending.pop() {
        if paired.iter().any(|(a, b)| {
            (*a == source.id || *b == target.id) && (*a != source.id || *b != target.id)
        }) {
            return false;
        }
        if paired.contains(&(source.id, target.id)) {
            continue;
        }
        if !same_domain_definition(left, source, right, target) {
            return false;
        }
        paired.push((source.id, target.id));
        let (Some(source_required), Some(target_required)) =
            (&source.required_memberships, &target.required_memberships)
        else {
            return false;
        };
        if source_required.len() != target_required.len() {
            return false;
        }
        for required in source_required {
            let Some(source_parent) = required_domain(left, source, *required) else {
                return false;
            };
            let mut matches = target_required.iter().filter_map(|required| {
                required_domain(right, target, *required).filter(|parent| {
                    NominalDeclarationIdentity::new(source_parent.identity).same_identity_as(
                        &left.identities,
                        &NominalDeclarationIdentity::new(parent.identity),
                        &right.identities,
                    )
                })
            });
            let Some(target_parent) = matches.next() else {
                return false;
            };
            if matches.next().is_some() {
                return false;
            }
            pending.push((source_parent, target_parent));
        }
    }
    true
}

fn required_domain<'a>(
    module: &'a TerminalModule,
    child: &StructuralDomainDeclaration,
    domain: semantic_vocabulary::DomainSemanticId,
) -> Option<&'a StructuralDomainDeclaration> {
    let mut matches = module
        .structural_domains
        .iter()
        .filter(|row| row.semantic_domain == domain && row.carrier == child.carrier);
    let row = matches.next()?;
    matches.next().is_none().then_some(row)
}

fn same_domain_definition(
    left: &TerminalModule,
    source: &StructuralDomainDeclaration,
    right: &TerminalModule,
    target: &StructuralDomainDeclaration,
) -> bool {
    if source.establishment_routes.is_none() != target.establishment_routes.is_none()
        || !terminal_semantics::structural_qualifications::same_access_policy(
            left, source, right, target,
        )
    {
        return false;
    }
    let same_name = |a, b| {
        NominalDeclarationIdentity::new(a).same_identity_as(
            &left.identities,
            &NominalDeclarationIdentity::new(b),
            &right.identities,
        )
    };
    let carrier = |module: &TerminalModule, id| {
        module
            .structural_types
            .iter()
            .find(|row| row.id == id)
            .map(|row| row.identity)
    };
    if !source.complete_predicate_theory
        || !target.complete_predicate_theory
        || !same_name(source.identity, target.identity)
        || !carrier(left, source.carrier)
            .zip(carrier(right, target.carrier))
            .is_some_and(|(a, b)| same_name(a, b))
        || source.byte_predicate != target.byte_predicate
        || source
            .content_projection
            .as_ref()
            .map(|row| (&row.algebra, &row.expression))
            != target
                .content_projection
                .as_ref()
                .map(|row| (&row.algebra, &row.expression))
    {
        return false;
    }
    match (&source.predicate, &target.predicate) {
        (Some(a), Some(b))
            if a.proposition == b.proposition
                && terminal_semantics::domain_predicate_observes_no_storage(
                    left.structural_types.iter(),
                    source.carrier,
                    a,
                )
                && terminal_semantics::domain_predicate_observes_no_storage(
                    right.structural_types.iter(),
                    target.carrier,
                    b,
                ) => {}
        _ => return false,
    }
    let routes = |module: &TerminalModule, domain: &StructuralDomainDeclaration| {
        let mut rows = domain
            .establishment_routes
            .as_deref()
            .unwrap_or_default()
            .iter()
            .map(|route| {
                let kind = match route {
                    StructuralEstablishmentRoute::CheckedRequirement { .. } => 0,
                    StructuralEstablishmentRoute::ExactMachine { .. } => 1,
                    StructuralEstablishmentRoute::BoundaryRequirement { .. } => 2,
                };
                let declaration = route.declaration();
                let identity = route.identity();
                if module.identities.is_invocation_local(identity)
                    || module
                        .identities
                        .is_invocation_local(declaration.identity())
                {
                    return None;
                }
                let owner = module
                    .identities
                    .text(declaration.identity())
                    .filter(|name| !name.is_empty())?;
                module
                    .identities
                    .text(identity)
                    .filter(|name| !name.is_empty())
                    .map(|name| (kind, owner.to_owned(), name.to_owned()))
            })
            .collect::<Option<Vec<_>>>()?;
        rows.sort();
        Some(rows)
    };
    routes(left, source)
        .zip(routes(right, target))
        .is_some_and(|(a, b)| a == b)
}
#[cfg(test)]
mod tests {
    use super::*;
    use semantic_vocabulary::{DomainSemanticId, MachineId, PlaceId, Proposition};
    use terminal_psi::{
        StructuralDomainPredicate, StructuralRecordStorage, StructuralTypeDeclaration,
        StructuralTypeShape, TerminalIdentityTableBuilder,
    };

    fn module(
        domain: u64,
        carrier: u64,
        pad_identity: bool,
        deny_issuance: bool,
    ) -> TerminalModule {
        let mut identities = TerminalIdentityTableBuilder::default();
        if pad_identity {
            identities.intern("unrelated declaration");
        }
        let carrier_identity = identities.intern("test::Cell");
        let domain_identity = identities.intern("test::Cell::Controlled");
        let carrier = StructuralTypeId::new(carrier).unwrap();
        let mut module = TerminalModule::for_entry(MachineId::new(1).unwrap());
        module.identities = identities.build();
        module.structural_types.push(StructuralTypeDeclaration {
            declaration_identity: Default::default(),
            requires: Vec::new(),
            id: carrier,
            identity: carrier_identity,
            quotient_carrier: None,
            owns_linear_claim: false,
            no_code_affine_cleanup: true,
            multiplicity: terminal_psi::StructuralMultiplicity::Unrestricted,
            descriptor_cleanup: false,
            nominal_cleanup: None,
            scalar_restriction: None,
            shape: StructuralTypeShape::Record {
                storage: StructuralRecordStorage::Packed,
                fields: Vec::new(),
            },
        });
        module.structural_domains.push(StructuralDomainDeclaration {
            subject_identity: Default::default(),
            access_policy: None,
            required_memberships: Some(Vec::new()),
            complete_predicate_theory: true,
            predicate: Some(StructuralDomainPredicate {
                subject: PlaceId::new(1).unwrap(),
                proposition: Proposition::Truth,
            }),
            id: StructuralDomainId::new(domain).unwrap(),
            semantic_domain: DomainSemanticId::new(domain).unwrap(),
            identity: domain_identity,
            carrier,
            content_projection: None,
            byte_predicate: None,
            establishment_routes: deny_issuance.then(Vec::new),
        });
        module
    }

    fn policy_module(carrier: u64, pad: bool, with_cases: bool) -> TerminalModule {
        use schema_reflection::CaseDescription;
        use schema_reflection::DeclarationDescription;
        use schema_reflection::FieldDescription;
        use schema_reflection::SchemaNode;
        use schema_reflection::SchemaNodeHandle;
        use schema_reflection::SchemaQueryAuthority;
        use schema_reflection::SchemaShape;
        use schema_reflection::SemanticSchemaGraph;
        use semantic_vocabulary::{StructuralCaseId, StructuralFieldId};
        use terminal_psi::{
            BindingRelevance, StructuralCaseDeclaration, StructuralFieldDeclaration,
            StructuralFieldType, StructuralPathSegment,
        };
        let mut module = module(carrier, carrier, pad, false);
        let mut identities = std::mem::take(&mut module.identities).into_builder();
        module.structural_types[0].declaration_identity = identities.intern("test::Cell");
        module.structural_domains[0].subject_identity = identities.intern("test::Cell");
        let root = SchemaNodeHandle::from_index(1);
        let left = SchemaNodeHandle::from_index(2);
        let right = SchemaNodeHandle::from_index(3);
        let value = SchemaNodeHandle::from_index(4);
        let mut nodes = vec![
            SchemaNode::Invalid,
            SchemaNode::Declaration(DeclarationDescription {
                owner_identity: "test::Cell".into(),
                identity_is_hermetic: true,
                name: "Cell".into(),
                shape: if with_cases {
                    SchemaShape::Sum
                } else {
                    SchemaShape::Empty
                },
                supply: "Ordinary".into(),
                multiplicity: "Unrestricted".into(),
                carry: None,
                type_parameters: Vec::new(),
                lifetime_parameters: Vec::new(),
                application_identity: Some("test::Cell".into()),
                members: if with_cases {
                    vec![left, right]
                } else {
                    Vec::new()
                },
                retired_identities: Vec::new(),
            }),
        ];
        let region = if with_cases {
            let case = StructuralCaseId::new(carrier + 1).unwrap();
            let key = identities.intern("value");
            module.structural_types[0].shape = StructuralTypeShape::Sum {
                cases: vec![
                    StructuralCaseDeclaration {
                        declaration_identity: identities.intern("test::Cell::Left"),
                        id: case,
                        identity: identities.intern("Left"),
                        fields: vec![StructuralFieldDeclaration {
                            declaration_identity: identities.intern("test::Cell::Left::value"),
                            inline_carrier: None,
                            id: StructuralFieldId::new(carrier + 3).unwrap(),
                            identity: key,
                            relevance: BindingRelevance::Relevant,
                            field_type: StructuralFieldType::Scalar(
                                semantic_vocabulary::ScalarType::Boolean,
                            ),
                            storage: None,
                        }],
                    },
                    StructuralCaseDeclaration {
                        declaration_identity: identities.intern("test::Cell::Right"),
                        id: StructuralCaseId::new(carrier + 2).unwrap(),
                        identity: identities.intern("Right"),
                        fields: Vec::new(),
                    },
                ],
            };
            nodes.extend([
                SchemaNode::Case(CaseDescription {
                    member_identity: "test::Cell::Left".into(),
                    name: "Left".into(),
                    declaration_position: 0,
                    stable_number: None,
                    payload_fields: vec![value],
                    retired_payload_identities: Vec::new(),
                }),
                SchemaNode::Case(CaseDescription {
                    member_identity: "test::Cell::Right".into(),
                    name: "Right".into(),
                    declaration_position: 1,
                    stable_number: None,
                    payload_fields: Vec::new(),
                    retired_payload_identities: Vec::new(),
                }),
                SchemaNode::Field(FieldDescription {
                    member_identity: "test::Cell::Left::value".into(),
                    name: "value".into(),
                    declaration_position: 0,
                    stable_number: None,
                    erased: false,
                    qualified_type: "bool".into(),
                    nominal_references: Vec::new(),
                    owner_case: left,
                }),
            ]);
            (
                value,
                Some(vec![
                    StructuralPathSegment::Case(case),
                    StructuralPathSegment::Field(key),
                ]),
            )
        } else {
            (root, Some(Vec::new()))
        };
        let schema = SemanticSchemaGraph::from_nodes(
            nodes,
            root,
            SchemaQueryAuthority::OwningScope {
                requester_identity: "test::Cell::Controlled".into(),
            },
        )
        .unwrap();
        module.structural_domains[0].access_policy =
            Some(terminal_psi::StructuralDomainAccessPlan {
                schema,
                issuance: None,
                readers: vec![(region, Vec::new())],
                writers: Vec::new(),
                observations: Vec::new(),
                reach: Default::default(),
            });
        module.identities = identities.build();
        module
    }

    // Equal definitions use different local numbers. Changing an empty
    // subject, a case owner or an explicit denial must still change the
    // admitted policy; a matching name cannot supply missing source data.
    #[test]
    fn component_policies_keep_their_actual_source_roots_and_case_fields() {
        use schema_reflection::SchemaNode;
        use schema_reflection::SchemaNodeHandle;
        use schema_reflection::SemanticSchemaGraph;
        for with_cases in [false, true] {
            let source = policy_module(1, false, with_cases);
            let target = policy_module(9, true, with_cases);
            let compatible = |target: &TerminalModule| {
                let source_domain = &source.structural_domains[0];
                let target_domain = &target.structural_domains[0];
                admit_memberships(
                    &source,
                    source_domain.carrier,
                    &[source_domain.id],
                    target,
                    target_domain.carrier,
                    &[target_domain.id],
                )
                .is_ok()
            };
            assert!(
                compatible(&target),
                "resolve each module's own case, field and identity numbers"
            );
            let change_schema = |target: &mut TerminalModule, edit: fn(&mut Vec<SchemaNode>)| {
                let plan = target.structural_domains[0].access_policy.as_mut().unwrap();
                let mut nodes = plan.schema.nodes().to_vec();
                edit(&mut nodes);
                plan.schema = SemanticSchemaGraph::from_nodes(
                    nodes,
                    plan.schema.root(),
                    plan.schema.authority().clone(),
                )
                .unwrap();
            };
            let mut unrelated = target.clone();
            change_schema(&mut unrelated, |nodes| {
                let SchemaNode::Declaration(root) = &mut nodes[1] else {
                    unreachable!()
                };
                root.owner_identity = "test::Other".into();
            });
            assert!(
                !compatible(&unrelated),
                "even an empty field roster needs its actual nominal root"
            );
            let mut application = target.clone();
            change_schema(&mut application, |nodes| {
                let SchemaNode::Declaration(root) = &mut nodes[1] else {
                    unreachable!()
                };
                root.application_identity = Some("test::Cell<Other>".into());
            });
            assert!(
                !compatible(&application),
                "the nominal root cannot replace its selected application"
            );
            let mut widened = target.clone();
            widened.structural_domains[0]
                .access_policy
                .as_mut()
                .unwrap()
                .readers
                .clear();
            assert!(
                !compatible(&widened),
                "an absent reader row cannot replace an explicit empty denial"
            );
            if with_cases {
                let mut wrong_case = target.clone();
                change_schema(&mut wrong_case, |nodes| {
                    let SchemaNode::Case(left) = &mut nodes[2] else {
                        unreachable!()
                    };
                    left.payload_fields.clear();
                    let SchemaNode::Case(right) = &mut nodes[3] else {
                        unreachable!()
                    };
                    right.payload_fields.push(SchemaNodeHandle::from_index(4));
                    let SchemaNode::Field(field) = &mut nodes[4] else {
                        unreachable!()
                    };
                    field.owner_case = SchemaNodeHandle::from_index(3);
                });
                assert!(
                    !compatible(&wrong_case),
                    "an existing field cannot borrow another case's source owner"
                );
                let mut synthetic = target.clone();
                synthetic.structural_types[0]
                    .shape
                    .fields_mut()
                    .next()
                    .unwrap()
                    .declaration_identity = Default::default();
                assert!(
                    !compatible(&synthetic),
                    "a matching field key cannot replace its source declaration"
                );
            }
        }
    }

    // Provider membership compatibility compares the actual declarations,
    // including issuer choice, even when each module numbers them differently.
    #[test]
    fn component_memberships_distinguish_empty_denial_from_unrestricted_issuance() {
        for deny in [false, true] {
            let receiver = module(1, 1, false, deny);
            let provider = module(7, 9, true, deny);
            let source = &receiver.structural_domains[0];
            let target = &provider.structural_domains[0];
            assert_ne!(source.identity, target.identity);
            assert_eq!(
                admit_memberships(
                    &receiver,
                    source.carrier,
                    &[source.id],
                    &provider,
                    target.carrier,
                    &[target.id]
                )
                .unwrap(),
                [(source.id, target.id)],
            );

            let mismatched = module(7, 9, true, !deny);
            let target = &mismatched.structural_domains[0];
            let reason = admit_memberships(&receiver, source.carrier, &[source.id], &mismatched, target.carrier, &[target.id])
                .expect_err("an empty issuer denial cannot substitute for unrestricted issuance in either direction");
            assert!(reason.contains("membership theories differ"), "{reason}");
        }
    }
}
