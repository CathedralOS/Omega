//! Closed-instance discovery, fixed-point synthesis and the ordered rewrite.
use crate::preparation::generic_data::ClosedArgumentIdentity;
use crate::preparation::generic_data::ConstFactValue;
use crate::preparation::generic_data::ConstScalarValue;
use crate::preparation::generic_data::GenericData;
use crate::preparation::generic_data::Instantiation;
use crate::preparation::generic_data::PendingRewrite;
use crate::preparation::generic_data::canonicalize_closed_domain_indices;
use crate::preparation::generic_data::capture_machine_runtime_template_names;
use crate::preparation::generic_data::uses::{
    closed_constructor_carrier, relabel_closed_data_uses_in_constants,
    relabel_closed_data_uses_in_exact_assignments,
    relabel_closed_data_uses_in_exact_calls_and_returns,
    relabel_closed_sum_memberships_from_local_types,
};

use super::substitution::substitute_copied_type_names;
use super::work_domain::{GenericDataWorkDomain, SynthesisWork, TypePositionPurpose};
use crate::preparation::generic_data::evaluate_const_fact_expression;
use crate::preparation::generic_data::evaluate_const_membership_fact;
use crate::preparation::generic_data::normalize_domain_head_positions;
use crate::preparation::generic_data::normalize_generic_template_const_expressions;
use crate::preparation::generic_data::prepare_closed_application;
use crate::preparation::generic_data::qualified_const_name;

use crate::preparation::generic_data::replace_const_expression_names_from;
use crate::preparation::generic_data::replace_machine_const_expression_names_from;
use crate::preparation::generic_data::selected_data_item;
use crate::preparation::generic_data::substitute_member;
use arena::Handle;
use arena::HandleSpan;
use diagnostics::Diagnostic;
use numerics::literals::IntegerLiteral;
use numerics::literals::IntegerRadix;
use std::collections::HashMap;
use syntax_trees::SyntaxTrees;
use syntax_trees::expression::ExpressionNode;
use syntax_trees::identifier::Identifier;
use syntax_trees::item::ConstDefinition;
use syntax_trees::item::DataDefinition;
use syntax_trees::item::DataMember;
use syntax_trees::item::Item;
use syntax_trees::item::ProofFact;
use syntax_trees::item::TypeParameterKind;
use syntax_trees::types::FixedArrayLength;
use syntax_trees::types::TypeReferenceHandle;
use syntax_trees::types::TypeReferenceNode;

pub(in crate::preparation::generic_data) fn desugar_generic_data_instances(
    syntax: &mut SyntaxTrees,
    warnings: &mut Vec<Diagnostic>,
) -> Result<(), Vec<Diagnostic>> {
    let selection = if syntax
        .root_items()
        .any(|item| matches!(item, Item::Module(_)))
    {
        Some(
            crate::preparation::declaration_selection::ConstantSelection::new(
                syntax,
                None,
                Vec::new(),
            )?,
        )
    } else {
        None
    };
    super::domain_heads::normalize(syntax, selection.as_ref())?;
    desugar_generic_data_instances_with_selection(syntax, warnings, selection.as_ref(), &[])
}

pub(in crate::preparation) fn desugar_generic_data_instances_with_selection(
    syntax: &mut SyntaxTrees,
    warnings: &mut Vec<Diagnostic>,
    selection: Option<&crate::preparation::declaration_selection::ConstantSelection>,
    closed_roots: &[TypeReferenceHandle],
) -> Result<(), Vec<Diagnostic>> {
    desugar_generic_data_instances_in(
        syntax,
        warnings,
        selection,
        GenericDataWorkDomain::All {
            additional_type_roots: closed_roots,
        },
    )
}

pub(crate) fn desugar_generic_data_instances_in(
    syntax: &mut SyntaxTrees,
    warnings: &mut Vec<Diagnostic>,
    selection: Option<&crate::preparation::declaration_selection::ConstantSelection>,
    domain: GenericDataWorkDomain<'_>,
) -> Result<(), Vec<Diagnostic>> {
    let work = SynthesisWork::new(syntax, domain);
    // Attachments belong to the carrier selected in their declaring source,
    // not to every generic declaration with the same leaf spelling.
    let mut attached_machines: HashMap<syntax_trees::item::ItemHandle, Vec<usize>> = HashMap::new();
    for (item_index, item) in syntax.root_items().enumerate() {
        if let Some(machine) = item.as_machine()
            && let Some(attached) = &machine.attached_data
            && let Some(declaration) = selected_data_item(syntax, selection, attached)
        {
            attached_machines
                .entry(declaration)
                .or_default()
                .push(item_index);
        }
    }

    let mut generic_data: HashMap<syntax_trees::item::ItemHandle, GenericData> = HashMap::new();
    for &declaration in syntax.root_item_handles() {
        let Some(definition) = syntax.root_item(declaration).as_data() else {
            continue;
        };
        if definition.type_parameters.is_empty() {
            continue;
        }
        // Quotient identity is semantic, not record shape. This temporary
        // record/container synthesizer cannot yet substitute the quotient's
        // carrier, relation, and exact Equivalence selection as one unit.
        // Leaving the generic quotient nominally visible lets the downstream
        // quotient fences reject representation literals; synthesizing an
        // empty record with `quotient: None` would make the class freely
        // constructible.
        if definition.quotient.is_some() {
            continue;
        }
        // These compiler-owned proof algebras never acquire runtime layout.
        // Keep their generic argument structurally visible so checked content
        // plans retain a normalized coordinate-space/unit identity instead of
        // collapsing it into a synthesized diagnostic spelling. The exemption
        // covers only the unmoduled declarations a bare algebra spelling can
        // select; a module-owned declaration with the same leaf is ordinary
        // user data reachable through its qualified or imported path, and it
        // normalizes as a template like any other module record.
        if matches!(
            definition.name.as_str(syntax),
            "IntervalSet" | "CountedQuantity"
        ) && super::module_constants::module_path(
            syntax,
            definition.name.source_span().source_id,
        )
        .is_none()
        {
            continue;
        }
        let definition_parameters = syntax
            .tables
            .items
            .type_parameters(definition.type_parameters);
        // Value indices retain descriptor fields installed from their binder
        // telescope during lowering. This type/const synthesis erases that
        // telescope and cannot represent those descriptors; preserve their
        // ordinary indexed application instead. Treating a value slot as a
        // type slot could also accept a type where an index was required.
        // Machine-symbol parameters likewise retain their template identity.
        if definition_parameters.iter().any(|parameter| {
            matches!(
                parameter.kind,
                TypeParameterKind::Value { .. } | TypeParameterKind::Machine { .. }
            )
        }) {
            continue;
        }
        if attached_machines.contains_key(&declaration) {
            // A CONTAINER (generic data with attached machines) monomorphizes
            // ONLY when every method's own type parameters are covered by the
            // data's parameter names (T-on-method matching T-on-data --
            // decision: per-instance mono, instances always spelled). The
            // instance clones each method with T substituted (Phase 2 slice
            // 1), so `self.b.stored()` on a `Box<i32>` field resolves against
            // a CONCRETE machine and the T-typed value call materializes
            // (was the runtime silent-0). An uncovered method leaves the
            // whole container for the type-check-only path, as before.
            let data_parameters: Vec<(String, bool)> = syntax
                .tables
                .items
                .type_parameters(definition.type_parameters)
                .iter()
                .map(|parameter| {
                    (
                        parameter.name.as_str(syntax).to_string(),
                        matches!(parameter.kind, TypeParameterKind::Const { .. }),
                    )
                })
                .collect();
            let all_methods_covered = attached_machines[&declaration].iter().all(|&item_index| {
                let Some(Item::Machine(machine)) = syntax.root_items().nth(item_index) else {
                    return false;
                };
                // An empty body is an ordinary implementation, including a
                // no-op Unit cleanup hook. Source admission owns missing-body
                // and return obligations; body length cannot decide whether
                // the carrier or its attached machines may specialize.
                syntax
                    .tables
                    .items
                    .type_parameters(machine.type_parameters)
                    .iter()
                    .all(|parameter| {
                        // A runtime-capable value binder counts as
                        // const-shaped here: it can only lawfully pair
                        // with a data `const` binder (a static argument),
                        // never with a data type binder.
                        let method_is_const = parameter.kind.is_const_or_value();
                        data_parameters.iter().any(|(name, data_is_const)| {
                            name == parameter.name.as_str(syntax)
                                && *data_is_const == method_is_const
                        })
                    })
            });
            if !all_methods_covered {
                continue;
            }
        }
        let parameter_names = definition_parameters
            .iter()
            .map(|parameter| parameter.name.as_str(syntax).to_string())
            .collect::<Vec<_>>();
        let const_parameter_types = definition_parameters
            .iter()
            .map(|parameter| match parameter.kind {
                TypeParameterKind::Const { type_reference } => Some(type_reference),
                _ => None,
            })
            .collect();
        generic_data.insert(
            declaration,
            GenericData {
                declaration,
                name: definition.name.as_str(syntax).to_owned(),
                origin_name: definition.name.clone(),
                is_public: definition.is_public,
                lifetime_parameters: definition.lifetime_parameters.clone(),
                parameter_names,
                const_parameter_types,
                cleanup_selection: definition.cleanup_selection.clone(),
                where_facts: definition.where_facts,
                type_equations: super::equations::classify_type_equations(
                    syntax,
                    definition_parameters,
                    syntax.tables.items.proof_facts(definition.where_facts),
                ),
                members: definition.members,
                properties: definition.properties,
                supply_mode: definition.supply_mode,
            },
        );
    }
    // Const-v0 declarations disappear during symbol resolution, but generic
    // data instances are selected before that stage. Retain every declaration
    // for structured-value canonicalization and the integer subset separately
    // for the existing expression/fact evaluator.
    let const_definitions: HashMap<String, ConstDefinition> = syntax
        .root_items()
        .filter_map(|item| {
            let Item::Const(definition) = item else {
                return None;
            };
            if super::module_constants::is_module_constant(syntax, definition) {
                return None;
            }
            Some((qualified_const_name(syntax, definition), definition.clone()))
        })
        .collect();
    let const_values = super::module_constants::lexical_integer_const_values(syntax);

    // A `Carrier::Domain<indices>` type spells the same declared application
    // as the constrained `Carrier in Domain<indices>` node. Rewrite every
    // reachable head BEFORE the closed-index canonicalization snapshot so a
    // module constant inside one folds exactly as it does in an authored
    // constraint, and every later read -- solving, identity, naming,
    // substitution -- observes the one canonical shape. Generic template
    // members are open positions whose substituted copies reach the fixpoint
    // below, so they are normalized while the template is still authored.
    let mut domain_head_positions =
        work.type_positions(syntax, TypePositionPurpose::Identity, true);
    for item in syntax.root_items().filter(|_| work.is_all()) {
        let Some(definition) = item.as_data() else {
            continue;
        };
        if definition.type_parameters.is_empty() {
            continue;
        }
        for member in syntax.items.data_members(definition.members) {
            match member {
                DataMember::Field(field) => super::collect_type_positions(
                    syntax,
                    field.type_reference,
                    &mut domain_head_positions,
                    true,
                ),
                DataMember::Variant(variant) => {
                    for field in syntax.items.data_payload_fields(variant.payload) {
                        super::collect_type_positions(
                            syntax,
                            field.type_reference,
                            &mut domain_head_positions,
                            true,
                        );
                    }
                }
                DataMember::Retired(_) => {}
            }
        }
    }
    normalize_domain_head_positions(syntax, selection, &domain_head_positions);

    canonicalize_closed_domain_indices(
        syntax,
        &const_definitions,
        &const_values,
        selection,
        warnings,
        &work,
    )
    .map_err(|diagnostic| vec![diagnostic])?;

    if generic_data.is_empty() {
        return Ok(());
    }
    // FIXPOINT. Each round scans every type-reference position for a
    // `Base<Args..>` spelling, synthesizes one concrete record per new distinct
    // spelling, and rewrites the spellings to the instances' plain names. A
    // NESTED generic (`Pair<T> { a: Box<T> }` used as `Pair<i32>`) synthesizes a
    // `Pair<i32>` record whose `a` field is a fresh `Box<i32>` spelling -- picked
    // up and monomorphized by the NEXT round. Only inline storage extends the
    // authored storage roster. A substituted reference describes identity,
    // not a layout demand, regardless of const binders, predicates or methods.
    // Every discovered application still prepares its complete argument tuple;
    // default-domain predicates are established by value construction/use,
    // not by naming a possibly uninhabited application.
    // Completion can add closed type owners after an earlier pass. Reuse
    // declarations by retained template/argument identity, never by the name
    // generated for them, and never clone their attached methods twice.
    let mut synthesized = Vec::new();
    for &declaration in syntax.root_item_handles() {
        let Some(definition) = syntax.root_item(declaration).as_data() else {
            continue;
        };
        let Some(origin) = definition.generic_instance else {
            continue;
        };
        let Some(ClosedArgumentIdentity::Instance(template, argument_identity)) =
            super::closed_argument_identity(syntax, selection, origin, false)
        else {
            return Err(vec![Diagnostic::error(
                "existing generic instance lost its exact template and argument identity",
            )]);
        };
        let TypeReferenceNode::Generic { arguments, .. } =
            syntax.type_references.type_reference(origin)
        else {
            return Err(vec![Diagnostic::error(
                "existing generic instance lost its original application structure",
            )]);
        };
        synthesized.push(Instantiation {
            synthetic_name: definition.name.as_str(syntax).to_owned(),
            declaration,
            template,
            argument_handles: syntax
                .type_references
                .type_reference_handles(*arguments)
                .to_vec(),
            argument_identity,
        });
    }
    loop {
        // Scoped work cannot normalize an authored template in place. Its
        // substituted declaration descendants acquire the same normal form
        // here, before this round inspects their closed applications.
        if !work.is_all() {
            let domain_positions = work.type_positions(syntax, TypePositionPurpose::Identity, true);
            normalize_domain_head_positions(syntax, selection, &domain_positions);
            canonicalize_closed_domain_indices(
                syntax,
                &const_definitions,
                &const_values,
                selection,
                warnings,
                &work,
            )
            .map_err(|diagnostic| vec![diagnostic])?;
        }
        let storage_positions =
            work.type_positions(syntax, TypePositionPurpose::Materialization, false);
        let mut rewrites: Vec<PendingRewrite> = Vec::new();
        let mut instantiations: Vec<Instantiation> = Vec::new();
        for position in work.type_positions(syntax, TypePositionPurpose::Identity, false) {
            let Some(application) = prepare_closed_application(
                syntax,
                &generic_data,
                &const_definitions,
                &const_values,
                selection,
                position,
                warnings,
            )
            .map_err(|diagnostic| vec![diagnostic])?
            else {
                continue;
            };
            let prior = synthesized
                .iter()
                .chain(instantiations.iter())
                .find(|instance| {
                    instance.template == application.template
                        && instance.argument_identity == application.argument_identity
                });
            if prior.is_none() && !storage_positions.contains(&position) {
                continue;
            }
            // Identity-only uses must still rejoin a declaration already
            // demanded elsewhere. Leaving a raw application beside its closed
            // spelling splits constructor/call matching into two routes.
            let synthetic_name = if let Some(prior) = prior {
                prior.synthetic_name.clone()
            } else {
                let display = format!(
                    "{}<{}>",
                    generic_data[&application.template].name,
                    application.argument_names.join(", ")
                );
                if synthesized
                    .iter()
                    .chain(instantiations.iter())
                    .any(|instance| instance.synthetic_name == display)
                {
                    format!("{display}__{}", synthesized.len() + instantiations.len())
                } else {
                    display
                }
            };
            let lookup_name = if let Some(selection) = selection {
                let Some(path) = selection.data_lookup_path(syntax, &application.base_name) else {
                    continue;
                };
                if let Some((prefix, _)) = path.rsplit_once("::") {
                    format!("{prefix}::{synthetic_name}")
                } else {
                    synthetic_name.clone()
                }
            } else {
                synthetic_name.clone()
            };
            rewrites.push(PendingRewrite {
                type_reference: position,
                synthetic_name: lookup_name,
                lifetime_arguments: application.lifetime_arguments,
            });
            if prior.is_none() {
                instantiations.push(Instantiation {
                    synthetic_name,
                    declaration: Handle::invalid(),
                    template: application.template,
                    argument_handles: application.argument_handles,
                    argument_identity: application.argument_identity,
                });
            }
        }
        if rewrites.is_empty() {
            break; // no more monomorphizable generic spellings
        }
        // Synthesize each not-yet-built instance: the base's members cloned with
        // the type parameters substituted for the arguments.
        if !instantiations.is_empty() {
            // Discovery closed every argument before this round copies any
            // declaration. Templates and attached bodies share that immutable
            // source; each instance still owns its copied expression graph.
            let snapshot = syntax.clone();
            for instance in &instantiations {
                synthesize_instance(
                    syntax,
                    &snapshot,
                    warnings,
                    &mut synthesized,
                    &generic_data,
                    &attached_machines,
                    &const_values,
                    selection,
                    instance,
                )?;
            }
        }

        // Rewrite this round's spellings to the synthesized instances' plain names.
        for rewrite in rewrites {
            // Keep occurrence custody separately from the instance's shared
            // derivation. Public/private exposure belongs to this use site.
            let original = syntax
                .tables
                .type_references
                .type_reference(rewrite.type_reference)
                .clone();
            let TypeReferenceNode::Generic { base_name, .. } = &original else {
                continue;
            };
            let instance_name = Identifier::new(rewrite.synthetic_name, base_name.source_span());
            let application = syntax.tables.type_references.insert(original);
            syntax
                .tables
                .type_references
                .retain_generic_application_origin(rewrite.type_reference, application);
            let rewritten = if rewrite.lifetime_arguments.is_empty() {
                TypeReferenceNode::Named(instance_name)
            } else {
                TypeReferenceNode::Generic {
                    base_name: instance_name,
                    lifetime_arguments: rewrite.lifetime_arguments,
                    arguments: HandleSpan::empty(),
                }
            };
            syntax
                .tables
                .type_references
                .replace_type_reference(rewrite.type_reference, rewritten);
        }
    }

    let declarations = work.declarations(syntax);
    relabel_closed_data_uses_in_constants(syntax, &synthesized, selection, &declarations);
    relabel_closed_data_uses_in_exact_assignments(syntax, &synthesized, selection, &declarations);
    relabel_closed_data_uses_in_exact_calls_and_returns(
        syntax,
        &synthesized,
        selection,
        &declarations,
    );
    relabel_closed_sum_memberships_from_local_types(syntax, &synthesized, selection, &declarations);

    normalize_generic_template_const_expressions(syntax, &const_values, warnings, &declarations)
        .map_err(|diagnostic| vec![diagnostic])?;
    Ok(())
}

/// Build one closed instance of a generic data declaration, unless an
/// earlier round already built the same template and argument tuple: the
/// template's members, facts and attached machines with its type and
/// `const` parameters substituted, as a new concrete record.
fn synthesize_instance(
    syntax: &mut SyntaxTrees,
    snapshot: &SyntaxTrees,
    warnings: &mut Vec<Diagnostic>,
    synthesized: &mut Vec<Instantiation>,
    generic_data: &HashMap<syntax_trees::item::ItemHandle, GenericData>,
    attached_machines: &HashMap<syntax_trees::item::ItemHandle, Vec<usize>>,
    const_values: &HashMap<String, i128>,
    selection: Option<&crate::preparation::declaration_selection::ConstantSelection>,
    instance: &Instantiation,
) -> Result<(), Vec<Diagnostic>> {
    if synthesized.iter().any(|prior| {
        prior.template == instance.template && prior.argument_identity == instance.argument_identity
    }) {
        return Ok(());
    }
    let base_info = &generic_data[&instance.template];
    let substitution: HashMap<String, TypeReferenceHandle> = base_info
        .parameter_names
        .iter()
        .cloned()
        .zip(instance.argument_handles.iter().copied())
        .collect();
    let const_parameter_values: HashMap<String, i128> = base_info
        .parameter_names
        .iter()
        .zip(&base_info.const_parameter_types)
        .filter_map(|(name, parameter_type)| {
            parameter_type.as_ref()?;
            let argument = substitution.get(name)?;
            let value = syntax
                .tables
                .type_references
                .type_reference(*argument)
                .as_named()?;
            Some((name.clone(), value.as_str(syntax).parse().ok()?))
        })
        .collect();
    let const_parameter_type_names: HashMap<String, String> = base_info
        .parameter_names
        .iter()
        .zip(&base_info.const_parameter_types)
        .filter_map(|(name, parameter_type)| {
            let parameter_type = parameter_type.as_ref()?;
            let type_name = syntax
                .tables
                .type_references
                .type_reference(*parameter_type)
                .as_named()?;
            Some((name.clone(), type_name.as_str(syntax).to_string()))
        })
        .collect();
    let mut const_expressions: HashMap<String, ExpressionNode> = const_parameter_values
        .iter()
        .map(|(name, value)| {
            use numerics::literals::{IntegerLanding, LandedIntegerType};
            // Substitution closes a declared value; it does not turn that
            // value back into an anonymous literal with no operator home.
            // Typed fact evaluation must still select against this carrier.
            let Some(landed_type) = const_parameter_type_names
                .get(name)
                .map(String::as_str)
                .and_then(LandedIntegerType::from_name)
            else {
                return Err(vec![Diagnostic::error(format!(
                    "const parameter `{name}` has no supported integer carrier for substitution"
                ))]);
            };
            let literal = IntegerLiteral::from_parts(
                *value < 0,
                IntegerRadix::Decimal,
                value.unsigned_abs().to_string().as_str(),
            )
            .expect("a concrete const argument is a valid decimal integer literal")
            .with_landing(IntegerLanding {
                landed_type,
                domain: numerics::arithmetic::ArithmeticDomain::Exact,
            });
            Ok((name.clone(), ExpressionNode::Integer(literal)))
        })
        .collect::<Result<_, Vec<Diagnostic>>>()?;
    // Structural matching selects Boolean indices without turning them
    // into integer endpoints. Synthesis must retain that same value in
    // constructor facts and attached bodies; leaving the binder symbolic
    // would lose the concrete predicate owed at value establishment.
    let mut const_parameter_scalars = const_parameter_values
        .iter()
        .map(|(name, value)| (name.clone(), ConstScalarValue::Integer(*value)))
        .collect::<HashMap<_, _>>();
    for (name, parameter_type) in base_info
        .parameter_names
        .iter()
        .zip(&base_info.const_parameter_types)
    {
        if parameter_type.is_none() {
            continue;
        }
        let Some(argument) = substitution.get(name) else {
            continue;
        };
        let Some(value) = syntax.type_references.type_reference(*argument).as_named() else {
            continue;
        };
        if let Some(value) =
            crate::preparation::type_equations::normalized_boolean_argument(value.as_str(syntax))
        {
            const_parameter_scalars.insert(name.clone(), ConstScalarValue::Boolean(value));
            const_expressions.insert(name.clone(), ExpressionNode::Boolean(value));
        }
    }

    // Data predicates define a default domain, which need not be inhabited.
    // Omit a proven truth, but retain false and mixed predicates with the same
    // substitution as members. Construction and observation own establishment;
    // discovering a contradiction must not make a type declaration illegal.
    let type_watermark = syntax.type_references.node_count();
    let fact_expression_watermark = syntax.expressions.frontier();
    let mut first_fact = Handle::invalid();
    let mut fact_count = 0u32;
    for (offset, fact) in snapshot
        .tables
        .items
        .proof_facts(base_info.where_facts)
        .iter()
        .enumerate()
    {
        // A type equation was decided against this instance's complete
        // argument tuple when the spelling was admitted; a verified
        // equation is a discharged instantiation obligation.
        if base_info
            .type_equations
            .iter()
            .any(|equation| equation.fact_offset == offset)
        {
            continue;
        }
        let fact_warning_start = warnings.len();
        let const_result = match fact {
            ProofFact::Expression(expression) => evaluate_const_fact_expression(
                snapshot,
                *expression,
                const_values,
                &const_parameter_scalars,
                selection,
                warnings,
            )
            .map(|value| match value {
                Some(ConstFactValue::Boolean(value)) => Some(value),
                _ => None,
            }),
            ProofFact::Membership(membership) => evaluate_const_membership_fact(
                snapshot,
                membership,
                const_values,
                &const_parameter_values,
                &const_parameter_type_names,
                selection,
                warnings,
            ),
        }
        .map_err(|reason| {
            vec![Diagnostic::error(format!(
                "const fact for generic instance `{}` is invalid: {reason}",
                instance.synthetic_name
            ))]
        })?;
        if const_result == Some(true) {
            continue;
        }
        warnings.truncate(fact_warning_start);
        let source = Handle::from_parts(
            base_info
                .where_facts
                .start()
                .arena_index()
                .checked_add(u32::try_from(offset).expect("proof fact offset overflow"))
                .expect("proof fact source handle overflow"),
            base_info.where_facts.start().generation(),
        );
        let handle = syntax.copy_proof_fact_from(snapshot, source);
        if fact_count == 0 {
            first_fact = handle;
        }
        fact_count += 1;
    }
    let where_facts = HandleSpan::from_parts(first_fact, fact_count);

    // Retain the authored generic application as structural evidence.
    // The synthesized display name is diagnostic-only; downstream
    // identity and substitution resolve this base and argument tuple.
    let origin_arguments = syntax
        .tables
        .type_references
        .insert_type_reference_handles(instance.argument_handles.iter().copied());
    let generic_instance =
        syntax
            .tables
            .type_references
            .insert(syntax_trees::types::TypeReferenceNode::Generic {
                base_name: base_info.origin_name.clone(),
                lifetime_arguments: Vec::new(),
                arguments: origin_arguments,
            });

    let members: Vec<DataMember> = syntax.tables.items.data_members(base_info.members).to_vec();
    let properties = base_info.properties;
    // CASE-CONSTRAINTS type-equality facts (`case ... where T == i32`)
    // decide against the closed argument identities this instance was
    // deduplicated on. Only `type` binders enter the map: `const`
    // binders rewrite to literal arguments, and the remaining binder
    // kinds are still refused upstream.
    let type_identities: HashMap<String, ClosedArgumentIdentity> = {
        let Some(template) = snapshot.root_item(instance.template).as_data() else {
            unreachable!("a discovered template is a data item");
        };
        snapshot
            .tables
            .items
            .type_parameters(template.type_parameters)
            .iter()
            .zip(instance.argument_identity.iter())
            .filter(|(parameter, _)| matches!(parameter.kind, TypeParameterKind::Type))
            .map(|(parameter, identity)| {
                (parameter.name.as_str(syntax).to_owned(), identity.clone())
            })
            .collect()
    };
    let mut first: Handle<DataMember> = Handle::invalid();
    let mut count = 0u32;
    for member in members {
        let substituted = substitute_member(
            syntax,
            snapshot,
            member,
            &substitution,
            &type_identities,
            selection,
            &instance.synthetic_name,
            const_values,
            &const_parameter_scalars,
            warnings,
        )
        .map_err(|diagnostic| vec![diagnostic])?;
        let handle = syntax.tables.items.append_data_member(substituted);
        if count == 0 {
            first = handle;
        }
        count += 1;
    }
    let copied_types = syntax
        .type_references
        .named_nodes_from(type_watermark, syntax);
    substitute_copied_type_names(syntax, copied_types, &substitution);
    // Run the const-binder literal rewrite only after member
    // substitution: a carried case `where` fact is copied inside the
    // loop above, so its `const` mentions land in the same rewritten
    // window as the data-level fact copies.
    replace_const_expression_names_from(syntax, fact_expression_watermark, &const_expressions);
    let cleanup_selection = base_info.cleanup_selection.clone().map(|selection| {
        super::substitution::substitute_cleanup_selection(
            syntax,
            snapshot,
            selection,
            &substitution,
            &const_parameter_values,
            warnings,
        )
    });
    let declaration = syntax.push_root_item(Item::Data(DataDefinition {
        // The closed instance is compiler-generated, but its mandatory
        // derivation origin is the exact authored generic declaration.
        // Retain that span under the synthetic semantic spelling so
        // package ownership never falls back to an unresolved name.
        name: Identifier::new(
            instance.synthetic_name.as_str(),
            base_info.origin_name.source_span(),
        ),
        is_public: base_info.is_public,
        supply_mode: base_info.supply_mode,
        lifetime_parameters: base_info.lifetime_parameters.clone(),
        type_parameters: HandleSpan::default(),
        generic_instance: Some(generic_instance),
        cleanup_selection,
        properties,
        where_facts,
        members: HandleSpan::from_parts(first, count),
        quotient: None,
    }));

    let mut materialized = instance.clone();
    materialized.declaration = declaration;
    synthesized.push(materialized);

    // CONTAINER instance: clone each attached machine with the type
    // parameters substituted (Phase 2 slice 1). The clone copies from
    // a SNAPSHOT of the tree (same-tree deep copies need a & source
    // while appending into &mut tables), then a WATERMARK pass
    // rewrites `Named(T)` nodes created by the copy -- only the
    // clone's own subtree is younger than the watermark.
    let Some(machine_items) = attached_machines.get(&instance.template) else {
        return Ok(());
    };
    for &item_index in machine_items {
        let Some(Item::Machine(machine)) = snapshot.root_items().nth(item_index) else {
            continue;
        };
        let type_watermark = syntax.tables.type_references.node_count();
        let expression_watermark = syntax.expressions.frontier();
        let Item::Machine(mut clone) =
            syntax.copy_item_from(snapshot, &Item::Machine(machine.clone()))
        else {
            continue;
        };
        // The clone is CONCRETE: attached to the synthetic record,
        // its type parameters cleared, its `Named(T)` type nodes
        // substituted with the instance arguments. The machine NAME
        // is the FULL parsed path ("Box::stored"), so the attached
        // segment is rewritten there too ("Box<i32>::stored") --
        // machine identity keys on the composed name.
        let method_tail = machine
            .name
            .as_str(syntax)
            .rsplit("::")
            .next()
            .unwrap_or(machine.name.as_str(syntax))
            .to_string();
        clone.name = Identifier::new(
            format!("{}::{}", instance.synthetic_name, method_tail),
            machine.name.source_span(),
        );
        // An attachment may be authored outside the carrier's module.
        // Reuse selected carrier lookup metadata in that source context;
        // retaining only the span would lose an imported qualification.
        clone.attached_data = Some(
            closed_constructor_carrier(
                snapshot,
                selection,
                instance,
                machine.attached_data.as_ref().expect("selected attachment"),
                false,
            )
            .ok_or_else(|| {
                vec![
                    Diagnostic::error(
                        "generic method lost its selected closed carrier lookup context",
                    )
                    .with_source_span(machine.name.source_span()),
                ]
            })?,
        );
        clone.generic_data_template = machine.name.clone();
        clone.type_parameters = HandleSpan::default();
        let runtime_captures = capture_machine_runtime_template_names(
            syntax,
            &clone,
            type_watermark,
            expression_watermark,
        );
        let copied_types = syntax
            .tables
            .type_references
            .named_nodes_from(type_watermark, syntax)
            .into_iter()
            // Runtime subjects must reach ordinary admission, not acquire a
            // template const value. The same type-copy law still applies.
            .filter(|(handle, name)| {
                !const_expressions.contains_key(name)
                    || !runtime_captures.captures_type_reference(*handle)
            });
        substitute_copied_type_names(syntax, copied_types, &substitution);
        for (handle, element_type, name) in syntax
            .tables
            .type_references
            .const_parameter_array_nodes_from(type_watermark, syntax)
        {
            let Some(length) = substitution.get(&name).and_then(|argument| {
                match syntax.tables.type_references.type_reference(*argument) {
                    TypeReferenceNode::Named(value) => value.as_str(syntax).parse::<usize>().ok(),
                    _ => None,
                }
            }) else {
                continue;
            };
            syntax.tables.type_references.replace_type_reference(
                handle,
                TypeReferenceNode::FixedArray {
                    element_type,
                    length: FixedArrayLength::Literal(length),
                },
            );
        }
        replace_machine_const_expression_names_from(
            syntax,
            expression_watermark,
            &const_expressions,
            &runtime_captures,
        );
        syntax.push_root_item(Item::Machine(clone));
    }
    Ok(())
}
