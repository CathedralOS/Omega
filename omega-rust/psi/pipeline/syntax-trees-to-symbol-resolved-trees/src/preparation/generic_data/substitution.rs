//! Member and type-reference substitution.
use crate::preparation::generic_data::ClosedArgumentIdentity;
use crate::preparation::generic_data::EvaluatedConst;
use crate::preparation::generic_data::constant_selection;
use crate::preparation::generic_data::evaluate_const_argument_expression;
use crate::preparation::generic_data::generic_const_integer_types;
use crate::preparation::generic_data::selected_data_item;
use arena::Handle;
use arena::HandleSpan;
use diagnostics::Diagnostic;
use numerics::literals::IntegerLiteral;
use numerics::literals::IntegerRadix;
use std::collections::HashMap;
use std::collections::HashSet;
use syntax_trees::SyntaxTrees;
use syntax_trees::expression::BinaryOperator;
use syntax_trees::expression::ExpressionHandle;
use syntax_trees::expression::ExpressionNode;
use syntax_trees::identifier::Identifier;
use syntax_trees::item::DataMember;
use syntax_trees::item::ProofFact;
use syntax_trees::types::DomainConstraint;
use syntax_trees::types::FixedArrayLength;
use syntax_trees::types::TypeConstraintNode;
use syntax_trees::types::TypeReferenceHandle;
use syntax_trees::types::TypeReferenceNode;

/// Clone a member with the type parameters substituted. Only reached for a base
/// `base_is_fully_monomorphizable` accepted. A field that IS a parameter points
/// at the argument; a NESTED generic (`a: Box<T>`) becomes a fresh concrete
/// spelling (`Box<i32>`) the fixpoint monomorphizes; a parameter-free field is
/// shared unchanged.
pub(crate) fn substitute_member(
    syntax: &mut SyntaxTrees,
    snapshot: &SyntaxTrees,
    member: DataMember,
    substitution: &HashMap<String, TypeReferenceHandle>,
    type_identities: &HashMap<String, ClosedArgumentIdentity>,
    selection: Option<&constant_selection::ConstantSelection>,
    instance_name: &str,
    const_values: &HashMap<String, i128>,
    warnings: &mut Vec<Diagnostic>,
) -> Result<DataMember, Diagnostic> {
    let member = match member {
        DataMember::Field(field) => DataMember::Field(substitute_data_field(
            syntax,
            snapshot,
            field,
            substitution,
            const_values,
            warnings,
        )),
        DataMember::Variant(mut variant) => {
            let payload = syntax
                .tables
                .items
                .data_payload_fields(variant.payload)
                .to_vec();
            let mut first = Handle::invalid();
            let mut count = 0u32;
            for field in payload {
                let field = substitute_data_field(
                    syntax,
                    snapshot,
                    field,
                    substitution,
                    const_values,
                    warnings,
                );
                let handle = syntax.tables.items.append_data_payload_field(field);
                if count == 0 {
                    first = handle;
                }
                count = count
                    .checked_add(1)
                    .expect("generic sum payload field count overflow");
            }
            variant.payload = HandleSpan::from_parts(first, count);
            // CASE-CONSTRAINTS generic case-data synthesis: the case's `where`
            // facts ride the instance with it. The syntax->resolved lowering
            // fence already refused any fact that names a parameter without a
            // fact-position substitution, so a deep copy out of the
            // pre-substitution snapshot is faithful -- field names, literals,
            // and top-level bindings spell identically on the instance, and a
            // `const` binder mention is rewritten to its literal argument by
            // the caller's rewrite over freshly copied expressions. Source
            // spans ride along, so a construction-side refusal still names the
            // authored clause.
            //
            // A `type` binder is the exception: the gate admits it only as one
            // side of a top-level `==`/`!=` conjunct, which this copy decides
            // against the closed argument identity. A decided-true conjunct
            // discharges out of the carried fact; a decided-false one leaves
            // the literal `0` witness behind so the case reads as impossible
            // for construction, zero gating, and coverage on this instance.
            variant.where_facts = copy_case_where_facts(
                syntax,
                snapshot,
                variant.where_facts,
                type_identities,
                selection,
                instance_name,
            )?;
            DataMember::Variant(variant)
        }
        DataMember::Retired(identity) => DataMember::Retired(identity),
    };
    Ok(member)
}

/// What one top-level `and` conjunct of a copied case fact became under this
/// instance's arguments.
enum ConjunctDecision {
    /// Not a type-parameter equality: the conjunct copies verbatim.
    Ordinary,
    /// A decided-true `T == name` / `T != name`: discharged at instantiation,
    /// the conjunct drops out of the carried fact.
    Established,
    /// A decided-false type equality: the whole fact collapses to the literal
    /// `0` witness -- this case can never be established for the instance.
    Refuted,
}

/// Deep-copy one case `where` fact span out of the template snapshot, deciding
/// admitted type-parameter equalities against this instance's closed argument
/// identities. The handles index the template's fact arena, which `snapshot`
/// preserves exactly; each kept copy appends to the live tree, so the rebuilt
/// span stays contiguous.
fn copy_case_where_facts(
    syntax: &mut SyntaxTrees,
    snapshot: &SyntaxTrees,
    facts: HandleSpan<ProofFact>,
    type_identities: &HashMap<String, ClosedArgumentIdentity>,
    selection: Option<&constant_selection::ConstantSelection>,
    instance_name: &str,
) -> Result<HandleSpan<ProofFact>, Diagnostic> {
    let mut copied = HandleSpan::empty();
    for offset in 0..facts.count() {
        let source = Handle::from_parts(
            facts
                .start()
                .arena_index()
                .checked_add(offset)
                .expect("case where-fact source handle overflow"),
            facts.start().generation(),
        );
        let ProofFact::Expression(template_root) = snapshot.items.proof_fact(source) else {
            copied.push_contiguous(syntax.copy_proof_fact_from(snapshot, source));
            continue;
        };
        let mut conjuncts = Vec::new();
        flatten_case_fact_conjuncts(snapshot, *template_root, &mut conjuncts);
        let mut decisions = Vec::with_capacity(conjuncts.len());
        for conjunct in &conjuncts {
            decisions.push(decide_type_equality_conjunct(
                snapshot,
                *conjunct,
                type_identities,
                selection,
                instance_name,
            )?);
        }
        if decisions
            .iter()
            .all(|decision| matches!(decision, ConjunctDecision::Ordinary))
        {
            copied.push_contiguous(syntax.copy_proof_fact_from(snapshot, source));
            continue;
        }
        if decisions
            .iter()
            .any(|decision| matches!(decision, ConjunctDecision::Refuted))
        {
            let handle = syntax.copy_proof_fact_from(snapshot, source);
            let copied_root = match syntax.items.proof_fact(handle) {
                ProofFact::Expression(expression) => *expression,
                _ => unreachable!("an expression fact copies as an expression fact"),
            };
            // The decided-false witness: `0` folds FALSE for construction,
            // zero gating, and coverage, and replays against the template's
            // equality on the seeded-instance check.
            syntax.expressions.replace_expression(
                copied_root,
                ExpressionNode::Integer(
                    IntegerLiteral::from_parts(false, IntegerRadix::Decimal, "0")
                        .expect("literal `0` is a valid integer literal"),
                ),
            );
            copied.push_contiguous(handle);
            continue;
        }
        if conjuncts.len() == decisions.len()
            && decisions
                .iter()
                .all(|decision| matches!(decision, ConjunctDecision::Established))
        {
            // Every conjunct was a type equality this instance proved: the
            // instantiation obligation is discharged, so the fact does not
            // ride the instance.
            continue;
        }
        // A mixed fact: rewrite the copied conjunction over only the conjuncts
        // that did not discharge. The copy preserves the template's tree, so
        // flattening it yields conjuncts in the same order the decisions were
        // taken.
        let handle = syntax.copy_proof_fact_from(snapshot, source);
        let copied_root = match syntax.items.proof_fact(handle) {
            ProofFact::Expression(expression) => *expression,
            _ => unreachable!("an expression fact copies as an expression fact"),
        };
        let mut copied_conjuncts = Vec::new();
        flatten_case_fact_conjuncts(syntax, copied_root, &mut copied_conjuncts);
        let kept: Vec<ExpressionHandle> = copied_conjuncts
            .iter()
            .copied()
            .zip(decisions.iter())
            .filter_map(|(conjunct, decision)| {
                matches!(decision, ConjunctDecision::Ordinary).then_some(conjunct)
            })
            .collect();
        let rebuilt = rebuild_conjunction(syntax, &kept);
        let rebuilt_node = syntax.expressions.expression(rebuilt).clone();
        syntax
            .expressions
            .replace_expression(copied_root, rebuilt_node);
        copied.push_contiguous(handle);
    }
    Ok(copied)
}

fn flatten_case_fact_conjuncts(
    syntax: &SyntaxTrees,
    expression: ExpressionHandle,
    conjuncts: &mut Vec<ExpressionHandle>,
) {
    if let ExpressionNode::Binary(binary) = syntax.expressions.expression(expression)
        && binary.operator == BinaryOperator::And
    {
        flatten_case_fact_conjuncts(syntax, binary.left, conjuncts);
        flatten_case_fact_conjuncts(syntax, binary.right, conjuncts);
        return;
    }
    conjuncts.push(expression);
}

/// Rebuild an `and` chain over the kept conjuncts, reusing their nodes.
/// Callers guarantee `kept` is nonempty and smaller than the original
/// conjunction.
fn rebuild_conjunction(syntax: &mut SyntaxTrees, kept: &[ExpressionHandle]) -> ExpressionHandle {
    let mut rebuilt = kept[0];
    for conjunct in &kept[1..] {
        rebuilt = syntax.expressions.insert(ExpressionNode::Binary(
            syntax_trees::expression::TableBinaryExpression {
                left: rebuilt,
                operator: BinaryOperator::And,
                right: *conjunct,
            },
        ));
    }
    rebuilt
}

/// Decide one copied conjunct as a type-parameter equality. The lowering gate
/// (`lowering/data.rs::case_fact_type_equality`) admits exactly this shape: one
/// side is a single-segment `type` binder, the other a single-segment
/// binder-free name. Both sides' identities are closed by construction -- the
/// instance's arguments already passed the closed-shape gate -- so an
/// unresolvable opposite name is an authored-fact error, never a silent drop.
fn decide_type_equality_conjunct(
    syntax: &SyntaxTrees,
    conjunct: ExpressionHandle,
    type_identities: &HashMap<String, ClosedArgumentIdentity>,
    selection: Option<&constant_selection::ConstantSelection>,
    instance_name: &str,
) -> Result<ConjunctDecision, Diagnostic> {
    let ExpressionNode::Binary(binary) = syntax.expressions.expression(conjunct) else {
        return Ok(ConjunctDecision::Ordinary);
    };
    if !matches!(
        binary.operator,
        BinaryOperator::Equal | BinaryOperator::NotEqual
    ) {
        return Ok(ConjunctDecision::Ordinary);
    }
    let single_name = |side| -> Option<&Identifier> {
        let ExpressionNode::Name(path) = syntax.expressions.expression(side) else {
            return None;
        };
        let [member] = syntax.expressions.identifier_path_members(*path) else {
            return None;
        };
        Some(member)
    };
    let Some((parameter, other)) = (match (single_name(binary.left), single_name(binary.right)) {
        (Some(left), Some(right)) if type_identities.contains_key(left.as_str()) => {
            Some((left, right))
        }
        (Some(left), Some(right)) if type_identities.contains_key(right.as_str()) => {
            Some((right, left))
        }
        _ => None,
    }) else {
        return Ok(ConjunctDecision::Ordinary);
    };
    let parameter_identity = type_identities
        .get(parameter.as_str())
        .expect("the gate admits only listed type binders");
    let Some(other_identity) = closed_name_identity(syntax, selection, other) else {
        return Err(Diagnostic::error(format!(
            "case constraint on generic instance `{instance_name}` cannot decide `{}`: `{}` does not name a closed type",
            parameter.as_str(),
            other.as_str(),
        )));
    };
    let equal = *parameter_identity == other_identity;
    let holds = match binary.operator {
        BinaryOperator::Equal => equal,
        BinaryOperator::NotEqual => !equal,
        _ => unreachable!("the equality check above admits only == or !="),
    };
    Ok(if holds {
        ConjunctDecision::Established
    } else {
        ConjunctDecision::Refuted
    })
}

/// The closed identity of a fact-position type name: builtins by their atom,
/// declared data by item, and module-retained declarations by symbol. Mirrors
/// the `TypeReferenceNode::Named` arm of `closed_argument_identity` without
/// materializing a reference node.
pub(crate) fn closed_name_identity(
    syntax: &SyntaxTrees,
    selection: Option<&constant_selection::ConstantSelection>,
    name: &Identifier,
) -> Option<ClosedArgumentIdentity> {
    if let Some(atom) = symbols::BuiltinTypeAtom::ALL
        .into_iter()
        .find(|atom| atom.symbol_name() == name.as_str())
    {
        return Some(ClosedArgumentIdentity::Builtin(atom));
    }
    if let Some(declaration) = selected_data_item(syntax, selection, name) {
        Some(ClosedArgumentIdentity::Nominal(declaration))
    } else {
        Some(ClosedArgumentIdentity::RetainedNominal(
            selection?.retained_identity(name, symbols::SymbolKind::Data)?,
        ))
    }
}

pub(crate) fn substitute_data_field(
    syntax: &mut SyntaxTrees,
    snapshot: &SyntaxTrees,
    mut field: syntax_trees::item::DataField,
    substitution: &HashMap<String, TypeReferenceHandle>,
    const_values: &HashMap<String, i128>,
    warnings: &mut Vec<Diagnostic>,
) -> syntax_trees::item::DataField {
    field.type_reference = substitute_type_reference(
        syntax,
        snapshot,
        field.type_reference,
        substitution,
        const_values,
        warnings,
    );
    field
}

pub(crate) fn substitute_type_reference(
    syntax: &mut SyntaxTrees,
    snapshot: &SyntaxTrees,
    type_reference: TypeReferenceHandle,
    substitution: &HashMap<String, TypeReferenceHandle>,
    const_values: &HashMap<String, i128>,
    warnings: &mut Vec<Diagnostic>,
) -> TypeReferenceHandle {
    let node = syntax
        .tables
        .type_references
        .type_reference(type_reference)
        .clone();
    match node {
        TypeReferenceNode::Named(name) => substitution
            .get(name.as_str())
            .copied()
            .unwrap_or(type_reference),
        TypeReferenceNode::Generic {
            base_name,
            lifetime_arguments,
            arguments,
        } => {
            let argument_handles: Vec<TypeReferenceHandle> = syntax
                .tables
                .type_references
                .type_reference_handles(arguments)
                .to_vec();
            let integer_types = generic_const_integer_types(syntax, base_name.as_str());
            let const_bindings: HashMap<String, i128> = substitution
                .iter()
                .filter_map(|(name, argument)| {
                    let TypeReferenceNode::Named(value) =
                        syntax.tables.type_references.type_reference(*argument)
                    else {
                        return None;
                    };
                    Some((name.clone(), value.as_str().parse::<i128>().ok()?))
                })
                .collect();
            let mut substituted_arguments = Vec::with_capacity(argument_handles.len());
            for (index, argument) in argument_handles.into_iter().enumerate() {
                let node = syntax
                    .tables
                    .type_references
                    .type_reference(argument)
                    .clone();
                let substituted = match node {
                    TypeReferenceNode::Named(name) => {
                        substitution.get(name.as_str()).copied().unwrap_or(argument)
                    }
                    TypeReferenceNode::ConstExpression(expression) => {
                        let warning_watermark = warnings.len();
                        match evaluate_const_argument_expression(
                            syntax,
                            expression,
                            const_values,
                            &const_bindings,
                            &HashSet::new(),
                            integer_types.get(index).copied().flatten(),
                            warnings,
                        )
                        .and_then(EvaluatedConst::into_concrete)
                        {
                            Ok(value) => syntax
                                .tables
                                .type_references
                                .insert_named(Identifier::generated(value.to_string())),
                            Err(_) => {
                                warnings.truncate(warning_watermark);
                                argument
                            }
                        }
                    }
                    _ => substitute_type_reference(
                        syntax,
                        snapshot,
                        argument,
                        substitution,
                        const_values,
                        warnings,
                    ),
                };
                substituted_arguments.push(substituted);
            }
            let new_span = syntax
                .tables
                .type_references
                .insert_type_reference_handles(substituted_arguments);
            syntax
                .tables
                .type_references
                .insert(TypeReferenceNode::Generic {
                    base_name,
                    lifetime_arguments,
                    arguments: new_span,
                })
        }
        TypeReferenceNode::FixedArray {
            element_type,
            length,
        } => {
            let substituted_element = substitute_type_reference(
                syntax,
                snapshot,
                element_type,
                substitution,
                const_values,
                warnings,
            );
            let substituted_length = match length {
                FixedArrayLength::ConstParameter(name) => substitution
                    .get(name.as_str())
                    .and_then(|argument| {
                        match syntax.tables.type_references.type_reference(*argument) {
                            TypeReferenceNode::Named(value) => value.as_str().parse::<usize>().ok(),
                            _ => None,
                        }
                    })
                    .map(FixedArrayLength::Literal)
                    .unwrap_or(FixedArrayLength::ConstParameter(name)),
                length => length,
            };
            syntax
                .tables
                .type_references
                .insert(TypeReferenceNode::FixedArray {
                    element_type: substituted_element,
                    length: substituted_length,
                })
        }
        TypeReferenceNode::Reference {
            referee,
            access,
            lifetime,
        } => {
            let referee = substitute_type_reference(
                syntax,
                snapshot,
                referee,
                substitution,
                const_values,
                warnings,
            );
            syntax
                .tables
                .type_references
                .insert(TypeReferenceNode::Reference {
                    referee,
                    access,
                    lifetime,
                })
        }
        TypeReferenceNode::Slice { element_type } => {
            let element_type = substitute_type_reference(
                syntax,
                snapshot,
                element_type,
                substitution,
                const_values,
                warnings,
            );
            syntax
                .tables
                .type_references
                .insert(TypeReferenceNode::Slice { element_type })
        }
        TypeReferenceNode::Constrained {
            base_type,
            constraints,
        } => substitute_constrained_type_reference(
            syntax,
            snapshot,
            type_reference,
            base_type,
            constraints,
            substitution,
            const_values,
            warnings,
        ),
        _ => type_reference,
    }
}

/// Substitute through one constrained type. A constraint that carries no
/// substituted binder keeps the template's node so identity and provenance
/// keyed on the authored occurrence still match; a `Counted<N>` domain index
/// or a `0..N` range endpoint that does mention one is rebuilt onto the
/// instance so the open binder never escapes its owning telescope.
fn substitute_constrained_type_reference(
    syntax: &mut SyntaxTrees,
    snapshot: &SyntaxTrees,
    type_reference: TypeReferenceHandle,
    base_type: TypeReferenceHandle,
    constraints: HandleSpan<TypeConstraintNode>,
    substitution: &HashMap<String, TypeReferenceHandle>,
    const_values: &HashMap<String, i128>,
    warnings: &mut Vec<Diagnostic>,
) -> TypeReferenceHandle {
    let substituted_base = substitute_type_reference(
        syntax,
        snapshot,
        base_type,
        substitution,
        const_values,
        warnings,
    );
    let source_constraints = syntax
        .tables
        .type_references
        .constraints(constraints)
        .to_vec();
    let mut changed = substituted_base != base_type;
    let mut rewritten = Vec::with_capacity(source_constraints.len());
    for constraint in source_constraints {
        let (constraint, constraint_changed) = match constraint {
            TypeConstraintNode::Domain(domain) => {
                let arguments = syntax
                    .tables
                    .type_references
                    .type_reference_handles(domain.arguments)
                    .to_vec();
                let mut domain_changed = false;
                let mut substituted_arguments = Vec::with_capacity(arguments.len());
                for argument in arguments {
                    let substituted = substitute_domain_index_argument(
                        syntax,
                        snapshot,
                        argument,
                        substitution,
                        const_values,
                        warnings,
                    );
                    domain_changed |= substituted != argument;
                    substituted_arguments.push(substituted);
                }
                if domain_changed {
                    let arguments = syntax
                        .tables
                        .type_references
                        .insert_type_reference_handles(substituted_arguments);
                    (
                        TypeConstraintNode::Domain(DomainConstraint {
                            name: domain.name,
                            arguments,
                        }),
                        true,
                    )
                } else {
                    (TypeConstraintNode::Domain(domain), false)
                }
            }
            TypeConstraintNode::Range {
                minimum,
                maximum,
                end_inclusive,
            } => {
                let substituted_minimum =
                    substitute_bound_expression(syntax, snapshot, minimum, substitution);
                let substituted_maximum =
                    substitute_bound_expression(syntax, snapshot, maximum, substitution);
                (
                    TypeConstraintNode::Range {
                        minimum: substituted_minimum,
                        maximum: substituted_maximum,
                        end_inclusive,
                    },
                    substituted_minimum != minimum || substituted_maximum != maximum,
                )
            }
            constraint => (constraint, false),
        };
        changed |= constraint_changed;
        rewritten.push(constraint);
    }
    if !changed {
        return type_reference;
    }
    // A fresh constraint span on a fresh owner: retained range normalizations
    // key on the exact authored occurrence, so nothing stale can match here.
    let constraints = syntax.tables.type_references.insert_constraints(rewritten);
    syntax
        .tables
        .type_references
        .insert(TypeReferenceNode::Constrained {
            base_type: substituted_base,
            constraints,
        })
}

/// Substitute one domain-index argument inside a constrained type. A bare
/// `N` takes the closed argument handle directly; an open `ConstExpression`
/// is copied out of the snapshot so the caller's const-binder rewrite lands
/// on the instance's own subtree and the authored operator spelling (and any
/// retained normalization provenance on the template node) is preserved;
/// anything richer recurses through the ordinary substitution.
fn substitute_domain_index_argument(
    syntax: &mut SyntaxTrees,
    snapshot: &SyntaxTrees,
    argument: TypeReferenceHandle,
    substitution: &HashMap<String, TypeReferenceHandle>,
    const_values: &HashMap<String, i128>,
    warnings: &mut Vec<Diagnostic>,
) -> TypeReferenceHandle {
    match syntax
        .tables
        .type_references
        .type_reference(argument)
        .clone()
    {
        TypeReferenceNode::Named(name) => {
            substitution.get(name.as_str()).copied().unwrap_or(argument)
        }
        TypeReferenceNode::ConstExpression(expression) => {
            if !expression_mentions_parameter(syntax, expression, substitution) {
                return argument;
            }
            let copied = syntax.copy_expression_from(snapshot, expression);
            syntax
                .tables
                .type_references
                .insert(TypeReferenceNode::ConstExpression(copied))
        }
        _ => substitute_type_reference(
            syntax,
            snapshot,
            argument,
            substitution,
            const_values,
            warnings,
        ),
    }
}

/// Copy one range endpoint into the instance when it mentions a substituted
/// binder (the caller's const-binder rewrite then reduces the fresh `Name`
/// leaf to the closed literal). A parameter-free endpoint keeps the template's
/// authored expression handle.
fn substitute_bound_expression(
    syntax: &mut SyntaxTrees,
    snapshot: &SyntaxTrees,
    endpoint: ExpressionHandle,
    substitution: &HashMap<String, TypeReferenceHandle>,
) -> ExpressionHandle {
    if !expression_mentions_parameter(syntax, endpoint, substitution) {
        return endpoint;
    }
    syntax.copy_expression_from(snapshot, endpoint)
}

/// Whether an expression mentions a bare name the substitution rewrites. Only
/// the expression shapes generic substitution rebuilds leaf-by-leaf unfold;
/// anything richer conservatively counts as a mention so the endpoint or index
/// is copied rather than shared into the instance unchanged.
fn expression_mentions_parameter(
    syntax: &SyntaxTrees,
    expression: ExpressionHandle,
    substitution: &HashMap<String, TypeReferenceHandle>,
) -> bool {
    match syntax.expressions.expression(expression) {
        ExpressionNode::Name(path) => {
            let [member] = syntax.expressions.identifier_path_members(*path) else {
                return false;
            };
            substitution.contains_key(member.as_str())
        }
        ExpressionNode::Binary(binary) => {
            expression_mentions_parameter(syntax, binary.left, substitution)
                || expression_mentions_parameter(syntax, binary.right, substitution)
        }
        ExpressionNode::Unary(unary) => {
            expression_mentions_parameter(syntax, unary.operand, substitution)
        }
        ExpressionNode::Integer(_)
        | ExpressionNode::Boolean(_)
        | ExpressionNode::Float(_)
        | ExpressionNode::String(_)
        | ExpressionNode::SelfValue => false,
        _ => true,
    }
}

/// Whether a type reference mentions any of the substituted parameter names
/// (recursively through composite nodes). Conservative: on an unhandled node
/// shape it returns `true` so the caller rejects rather than silently sharing a
/// parameter-bearing type.
pub(crate) fn type_reference_mentions_parameter(
    syntax: &SyntaxTrees,
    handle: TypeReferenceHandle,
    substitution: &HashMap<String, TypeReferenceHandle>,
) -> bool {
    match syntax.tables.type_references.type_reference(handle) {
        TypeReferenceNode::Named(name) => substitution.contains_key(name.as_str()),
        TypeReferenceNode::Generic { arguments, .. } => syntax
            .tables
            .type_references
            .type_reference_handles(*arguments)
            .iter()
            .any(|&argument| type_reference_mentions_parameter(syntax, argument, substitution)),
        // The common composite shells recurse precisely, so a parameter-FREE
        // field like `touched: i32 in Wrapping` or `tags: [u8; 4]` shares
        // unchanged instead of refusing the whole container. An indexed
        // domain or range constraint does carry parameter-bearing leaves --
        // `Counted<N>`'s index is a type reference and `0..N`'s endpoints are
        // expressions -- so those are inspected too.
        TypeReferenceNode::Constrained {
            base_type,
            constraints,
        } => {
            type_reference_mentions_parameter(syntax, *base_type, substitution)
                || syntax
                    .tables
                    .type_references
                    .constraints(*constraints)
                    .iter()
                    .any(|constraint| match constraint {
                        TypeConstraintNode::Domain(domain) => syntax
                            .tables
                            .type_references
                            .type_reference_handles(domain.arguments)
                            .iter()
                            .any(|&argument| {
                                type_reference_mentions_parameter(syntax, argument, substitution)
                            }),
                        TypeConstraintNode::Range {
                            minimum, maximum, ..
                        } => {
                            expression_mentions_parameter(syntax, *minimum, substitution)
                                || expression_mentions_parameter(syntax, *maximum, substitution)
                        }
                        TypeConstraintNode::Named(_) | TypeConstraintNode::ArithmeticDomain(_) => {
                            false
                        }
                    })
        }
        TypeReferenceNode::FixedArray { element_type, .. } => {
            type_reference_mentions_parameter(syntax, *element_type, substitution)
        }
        TypeReferenceNode::Slice { element_type } => {
            type_reference_mentions_parameter(syntax, *element_type, substitution)
        }
        TypeReferenceNode::Reference { referee, .. } => {
            type_reference_mentions_parameter(syntax, *referee, substitution)
        }
        // Anything else: conservative -- possibly parameter-bearing, refuse
        // rather than share a wrong type.
        _ => true,
    }
}
