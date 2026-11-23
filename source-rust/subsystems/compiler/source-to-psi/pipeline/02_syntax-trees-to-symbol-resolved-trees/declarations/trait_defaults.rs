/*
    Pre-resolution synthesis for trait machines with authored bodies, and for
    the sealed core `Equatable<T>` row.

    A named implementation may omit a requirement whose trait supplies a
    default body. Its closed block receives each omitted default as its own
    member row before symbol resolution, so typing, effects, proofs, dispatch
    and both execution engines reuse the ordinary member path.

    A closed `Name satisfies Equatable<T> { }` block that authors no `equals`
    member receives a compiler-emitted `equals` member whose
    body compares its two operands with `==`; typed lowering expands that
    comparison structurally for `T` (core equality acquisition in
    `wiki/spec/language/conformances.md`). The row belongs to the named
    implementation, never to `T`.

    Trait identity joins by exact source selection through the
    shared constant-header table, never by leaf spelling: same-leaf module
    declarations cannot share a default template, and an ambiguous authored
    trait name defers to ordinary resolution instead of guessing an owner.
    Package-aware callers retain the source map and loader-certified import
    bindings: rebuilding a source-free header table could merge same-path
    declarations from different packages during namespace/value validation.
    Synthesized references carry the selected declaration's logical path (or
    the authored occurrence itself) so complete resolution rejoins the same
    declaration. Each instantiated body and its helpers keep their own
    declaration selections even when their tokens came from the same default.
*/

use crate::resolution::declarations::declaration_selection::ConstantSelection;
use crate::resolution::tables::syntax_items;
use crate::resolution::tables::syntax_type_references;
#[cfg(any(test, feature = "test-support"))]
use crate::source_inputs::SourceMap;
#[cfg(any(test, feature = "test-support"))]
use crate::source_symbols::SourceScopedTopLevelBinding;
use arena::HandleSpan;
use diagnostics::Diagnostic;
use std::collections::{HashMap, HashSet};
#[cfg(any(test, feature = "test-support"))]
use std::sync::Arc;
use symbols::SymbolHandle;
use syntax_trees::SyntaxTrees;
use syntax_trees::expression::{
    BinaryOperator, ExpressionHandle, ExpressionNode, TableBinaryExpression,
};
use syntax_trees::identifier::Identifier;
use syntax_trees::item::{
    ConformanceItem, ConformanceMember, Item, ItemHandle, Machine, SatisfiesClause, State,
    StateSignature,
};
use syntax_trees::statement::StatementNode;
use syntax_trees::types::{TypeReferenceHandle, TypeReferenceNode};

#[derive(Clone)]
struct TraitDefaultsInput {
    /*
        Logical namespace path of the selected trait declaration. Synthesized
        references spell this path so they rejoin the exact owner instead of a
        same-leaf competitor.
    */
    path: String,
    lifetime_parameters: Vec<Identifier>,
    parameter_names: Vec<String>,
    signatures: Vec<TraitSignatureInput>,
    requirements: Vec<TraitRequirementInput>,
}

#[derive(Clone)]
struct TraitSignatureInput {
    ordinal: usize,
    signature: StateSignature,
}

#[derive(Clone)]
struct TraitRequirementInput {
    /*
        The exact required trait selected in the declaring trait's own source
        context. An invalid handle means the requirement name did not select
        one trait; ordinary resolution owns that diagnostic.
    */
    symbol: SymbolHandle,
    arguments: Vec<TypeReferenceHandle>,
}

#[derive(Clone)]
struct ConformanceInput {
    handle: ItemHandle,
    declaration: ConformanceItem,
}

#[derive(Clone)]
struct RequirementInstance {
    declaring_trait: SymbolHandle,
    requirement_ordinal: usize,
    signature: StateSignature,
    substitution: HashMap<String, TypeReferenceHandle>,
}

#[cfg(any(test, feature = "test-support"))]
pub fn synthesize_trait_defaults(
    syntax: &mut SyntaxTrees,
    sources: Option<Arc<SourceMap>>,
    bindings: Vec<SourceScopedTopLevelBinding>,
    retained_base: Option<&crate::source_resolved::SymbolResolvedTrees>,
) -> Result<(), Vec<Diagnostic>> {
    let selection =
        ConstantSelection::with_retained_base(syntax, sources, bindings, retained_base)?;
    crate::resolution::declarations::generic_data::domain_heads::normalize(
        syntax,
        Some(&selection),
    )?;
    synthesize_trait_defaults_with_selection(syntax, &selection)
}

pub(crate) fn synthesize_trait_defaults_with_selection(
    syntax: &mut SyntaxTrees,
    selection: &ConstantSelection,
) -> Result<(), Vec<Diagnostic>> {
    let traits = syntax
        .root_items()
        .filter_map(|item| {
            let Item::Trait(trait_definition) = item else {
                return None;
            };
            let symbol = selection
                .trait_lookup(syntax, &trait_definition.name)
                .unique()?;
            let parameter_names = syntax_items(syntax)
                .type_parameters(trait_definition.type_parameters)
                .iter()
                .map(|parameter| parameter.name.as_str(syntax).to_string())
                .collect();
            let signatures = syntax_items(syntax)
                .state_signatures(trait_definition.machines)
                .iter()
                .enumerate()
                .map(|(ordinal, handle)| TraitSignatureInput {
                    ordinal,
                    signature: syntax.items.state_signature(*handle).clone(),
                })
                .collect::<Vec<_>>();
            let mut requirements = Vec::new();
            let requirement_symbol = |name: &Identifier| {
                selection
                    .trait_lookup(syntax, name)
                    .unique()
                    .unwrap_or_else(SymbolHandle::invalid)
            };
            for handle in
                syntax_type_references(syntax).type_reference_handles(trait_definition.parents)
            {
                match syntax.type_references.type_reference(*handle) {
                    TypeReferenceNode::Named(name) => requirements.push(TraitRequirementInput {
                        symbol: requirement_symbol(name),
                        arguments: Vec::new(),
                    }),
                    TypeReferenceNode::Generic {
                        base_name,
                        arguments,
                        ..
                    } => requirements.push(TraitRequirementInput {
                        symbol: requirement_symbol(base_name),
                        arguments: syntax_type_references(syntax)
                            .type_reference_handles(*arguments)
                            .to_vec(),
                    }),
                    _ => {}
                }
            }
            requirements.extend(
                syntax_items(syntax)
                    .identifier_path_members(trait_definition.requires)
                    .iter()
                    .map(|name| TraitRequirementInput {
                        symbol: requirement_symbol(name),
                        arguments: Vec::new(),
                    }),
            );
            Some((
                symbol,
                TraitDefaultsInput {
                    path: selection.declaration_path(symbol),
                    lifetime_parameters: trait_definition.lifetime_parameters.clone(),
                    parameter_names,
                    signatures,
                    requirements,
                },
            ))
        })
        .collect::<HashMap<_, _>>();

    inherit_refinement_telescopes(syntax, selection, &|symbol| traits.get(&symbol))?;

    let conformances = syntax
        .root_item_handles()
        .iter()
        .filter_map(|handle| match syntax.root_item(*handle) {
            Item::Conformance(conformance) => Some(ConformanceInput {
                handle: *handle,
                declaration: conformance.clone(),
            }),
            _ => None,
        })
        .collect::<Vec<_>>();

    let mut diagnostics = Vec::new();
    for conformance in conformances {
        let Some(trait_symbol) = selection
            .trait_lookup(syntax, &conformance.declaration.trait_name)
            .unique()
        else {
            continue;
        };
        let trait_name = conformance
            .declaration
            .trait_name
            .as_str(syntax)
            .to_string();
        let arguments = syntax_type_references(syntax)
            .type_reference_handles(conformance.declaration.trait_arguments)
            .to_vec();
        let Some(conformed_trait) = traits.get(&trait_symbol) else {
            continue;
        };
        let conformance_name = conformance
            .declaration
            .alias
            .as_ref()
            .expect("parsed named implementations carry their name")
            .as_str(syntax)
            .to_string();
        if arguments.len() != conformed_trait.parameter_names.len() {
            diagnostics.push(Diagnostic::error(format!(
            "conformance `{conformance_name} satisfies {trait_name}` expects {} generic argument(s), got {}",
            conformed_trait.parameter_names.len(),
            arguments.len()
        )));
            continue;
        }
        let substitution = conformed_trait
            .parameter_names
            .iter()
            .cloned()
            .zip(arguments.iter().copied())
            .collect::<HashMap<_, _>>();

        let mut closed_members = syntax_items(syntax)
            .conformance_members(conformance.declaration.members)
            .to_vec();
        let mut requirements = Vec::new();
        collect_requirement_instances(
            syntax,
            trait_symbol,
            &substitution,
            &traits,
            &mut HashSet::new(),
            &mut HashSet::new(),
            &mut requirements,
        );
        let mut existing_defaults = HashSet::new();
        for member in &closed_members {
            if let ConformanceMember::TraitDefault {
                declaring_trait,
                requirement_ordinal,
                ..
            } = member
            {
                existing_defaults.insert((
                    declaring_trait.as_str(syntax).to_string(),
                    *requirement_ordinal,
                ));
            }
        }

        let mut added = false;
        for requirement in requirements {
            let declaring_path = traits
                .get(&requirement.declaring_trait)
                .map(|definition| definition.path.clone())
                .unwrap_or_else(|| selection.declaration_path(requirement.declaring_trait));
            let key = (declaring_path.clone(), requirement.requirement_ordinal);
            if existing_defaults.contains(&key) || requirement.signature.default_body.is_none() {
                continue;
            }
            let signature = instantiate_default_signature(
                syntax,
                &requirement.signature,
                &requirement.substitution,
                conformance.handle,
                &[],
            );
            let exact_requirement_owner = traits
                .get(&requirement.declaring_trait)
                .filter(|definition| {
                    definition.lifetime_parameters.is_empty()
                        && definition.parameter_names.is_empty()
                })
                .map(|definition| definition.path.as_str());
            let machine = machine_from_signature(
                syntax,
                implementation_member_name(&conformance.declaration, signature.name.as_str(syntax)),
                &signature,
                exact_requirement_owner,
                Identifier::generated(declaring_path.as_str()),
            );
            closed_members.push(ConformanceMember::TraitDefault {
                declaring_trait: Identifier::generated(declaring_path),
                requirement_ordinal: requirement.requirement_ordinal,
                machine,
            });
            existing_defaults.insert(key);
            added = true;
        }
        if selection.header_symbols().sealed_intrinsic(trait_symbol)
            == Some(language_core::sealed_intrinsics::SealedIntrinsic::EqualityProtocol)
            && let Some(member) = synthesized_equality_member(
                syntax,
                &conformance.declaration,
                conformed_trait,
                &closed_members,
                &substitution,
                conformance.handle,
            )
        {
            closed_members.push(member);
            added = true;
        }
        if added {
            replace_closed_members(
                syntax,
                conformance.handle,
                conformance.declaration,
                closed_members,
            );
        }
    }

    if diagnostics.is_empty() {
        Ok(())
    } else {
        Err(diagnostics)
    }
}

// A targeted postcondition uses the selected base requirement's telescope.
// Copy its types under this refinement's actuals before resolving its names.
fn inherit_refinement_telescopes<'catalog>(
    syntax: &mut SyntaxTrees,
    selection: &ConstantSelection,
    selected_trait: &impl Fn(SymbolHandle) -> Option<&'catalog TraitDefaultsInput>,
) -> Result<(), Vec<Diagnostic>> {
    let refinements = syntax
        .root_item_handles()
        .iter()
        .filter_map(|handle| {
            let Item::Trait(declaration) = syntax.root_item(*handle) else {
                return None;
            };
            declaration
                .refines
                .filter(|_| {
                    declaration
                        .refinement_clauses
                        .iter()
                        .any(|clause| !clause.signature.contracts.is_empty())
                })
                .map(|base| (*handle, declaration.clone(), base))
        })
        .collect::<Vec<_>>();
    for (handle, mut refinement, base) in refinements {
        let (name, arguments, lifetimes) = match syntax.type_references.type_reference(base) {
            TypeReferenceNode::Named(name) => (name.clone(), Vec::new(), Vec::new()),
            TypeReferenceNode::Generic {
                base_name,
                arguments,
                lifetime_arguments,
            } => (
                base_name.clone(),
                syntax_type_references(syntax)
                    .type_reference_handles(*arguments)
                    .to_vec(),
                lifetime_arguments.clone(),
            ),
            _ => {
                return Err(vec![Diagnostic::error(
                    "a refinement postcondition needs a selected base trait",
                )]);
            }
        };
        let base = selection
            .trait_lookup(syntax, &name)
            .unique()
            .and_then(selected_trait)
            .ok_or_else(|| {
                vec![Diagnostic::error(
                    "a refinement postcondition lost its selected base declaration",
                )]
            })?;
        if arguments.len() != base.parameter_names.len()
            || lifetimes.len() != base.lifetime_parameters.len()
        {
            return Err(vec![Diagnostic::error(
                "a refinement postcondition needs the complete base trait application",
            )]);
        }
        let substitution = base
            .parameter_names
            .iter()
            .cloned()
            .zip(arguments)
            .collect::<HashMap<_, _>>();
        let lifetimes = base
            .lifetime_parameters
            .iter()
            .zip(lifetimes)
            .map(|(formal, actual)| (formal.as_str(syntax).to_owned(), actual))
            .collect::<Vec<_>>();
        for clause in &mut refinement.refinement_clauses {
            if clause.signature.contracts.is_empty() {
                continue;
            }
            let Some(requirement) = &clause.requirement else {
                return Err(vec![Diagnostic::error(
                    "a refinement postcondition names one exact base requirement",
                )]);
            };
            let matching = base
                .signatures
                .iter()
                .filter(|candidate| {
                    candidate.signature.name.as_str(syntax) == requirement.as_str(syntax)
                })
                .collect::<Vec<_>>();
            let [selected] = matching.as_slice() else {
                return Err(vec![Diagnostic::error(
                    "a refinement postcondition must select exactly one base requirement",
                )]);
            };
            let inherited = instantiate_default_signature(
                syntax,
                &selected.signature,
                &substitution,
                handle,
                &lifetimes,
            );
            clause.signature.parameters = inherited.parameters;
            clause.signature.type_parameters = inherited.type_parameters;
            clause.signature.lifetime_parameters = inherited.lifetime_parameters;
            clause.signature.conformance_bounds = inherited.conformance_bounds;
            clause.signature.native_callback_parameters = inherited.native_callback_parameters;
            clause.signature.return_type = inherited.return_type;
        }
        syntax.items.replace_item(handle, Item::Trait(refinement));
    }
    Ok(())
}

/*
    Collect every exact requirement identity in the inherited closure together
    with the generic substitution at this conformance. Closed conformances keep
    exact `(declaring trait, requirement)` rows, so unlike bodyless attached
    lookup this deliberately does not collapse or shadow same-leaf names.
*/
fn collect_requirement_instances(
    syntax: &mut SyntaxTrees,
    owner: SymbolHandle,
    substitution: &HashMap<String, TypeReferenceHandle>,
    traits: &HashMap<SymbolHandle, TraitDefaultsInput>,
    visiting: &mut HashSet<SymbolHandle>,
    seen: &mut HashSet<(SymbolHandle, usize)>,
    output: &mut Vec<RequirementInstance>,
) {
    if !visiting.insert(owner) {
        return;
    }
    let Some(trait_definition) = traits.get(&owner) else {
        visiting.remove(&owner);
        return;
    };

    for declaration in &trait_definition.signatures {
        let key = (owner, declaration.ordinal);
        if seen.insert(key) {
            output.push(RequirementInstance {
                declaring_trait: owner,
                requirement_ordinal: declaration.ordinal,
                signature: declaration.signature.clone(),
                substitution: substitution.clone(),
            });
        }
    }

    for requirement in &trait_definition.requirements {
        let Some(required_trait) = traits.get(&requirement.symbol) else {
            continue;
        };
        if requirement.arguments.len() != required_trait.parameter_names.len() {
            continue;
        }
        let required_arguments = if substitution.is_empty() || requirement.arguments.is_empty() {
            requirement.arguments.clone()
        } else {
            let snapshot = syntax.clone();
            let arguments = supplied_arguments(syntax, &snapshot, substitution, ItemHandle::invalid());
            syntax.copy_type_arguments_with_bindings(&snapshot, &requirement.arguments, arguments)
        };
        let required_substitution = required_trait
            .parameter_names
            .iter()
            .cloned()
            .zip(required_arguments)
            .collect::<HashMap<_, _>>();
        collect_requirement_instances(
            syntax,
            requirement.symbol,
            &required_substitution,
            traits,
            visiting,
            seen,
            output,
        );
    }

    visiting.remove(&owner);
}

fn replace_closed_members(
    syntax: &mut SyntaxTrees,
    handle: ItemHandle,
    mut conformance: ConformanceItem,
    members: Vec<ConformanceMember>,
) {
    let mut start = arena::Handle::invalid();
    let mut count = 0u32;
    for member in members {
        let member = syntax.items.append_conformance_member(member);
        if count == 0 {
            start = member;
        }
        count = count
            .checked_add(1)
            .expect("conformance member span count overflow");
    }
    conformance.members = HandleSpan::from_parts_or_empty(start, count);
    syntax
        .items
        .replace_item(handle, Item::Conformance(conformance));
}

/*
    The root trait whose closed implementations acquire a synthesized
    structural `equals` row. Only the authenticated bundled declaration
    grants synthesis; a user trait with the same name remains ordinary code.
*/

/*
    The `equals` member a closed `Equatable<T>` implementation receives when it
    authors no `equals` member for it: the trait's
    requirement signature instantiated at `T`, with the body `left == right`
    over its two parameters. Typed lowering expands that comparison
    structurally and routes every use of this row to the same expansion. A
    trait without exactly one type parameter and one two-parameter `equals`
    requirement synthesizes nothing; ordinary row completeness reports it.
*/
fn synthesized_equality_member(
    syntax: &mut SyntaxTrees,
    declaration: &ConformanceItem,
    equatable: &TraitDefaultsInput,
    members: &[ConformanceMember],
    substitution: &HashMap<String, TypeReferenceHandle>,
    supplying_declaration: ItemHandle,
) -> Option<ConformanceMember> {
    const EQUALS: &str = "equals";
    if equatable.parameter_names.len() != 1
        || members.iter().any(|member| match member {
            ConformanceMember::Machine(machine) => machine.name.as_str(syntax) == EQUALS,
            ConformanceMember::Selection { requirement, .. } => {
                requirement.as_str(syntax) == EQUALS
            }
            ConformanceMember::Synthesized(_) => true,
            ConformanceMember::TraitDefault { .. } => false,
        })
    {
        return None;
    }
    let requirement = equatable
        .signatures
        .iter()
        .map(|declaration| &declaration.signature)
        .find(|signature| signature.name.as_str(syntax) == EQUALS && signature.default_body.is_none())?;
    if syntax.items.state_parameters(requirement.parameters).len() != 2 {
        return None;
    }
    let mut signature = instantiate_default_signature(
        syntax,
        requirement,
        substitution,
        supplying_declaration,
        &[],
    );
    let [left, right] = syntax.items.state_parameters(signature.parameters) else {
        return None;
    };
    let [left, right] = [*left, *right].map(|parameter| {
        let name =
            Identifier::generated(syntax.items.state_parameter(parameter).name.as_str(syntax));
        let member = syntax.expressions.append_identifier_path_member(name);
        syntax.expressions.insert(ExpressionNode::Name {
            members: HandleSpan::from_parts(member, 1),
            application: None,
        })
    });
    let equality = syntax
        .expressions
        .insert(ExpressionNode::Binary(TableBinaryExpression {
            policy: numerics::arithmetic::ArithmeticDomain::Exact,
            negation: false,
            left,
            operator: BinaryOperator::Equal,
            right,
        }));
    let statement = syntax
        .statements
        .insert(StatementNode::Expression(equality));
    let statement = syntax.items.append_statement_handle(statement);
    signature.default_body = Some(syntax_trees::item::MachineBody {
        statements: HandleSpan::from_parts(statement, 1),
        nested_machines: HandleSpan::empty(),
    });
    Some(ConformanceMember::Synthesized(machine_from_signature(
        syntax,
        implementation_member_name(declaration, EQUALS),
        &signature,
        None,
        Identifier::default(),
    )))
}

/*
    A member the compiler adds to a named implementation is named at the
    implementation's own name occurrence. That occurrence gives the row its
    package provenance and exact declaration owner: a generated name would
    belong to no package that may select it, and Terminal production could not
    identify the machine it emits for the row.
*/
fn implementation_member_name(declaration: &ConformanceItem, member: &str) -> Identifier {
    declaration.alias.as_ref().map_or_else(
        || Identifier::generated(member),
        |alias| Identifier::new(member, alias.source_span()),
    )
}

/*
    Copy one default for one implementation, binding the trait's generic
    parameters to that implementation's arguments. The copy includes the body,
    so each instantiation owns its type nodes while its authored identifiers
    keep their source custody.
*/
fn instantiate_default_signature(
    syntax: &mut SyntaxTrees,
    signature: &StateSignature,
    substitution: &HashMap<String, TypeReferenceHandle>,
    supplying_declaration: ItemHandle,
    lifetimes: &[(String, Identifier)],
) -> StateSignature {
    if substitution.is_empty() && lifetimes.is_empty() && signature.default_body.is_none() {
        return signature.clone();
    }
    let snapshot = syntax.clone();
    let arguments = supplied_arguments(syntax, &snapshot, substitution, supplying_declaration);
    syntax.copy_state_signature_with_arguments(&snapshot, signature, arguments, lifetimes)
}

fn supplied_arguments(
    syntax: &mut SyntaxTrees,
    snapshot: &SyntaxTrees,
    substitution: &HashMap<String, TypeReferenceHandle>,
    supplying_declaration: ItemHandle,
) -> Vec<syntax_trees::SuppliedSyntaxArgument> {
    let mut substitutions = substitution.iter().collect::<Vec<_>>();
    substitutions.sort_unstable_by_key(|(name, _)| *name);
    let mut arguments = Vec::with_capacity(substitutions.len());
    for (binder, actual) in substitutions {
        let ty = syntax.copy_type_reference_handle(snapshot, *actual);
        if supplying_declaration.is_valid() {
            let applications = syntax
                .type_references
                .type_alias_applications(ty)
                .map(|row| row.application)
                .collect::<Vec<_>>();
            for operand in applications.into_iter().chain([ty]) {
                if matches!(syntax.type_references.supplied_operand_owner(operand), Ok(owner) if !owner.is_valid())
                {
                    syntax
                        .type_references
                        .retain_supplied_operand_owner(operand, supplying_declaration);
                }
            }
        }
        arguments.push(syntax_trees::SuppliedSyntaxArgument {
            binder: binder.clone(),
            ty,
        });
    }
    arguments
}

/*
    `default_body_trait` names the trait that authored `signature`'s body. The
    body keeps that trait's lexical scope for bare requirement names; it is
    empty for compiler-written bodies such as synthesized structural equality.
*/
fn machine_from_signature(
    syntax: &mut SyntaxTrees,
    machine_name: Identifier,
    signature: &StateSignature,
    requirement_owner: Option<&str>,
    default_body_trait: Identifier,
) -> Machine {
    let body = signature
        .default_body
        .expect("selected implementation has a body");
    let state = State {
        name: Identifier::generated(signature.name.as_str(syntax)),
        parameters: signature.parameters,
        return_type: signature.return_type,
        contracts: HandleSpan::empty(),
        statements: body.statements,
    };
    let state = syntax.items.insert_state(&state);
    let state = syntax.items.append_state_handle(state);
    let satisfies = requirement_owner.map_or_else(HandleSpan::empty, |trait_path| {
        let requirement = Identifier::generated(signature.name.as_str(syntax));
        let clause = syntax.items.append_satisfies_clause(SatisfiesClause {
            trait_name: Identifier::generated(trait_path),
            lifetime_arguments: Vec::new(),
            arguments: HandleSpan::empty(),
            requirement: Some(requirement),
            alias: None,
            via: None,
            via_expression: ExpressionHandle::invalid(),
            via_keyword_source_span: None,
        });
        HandleSpan::from_parts(clause, 1)
    });
    let ordinary_parameters = syntax
        .items
        .type_parameters(signature.type_parameters)
        .iter()
        .filter(|parameter| {
            !matches!(
                parameter.kind,
                syntax_trees::item::TypeParameterKind::Implementation
            )
        })
        .cloned()
        .collect::<Vec<_>>();
    let mut type_parameters = HandleSpan::empty();
    for parameter in ordinary_parameters {
        let handle = syntax.items.append_type_parameter(parameter);
        type_parameters.push_contiguous(handle);
    }
    Machine {
        target_family_interface: false,
        name: machine_name,
        default_body_trait,
        attached_data: None,
        spelling: None,
        is_public: false,
        bodyless: false,
        target: None,
        authored_target: None,
        boundary: false,
        generic_data_template: Default::default(),
        lifetime_parameters: signature.lifetime_parameters.clone(),
        type_parameters,
        satisfies,
        conformance_bounds: signature.conformance_bounds.clone(),
        terminates_guarantee: signature.terminates_guarantee,
        ranking_subjects: HandleSpan::<ExpressionHandle>::empty(),
        ranking_view: HandleSpan::empty(),
        ranking_view_arguments: HandleSpan::empty(),
        ranking_range: ExpressionHandle::invalid(),
        service_reach_is_installation_bound: false,
        service_reach_keyword_source_spans: signature.service_reach_keyword_source_spans.clone(),
        service_reaches: signature.service_reaches,
        invokes: signature.invokes,
        suspends: signature.suspends,
        suspends_keyword_source_spans: signature.suspends_keyword_source_spans.clone(),
        blocks: signature.blocks,
        blocks_keyword_source_spans: signature.blocks_keyword_source_spans.clone(),
        admitted_same_stack_bytes: 0,
        declares_no_crash_route: signature.declares_no_crash_route,
        contracts: signature.contracts,
        states: HandleSpan::from_parts(state, 1),
        nested_machines: body.nested_machines,
    }
}
