//! Ordered exact comparisons. A payload equality has two ways to fail: the
//! discriminator does not select its case, or the selected field does not match.
use super::{BlockFlow, check_container, site_at, successor, ungroup_pattern, unsupported_node};
use crate::machines::expressions::{EmitOperation, Expressions, OperationOutput};
use crate::{
    ArithmeticPolicy, BlockId, BlockTerminator, CaseSelection, DataId, IntegerOperation, IntegerPrimitive, LoweringError, MachineBody, OperationKind,
    OperationResult, PlaceProjection, ProjectedPlace, TypeId, TypeShape, ValueId,
};
use omega_resolved::{IndexedSource, Symbol};
use omega_syntax::NodeKind;
use omega_typed::{Access, NodeSemantics, ObligationKind, Projection, Site, ValueType};

pub(super) fn has_literal_test(
    file: &IndexedSource<'_>,
    arms: &[usize],
) -> bool {
    arms.iter().any(|&arm| {
        let Some(&pattern) = file.nodes[arm].children.first() else { return false };
        let Ok(pattern) = ungroup_pattern(file, pattern, |_| Ok(())) else {
            return false;
        };
        let pattern = &file.nodes[pattern];
        match (pattern.kind, pattern.children.as_slice()) {
            (NodeKind::ValuePattern, [lit]) => file.nodes[*lit].kind == NodeKind::Literal,
            (NodeKind::RecordPattern, [_, fields @ ..]) => fields
                .iter()
                .any(|&field| matches!(file.nodes[field].children.as_slice(), [child] if file.nodes[*child].kind == NodeKind::Literal)),
            _ => false,
        }
    })
}

pub(in crate::machines) enum PayloadBinding {
    Borrowed(PayloadPlace),
    Captured(ValueId),
}

impl PayloadBinding {
    pub(in crate::machines) fn read(
        &self,
        expressions: &Expressions<'_>,
        binder: usize,
        node: usize,
        carrier: &ValueType,
        body: &mut MachineBody,
    ) -> Result<ValueId, LoweringError> {
        match self {
            Self::Borrowed(place) => place.read_binding(expressions, binder, node, carrier, body),
            Self::Captured(value) => {
                let mut rows = expressions.tree.semantics_at(node);
                if matches!(rows.next(), Some(NodeSemantics::Expression(e)) if e.static_argument_slot.is_none() && &e.ty == carrier)
                    && rows.next().is_none() && expressions.types.id_for(carrier).ok() == Some(body.values[value.index()].ty)
                {
                    Ok(*value)
                } else {
                    Err(unsupported_node(expressions.tree.resolved().file, node, "owned payload use disagrees with its captured scalar"))
                }
            }
        }
    }
}

/// Keep the borrowed backing, not a speculatively copied scalar. For a named
/// binding L4 attaches CopyRead to the use, not to the field declaration.
pub(in crate::machines) struct PayloadPlace {
    source: ProjectedPlace,
    pub(in crate::machines) ty: TypeId,
}
impl PayloadPlace {
    pub(in crate::machines) fn emit(
        &self,
        body: &mut MachineBody,
        site: Site,
    ) -> ValueId {
        emit_scalar(body, self.ty, OperationKind::ProjectedScalarRead { source: self.source.clone() }, site)
    }

    fn read_binding(
        &self,
        expressions: &Expressions<'_>,
        binder: usize,
        node: usize,
        carrier: &ValueType,
        body: &mut MachineBody,
    ) -> Result<ValueId, LoweringError> {
        let tree = expressions.tree;
        let file = tree.resolved().file;
        let typed = expressions.tree.forest();
        if typed.place_path(file.node_id(node)) != typed.place_path(file.node_id(binder)) {
            return Err(unsupported_node(file, node, "payload binding use disagrees with its borrowed backing"));
        }
        let mut expression = false;
        let mut copy = None;
        for row in tree.semantics_at(node) {
            match row {
                NodeSemantics::Expression(e) if !expression && e.static_argument_slot.is_none() && &e.ty == carrier => expression = true,
                NodeSemantics::Obligation(o) if copy.is_none() && matches!(&o.kind, ObligationKind::CopyRead(ty) if ty == carrier) => copy = Some(&o.site),
                _ => return Err(unsupported_node(file, node, "unsupported payload read record")),
            }
        }
        if !expression || expressions.types.id_for(carrier).ok() != Some(self.ty) {
            return Err(unsupported_node(file, node, "payload binding lacks its typed scalar use"));
        }
        let site = copy.ok_or_else(|| unsupported_node(file, node, "payload lacks its scalar copy-read obligation"))?;
        Ok(self.emit(body, site.clone()))
    }
}

pub(in crate::machines) enum Subject<'a> {
    Integer(ValueId, &'a ValueType),
    Borrowed {
        source: ProjectedPlace,
        owner: DataId,
        access: &'a Access,
    },
}

pub(in crate::machines) enum SubjectPlan<'a> {
    EvaluateInteger(&'a ValueType),
    Captured(Subject<'a>),
}

pub(in crate::machines) fn plan<'a>(
    expressions: &'a Expressions<'_>,
    subject: usize,
    body: &MachineBody,
) -> Result<SubjectPlan<'a>, LoweringError> {
    let tree = expressions.tree;
    let file = tree.resolved().file;
    let Some(ty) = tree.semantics_at(subject).find_map(|r| match r {
        NodeSemantics::Expression(e) => Some(&e.ty),
        _ => None,
    }) else {
        return Err(unsupported_node(file, subject, "value pattern needs a typed subject"));
    };
    if matches!(ty, ValueType::Integer { width: Some(1..), .. }) {
        return Ok(SubjectPlan::EvaluateInteger(ty));
    }
    if let Some((access @ (Access::Shared | Access::Mut), inner)) = ty.reference_parts()
        && let Some(TypeShape::Nominal(owner)) = expressions
            .types
            .id_for(inner)
            .ok()
            .and_then(|id| expressions.types.get(id))
    {
        let (mut source, _) = crate::machines::references::original_place(expressions, subject, body)?;
        source.path.push(PlaceProjection::Referent);
        return Ok(SubjectPlan::Captured(Subject::Borrowed { source, owner: *owner, access }));
    }
    Err(unsupported_node(
        file,
        subject,
        "literal dispatch needs a fixed integer or a borrowed case subject",
    ))
}

/// Resolve the typed projection against the selected case, preserving the full
/// source path, including any reference crossing above this sum.
pub(in crate::machines) fn payload_place<'a>(
    expressions: &'a Expressions<'_>,
    subject: usize,
    source: &ProjectedPlace,
    schema: &crate::LoweredCase,
    access: Option<&Access>,
    field: usize,
) -> Result<(&'a omega_resolved::DeclarationId, PayloadPlace), LoweringError> {
    let typed = expressions.tree.forest();
    let file = expressions.tree.resolved().file;
    let backing = typed.place_path(file.node_id(field));
    let step = backing
        .1
        .last()
        .filter(|s| s.base == file.node_id(file.ungroup(subject)) && s.through.as_ref() == access);
    let Some(Projection::Field(id)) = step.and_then(|s| s.projection.as_ref()) else {
        return Err(unsupported_node(file, field, "payload lacks its typed field projection"));
    };
    let field_id = crate::identity::declaration(id, &site_at(file, field))?;
    let member = schema
        .fields
        .iter()
        .find(|f| f.declaration == field_id)
        .ok_or_else(|| unsupported_node(file, field, "payload field does not belong to the selected case"))?;
    let mut place = source.clone();
    place.path.extend([
        PlaceProjection::Case(schema.declaration.clone()),
        PlaceProjection::Field(field_id),
    ]);
    Ok((id.as_ref(), PayloadPlace {
        source: place,
        ty: member.carrier,
    }))
}

struct PayloadTest<'a> {
    case: CaseSelection,
    field: PayloadPlace,
    field_site: Site,
    carrier: &'a ValueType,
    literal: usize,
    bindings: Vec<(usize, PayloadPlace)>,
}

/// Verify both the case-owned field identity and the read's precise L4 type
/// and obligation. Keep the remaining projected places for reads in the selected arm.
fn payload_test<'a>(
    expressions: &'a Expressions<'_>,
    subject: usize,
    source: &ProjectedPlace,
    owner: DataId,
    access: &Access,
    pattern: usize,
) -> Result<PayloadTest<'a>, LoweringError> {
    let tree = expressions.tree;
    let resolved = tree.resolved();
    let file = resolved.file;
    let [head, fields @ ..] = &file.nodes[pattern].children[..] else {
        return Err(unsupported_node(file, pattern, "expected a case pattern"));
    };
    check_container(tree, *head)?;
    let Some(Symbol::Declaration(id)) = resolved.symbol(*head) else {
        return Err(unsupported_node(file, *head, "expected a selected case"));
    };
    let case = crate::identity::declaration(id, &site_at(file, *head))?;
    let schema = expressions.data[owner.get() - 1]
        .cases
        .iter()
        .find(|c| c.declaration == case)
        .ok_or_else(|| unsupported_node(file, *head, "case does not belong to the subject"))?;
    let mut test = None;
    let mut bindings = Vec::new();
    for &field in fields {
        if file.nodes[field].kind != NodeKind::FieldPattern {
            return Err(unsupported_node(file, field, "expected an explicit payload field"));
        }
        let (id, payload) = payload_place(expressions, subject, source, schema, Some(access), field)?;
        let literal = match file.nodes[field].children[..] {
            [literal] if test.is_none() => literal,
            [] if resolved.pattern_binding(field).is_some()
                && matches!(
                    expressions.types.get(payload.ty),
                    Some(TypeShape::PrimitiveScalar(crate::ScalarType::Integer { .. }))
                ) =>
            {
                if tree
                    .semantics_at(field)
                    .any(|row| !matches!(row,NodeSemantics::Field(selected) if selected==id))
                {
                    return Err(unsupported_node(file, field, "unsupported payload binding record"));
                }
                bindings.push((field, payload));
                continue;
            }
            _ => return Err(unsupported_node(file, field, "payload requires one literal test and direct value bindings")),
        };
        let mut copied = None;
        for row in tree.semantics_at(field) {
            match row {
                NodeSemantics::Field(f) if f == id => {}
                NodeSemantics::Obligation(o)
                    if copied.is_none()
                        && let ObligationKind::CopyRead(ty @ ValueType::Integer { width: Some(1..), .. }) = &o.kind
                        && expressions.types.id_for(ty).ok() == Some(payload.ty) =>
                {
                    copied = Some((ty, o.site.clone()))
                }
                _ => return Err(unsupported_node(file, field, "unsupported payload read record")),
            }
        }
        let (carrier, site) = copied.ok_or_else(|| unsupported_node(file, field, "payload lacks its integer copy-read obligation"))?;
        test = Some((payload, site, carrier, literal));
    }
    let (field, field_site, carrier, literal) = test.ok_or_else(|| unsupported_node(file, pattern, "payload alternative needs its literal test"))?;
    Ok(PayloadTest {
        case: CaseSelection { source: source.clone(), case },
        field,
        carrier,
        literal,
        field_site,
        bindings,
    })
}

pub(in crate::machines) fn lower(
    expressions: &Expressions<'_>,
    dispatch: usize,
    captured: Subject<'_>,
    body: &mut MachineBody,
    flow: &mut BlockFlow,
) -> Result<Vec<(BlockId, usize)>, LoweringError> {
    let tree = expressions.tree;
    let file = tree.resolved().file;
    let (&subject, arms) = file.nodes[dispatch].children.split_first().expect("planned subject");
    let bool_ty = expressions
        .types
        .id_for(&ValueType::Bool)
        .map_err(|e| LoweringError::new(&site_at(file, subject), e.to_string()))?;
    let mut targets = Vec::with_capacity(arms.len());
    let mut patterns = Vec::with_capacity(arms.len());
    for &arm in arms {
        check_container(tree, arm)?;
        let [pattern, result] = file.nodes[arm].children[..] else {
            return Err(unsupported_node(file, arm, "expected pattern and result"));
        };
        let pattern = ungroup_pattern(file, pattern, |group| check_container(tree, group))?;
        check_container(tree, pattern)?;
        patterns.push(pattern);
        targets.push((flow.reserve(), result));
    }
    let Some((&fallback, tests)) = patterns.split_last() else {
        return Err(unsupported_node(file, subject, "empty literal dispatch"));
    };
    if !matches!((file.nodes[fallback].kind, &file.nodes[fallback].children[..]), (NodeKind::WildcardPattern, [])) || tests.is_empty() {
        return Err(unsupported_node(file, fallback, "literal alternatives require a final wildcard"));
    }
    for (i, &pattern) in tests.iter().enumerate() {
        let (target, _) = targets[i];
        let last = i + 1 == tests.len();
        let next = if last { targets[i + 1].0 } else { flow.reserve() };
        let failed_site = site_at(file, patterns[i + 1]);
        let (left, carrier, literal) = match (&captured, file.nodes[pattern].kind, &file.nodes[pattern].children[..]) {
            (Subject::Integer(value, carrier), NodeKind::ValuePattern, [literal]) => (*value, *carrier, *literal),
            (Subject::Borrowed { source, owner, access }, NodeKind::RecordPattern, _) => {
                let test = payload_test(expressions, subject, source, *owner, access, pattern)?;
                flow.payload_bindings.extend(test.bindings.into_iter().map(|(binder, place)| (binder, PayloadBinding::Borrowed(place))));
                let site = site_at(file, pattern);
                let condition = emit_scalar(
                    body,
                    bool_ty,
                    OperationKind::StructuralCaseMembership {
                        source: source.clone(),
                        case: test.case.case.clone(),
                    },
                    site.clone(),
                );
                let compare = flow.reserve();
                let mut when_true = successor(body, compare, vec![], site);
                when_true.selected_case = Some(test.case);
                let when_false = successor(body, next, vec![], failed_site.clone());
                flow.close(
                    body,
                    BlockTerminator::Conditional {
                        condition,
                        when_true,
                        when_false,
                    },
                );
                flow.open(body, compare, None);
                (test.field.emit(body, test.field_site), test.carrier, test.literal)
            }
            _ => return Err(unsupported_node(file, pattern, "unsupported alternative in literal dispatch")),
        };
        if file.nodes[literal].kind != NodeKind::Literal {
            return Err(unsupported_node(file, literal, "expected an integer literal pattern"));
        }
        // Only this validated leaf uses a fresh expression stack. A subject,
        // unlike the literal, may itself contain arbitrarily nested matches.
        let OperationResult::Scalar(right) = expressions.lower(literal, carrier, body, flow)? else {
            return Err(unsupported_node(file, literal, "expected a scalar literal"));
        };
        let condition = emit_scalar(
            body,
            bool_ty,
            OperationKind::Integer {
                operation: IntegerOperation {
                    primitive: IntegerPrimitive::Equal,
                    policy: ArithmeticPolicy::Exact,
                    obligations: vec![],
                },
                operands: vec![left, right],
            },
            site_at(file, literal),
        );
        let when_true = successor(body, target, vec![], site_at(file, pattern));
        let when_false = successor(body, next, vec![], failed_site);
        flow.close(
            body,
            BlockTerminator::Conditional {
                condition,
                when_true,
                when_false,
            },
        );
        if !last {
            flow.open(body, next, None);
        }
    }
    Ok(targets)
}

fn emit_scalar(
    body: &mut MachineBody,
    ty: TypeId,
    kind: OperationKind,
    site: Site,
) -> ValueId {
    let OperationResult::Scalar(value) = body.push_operation(OperationOutput::Scalar(ty), kind, site) else {
        unreachable!()
    };
    value
}
