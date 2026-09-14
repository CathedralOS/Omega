//! Resolve checked boundary calls to exact selected adapters without rewriting
//! their source meaning. Exact and receiver-forwarding adapters share the same
//! association; execution consumes it, while Terminal retains the requirement.

use checked_trees::CheckedTrees;
use diagnostics::Diagnostic;
use std::sync::Arc;
use typed_trees::TypedTrees;

#[derive(Debug, Clone, PartialEq, Eq)]
struct AdapterRow {
    receiver_trait: symbols::SymbolHandle,
    provider_plan_digest: [u8; 32],
    receiver_trait_name: String,
    requirement: String,
    requirement_identity: String,
    requirement_symbol: symbols::SymbolHandle,
    adapter_target: String,
    symbol: symbols::SymbolHandle,
    /// Self-forwarding shape: prepend the call's receiver as argument 0.
    forward_receiver: bool,
    /// Finite-family tuple this row realizes: canonical const identities in
    /// the requirement's value-binder declaration order (the same strings a
    /// `MachineSpecialization` retains in `const_argument_identities`). Empty
    /// on an exact nongeneric row, which matches every call to its
    /// requirement without consulting static arguments.
    family_tuple: Box<[String]>,
    /// Readable tuple spellings for diagnostics only; `family_tuple` is the
    /// semantic key.
    family_tuple_display: Box<[String]>,
}

/// A selected requirement that declares local generic binders but cannot
/// produce dispatch rows: either it declares no complete finite `where`
/// family, or no tuple's checked provider specialization exists.
/// Ineligibility is individual: the requirement supplies no dispatch row,
/// sibling requirements still settle, and a call targeting it rejects below
/// instead of silently dispatching every tuple to the unbound generic
/// template.
#[derive(Debug, Clone, PartialEq, Eq)]
struct GenericBoundaryRequirement {
    receiver_trait: symbols::SymbolHandle,
    provider_plan_digest: [u8; 32],
    requirement_symbol: symbols::SymbolHandle,
    requirement_identity: String,
    /// Why the family supplied no rows, retained for the call diagnostic.
    reason: String,
}

#[derive(Debug, PartialEq, Eq)]
enum ResolvedAdapterRow {
    /// One exact checked realization of the requirement: nongeneric, or one
    /// tuple of a finite generic family.
    Adapter(AdapterRow),
    /// The requirement is dynamically ineligible for want of tuple rows.
    GenericRequirement(GenericBoundaryRequirement),
}

#[cfg(test)]
impl ResolvedAdapterRow {
    fn expect_adapter(self) -> AdapterRow {
        match self {
            Self::Adapter(row) => row,
            Self::GenericRequirement(requirement) => panic!(
                "requirement `{}` is dynamically ineligible, not an exact adapter",
                requirement.requirement_identity
            ),
        }
    }
}

/// One canonical value in a finite-family tuple. `identity` is the string a
/// `MachineSpecialization` retains in `const_argument_identities` for the
/// same static argument; `display` is diagnostic-only spelling.
#[derive(Debug, Clone, PartialEq, Eq)]
struct FamilyValue {
    identity: String,
    display: String,
}

/// One complete tuple of a finite family: every const/value binder bound to
/// a closed canonical value, in the signature's binder declaration order.
#[derive(Debug, Clone, PartialEq, Eq)]
struct FamilyTuple {
    identities: Box<[String]>,
    display: Box<[String]>,
}

/// The outcome of reading a generic requirement's signature `where` clause.
enum FamilyProbe {
    /// Not an explicit finite enumeration: no `where` clause, or facts that
    /// are not one disjunction of complete binder equalities. Opaque
    /// predicates, inequality ranges, and independently listed values never
    /// enumerate; the requirement stays dynamically ineligible for the
    /// recorded reason.
    NotFinite(String),
    /// A normalized duplicate-free roster of complete tuples.
    Finite {
        arity: usize,
        tuples: Vec<FamilyTuple>,
    },
}

/// Extract the explicit finite family declared by a generic requirement's
/// signature `where` clause: one disjunction of complete binder equalities
/// such as `where Width == 16 || Width == 32`. Multi-binder alternatives must
/// write each tuple's full correlation (`W == 16 && L == 4 || W == 32 && L
/// == 8`); the compiler never invents a Cartesian product from independently
/// listed values.
fn finite_signature_family(
    typed: &TypedTrees,
    signature: &typed_trees::signature::StateSignature,
) -> FamilyProbe {
    let type_parameters = typed.state_signature_type_parameters(signature);
    let binders = type_parameters
        .iter()
        .filter(|parameter| {
            matches!(
                parameter.kind,
                typed_trees::data::TypeParameterKind::Const { .. }
                    | typed_trees::data::TypeParameterKind::Value { .. }
            )
        })
        .collect::<Vec<_>>();
    if binders.len() != type_parameters.len() {
        return FamilyProbe::NotFinite(
            "the requirement carries non-value generic binders, which value equality cannot enumerate"
                .to_owned(),
        );
    }
    let facts = typed.proof_facts.span_or_empty(signature.where_facts);
    let [fact] = facts else {
        return if facts.is_empty() {
            FamilyProbe::NotFinite("declares no finite-family `where` clause".to_owned())
        } else {
            FamilyProbe::NotFinite(
                "the finite family must be one explicit disjunction of complete binder equalities"
                    .to_owned(),
            )
        };
    };
    let typed_trees::domain::ProofFact::Expression(root) = fact else {
        return FamilyProbe::NotFinite(
            "the `where` clause is a domain membership, not an explicit finite enumeration"
                .to_owned(),
        );
    };

    let mut tuples = Vec::new();
    for alternative in or_alternatives(typed, *root) {
        let mut assignments: Vec<Option<FamilyValue>> = vec![None; binders.len()];
        for conjunct in and_conjuncts(typed, alternative) {
            let Some((binder_index, value)) = equality_assignment(typed, &binders, conjunct) else {
                return FamilyProbe::NotFinite(
                    "the `where` clause is not explicit `Binder == literal` equalities; opaque \
                     predicates and inequalities do not enumerate a finite family"
                        .to_owned(),
                );
            };
            if assignments[binder_index].is_some() {
                return FamilyProbe::NotFinite(
                    "an alternative binds the same value binder more than once".to_owned(),
                );
            }
            assignments[binder_index] = Some(value);
        }
        if assignments.iter().any(Option::is_none) {
            return FamilyProbe::NotFinite(
                "an alternative does not bind every value binder; a finite family requires \
                 complete tuples so authored correlations are preserved"
                    .to_owned(),
            );
        }
        let tuple = assignments
            .into_iter()
            .map(|value| value.expect("complete alternative tuple"))
            .collect::<Vec<_>>();
        tuples.push(FamilyTuple {
            identities: tuple.iter().map(|value| value.identity.clone()).collect(),
            display: tuple.iter().map(|value| value.display.clone()).collect(),
        });
    }
    tuples.sort_by(|left, right| left.identities.cmp(&right.identities));
    tuples.dedup_by(|left, right| left.identities == right.identities);
    FamilyProbe::Finite {
        arity: binders.len(),
        tuples,
    }
}

/// Flatten an authored `||` chain into its alternatives.
fn or_alternatives(
    typed: &TypedTrees,
    expression: typed_trees::expression::ExpressionHandle,
) -> Vec<typed_trees::expression::ExpressionHandle> {
    let mut alternatives = Vec::new();
    let mut stack = vec![expression];
    while let Some(current) = stack.pop() {
        match typed.expression_table.expression(current) {
            typed_trees::expression::ExpressionNode::Binary(binary)
                if binary.operator == typed_trees::expression::BinaryOperator::Or =>
            {
                stack.push(binary.right);
                stack.push(binary.left);
            }
            _ => alternatives.push(current),
        }
    }
    alternatives
}

/// Flatten an authored `&&` chain into its conjuncts.
fn and_conjuncts(
    typed: &TypedTrees,
    expression: typed_trees::expression::ExpressionHandle,
) -> Vec<typed_trees::expression::ExpressionHandle> {
    let mut conjuncts = Vec::new();
    let mut stack = vec![expression];
    while let Some(current) = stack.pop() {
        match typed.expression_table.expression(current) {
            typed_trees::expression::ExpressionNode::Binary(binary)
                if binary.operator == typed_trees::expression::BinaryOperator::And =>
            {
                stack.push(binary.right);
                stack.push(binary.left);
            }
            _ => conjuncts.push(current),
        }
    }
    conjuncts
}

/// Read one `Binder == literal` conjunct against the signature's declared
/// value binders, in either operand order. Returns the binders' declaration
/// index and the canonical literal value.
fn equality_assignment(
    typed: &TypedTrees,
    binders: &[&typed_trees::data::TypeParameter],
    expression: typed_trees::expression::ExpressionHandle,
) -> Option<(usize, FamilyValue)> {
    let typed_trees::expression::ExpressionNode::Binary(binary) =
        typed.expression_table.expression(expression)
    else {
        return None;
    };
    if binary.operator != typed_trees::expression::BinaryOperator::Equal {
        return None;
    }
    let (binder, value) = match (
        binder_operand(typed, binders, binary.left),
        binder_operand(typed, binders, binary.right),
        literal_operand(typed, binary.left),
        literal_operand(typed, binary.right),
    ) {
        (Some(binder), None, None, Some(value)) | (None, Some(binder), Some(value), None) => {
            (binder, value)
        }
        _ => return None,
    };
    let carrier = match &binders[binder].kind {
        typed_trees::data::TypeParameterKind::Const { type_reference }
        | typed_trees::data::TypeParameterKind::Value { type_reference } => *type_reference,
        _ => return None,
    };
    value_fits_carrier(typed, carrier, &value.identity).then_some((binder, value))
}

/// Whether the operand names one of the signature's declared value binders.
/// Signature `where` expressions are not symbol-bound at this stage; the
/// binder's own declaration name is the scoped reference.
fn binder_operand(
    typed: &TypedTrees,
    binders: &[&typed_trees::data::TypeParameter],
    expression: typed_trees::expression::ExpressionHandle,
) -> Option<usize> {
    let typed_trees::expression::ExpressionNode::Name(path) =
        typed.expression_table.expression(expression)
    else {
        return None;
    };
    let members = typed.expression_table.name_path_members(path.members);
    let [member] = members else { return None };
    binders.iter().position(|parameter| {
        parameter.name.as_str() == member.as_str()
            || (path.symbol.is_valid() && parameter.symbol == path.symbol)
    })
}

/// Read a closed literal operand: integer, Boolean, or a named const
/// declaration whose canonical encoding is a scalar.
fn literal_operand(
    typed: &TypedTrees,
    expression: typed_trees::expression::ExpressionHandle,
) -> Option<FamilyValue> {
    match typed.expression_table.expression(expression) {
        typed_trees::expression::ExpressionNode::Integer(literal) => {
            let value = literal
                .value_i64()
                .map(i128::from)
                .or_else(|| literal.value_u64().map(i128::from))?;
            Some(FamilyValue {
                identity: integer_const_identity(value),
                display: value.to_string(),
            })
        }
        typed_trees::expression::ExpressionNode::Boolean(value) => Some(FamilyValue {
            identity: canonical_const_identity(
                "bool",
                &language_semantics::const_value::CanonicalConstValue::boolean(*value).encoding,
            ),
            display: value.to_string(),
        }),
        typed_trees::expression::ExpressionNode::Name(path) => {
            let members = typed.expression_table.name_path_members(path.members);
            let [member] = members else { return None };
            let declaration = typed.const_declarations().iter().find(|declaration| {
                typed.symbols.name(declaration.symbol) == member.as_str()
                    || (path.symbol.is_valid() && declaration.symbol == path.symbol)
            })?;
            let value = language_semantics::const_value::CanonicalConstValue::new(
                typed.display_type_reference(declaration.declared_type),
                declaration.canonical_value_encoding.as_ref()?.clone(),
                member.as_str(),
            );
            match value.decode_encoding()? {
                language_semantics::const_value::DecodedCanonicalConstValue::Integer {
                    value,
                    ..
                } => Some(FamilyValue {
                    identity: integer_const_identity(value),
                    display: value.to_string(),
                }),
                language_semantics::const_value::DecodedCanonicalConstValue::Boolean(value) => {
                    Some(FamilyValue {
                        identity: canonical_const_identity(
                            "bool",
                            &language_semantics::const_value::CanonicalConstValue::boolean(value)
                                .encoding,
                        ),
                        display: value.to_string(),
                    })
                }
                _ => None,
            }
        }
        _ => None,
    }
}

/// The canonical const identity a specialization retains for one closed
/// integer argument: `named(integer-const(v))` in normalized-type-identity
/// terms.
fn integer_const_identity(value: i128) -> String {
    format!("named(integer-const({value}))")
}

/// The canonical const identity for one non-integer canonical value:
/// `named(canonical-const(type(T),encoding(E)))`.
fn canonical_const_identity(type_name: &str, encoding: &str) -> String {
    fn escape(value: &str) -> String {
        let mut out = String::with_capacity(value.len());
        for character in value.chars() {
            if matches!(character, '\\' | '(' | ')' | ',') {
                out.push('\\');
            }
            out.push(character);
        }
        out
    }
    format!(
        "named(canonical-const(type({}),encoding({})))",
        escape(type_name),
        escape(encoding)
    )
}

/// Carrier legality for one roster literal: integer literals need an integer
/// carrier whose range contains them; Boolean literals need `bool`.
fn value_fits_carrier(
    typed: &TypedTrees,
    carrier: typed_trees::types::TypeReferenceHandle,
    identity: &str,
) -> bool {
    let Some(primitive) = typed.primitive_type_reference(carrier) else {
        return false;
    };
    if let Some(value) = identity
        .strip_prefix("named(integer-const(")
        .and_then(|inner| inner.strip_suffix("))"))
        .and_then(|inner| inner.parse::<i128>().ok())
    {
        return primitive_integer_range(primitive)
            .is_some_and(|(low, high)| low <= value && value <= high);
    }
    primitive == typed_trees::types::PrimitiveType::Bool
}

fn primitive_integer_range(primitive: typed_trees::types::PrimitiveType) -> Option<(i128, i128)> {
    use typed_trees::types::PrimitiveType::*;
    match primitive {
        Bool | F32 | F64 => None,
        I8 => Some((i128::from(i8::MIN), i128::from(i8::MAX))),
        I16 => Some((i128::from(i16::MIN), i128::from(i16::MAX))),
        I32 => Some((i128::from(i32::MIN), i128::from(i32::MAX))),
        I64 => Some((i128::from(i64::MIN), i128::from(i64::MAX))),
        U8 => Some((0, i128::from(u8::MAX))),
        U16 => Some((0, i128::from(u16::MAX))),
        U32 => Some((0, i128::from(u32::MAX))),
        U64 | Addr => Some((0, i128::from(u64::MAX))),
    }
}

/// The canonical const identity one call-site static machine argument
/// contributes to a family tuple, or `None` when the argument is not a
/// closed static value. This is `TypedTrees::static_const_argument_identity`,
/// which mirrors the specialization pipeline's spelling and identity exactly.
fn call_argument_const_identity(
    typed: &TypedTrees,
    argument: &typed_trees::expression::StaticMachineArgument,
) -> Option<String> {
    typed.static_const_argument_identity(argument)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct BoundaryField {
    symbol: symbols::SymbolHandle,
    trait_symbol: symbols::SymbolHandle,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct BoundaryFieldDeclaration {
    owner: symbols::SymbolHandle,
    owner_name: String,
    field_name: String,
    field: BoundaryField,
}

/// Bind selected execution without changing typed source or source-derived plans.
/// All fallible work completes before publishing the association set.
pub fn settle_selected_boundary_adapter_dispatch(
    checked: &mut Arc<CheckedTrees>,
    selected_plans: &effects::SelectedProviderPlanFacts,
) -> Result<(), Vec<Diagnostic>> {
    let dispatch = plan_selected_boundary_adapter_dispatch(checked, selected_plans)?;
    if checked.facts.boundary_adapter_dispatch != dispatch {
        Arc::make_mut(checked).facts.boundary_adapter_dispatch = dispatch;
    }
    Ok(())
}

fn plan_selected_boundary_adapter_dispatch(
    checked: &CheckedTrees,
    selected_plans: &effects::SelectedProviderPlanFacts,
) -> Result<Vec<checked_trees::CheckedBoundaryAdapterDispatch>, Vec<Diagnostic>> {
    let typed = &checked.typed;
    let mut adapters = Vec::new();
    let mut generic_requirements: Vec<GenericBoundaryRequirement> = Vec::new();
    let mut diagnostics = Vec::new();
    for plan in selected_plans.plans() {
        for row in &plan.rows {
            match resolve_selected_adapter_row(typed, plan, row) {
                Ok(rows) => {
                    for resolved in rows {
                        match resolved {
                            ResolvedAdapterRow::Adapter(adapter) => {
                                if let Some(existing) =
                                    adapters.iter().find(|existing: &&AdapterRow| {
                                        existing.receiver_trait == adapter.receiver_trait
                                            && existing.requirement_symbol
                                                == adapter.requirement_symbol
                                            && existing.family_tuple == adapter.family_tuple
                                    })
                                {
                                    diagnostics.push(Diagnostic::error(format!(
                                        "selected boundary requirement `{}` tuple `({})` has two checked adapters (`{}` and `{}`)",
                                        adapter.requirement_identity,
                                        adapter.family_tuple_display.join(", "),
                                        existing.adapter_target,
                                        adapter.adapter_target,
                                    )));
                                } else {
                                    adapters.push(adapter);
                                }
                            }
                            ResolvedAdapterRow::GenericRequirement(requirement) => {
                                if !generic_requirements.iter().any(|existing| {
                                    existing.receiver_trait == requirement.receiver_trait
                                        && existing.requirement_symbol
                                            == requirement.requirement_symbol
                                        && existing.provider_plan_digest
                                            == requirement.provider_plan_digest
                                }) {
                                    generic_requirements.push(requirement);
                                }
                            }
                        }
                    }
                }
                Err(diagnostic) => diagnostics.push(diagnostic),
            }
        }
    }
    if adapters.is_empty() && generic_requirements.is_empty() {
        return diagnostics.is_empty().then(Vec::new).ok_or(diagnostics);
    }

    // Exact typed field symbol -> exact boundary-trait symbol. Field spellings
    // may repeat across data owners and never participate in dispatch.
    let mut boundary_field_declarations = Vec::new();
    for data in typed.data_definitions() {
        for member in typed.data_members(data) {
            let typed_trees::data::DataMember::Field(field) = member else {
                continue;
            };
            let symbol = if let Some(requirement) =
                typed_trees::service::exact_bound_service_requirement(typed, field.type_reference)
            {
                let Some(authorization) = typed.fused_service_erasure(requirement) else {
                    continue;
                };
                if !adapters.iter().any(|adapter| {
                    adapter.receiver_trait == requirement
                        && adapter.provider_plan_digest == authorization.provider_plan_digest
                }) && !generic_requirements.iter().any(|selected| {
                    selected.receiver_trait == requirement
                        && selected.provider_plan_digest == authorization.provider_plan_digest
                }) {
                    diagnostics.push(Diagnostic::error(format!(
                        "routed service field `{}::{}` has no exact Fused selected-provider-plan join",
                        data.name, field.name,
                    )));
                    continue;
                }
                requirement
            } else {
                let typed_trees::types::TypeReferenceNode::Named { symbol, .. } = typed
                    .type_reference_table
                    .type_reference(field.type_reference)
                else {
                    continue;
                };
                *symbol
            };
            if !adapters
                .iter()
                .any(|adapter| adapter.receiver_trait == symbol)
                && !generic_requirements
                    .iter()
                    .any(|selected| selected.receiver_trait == symbol)
            {
                continue;
            }
            if !field.symbol.is_valid() {
                diagnostics.push(Diagnostic::error(format!(
                    "boundary field `{}::{}` has no exact typed symbol for adapter dispatch",
                    data.name, field.name,
                )));
                continue;
            }
            if let Some(existing) = boundary_field_declarations
                .iter()
                .find(|existing: &&BoundaryFieldDeclaration| existing.field.symbol == field.symbol)
            {
                diagnostics.push(Diagnostic::error(format!(
                    "boundary field symbol {:?} maps to both trait symbols {:?} and {:?}",
                    field.symbol, existing.field.trait_symbol, symbol,
                )));
                continue;
            }
            boundary_field_declarations.push(BoundaryFieldDeclaration {
                owner: data.symbol,
                owner_name: data.name.as_str().to_owned(),
                field_name: field.name.as_str().to_owned(),
                field: BoundaryField {
                    symbol: field.symbol,
                    trait_symbol: symbol,
                },
            });
        }
    }

    // A direct `self.<field>` occurrence is stamped with the exact inherited
    // FIELD child of the attached machine, not the DATA declaration's field
    // symbol. Retain both coordinates: the declaration symbol serves nested
    // receiver leaves, while every exact machine-owned inherited symbol serves
    // direct statement and value receivers. Spelling is used only inside the
    // already-exact machine owner to resolve its unique child.
    let mut boundary_fields = boundary_field_declarations
        .iter()
        .map(|declaration| declaration.field)
        .collect::<Vec<_>>();
    for machine in typed.machines() {
        let Some(attached_data) = machine.attached_data.as_ref() else {
            continue;
        };
        let owners = typed
            .data_definitions()
            .iter()
            .filter(|data| data.name == *attached_data)
            .collect::<Vec<_>>();
        let [owner] = owners.as_slice() else {
            if boundary_field_declarations
                .iter()
                .any(|declaration| declaration.owner_name == attached_data.as_str())
            {
                diagnostics.push(Diagnostic::error(format!(
                    "adapter-dispatch machine `{}` resolves attached data `{attached_data}` to {} exact definitions",
                    machine.name,
                    owners.len(),
                )));
            }
            continue;
        };
        for declaration in boundary_field_declarations
            .iter()
            .filter(|declaration| declaration.owner == owner.symbol)
        {
            let symbols = machine
                .symbol
                .is_valid()
                .then(|| typed.symbols.child_handles(machine.symbol))
                .flatten()
                .into_iter()
                .flatten()
                .filter(|symbol| {
                    typed.symbols.get(*symbol).kind == symbols::SymbolKind::Field
                        && typed.symbols.name(*symbol) == declaration.field_name
                })
                .collect::<Vec<_>>();
            let [symbol] = symbols.as_slice() else {
                diagnostics.push(Diagnostic::error(format!(
                    "adapter-dispatch machine `{}` resolves inherited boundary field `{}::{}` to {} exact FIELD children",
                    machine.name,
                    declaration.owner_name,
                    declaration.field_name,
                    symbols.len(),
                )));
                continue;
            };
            boundary_fields.push(BoundaryField {
                symbol: *symbol,
                trait_symbol: declaration.field.trait_symbol,
            });
        }
    }

    // A borrowed nominal boundary-trait parameter uses the same selected
    // requirement as a boundary field. Its state-local parameter symbol, not
    // the repeated parameter spelling, identifies the receiver. This is not
    // routed Service erasure: generic and qualified carriers still require
    // the checked receipt below and must not enter through this narrow shape.
    for machine in typed.machines() {
        for state in typed.machine_states(machine) {
            for parameter in typed.state_parameters(state) {
                let typed_trees::types::TypeReferenceNode::Reference { referee, .. } = typed
                    .type_reference_table
                    .type_reference(parameter.type_reference)
                else {
                    continue;
                };
                let Some(trait_symbol) = named_type_symbol(typed, *referee) else {
                    continue;
                };
                if parameter.is_self
                    || parameter.is_const
                    || !typed.traits().iter().any(|definition| {
                        definition.symbol == trait_symbol && definition.is_boundary
                    })
                    || (!adapters
                        .iter()
                        .any(|adapter| adapter.receiver_trait == trait_symbol)
                        && !generic_requirements
                            .iter()
                            .any(|selected| selected.receiver_trait == trait_symbol))
                {
                    continue;
                }
                if !parameter.symbol.is_valid() {
                    diagnostics.push(Diagnostic::error(format!(
                        "borrowed boundary parameter `{}::{}` has no exact typed symbol for adapter dispatch",
                        machine.name, parameter.name,
                    )));
                    continue;
                }
                if let Some(existing) = boundary_fields
                    .iter()
                    .find(|field| field.symbol == parameter.symbol)
                {
                    if existing.trait_symbol != trait_symbol {
                        diagnostics.push(Diagnostic::error(format!(
                            "boundary receiver symbol {:?} maps to both trait symbols {:?} and {:?}",
                            parameter.symbol, existing.trait_symbol, trait_symbol,
                        )));
                    }
                    continue;
                }
                boundary_fields.push(BoundaryField {
                    symbol: parameter.symbol,
                    trait_symbol,
                });
            }
        }
    }

    // The first routed-Service parameter rung is admitted only when checked Unit planning
    // retained an exact typed-parameter symbol plus the matching Fused plan
    // digest. Do not rediscover arbitrary Service-looking parameters from
    // names or types here: receipt custody is what authorizes receiver
    // erasure and direct adapter dispatch.
    for machine in &checked.facts.flow.terminal_unit_effects.machines {
        for parameter in &machine.structural_parameters {
            let Some(receipt) = &parameter.fused_service_erasure else {
                continue;
            };
            if !adapters.iter().any(|adapter| {
                adapter.receiver_trait == receipt.requirement
                    && adapter.provider_plan_digest == receipt.provider_plan_digest
            }) && !generic_requirements.iter().any(|selected| {
                selected.receiver_trait == receipt.requirement
                    && selected.provider_plan_digest == receipt.provider_plan_digest
            }) {
                diagnostics.push(Diagnostic::error(format!(
                    "routed service parameter {:?} has no exact Fused selected-provider-plan join",
                    receipt.source_parameter,
                )));
                continue;
            }
            if let Some(existing) = boundary_fields
                .iter()
                .find(|existing| existing.symbol == receipt.source_parameter)
            {
                if existing.trait_symbol != receipt.requirement {
                    diagnostics.push(Diagnostic::error(format!(
                        "boundary receiver symbol {:?} maps to both trait symbols {:?} and {:?}",
                        receipt.source_parameter, existing.trait_symbol, receipt.requirement,
                    )));
                }
                continue;
            }
            boundary_fields.push(BoundaryField {
                symbol: receipt.source_parameter,
                trait_symbol: receipt.requirement,
            });
        }
    }

    if !diagnostics.is_empty() {
        return Err(diagnostics);
    }
    // Check every source occurrence against the exact selected requirement.
    // Source names remain diagnostics, never dispatch identity.
    for machine in typed.machines() {
        for state in typed.machine_states(machine) {
            for statement in typed.statement_table.statements(state.statement_nodes) {
                if let typed_trees::statement::StatementNode::Call(call) = statement {
                    resolve_adapter_call(
                        typed,
                        &adapters,
                        &generic_requirements,
                        &boundary_fields,
                        call.receiver_symbol,
                        call.target_symbol,
                        call.target.as_str(),
                        &call.machine_arguments,
                    )
                    .map_err(|error| vec![error])?;
                }
            }
        }
    }
    for (_, expression) in typed.expression_table.expression_entries() {
        let typed_trees::expression::ExpressionNode::Call(call) = expression else {
            continue;
        };
        let receiver = match typed.expression_table.expression(call.receiver) {
            typed_trees::expression::ExpressionNode::Member(member) => member.member_symbol,
            typed_trees::expression::ExpressionNode::Name(path) => path.symbol,
            _ => continue,
        };
        resolve_adapter_call(
            typed,
            &adapters,
            &generic_requirements,
            &boundary_fields,
            receiver,
            call.target_symbol,
            call.target.as_str(),
            &call.machine_arguments,
        )
        .map_err(|error| vec![error])?;
    }
    let mut dispatch = Vec::new();
    for receiver in boundary_fields {
        for adapter in adapters
            .iter()
            .filter(|adapter| adapter.receiver_trait == receiver.trait_symbol)
        {
            dispatch.push(checked_trees::CheckedBoundaryAdapterDispatch {
                receiver: receiver.symbol,
                requirement: adapter.requirement_symbol,
                realization_state: adapter.symbol,
                forward_receiver: adapter.forward_receiver,
                family_tuple: adapter.family_tuple.clone(),
            });
        }
    }
    Ok(dispatch)
}

fn resolve_selected_adapter_row(
    typed: &TypedTrees,
    plan: &effects::provider_plan::ProviderPlan,
    row: &effects::provider_plan::ProviderPlanRow,
) -> Result<Vec<ResolvedAdapterRow>, Diagnostic> {
    use effects::provider_plan::ProviderBinding;

    let ProviderBinding::CheckedAdapter {
        machine_identity, ..
    } = &row.binding
    else {
        return Ok(Vec::new());
    };
    if typed.operators().iter().any(|operator| {
        provider_planning::service_schema::schema_binds_exact_boundary_operator(
            typed,
            &plan.schema,
            operator,
        )
    }) {
        return Ok(Vec::new());
    }
    if row.requirement_identity.is_empty() {
        return Err(Diagnostic::error(format!(
            "selected checked-adapter row `{}` in ProviderPlan `{}` has no exact overload identity",
            row.method, plan.name,
        )));
    }
    let methods = plan
        .schema
        .methods
        .iter()
        .filter(|method| plan.schema.row_binds_method(row, method))
        .collect::<Vec<_>>();
    let [method] = methods.as_slice() else {
        return Err(Diagnostic::error(format!(
            "selected checked-adapter row `{}` / `{}` in ProviderPlan `{}` binds {} exact schema methods",
            row.method,
            row.requirement_identity,
            plan.name,
            methods.len(),
        )));
    };

    let receiver_trait = exact_boundary_trait(
        typed,
        &plan.schema.trait_name,
        plan.schema.trait_package_identity,
        "selected schema",
    )?;
    let requirement_owner = exact_boundary_trait(
        typed,
        &method.requirement_owner,
        method.requirement_owner_package_identity,
        "requirement owner",
    )?;
    let signatures = typed
        .trait_machine_signatures(requirement_owner)
        .iter()
        .filter(|signature| {
            signature.name.as_str() == method.name
                && typed
                    .normalized_trait_requirement_overload_identity(requirement_owner, signature)
                    .identity()
                    == method.requirement_identity
        })
        .collect::<Vec<_>>();
    let [signature] = signatures.as_slice() else {
        return Err(Diagnostic::error(format!(
            "selected checked-adapter requirement `{}` resolves to {} exact typed signatures",
            method.requirement_identity,
            signatures.len(),
        )));
    };
    if !signature.symbol.is_valid() {
        return Err(Diagnostic::error(format!(
            "selected checked-adapter requirement `{}` has no exact typed symbol",
            method.requirement_identity,
        )));
    }

    // Requirement-local binders make this a family of rows keyed by canonical
    // value tuple, not one exact overload. The signature's `where` clause
    // must declare the complete finite roster explicitly.
    if !typed.state_signature_type_parameters(signature).is_empty() {
        return resolve_family_adapter_row(
            typed,
            plan,
            row,
            method,
            receiver_trait,
            requirement_owner,
            signature,
            machine_identity,
        );
    }

    if plan.provider_type.is_empty() {
        return Err(Diagnostic::error(format!(
            "selected checked-adapter ProviderPlan `{}` has no nominal provider type",
            plan.name,
        )));
    }
    let adapter = provider_planning::exact_checked_adapter(typed, plan, row)?;
    if adapter.attached_data.as_ref().map(|owner| owner.as_str())
        != Some(plan.provider_type.as_str())
    {
        return Err(Diagnostic::error(format!(
            "selected checked adapter `{machine_identity}` does not belong to nominal provider `{}`",
            plan.provider_type,
        )));
    }
    if !adapter.supply_mode.is_checked_body() {
        return Err(Diagnostic::error(format!(
            "selected checked adapter `{machine_identity}` is not a checked body",
        )));
    }
    // A generic machine cannot be the exact realization of a nongeneric
    // requirement: its unbound binders have no tuple to close them.
    if !typed.machine_type_parameters(adapter).is_empty() {
        return Err(Diagnostic::error(format!(
            "selected checked adapter `{machine_identity}` is generic over {} machine binders; only an exact nongeneric realization can settle a boundary row",
            typed.machine_type_parameters(adapter).len(),
        )));
    }
    let Some(entry) = typed.machine_states(adapter).first() else {
        return Err(Diagnostic::error(format!(
            "selected checked adapter `{machine_identity}` has no executable entry state",
        )));
    };
    if !entry.symbol.is_valid() {
        return Err(Diagnostic::error(format!(
            "selected checked adapter `{machine_identity}` has no exact entry-state symbol",
        )));
    }

    let conformances = typed
        .machine_trait_conformances(adapter)
        .iter()
        .filter(|conformance| {
            conformance.external_binding.is_none()
                && conformance.symbol == requirement_owner.symbol
                && conformance
                    .requirement
                    .as_ref()
                    .is_some_and(|requirement| requirement.as_str() == method.name)
                && exact_conformance_requirement_identity(
                    typed,
                    adapter,
                    requirement_owner,
                    method.name.as_str(),
                )
                .as_deref()
                    == Some(method.requirement_identity.as_str())
        })
        .count();
    if conformances != 1 {
        return Err(Diagnostic::error(format!(
            "selected checked adapter `{machine_identity}` binds exact overload `{}` through {conformances} checked conformances",
            method.requirement_identity,
        )));
    }

    // ServiceMethod arity excludes the language receiver. A concrete
    // provider's `self` is therefore part of its exact realization, not the
    // explicit boundary binding that adapter dispatch may forward.
    let actual_parameters = typed
        .state_parameters(entry)
        .iter()
        .filter(|parameter| !parameter.is_self)
        .collect::<Vec<_>>();
    let forward_receiver = match exact_adapter_receiver_shape(
        typed,
        &actual_parameters,
        method.parameter_count,
        requirement_owner.symbol,
    ) {
        Some(forward_receiver) => forward_receiver,
        None => {
            let count = actual_parameters.len();
            return Err(Diagnostic::error(format!(
                "selected checked adapter `{machine_identity}` has {count} non-self entry parameters; exact overload `{}` requires {} or one leading `{}` receiver",
                method.requirement_identity, method.parameter_count, requirement_owner.name,
            )));
        }
    };

    Ok(vec![ResolvedAdapterRow::Adapter(AdapterRow {
        receiver_trait: receiver_trait.symbol,
        provider_plan_digest: *plan.identity_digest().as_bytes(),
        receiver_trait_name: receiver_trait.name.as_str().to_owned(),
        requirement: method.name.clone(),
        requirement_identity: method.requirement_identity.clone(),
        requirement_symbol: signature.symbol,
        adapter_target: adapter.name.as_str().to_owned(),
        symbol: entry.symbol,
        forward_receiver,
        family_tuple: Box::default(),
        family_tuple_display: Box::default(),
    })])
}

/// Realize one requirement's authored finite family as one dispatch row per
/// roster tuple whose checked provider specialization exists. Tuples without
/// a demanded specialization stay selectable only in name: a call selecting
/// one rejects below, and a roster that realizes no row at all leaves the
/// requirement dynamically ineligible.
#[allow(clippy::too_many_arguments)]
fn resolve_family_adapter_row(
    typed: &TypedTrees,
    plan: &effects::provider_plan::ProviderPlan,
    row: &effects::provider_plan::ProviderPlanRow,
    method: &effects::provider_plan::ServiceMethod,
    receiver_trait: &typed_trees::trait_definition::TraitDefinition,
    requirement_owner: &typed_trees::trait_definition::TraitDefinition,
    signature: &typed_trees::signature::StateSignature,
    machine_identity: &str,
) -> Result<Vec<ResolvedAdapterRow>, Diagnostic> {
    let ineligible = |reason: String| {
        vec![ResolvedAdapterRow::GenericRequirement(
            GenericBoundaryRequirement {
                receiver_trait: receiver_trait.symbol,
                provider_plan_digest: *plan.identity_digest().as_bytes(),
                requirement_symbol: signature.symbol,
                requirement_identity: method.requirement_identity.clone(),
                reason,
            },
        )]
    };
    let (arity, tuples) = match finite_signature_family(typed, signature) {
        FamilyProbe::Finite { arity, tuples } => (arity, tuples),
        FamilyProbe::NotFinite(reason) => {
            return Ok(ineligible(format!(
                "finite generic requirement `{}` is ineligible: {reason}",
                method.requirement_identity,
            )));
        }
    };

    if plan.provider_type.is_empty() {
        return Err(Diagnostic::error(format!(
            "selected checked-adapter ProviderPlan `{}` has no nominal provider type",
            plan.name,
        )));
    }
    let adapter = provider_planning::exact_checked_adapter(typed, plan, row)?;
    if adapter.attached_data.as_ref().map(|owner| owner.as_str())
        != Some(plan.provider_type.as_str())
    {
        return Err(Diagnostic::error(format!(
            "selected checked adapter `{machine_identity}` does not belong to nominal provider `{}`",
            plan.provider_type,
        )));
    }
    if !adapter.supply_mode.is_checked_body() {
        return Err(Diagnostic::error(format!(
            "selected checked adapter `{machine_identity}` is not a checked body",
        )));
    }
    // A family provider must be generic over exactly the requirement's
    // enumerable value binders; other binder kinds have no `where` equality
    // to close them and mixed static arguments cannot key a value tuple.
    let provider_parameters = typed.machine_type_parameters(adapter);
    let provider_value_parameters = provider_parameters
        .iter()
        .filter(|parameter| {
            matches!(
                parameter.kind,
                typed_trees::data::TypeParameterKind::Const { .. }
                    | typed_trees::data::TypeParameterKind::Value { .. }
            )
        })
        .count();
    if provider_parameters.len() != provider_value_parameters {
        return Err(Diagnostic::error(format!(
            "selected checked adapter `{machine_identity}` carries {} non-value machine binders; a finite family can only close the requirement's {arity} value binders",
            provider_parameters.len() - provider_value_parameters,
        )));
    }
    if provider_value_parameters != arity {
        return Err(Diagnostic::error(format!(
            "selected checked adapter `{machine_identity}` takes {provider_value_parameters} value binders but requirement `{}` declares a family of arity {arity}",
            method.requirement_identity,
        )));
    }
    let Some(template_entry) = typed.machine_states(adapter).first() else {
        return Err(Diagnostic::error(format!(
            "selected checked adapter `{machine_identity}` has no executable entry state",
        )));
    };

    let conformances = typed
        .machine_trait_conformances(adapter)
        .iter()
        .filter(|conformance| {
            conformance.external_binding.is_none()
                && conformance.symbol == requirement_owner.symbol
                && conformance
                    .requirement
                    .as_ref()
                    .is_some_and(|requirement| requirement.as_str() == method.name)
                && exact_conformance_requirement_identity(
                    typed,
                    adapter,
                    requirement_owner,
                    method.name.as_str(),
                )
                .as_deref()
                    == Some(method.requirement_identity.as_str())
        })
        .count();
    if conformances != 1 {
        return Err(Diagnostic::error(format!(
            "selected checked adapter `{machine_identity}` binds exact overload `{}` through {conformances} checked conformances",
            method.requirement_identity,
        )));
    }

    // The template entry's parameter shape is invariant under const
    // specialization, so the receiver check is computed once on the template.
    let actual_parameters = typed
        .state_parameters(template_entry)
        .iter()
        .filter(|parameter| !parameter.is_self)
        .collect::<Vec<_>>();
    let forward_receiver = match exact_adapter_receiver_shape(
        typed,
        &actual_parameters,
        method.parameter_count,
        requirement_owner.symbol,
    ) {
        Some(forward_receiver) => forward_receiver,
        None => {
            let count = actual_parameters.len();
            return Err(Diagnostic::error(format!(
                "selected checked adapter `{machine_identity}` has {count} non-self entry parameters; exact overload `{}` requires {} or one leading `{}` receiver",
                method.requirement_identity, method.parameter_count, requirement_owner.name,
            )));
        }
    };

    let mut rows = Vec::new();
    let mut missing = Vec::new();
    for tuple in &tuples {
        let specialization = typed.machine_specializations.iter().find(|specialization| {
            specialization.template == adapter.symbol
                && specialization.const_argument_identities.as_slice() == tuple.identities.as_ref()
                && specialization.type_argument_identities.is_empty()
                && specialization.machine_arguments.is_empty()
                && specialization.conformance_arguments.is_empty()
                && specialization.inferred_conformance_arguments.is_empty()
        });
        let Some(specialization) = specialization else {
            missing.push(format!("({})", tuple.display.join(", ")));
            continue;
        };
        let Some(instance) = typed
            .machines()
            .iter()
            .find(|machine| machine.symbol == specialization.instance)
        else {
            return Err(Diagnostic::error(format!(
                "finite family specialization of `{machine_identity}` for tuple `({})` has no retained machine instance",
                tuple.display.join(", "),
            )));
        };
        let Some(entry) = typed.machine_states(instance).first() else {
            return Err(Diagnostic::error(format!(
                "finite family specialization of `{machine_identity}` for tuple `({})` has no executable entry state",
                tuple.display.join(", "),
            )));
        };
        if !entry.symbol.is_valid() {
            return Err(Diagnostic::error(format!(
                "finite family specialization of `{machine_identity}` for tuple `({})` has no exact entry-state symbol",
                tuple.display.join(", "),
            )));
        }
        rows.push(ResolvedAdapterRow::Adapter(AdapterRow {
            receiver_trait: receiver_trait.symbol,
            provider_plan_digest: *plan.identity_digest().as_bytes(),
            receiver_trait_name: receiver_trait.name.as_str().to_owned(),
            requirement: method.name.clone(),
            requirement_identity: method.requirement_identity.clone(),
            requirement_symbol: signature.symbol,
            adapter_target: instance.name.as_str().to_owned(),
            symbol: entry.symbol,
            forward_receiver,
            family_tuple: tuple.identities.clone(),
            family_tuple_display: tuple.display.clone(),
        }));
    }
    if rows.is_empty() {
        return Ok(ineligible(format!(
            "finite generic requirement `{}` declares {} tuples but no checked provider specialization exists for any of them{}",
            method.requirement_identity,
            tuples.len(),
            if missing.is_empty() {
                String::new()
            } else {
                format!("; missing: {}", missing.join(", "))
            },
        )));
    }
    Ok(rows)
}

fn exact_conformance_requirement_identity(
    typed: &TypedTrees,
    adapter: &typed_trees::machine::Machine,
    owner: &typed_trees::trait_definition::TraitDefinition,
    requirement: &str,
) -> Option<String> {
    let signatures = typed
        .trait_machine_signatures(owner)
        .iter()
        .filter(|signature| signature.name.as_str() == requirement)
        .collect::<Vec<_>>();
    let signature = match signatures.as_slice() {
        [signature] => *signature,
        many => {
            let implementation_dispatch = typed
                .machine_states(adapter)
                .first()
                .map(|entry| typed.normalized_result_dispatch_set(entry.return_type));
            let matches = many
                .iter()
                .copied()
                .filter(|signature| {
                    implementation_dispatch.as_ref().is_some_and(|dispatch| {
                        typed.normalized_result_dispatch_set(signature.return_type) == *dispatch
                    })
                })
                .collect::<Vec<_>>();
            let [signature] = matches.as_slice() else {
                return None;
            };
            *signature
        }
    };
    Some(
        typed
            .normalized_trait_requirement_overload_identity(owner, signature)
            .identity(),
    )
}

fn exact_boundary_trait<'typed>(
    typed: &'typed TypedTrees,
    name: &str,
    package_identity: Option<semantic_vocabulary::PackageKeyIdentity>,
    role: &str,
) -> Result<&'typed typed_trees::trait_definition::TraitDefinition, Diagnostic> {
    if name.is_empty() {
        return Err(Diagnostic::error(format!(
            "selected checked-adapter {role} has no canonical identity",
        )));
    }
    // Trait names are package-blind; the retained package identity is the
    // only exact join against same-spelled declarations in other packages.
    let definitions = typed
        .traits()
        .iter()
        .filter(|definition| {
            definition.is_boundary
                && definition.name.as_str() == name
                && typed.symbols.symbol_package_identity(definition.symbol) == package_identity
        })
        .collect::<Vec<_>>();
    let [definition] = definitions.as_slice() else {
        return Err(Diagnostic::error(format!(
            "selected checked-adapter {role} `{name}` resolves to {} exact boundary traits",
            definitions.len(),
        )));
    };
    if !definition.symbol.is_valid() {
        return Err(Diagnostic::error(format!(
            "selected checked-adapter {role} `{name}` has no exact typed symbol",
        )));
    }
    Ok(*definition)
}

fn named_type_symbol(
    typed: &TypedTrees,
    type_reference: typed_trees::types::TypeReferenceHandle,
) -> Option<symbols::SymbolHandle> {
    let typed_trees::types::TypeReferenceNode::Named { symbol, .. } =
        typed.type_reference_table.type_reference(type_reference)
    else {
        return None;
    };
    symbol.is_valid().then_some(*symbol)
}

fn exact_adapter_receiver_shape(
    typed: &TypedTrees,
    actual_parameters: &[&typed_trees::signature::StateParameter],
    requirement_parameter_count: usize,
    requirement_owner: symbols::SymbolHandle,
) -> Option<bool> {
    match actual_parameters.len() {
        count if count == requirement_parameter_count => Some(false),
        count
            if requirement_parameter_count.checked_add(1) == Some(count)
                && actual_parameters.first().is_some_and(|parameter| {
                    named_type_symbol(typed, parameter.type_reference) == Some(requirement_owner)
                }) =>
        {
            Some(true)
        }
        _ => None,
    }
}

#[allow(clippy::too_many_arguments)]
fn resolve_adapter_call<'adapter>(
    typed: &TypedTrees,
    adapters: &'adapter [AdapterRow],
    generic_requirements: &[GenericBoundaryRequirement],
    fields: &[BoundaryField],
    receiver_symbol: symbols::SymbolHandle,
    target_symbol: symbols::SymbolHandle,
    target_name: &str,
    machine_arguments: &[typed_trees::expression::StaticMachineArgument],
) -> Result<Option<&'adapter AdapterRow>, Diagnostic> {
    let field = fields.iter().find(|field| field.symbol == receiver_symbol);
    let Some(field) = field else {
        return Ok(None);
    };
    let matches = adapters
        .iter()
        .filter(|adapter| {
            adapter.receiver_trait == field.trait_symbol
                && adapter.requirement_symbol == target_symbol
        })
        .collect::<Vec<_>>();
    // A family requirement's rows are tuple-keyed: the call's static machine
    // arguments must canonically select exactly one declared tuple. An
    // ineligible generic requirement supplies no rows and rejects below.
    let matches = if matches
        .iter()
        .any(|adapter| !adapter.family_tuple.is_empty())
    {
        let call_tuple = machine_arguments
            .iter()
            .map(|argument| call_argument_const_identity(typed, argument))
            .collect::<Option<Vec<_>>>();
        let Some(call_tuple) = call_tuple else {
            return Err(Diagnostic::error(format!(
                "boundary call `{target_name}` supplies a static argument that is not a closed const value; a finite-family call must select one declared tuple",
            )));
        };
        matches
            .iter()
            .filter(|adapter| adapter.family_tuple.as_ref() == call_tuple.as_slice())
            .copied()
            .collect::<Vec<_>>()
    } else {
        matches
    };
    let adapter = match matches.as_slice() {
        [adapter] => *adapter,
        [] => {
            // A generic requirement supplies no row: a call to it would
            // silently reach the unbound template at every width.
            if let Some(requirement) = generic_requirements.iter().find(|requirement| {
                requirement.receiver_trait == field.trait_symbol
                    && requirement.requirement_symbol == target_symbol
            }) {
                return Err(Diagnostic::error(format!(
                    "boundary call `{target_name}` selects generic requirement `{}`, which supplies no executable dispatch row: {}",
                    requirement.requirement_identity, requirement.reason,
                )));
            }
            // The requirement is a declared family with settled rows, but
            // this call's tuple matched none of them.
            if let Some(family) = adapters.iter().find(|adapter| {
                adapter.receiver_trait == field.trait_symbol
                    && adapter.requirement_symbol == target_symbol
                    && !adapter.family_tuple.is_empty()
            }) {
                return Err(Diagnostic::error(format!(
                    "boundary call `{target_name}` does not select a settled tuple of requirement `{}`",
                    family.requirement_identity,
                )));
            }
            let readable = adapters
                .iter()
                .filter(|adapter| {
                    adapter.receiver_trait == field.trait_symbol
                        && adapter.requirement == target_name
                })
                .count();
            if readable == 0 {
                return Ok(None);
            }
            return Err(Diagnostic::error(format!(
                "boundary call `{}`::`{target_name}` matches {readable} selected checked-adapter rows by display name but not exact target symbol {:?}",
                adapters
                    .iter()
                    .find(|adapter| adapter.receiver_trait == field.trait_symbol)
                    .map(|adapter| adapter.receiver_trait_name.as_str())
                    .unwrap_or("<unknown>"),
                target_symbol,
            )));
        }
        many => {
            return Err(Diagnostic::error(format!(
                "boundary call target symbol {:?} matches {} selected checked-adapter rows",
                target_symbol,
                many.len(),
            )));
        }
    };
    if adapter.requirement != target_name {
        return Err(Diagnostic::error(format!(
            "boundary call target symbol {:?} names exact overload `{}`, but its readable method drifted to `{target_name}`",
            target_symbol, adapter.requirement_identity,
        )));
    }
    Ok(Some(adapter))
}

#[cfg(test)]
mod tests {
    mod borrowed_parameters;
    mod finite_family;
    mod generic_requirements;
    mod source_retention;

    use super::*;
    use effects::provider_plan::{ProviderBinding, ProviderPlan};
    use typed_trees::expression::ExpressionNode;

    const SOURCE: &str = r#"
        boundary trait Echo {
            machine echo(value: i32) -> i32;
            machine emit(value: i32);
        }
        boundary trait Other {
            machine echo(value: i32) -> i32;
            machine emit(value: i32);
        }
        boundary trait Stateful {
            machine touch(&mut self);
        }
        boundary trait Forward {
            machine send(value: i32);
            machine reflect(value: i32) -> i32;
        }

        data EchoProvider {}
        machine EchoProvider::echo_adapter(value: i32) -> i32 satisfies Echo::echo {
            transition { _ -> (value) }
        }
        machine EchoProvider::emit_adapter(value: i32) satisfies Echo::emit {}

        data OtherProvider {}
        machine OtherProvider::echo_adapter(value: i32) -> i32 satisfies Other::echo {
            transition { _ -> (value) }
        }
        machine OtherProvider::emit_adapter(value: i32) satisfies Other::emit {}

        data StatefulProvider {}
        machine StatefulProvider::touch(&mut self) satisfies Stateful::touch {}

        data ForwardProvider {}
        machine ForwardProvider::send_adapter(service: Forward, value: i32)
            satisfies Forward::send {}
        machine ForwardProvider::reflect_adapter(service: Forward, value: i32) -> i32
            satisfies Forward::reflect {
            transition { _ -> (value) }
        }

        data EchoClient { service: Echo; }
        machine EchoClient::run(&mut self) -> i32 reaches Echo {
            self.service.emit(1);
            transition { _ -> (self.service.echo(35)) }
        }

        data OtherClient { service: Other; }
        machine OtherClient::run(&mut self) -> i32 reaches Other {
            self.service.emit(2);
            transition { _ -> (self.service.echo(35)) }
        }

        data ForwardClient { service: Forward; }
        machine ForwardClient::run(&mut self) -> i32 reaches Forward {
            self.service.send(3);
            transition { _ -> (self.service.reflect(35)) }
        }
    "#;

    struct Fixture {
        typed: TypedTrees,
        plans: Vec<ProviderPlan>,
    }

    fn fixture() -> Fixture {
        let tokens = source_files_to_tokens::Lexer::new(SOURCE)
            .tokenize()
            .expect("tokenize exact adapter-dispatch fixture");
        let syntax = tokens_to_syntax_trees::parse_syntax_trees(&tokens)
            .expect("parse exact adapter-dispatch fixture");
        let resolved = syntax_trees_to_symbol_resolved_trees::resolve(
            syntax_trees_to_symbol_resolved_trees::ResolutionRequest::new(&syntax),
        )
        .expect("resolve exact adapter-dispatch fixture");
        let typed = symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees(&resolved)
            .expect("type exact adapter-dispatch fixture");
        let plans = provider_planning::derive_satisfies_plans(&typed, None);
        Fixture { typed, plans }
    }

    fn checked_fixture() -> (CheckedTrees, Vec<ProviderPlan>) {
        let fixture = fixture();
        let checked = typed_trees_to_checked_trees::lower_typed_trees(fixture.typed)
            .expect("check exact adapter-dispatch fixture");
        (checked, fixture.plans)
    }

    fn selected_plan(plans: &[ProviderPlan], schema: &str) -> effects::SelectedProviderPlanFacts {
        let selected = plan(plans, schema);
        effects::SelectedProviderPlanFacts::from_selection(
            std::slice::from_ref(selected),
            std::slice::from_ref(&selected.name),
        )
        .expect("select exact adapter plan")
    }

    fn plan<'plans>(plans: &'plans [ProviderPlan], schema: &str) -> &'plans ProviderPlan {
        plans
            .iter()
            .find(|plan| plan.schema.trait_name == schema)
            .unwrap_or_else(|| panic!("missing `{schema}` provider plan"))
    }

    fn checked_row(plan: &ProviderPlan, method: &str) -> usize {
        plan.rows
            .iter()
            .position(|row| {
                row.method == method
                    && matches!(&row.binding, ProviderBinding::CheckedAdapter { .. })
            })
            .unwrap_or_else(|| panic!("missing checked `{method}` row"))
    }

    #[derive(Clone, Copy, Debug)]
    enum Drift {
        None,
        EmptyOverload,
        CrossOverload,
        AbsentSchema,
        AbsentSignature,
        AbsentMachine,
        DuplicateMachine,
        WrongOwner,
        NonCheckedMachine,
        WrongConformance,
        InvalidShape,
        NonAdapterBinding,
    }

    #[test]
    fn selected_rows_reject_every_exact_identity_drift() {
        let cases = [
            (Drift::None, None),
            (Drift::EmptyOverload, Some("no exact overload identity")),
            (Drift::CrossOverload, Some("binds 0 exact schema methods")),
            (
                Drift::AbsentSchema,
                Some("resolves to 0 exact boundary traits"),
            ),
            (
                Drift::AbsentSignature,
                Some("resolves to 0 exact typed signatures"),
            ),
            (Drift::AbsentMachine, Some("is absent from typed machines")),
            (
                Drift::DuplicateMachine,
                Some("resolves to 2 exact typed machines"),
            ),
            (
                Drift::WrongOwner,
                Some("does not belong to nominal provider"),
            ),
            (Drift::NonCheckedMachine, Some("is not a checked body")),
            (
                Drift::WrongConformance,
                Some("through 0 checked conformances"),
            ),
            (Drift::InvalidShape, Some("entry parameters")),
            (Drift::NonAdapterBinding, None),
        ];

        for (drift, expected_error) in cases {
            let mut fixture = fixture();
            let mut selected = plan(&fixture.plans, "Echo").clone();
            let row_index = checked_row(&selected, "echo");
            let method_index = selected
                .schema
                .methods
                .iter()
                .position(|method| {
                    method.requirement_identity == selected.rows[row_index].requirement_identity
                })
                .expect("exact echo schema method");
            let other = plan(&fixture.plans, "Other");
            let other_row = &other.rows[checked_row(other, "echo")];
            match drift {
                Drift::None => {}
                Drift::EmptyOverload => selected.rows[row_index].requirement_identity.clear(),
                Drift::CrossOverload => {
                    selected.rows[row_index].requirement_identity =
                        other_row.requirement_identity.clone();
                }
                Drift::AbsentSchema => selected.schema.trait_name = "Missing".into(),
                Drift::AbsentSignature => {
                    selected.schema.methods[method_index].requirement_owner = "Other".into();
                }
                Drift::AbsentMachine => {
                    selected.rows[row_index].binding = ProviderBinding::CheckedAdapter {
                        machine_identity: "EchoProvider::missing".into(),
                        machine_package_identity: None,
                    };
                }
                Drift::DuplicateMachine => {
                    let machine_identity = match &selected.rows[row_index].binding {
                        ProviderBinding::CheckedAdapter {
                            machine_identity, ..
                        } => machine_identity.clone(),
                        _ => unreachable!(),
                    };
                    let duplicate = fixture
                        .typed
                        .machine_by_normalized_overload_identity(&machine_identity)
                        .expect("selected adapter")
                        .clone();
                    fixture.typed.push_machine(duplicate);
                }
                Drift::WrongOwner => selected.provider_type = "OtherProvider".into(),
                Drift::NonCheckedMachine => {
                    fixture
                        .typed
                        .machines_mut()
                        .iter_mut()
                        .find(|machine| machine.name.as_str() == "EchoProvider::echo_adapter")
                        .expect("echo adapter")
                        .supply_mode = language_semantics::MachineSupplyMode::Boundary;
                }
                Drift::WrongConformance => {
                    selected.provider_type = "OtherProvider".into();
                    selected.rows[row_index].binding = other_row.binding.clone();
                }
                Drift::InvalidShape => {
                    selected.schema.methods[method_index].parameter_count = usize::MAX;
                }
                Drift::NonAdapterBinding => {
                    selected.rows[row_index].binding = ProviderBinding::CompilerIntrinsic {
                        machine: "Echo::echo.i32".into(),
                    };
                }
            }

            let result =
                resolve_selected_adapter_row(&fixture.typed, &selected, &selected.rows[row_index]);
            match expected_error {
                Some(expected) => {
                    let diagnostic = result.expect_err("identity drift must fail closed");
                    assert!(
                        diagnostic.message.contains(expected),
                        "{drift:?}: expected `{expected}`, got `{}`",
                        diagnostic.message,
                    );
                }
                None if matches!(drift, Drift::NonAdapterBinding) => {
                    assert!(
                        result
                            .expect("non-adapter rows remain delegated")
                            .is_empty()
                    );
                }
                None => {
                    let adapter = result
                        .expect("exact row resolves")
                        .into_iter()
                        .next()
                        .expect("checked row yields adapter")
                        .expect_adapter();
                    assert_eq!(adapter.receiver_trait_name, "Echo");
                    assert_eq!(adapter.adapter_target, "EchoProvider::echo_adapter");
                    assert!(!adapter.forward_receiver);
                }
            }
        }
    }

    #[test]
    fn concrete_provider_self_is_excluded_from_adapter_arity() {
        let fixture = fixture();
        let selected = plan(&fixture.plans, "Stateful");
        let row = &selected.rows[checked_row(selected, "touch")];

        let adapter = resolve_selected_adapter_row(&fixture.typed, selected, row)
            .expect("concrete provider self is an exact realization receiver")
            .into_iter()
            .next()
            .expect("checked row yields adapter")
            .expect_adapter();
        assert_eq!(adapter.adapter_target, "StatefulProvider::touch");
        assert!(!adapter.forward_receiver);
    }

    #[test]
    fn exact_leading_non_self_requirement_receiver_is_forwarded() {
        let fixture = fixture();
        let selected = plan(&fixture.plans, "Forward");
        let row = &selected.rows[checked_row(selected, "send")];

        let adapter = resolve_selected_adapter_row(&fixture.typed, selected, row)
            .expect("exact leading boundary binding resolves")
            .into_iter()
            .next()
            .expect("checked row yields adapter")
            .expect_adapter();
        assert_eq!(adapter.adapter_target, "ForwardProvider::send_adapter");
        assert!(adapter.forward_receiver);
    }

    #[test]
    fn wrong_nonleading_or_multiple_non_self_receivers_never_forward() {
        let fixture = fixture();
        let selected = plan(&fixture.plans, "Forward");
        let row = &selected.rows[checked_row(selected, "send")];
        let ProviderBinding::CheckedAdapter {
            machine_identity, ..
        } = &row.binding
        else {
            unreachable!()
        };
        let adapter = fixture
            .typed
            .machine_by_normalized_overload_identity(machine_identity)
            .expect("forwarding adapter");
        let entry = fixture
            .typed
            .machine_states(adapter)
            .first()
            .expect("forwarding entry");
        let parameters = fixture
            .typed
            .state_parameters(entry)
            .iter()
            .filter(|parameter| !parameter.is_self)
            .collect::<Vec<_>>();
        let owner = fixture
            .typed
            .traits()
            .iter()
            .find(|definition| definition.name.as_str() == "Forward")
            .expect("Forward boundary trait")
            .symbol;
        let cases = [
            (vec![parameters[1]], 0),
            (vec![parameters[1], parameters[0]], 1),
            (parameters, 0),
        ];

        for (actual, required) in cases {
            assert_eq!(
                exact_adapter_receiver_shape(&fixture.typed, &actual, required, owner),
                None,
            );
        }
    }

    fn symbol(index: u32) -> symbols::SymbolHandle {
        symbols::SymbolHandle::from_parts(index, 0)
    }

    fn adapter(
        receiver_trait: symbols::SymbolHandle,
        requirement_symbol: symbols::SymbolHandle,
        requirement: &str,
        target: &str,
    ) -> AdapterRow {
        AdapterRow {
            receiver_trait,
            provider_plan_digest: [0; 32],
            receiver_trait_name: format!("Trait{receiver_trait:?}"),
            requirement: requirement.into(),
            requirement_identity: format!("exact::{target}"),
            requirement_symbol,
            adapter_target: target.into(),
            symbol: symbol(requirement_symbol.arena_index() + 100),
            forward_receiver: false,
            family_tuple: Box::default(),
            family_tuple_display: Box::default(),
        }
    }

    #[test]
    fn same_spelled_fields_and_readable_names_never_select_adapter_rows() {
        let first_trait = symbol(1);
        let second_trait = symbol(2);
        let first_field = symbol(3);
        let second_field = symbol(4);
        let first_requirement = symbol(5);
        let second_requirement = symbol(6);
        let adapters = vec![
            adapter(
                first_trait,
                first_requirement,
                "echo",
                "FirstProvider::echo",
            ),
            adapter(
                second_trait,
                second_requirement,
                "echo",
                "SecondProvider::echo",
            ),
        ];
        let fields = [
            (
                "service",
                BoundaryField {
                    symbol: first_field,
                    trait_symbol: first_trait,
                },
            ),
            (
                "service",
                BoundaryField {
                    symbol: second_field,
                    trait_symbol: second_trait,
                },
            ),
        ];
        assert_eq!(fields[0].0, fields[1].0, "fixture field names must collide");
        let exact_fields = fields.map(|(_, field)| field);

        let cases = [
            (
                first_field,
                first_requirement,
                "echo",
                Some("FirstProvider::echo"),
                None,
            ),
            (
                second_field,
                second_requirement,
                "echo",
                Some("SecondProvider::echo"),
                None,
            ),
            (
                first_field,
                second_requirement,
                "echo",
                None,
                Some("display name but not exact target symbol"),
            ),
            (
                first_field,
                first_requirement,
                "renamed",
                None,
                Some("readable method drifted"),
            ),
            (symbol(99), first_requirement, "echo", None, None),
        ];
        for (field, target, name, expected_adapter, expected_error) in cases {
            let result = resolve_adapter_call(
                &TypedTrees::default(),
                &adapters,
                &[],
                &exact_fields,
                field,
                target,
                name,
                &[],
            );
            match (expected_adapter, expected_error) {
                (Some(expected), None) => assert_eq!(
                    result
                        .expect("exact symbols resolve")
                        .expect("adapter selected")
                        .adapter_target,
                    expected,
                ),
                (None, Some(expected)) => assert!(
                    result
                        .expect_err("name-only drift must reject")
                        .message
                        .contains(expected),
                ),
                (None, None) => assert_eq!(result.expect("unrelated fields are ignored"), None),
                _ => unreachable!(),
            }
        }

        let duplicate = adapters[0].clone();
        assert!(
            resolve_adapter_call(
                &TypedTrees::default(),
                &[adapters[0].clone(), duplicate],
                &[],
                &exact_fields,
                first_field,
                first_requirement,
                "echo",
                &[],
            )
            .expect_err("duplicate exact rows must reject")
            .message
            .contains("matches 2 selected checked-adapter rows")
        );
    }

    fn statement_call(
        checked: &CheckedTrees,
        target: &str,
    ) -> (
        arena::HandleSpan<typed_trees::statement::StatementNode>,
        usize,
        typed_trees::statement::TableCall,
    ) {
        checked
            .typed
            .machines()
            .iter()
            .flat_map(|machine| checked.typed.machine_states(machine))
            .flat_map(|state| {
                checked
                    .typed
                    .statement_table
                    .statements(state.statement_nodes)
                    .iter()
                    .enumerate()
                    .filter_map(move |(index, statement)| match statement {
                        typed_trees::statement::StatementNode::Call(call)
                            if call.target.as_str() == target =>
                        {
                            Some((state.statement_nodes, index, call.clone()))
                        }
                        _ => None,
                    })
            })
            .next()
            .unwrap_or_else(|| panic!("missing statement call `{target}`"))
    }

    fn expression_call(
        checked: &CheckedTrees,
        target: &str,
    ) -> (
        typed_trees::expression::ExpressionHandle,
        typed_trees::expression::TableCallExpression,
    ) {
        checked
            .typed
            .expression_table
            .expression_entries()
            .find_map(|(handle, expression)| match expression {
                ExpressionNode::Call(call) if call.target.as_str() == target => {
                    Some((handle, call.clone()))
                }
                _ => None,
            })
            .unwrap_or_else(|| panic!("missing value call `{target}`"))
    }

    fn adapter_entry_symbol(checked: &CheckedTrees, machine_name: &str) -> symbols::SymbolHandle {
        let machine = checked
            .typed
            .machines()
            .iter()
            .find(|machine| machine.name.as_str() == machine_name)
            .unwrap_or_else(|| panic!("missing adapter `{machine_name}`"));
        checked
            .typed
            .machine_states(machine)
            .first()
            .expect("adapter entry")
            .symbol
    }

    #[test]
    fn selected_execution_keeps_source_and_records_exact_and_forwarding_targets() {
        for (
            trait_name,
            statement_name,
            expression_name,
            statement_adapter,
            expression_adapter,
            forwarding,
        ) in [
            (
                "Echo",
                "emit",
                "echo",
                "EchoProvider::emit_adapter",
                "EchoProvider::echo_adapter",
                false,
            ),
            (
                "Forward",
                "send",
                "reflect",
                "ForwardProvider::send_adapter",
                "ForwardProvider::reflect_adapter",
                true,
            ),
        ] {
            let (checked, plans) = checked_fixture();
            let selected = selected_plan(&plans, trait_name);
            let (_, _, statement) = statement_call(&checked, statement_name);
            let (_, expression) = expression_call(&checked, expression_name);
            let original = Arc::new(checked);
            let mut settled = Arc::clone(&original);
            settle_selected_boundary_adapter_dispatch(&mut settled, &selected).unwrap();
            assert_eq!(
                settled.typed, original.typed,
                "no rewritten calls or synthetic receiver expressions"
            );
            assert!(
                settled
                    .facts
                    .boundary_adapter_dispatch
                    .iter()
                    .any(|row| row.receiver == statement.receiver_symbol
                        && row.requirement == statement.target_symbol
                        && row.realization_state
                            == adapter_entry_symbol(&settled, statement_adapter)
                        && row.forward_receiver == forwarding)
            );
            assert!(
                settled
                    .facts
                    .boundary_adapter_dispatch
                    .iter()
                    .any(|row| row.requirement == expression.target_symbol
                        && row.realization_state
                            == adapter_entry_symbol(&settled, expression_adapter)
                        && row.forward_receiver == forwarding)
            );
            let mut other_facts = settled.facts.clone();
            other_facts.boundary_adapter_dispatch.clear();
            assert_eq!(
                other_facts, original.facts,
                "source-derived facts remain unchanged"
            );
            let first = Arc::clone(&settled);
            settle_selected_boundary_adapter_dispatch(&mut settled, &selected).unwrap();
            assert!(
                Arc::ptr_eq(&first, &settled),
                "unchanged selections require no tree copy"
            );
        }
    }

    #[test]
    fn late_invalid_call_prevents_statement_and_value_rewrites() {
        let (mut checked, plans) = checked_fixture();
        let selected_names = plans
            .iter()
            .map(|plan| plan.name.clone())
            .collect::<Vec<_>>();
        let selected = effects::SelectedProviderPlanFacts::from_selection(&plans, &selected_names)
            .expect("select every exact fixture plan");
        let calls = checked
            .typed
            .expression_table
            .expression_entries()
            .filter_map(|(handle, expression)| {
                matches!(expression, ExpressionNode::Call(call) if call.target.as_str() == "echo")
                    .then_some(handle)
            })
            .collect::<Vec<_>>();
        assert_eq!(calls.len(), 2);
        let ExpressionNode::Call(mut invalid) =
            checked.typed.expression_table.expression(calls[1]).clone()
        else {
            unreachable!()
        };
        invalid.target_symbol = symbols::SymbolHandle::invalid();
        *checked.typed.expression_table.expression_mut(calls[1]) = ExpressionNode::Call(invalid);
        let before = checked.clone();
        let original = Arc::new(checked);
        let mut rejected = Arc::clone(&original);

        let diagnostics = settle_selected_boundary_adapter_dispatch(&mut rejected, &selected)
            .expect_err("one invalid late call rejects the complete batch");
        assert!(
            diagnostics
                .iter()
                .any(|diagnostic| diagnostic.message.contains("not exact target symbol"))
        );
        assert!(Arc::ptr_eq(&rejected, &original));
        assert_eq!(rejected.as_ref(), &before);
    }

    #[test]
    fn empty_settlement_preserves_shared_arc_identity_and_contents() {
        let (checked, _) = checked_fixture();
        let original_contents = checked.clone();
        let original = Arc::new(checked);
        let mut settled = Arc::clone(&original);

        settle_selected_boundary_adapter_dispatch(
            &mut settled,
            &effects::SelectedProviderPlanFacts::default(),
        )
        .expect("a program without selected boundary adapters is already settled");

        assert!(Arc::ptr_eq(&settled, &original));
        assert_eq!(settled.as_ref(), &original_contents);
    }
}
