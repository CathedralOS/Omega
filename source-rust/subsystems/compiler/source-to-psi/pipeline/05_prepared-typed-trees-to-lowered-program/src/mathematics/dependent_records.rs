//! A field that stores a type binds that exact type in later fields. A nested
//! Sigma retains those dependencies when the record is passed to a definition.
use crate::Unserved;
use crate::read::{Program, refuse_at};
use crate::source_state::{
    TypedTrees,
    data::DataDefinition,
    expression::{ExpressionHandle, ExpressionNode, MemberSelection},
    mathematical::{MathematicalBody, MathematicalDefinition, MathematicalType},
    types::{TypeReferenceHandle, TypeReferenceNode},
};
use mathematical_kernel::{
    Budget, Context, Declaration, Level, Sort, Term, TermHandle, infer_sort,
};
use semantic_vocabulary::{MachineId, PackageKeyIdentity};
use source_provenance::DependencyScope;
use symbols::{FixedCoreDeclaration, SymbolHandle};
use terminal_check_tables::{
    Checked,
    mathematics::{MathematicalSignature, SignatureEntry, SignatureEntryKind},
};

fn named(typed: &TypedTrees, reference: TypeReferenceHandle) -> Option<SymbolHandle> {
    if !typed
        .type_reference_table
        .contains_type_reference(reference)
    {
        return None;
    }
    match typed.type_reference_table.type_reference(reference) {
        TypeReferenceNode::Named { symbol, .. } if symbol.is_valid() => Some(*symbol),
        _ => None,
    }
}

pub(crate) fn has_field_binder(typed: &TypedTrees, data: &DataDefinition) -> bool {
    typed
        .data_members(data)
        .iter()
        .filter_map(|member| member.as_field())
        .any(|field| {
            named(typed, field.type_reference)
                .is_some_and(|symbol| typed.symbols.get(symbol).kind == symbols::SymbolKind::Field)
        })
}

pub(crate) fn projection_domain<'a>(
    typed: &'a TypedTrees,
    definition: &MathematicalDefinition,
) -> Option<&'a DataDefinition> {
    let [parameter] = typed
        .mathematical_parameters(definition.parameters)
        .as_array()?;
    let MathematicalType::Ordinary(reference) = typed.mathematical_type(parameter.ty) else {
        return None;
    };
    let data = typed.unique_data_definition_by_symbol(named(typed, *reference)?)?;
    has_field_binder(typed, data).then_some(data)
}

fn universe(typed: &TypedTrees, reference: TypeReferenceHandle) -> Option<Sort> {
    let table = &typed.type_reference_table;
    if !table.contains_type_reference(reference) {
        return None;
    }
    let TypeReferenceNode::Generic {
        base_symbol,
        arguments,
        lifetime_arguments,
        ..
    } = table.type_reference(reference)
    else {
        return None;
    };
    if typed
        .symbols
        .fixed_core_declaration_for_symbol(*base_symbol)
        != Some(FixedCoreDeclaration::Type)
        || !lifetime_arguments.is_empty()
    {
        return None;
    }
    let [arg] = table.type_reference_handles(*arguments) else {
        return None;
    };
    let TypeReferenceNode::ConstExpression(expr) = table.type_reference(*arg) else {
        return None;
    };
    let ExpressionNode::Integer(n) = typed.expression_table.expression(*expr) else {
        return None;
    };
    Some(Sort::Type(Level::Constant(n.value_u64()?.try_into().ok()?)))
}

fn append_record(
    typed: &TypedTrees,
    id: MachineId,
    data: &DataDefinition,
    signature: &mut MathematicalSignature,
) -> Checked<u32> {
    let unsupported = |reason| refuse_at(id, Unserved::Type(reason));
    if let Some(index) = signature.entries.iter().position(|e| matches!(e.kind, SignatureEntryKind::DependentRecord {declaration,..} if declaration == data.symbol)) {
        return u32::try_from(index).map_err(|_| unsupported("too many mathematical declarations"));
    }
    if !data.requires_facts.is_empty()
        || !data.conformance_bounds.is_empty()
        || !data.type_parameters.is_empty()
        || !data.package_parameters.is_empty()
        || !data.lifetime_parameters.is_empty()
        || data.generic_instance.is_some()
        || data.quotient.is_some()
        || data.cleanup_selection.is_some()
        || !data.retired_identities.is_empty()
        || data.zero_gated
        || data.properties != Default::default()
        || data.supply_mode != language_core::DataSupplyMode::CheckedShape
    {
        return Err(unsupported(
            "qualified or parameterized dependent mathematical record",
        ));
    }
    let mut fields = Vec::new();
    let members = typed.data_members(data);
    for (index, member) in members.iter().enumerate() {
        let field = member
            .as_field()
            .ok_or_else(|| unsupported("dependent record with a variant"))?;
        if field.relevance.is_erased() {
            return Err(unsupported("erased dependent record field"));
        }
        let ty = if let Some(sort) = universe(typed, field.type_reference) {
            Term::Sort(sort)
        } else {
            let symbol = named(typed, field.type_reference).ok_or_else(|| {
                unsupported("dependent field needs a formed universe or an earlier type field")
            })?;
            let before = members
                .slice(0..index)
                .iter()
                .rev()
                .position(|m| m.as_field().is_some_and(|f| f.symbol == symbol))
                .ok_or_else(|| unsupported("dependent field type outside the record telescope"))?;
            Term::Variable(
                u32::try_from(before)
                    .map_err(|_| unsupported("too many dependent record fields"))?,
            )
        };
        fields.push(signature.arena.insert(ty));
    }
    let mut fields = fields.into_iter().rev();
    let last = fields
        .next()
        .ok_or_else(|| unsupported("empty dependent record"))?;
    let record = fields.fold(last, |codomain, domain| {
        signature.arena.insert(Term::Sigma { domain, codomain })
    });
    let sort = infer_sort(
        &mut signature.arena,
        &Context::empty(),
        record,
        &mut Budget::default(),
    )
    .map_err(|_| unsupported("dependent record has an ill-formed field type"))?;
    let level = signature.arena.insert(Term::Sort(sort));
    let type_identity = typed
        .normalized_nominal_type_identity(data.symbol)
        .and_then(|identity| typed.type_identity_text(identity))
        .ok_or_else(|| unsupported("dependent record without a source type identity"))?;
    let position = u32::try_from(signature.declarations.len())
        .map_err(|_| unsupported("too many mathematical declarations"))?;
    signature
        .declarations
        .push(Declaration::definition(0, level, record));
    signature.entries.push(SignatureEntry {
        name: data.name.as_str().into(),
        kind: SignatureEntryKind::DependentRecord {
            declaration: data.symbol,
            type_identity,
        },
    });
    Ok(position)
}

fn is_parameter(typed: &TypedTrees, expr: ExpressionHandle, symbol: SymbolHandle) -> bool {
    matches!(typed.expression_table.expression(expr), ExpressionNode::Name(name) if name.members.len() == 1 && name.symbol == symbol)
}

fn constant(signature: &mut MathematicalSignature, declaration: u32) -> TermHandle {
    signature.arena.insert(Term::Constant {
        declaration,
        levels: Vec::new(),
    })
}

fn projection_position(signature: &MathematicalSignature, symbol: SymbolHandle) -> Option<usize> {
    signature.entries.iter().position(|entry| {
        matches!(entry.kind, SignatureEntryKind::FieldProjection { declaration } if declaration == symbol)
    })
}

fn append_projection(
    typed: &TypedTrees,
    id: MachineId,
    definition: &MathematicalDefinition,
    data: &DataDefinition,
    signature: &mut MathematicalSignature,
) -> Checked<()> {
    let unsupported = |reason| refuse_at(id, Unserved::Type(reason));
    let domain = append_record(typed, id, data, signature)?;
    let Some([parameter_decl]) = typed
        .mathematical_parameters(definition.parameters)
        .as_array()
    else {
        return Err(unsupported(
            "dependent projection without a single record parameter",
        ));
    };
    let MathematicalBody::Definition(expr) = definition.body else {
        return Err(unsupported("dependent projection without a body"));
    };
    if !definition.binders.is_empty() || parameter_decl.relevance.is_erased() {
        return Err(unsupported(
            "dependent projection with extra binders or erasure",
        ));
    }
    let ExpressionNode::Member(member) = typed.expression_table.expression(expr) else {
        return Err(unsupported(
            "expected a field of the dependent record parameter",
        ));
    };
    if member.case_variant.is_some() || !is_parameter(typed, member.receiver, parameter_decl.symbol)
    {
        return Err(unsupported(
            "field is not projected from the dependent record parameter",
        ));
    }
    let fields = typed.data_members(data);
    let position = fields
        .iter()
        .position(|f| {
            f.as_field().is_some_and(|field| {
                field.name == member.member
                    && match member.selection {
                        MemberSelection::Unresolved => true,
                        MemberSelection::Field(symbol) => field.symbol == symbol,
                        MemberSelection::CollectionMeasure(_) => false,
                    }
            })
        })
        .ok_or_else(|| unsupported("unknown field in dependent record projection"))?;
    let parameter = signature.arena.insert(Term::Variable(0));
    let expected = match typed.mathematical_type(definition.result) {
        MathematicalType::Ordinary(reference) => {
            let sort = universe(typed, *reference).ok_or_else(|| {
                unsupported("dependent projection result needs a formed universe")
            })?;
            signature.arena.insert(Term::Sort(sort))
        }
        MathematicalType::Application { callee, arguments } => {
            let MathematicalType::Ordinary(reference) = typed.mathematical_type(*callee) else {
                return Err(unsupported(
                    "dependent projection result needs a named function",
                ));
            };
            let callee = named(typed, *reference)
                .ok_or_else(|| unsupported("unresolved result type function"))?;
            let [arg] = typed.expression_table.expression_handles(*arguments) else {
                return Err(unsupported(
                    "result type function needs the record parameter",
                ));
            };
            if !is_parameter(typed, *arg, parameter_decl.symbol) {
                return Err(unsupported("result type function applied to another value"));
            }
            let at = projection_position(signature, callee)
                .ok_or_else(|| unsupported("unformed result type function"))?;
            let function = constant(
                signature,
                u32::try_from(at).map_err(|_| unsupported("too many mathematical declarations"))?,
            );
            signature.arena.insert(Term::Apply {
                function,
                argument: parameter,
            })
        }
        _ => {
            return Err(unsupported(
                "dependent record projection with unsupported result",
            ));
        }
    };
    let record = constant(signature, domain);
    let ty = signature.arena.insert(Term::Pi {
        domain: record,
        codomain: expected,
    });
    let field = signature
        .arena
        .tuple_component(parameter, position, fields.len());
    let value = signature.arena.insert(Term::Lambda {
        domain: record,
        body: field,
    });
    signature
        .declarations
        .push(Declaration::definition(0, ty, value));
    signature.entries.push(SignatureEntry {
        name: definition.name.as_str().into(),
        kind: SignatureEntryKind::FieldProjection {
            declaration: definition.symbol,
        },
    });
    Ok(())
}

pub(super) fn append(
    program: &Program<'_>,
    id: MachineId,
    subjects: &[Option<PackageKeyIdentity>],
    purpose: DependencyScope,
    signature: &mut MathematicalSignature,
) -> Checked<()> {
    let typed = program.typed;
    for data in typed.data_definitions() {
        if has_field_binder(typed, data)
            && program.owned_by_subject(id, data.symbol, subjects, purpose)?
        {
            append_record(typed, id, data, signature)?;
        }
    }
    // A selector's written result may apply another selector. Bring in that
    // declaration first, including when the source imported it or wrote it later.
    for root in typed.mathematical_definitions() {
        let Some(data) = projection_domain(typed, root) else {
            continue;
        };
        if !program.owned_by_subject(id, root.symbol, subjects, purpose)? {
            continue;
        }
        let mut next = root;
        let mut pending = Vec::new();
        while projection_position(signature, next.symbol).is_none() {
            if pending
                .iter()
                .any(|prior: &&MathematicalDefinition| prior.symbol == next.symbol)
            {
                return Err(refuse_at(
                    id,
                    Unserved::Type("cyclic dependent record result type function"),
                ));
            }
            pending.push(next);
            let MathematicalType::Application { callee, .. } = typed.mathematical_type(next.result)
            else {
                break;
            };
            let target = match typed.mathematical_type(*callee) {
                MathematicalType::Ordinary(reference) => named(typed, *reference),
                _ => None,
            }
            .and_then(|symbol| {
                typed
                    .mathematical_definitions()
                    .iter()
                    .find(|candidate| candidate.symbol == symbol)
            });
            next = target
                .filter(|definition| {
                    projection_domain(typed, definition)
                        .is_some_and(|owner| owner.symbol == data.symbol)
                })
                .ok_or_else(|| {
                    refuse_at(
                        id,
                        Unserved::Type("result type function needs the same declared record"),
                    )
                })?;
        }
        for definition in pending.into_iter().rev() {
            append_projection(typed, id, definition, data, signature)?;
        }
    }
    Ok(())
}
