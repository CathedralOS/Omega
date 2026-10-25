//! Pre-resolution synthesis for trait machines with authored bodies, and for
//! the sealed core `Equatable<T>` row.
//!
//! A named implementation may omit a requirement whose trait supplies a
//! default body. Its closed block receives each omitted default as its own
//! member row before symbol resolution, so typing, effects, proofs, dispatch
//! and both execution engines reuse the ordinary member path.
//!
//! A closed `Name satisfies Equatable<T> { }` block that authors no `equals`
//! member or reference row receives a compiler-emitted `equals` member whose
//! body compares its two operands with `==`; typed lowering expands that
//! comparison structurally for `T` (core equality acquisition in
//! `wiki/spec/language/conformances.md`). The row belongs to the named
//! implementation, never to `T`.
//!
//! Trait identity joins by exact source selection through the
//! shared constant-header table, never by leaf spelling: same-leaf module
//! declarations cannot share a default template, and an ambiguous authored
//! trait name defers to ordinary resolution instead of guessing an owner.
//! Package-aware callers retain the source map and loader-certified import
//! bindings: rebuilding a source-free header table could merge same-path
//! declarations from different packages during namespace/value validation.
//! Synthesized references carry the selected declaration's logical path (or
//! the authored occurrence itself) so complete resolution rejoins the same
//! declaration; the compiler-derived selection partition still requires
//! generated spellings, so requirement edges use qualified paths that resolve
//! inside the declaration's package instead of falling back to a same-leaf
//! competitor.

use arena::HandleSpan;
use diagnostics::Diagnostic;
use source::SourceMap;
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use symbols::{SourceScopedTopLevelBinding, SymbolHandle};
use syntax_trees::SyntaxTrees;
use syntax_trees::expression::{
    BinaryOperator, ExpressionHandle, ExpressionNode, StaticMachineArgument,
    StaticSymbolApplication, TableBinaryExpression,
};
use syntax_trees::identifier::Identifier;
use syntax_trees::item::{
    ConformanceItem, ConformanceMember, Item, ItemHandle, Machine, SatisfiesClause, State,
    StateSignatureNode,
};
use syntax_trees::statement::StatementNode;
use syntax_trees::types::{FixedArrayLength, TypeReferenceHandle, TypeReferenceNode};

#[derive(Clone)]
struct TraitDefaultsInput {
    /// Logical namespace path of the selected trait declaration. Synthesized
    /// references spell this path so they rejoin the exact owner instead of a
    /// same-leaf competitor.
    path: String,
    has_lifetime_parameters: bool,
    parameter_names: Vec<String>,
    signatures: Vec<TraitSignatureInput>,
    requirements: Vec<TraitRequirementInput>,
}

#[derive(Clone)]
struct TraitSignatureInput {
    ordinal: usize,
    signature: StateSignatureNode,
}

#[derive(Clone)]
struct TraitRequirementInput {
    /// The exact required trait selected in the declaring trait's own source
    /// context. An invalid handle means the requirement name did not select
    /// one trait; ordinary resolution owns that diagnostic.
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
    signature: StateSignatureNode,
    substitution: HashMap<String, TypeReferenceHandle>,
}

pub fn synthesize_trait_defaults(
    syntax: &mut SyntaxTrees,
    sources: Option<Arc<SourceMap>>,
    bindings: Vec<SourceScopedTopLevelBinding>,
) -> Result<(), Vec<Diagnostic>> {
    let selection = crate::preparation::declaration_selection::ConstantSelection::new(
        syntax, sources, bindings,
    )?;
    crate::preparation::generic_data::domain_heads::normalize(syntax, Some(&selection))?;
    crate::preparation::module_normalization::validate_with_selection(syntax, &selection)?;
    synthesize_trait_defaults_after_module_validation(syntax, &selection)
}

/// Resolution has already checked the complete namespace frontier in its
/// current mode. Rechecking in complete-value mode would reject the deliberately
/// unevaluated scalar declarations used only for initializer selection evidence.
/// The shared selection supplies exact trait identity: every name this
/// pass follows was already admitted by the caller's namespace validation.
pub(crate) fn synthesize_trait_defaults_after_module_validation(
    syntax: &mut SyntaxTrees,
    selection: &crate::preparation::declaration_selection::ConstantSelection,
) -> Result<(), Vec<Diagnostic>> {
    let traits = syntax
        .root_items()
        .filter_map(|item| {
            let Item::Trait(trait_definition) = item else {
                return None;
            };
            // A trait's own name selects itself in its declaring source; an
            // absent or ambiguous selection cannot safely back defaults and
            // belongs to ordinary resolution's diagnostic instead.
            let symbol = selection
                .trait_lookup(syntax, &trait_definition.name)
                .unique()?;
            let parameter_names = syntax
                .items
                .type_parameters(trait_definition.type_parameters)
                .iter()
                .map(|parameter| parameter.name.as_str(syntax).to_string())
                .collect();
            let signatures = syntax
                .items
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
            for handle in syntax
                .type_references
                .type_reference_handles(trait_definition.parents)
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
                        arguments: syntax
                            .type_references
                            .type_reference_handles(*arguments)
                            .to_vec(),
                    }),
                    _ => {}
                }
            }
            requirements.extend(
                syntax
                    .items
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
                    has_lifetime_parameters: !trait_definition.lifetime_parameters.is_empty(),
                    parameter_names,
                    signatures,
                    requirements,
                },
            ))
        })
        .collect::<HashMap<_, _>>();

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
            // Absent and ambiguous trait selections are ordinary resolution
            // diagnostics; synthesizing from either would attach an arbitrary
            // namespace's defaults.
            continue;
        };
        let trait_name = conformance
            .declaration
            .trait_name
            .as_str(syntax)
            .to_string();
        let arguments = syntax
            .type_references
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

        let mut closed_members = syntax
            .items
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
            if existing_defaults.contains(&key) || !requirement.signature.is_default {
                continue;
            }
            let signature = instantiate_default_signature(
                syntax,
                &requirement.signature,
                &requirement.substitution,
                conformance.handle,
            );
            let machine = machine_from_signature(
                syntax,
                implementation_member_name(&conformance.declaration, signature.name.as_str(syntax)),
                &signature,
                exact_unargumented_requirement_owner(&traits, requirement.declaring_trait),
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
        if selection.declaration_path(trait_symbol) == EQUATABLE_TRAIT_PATH
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

/// Collect every exact requirement identity in the inherited closure together
/// with the generic substitution at this conformance. Closed conformances keep
/// exact `(declaring trait, requirement)` rows, so unlike bodyless attached
/// lookup this deliberately does not collapse or shadow same-leaf names.
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
        let required_arguments = requirement
            .arguments
            .iter()
            .map(|argument| substitute_type_reference(syntax, *argument, substitution))
            .collect::<Vec<_>>();
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

/// The root trait whose closed implementations acquire a synthesized
/// structural `equals` row. Selection is by the exact declaration path, so a
/// module-scoped trait that merely shares the leaf name gains no privilege.
const EQUATABLE_TRAIT_PATH: &str = "Equatable";

/// The `equals` member a closed `Equatable<T>` implementation receives when it
/// authors neither an `equals` member nor a reference row for it: the trait's
/// requirement signature instantiated at `T`, with the body `left == right`
/// over its two parameters. Typed lowering expands that comparison
/// structurally and routes every use of this row to the same expansion. A
/// trait without exactly one type parameter and one two-parameter `equals`
/// requirement synthesizes nothing; ordinary row completeness reports it.
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
            ConformanceMember::Reference { requirement, .. } => {
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
        .find(|signature| signature.name.as_str(syntax) == EQUALS && !signature.is_default)?;
    if syntax.items.state_parameters(requirement.parameters).len() != 2 {
        return None;
    }
    let mut signature =
        instantiate_default_signature(syntax, requirement, substitution, supplying_declaration);
    let [left, right] = syntax.items.state_parameters(signature.parameters) else {
        return None;
    };
    let [left, right] = [*left, *right].map(|parameter| {
        let name =
            Identifier::generated(syntax.items.state_parameter(parameter).name.as_str(syntax));
        let member = syntax.expressions.append_identifier_path_member(name);
        syntax
            .expressions
            .insert(ExpressionNode::Name(HandleSpan::from_parts(member, 1)))
    });
    let equality = syntax
        .expressions
        .insert(ExpressionNode::Binary(TableBinaryExpression {
            left,
            operator: BinaryOperator::Equal,
            right,
        }));
    let statement = syntax
        .statements
        .insert(StatementNode::Expression(equality));
    let statement = syntax.items.append_statement_handle(statement);
    signature.default_body = HandleSpan::from_parts(statement, 1);
    Some(ConformanceMember::Synthesized(machine_from_signature(
        syntax,
        implementation_member_name(declaration, EQUALS),
        &signature,
        None,
        Identifier::default(),
    )))
}

/// A member the compiler adds to a named implementation is named at the
/// implementation's own name occurrence. That occurrence gives the row its
/// package provenance and exact declaration owner: a generated name would
/// belong to no package that may select it, and Terminal production could not
/// identify the machine it emits for the row.
fn implementation_member_name(declaration: &ConformanceItem, member: &str) -> Identifier {
    declaration.alias.as_ref().map_or_else(
        || Identifier::generated(member),
        |alias| Identifier::new(member, alias.source_span()),
    )
}

fn substitute_type_reference(
    syntax: &mut SyntaxTrees,
    handle: TypeReferenceHandle,
    substitution: &HashMap<String, TypeReferenceHandle>,
) -> TypeReferenceHandle {
    let node = syntax.type_references.type_reference(handle).clone();
    match node {
        TypeReferenceNode::Named(name) => substitution
            .get(name.as_str(syntax))
            .copied()
            .unwrap_or(handle),
        TypeReferenceNode::Reference {
            referee,
            access,
            lifetime,
        } => {
            let substituted = substitute_type_reference(syntax, referee, substitution);
            if substituted == referee {
                handle
            } else {
                syntax
                    .type_references
                    .insert_reference_with_lifetime(substituted, access, lifetime)
            }
        }
        TypeReferenceNode::Constrained {
            base_type,
            constraints,
        } => {
            let substituted = substitute_type_reference(syntax, base_type, substitution);
            if substituted == base_type {
                handle
            } else {
                syntax
                    .type_references
                    .insert_constrained(substituted, constraints)
            }
        }
        TypeReferenceNode::FixedArray {
            element_type,
            length,
        } => {
            let substituted_element = substitute_type_reference(syntax, element_type, substitution);
            let substituted_length = match &length {
                FixedArrayLength::ConstParameter(name) => substitution
                    .get(name.as_str(syntax))
                    .and_then(
                        |argument| match syntax.type_references.type_reference(*argument) {
                            TypeReferenceNode::Named(value) => value.as_str(syntax).parse().ok(),
                            _ => None,
                        },
                    )
                    .map(FixedArrayLength::Literal)
                    .unwrap_or_else(|| length.clone()),
                _ => length.clone(),
            };
            if substituted_element == element_type && substituted_length.same_value(&length, syntax)
            {
                handle
            } else {
                syntax
                    .type_references
                    .insert_fixed_array(substituted_element, substituted_length)
            }
        }
        TypeReferenceNode::Slice { element_type } => {
            let substituted = substitute_type_reference(syntax, element_type, substitution);
            if substituted == element_type {
                handle
            } else {
                syntax.type_references.insert_slice(substituted)
            }
        }
        TypeReferenceNode::Generic {
            base_name,
            arguments,
            lifetime_arguments,
        } => {
            let original = syntax
                .type_references
                .type_reference_handles(arguments)
                .to_vec();
            let substituted = original
                .iter()
                .map(|argument| substitute_type_reference(syntax, *argument, substitution))
                .collect::<Vec<_>>();
            if substituted == original {
                handle
            } else {
                let arguments = syntax
                    .type_references
                    .insert_type_reference_handles(substituted);
                syntax.type_references.insert(TypeReferenceNode::Generic {
                    base_name,
                    lifetime_arguments,
                    arguments,
                })
            }
        }
        // A selector's trait arguments are ordinary type positions: a default
        // instantiated at `T = Card` turns `dyn Ranked<T>` into
        // `dyn Ranked<Card>`.
        TypeReferenceNode::DynamicTrait {
            trait_name,
            trait_arguments,
        } => {
            let original = syntax
                .type_references
                .type_reference_handles(trait_arguments)
                .to_vec();
            let substituted = original
                .iter()
                .map(|argument| substitute_type_reference(syntax, *argument, substitution))
                .collect::<Vec<_>>();
            if substituted == original {
                handle
            } else {
                let trait_arguments = syntax
                    .type_references
                    .insert_type_reference_handles(substituted);
                syntax
                    .type_references
                    .insert(TypeReferenceNode::DynamicTrait {
                        trait_name,
                        trait_arguments,
                    })
            }
        }
        TypeReferenceNode::ConstExpression(_) | TypeReferenceNode::Unit => handle,
    }
}

/// Copy one default for one implementation, binding the trait's generic
/// parameters to that implementation's arguments. The copy includes the body,
/// so each instantiation owns its type nodes while its authored identifiers
/// keep their source custody.
fn instantiate_default_signature(
    syntax: &mut SyntaxTrees,
    signature: &StateSignatureNode,
    substitution: &HashMap<String, TypeReferenceHandle>,
    supplying_declaration: ItemHandle,
) -> StateSignatureNode {
    if substitution.is_empty() {
        return signature.clone();
    }
    let snapshot = syntax.clone();
    let type_watermark = syntax.type_references.node_count();
    let copied = syntax.copy_state_signature_node_from(&snapshot, signature);
    for (handle, name) in syntax
        .type_references
        .named_nodes_from(type_watermark, syntax)
    {
        let Some(argument) = substitution.get(&name) else {
            continue;
        };
        // A supplied subtree belongs to the conformance telescope, not the
        // copied requirement's invocation telescope. Deep copying keeps nested
        // operand custody while leaving fresh surrounding wrappers untouched.
        let supplied = syntax.copy_type_reference_handle(&snapshot, *argument);
        let replacement = syntax.type_references.type_reference(supplied).clone();
        syntax
            .type_references
            .replace_type_reference(handle, replacement);
        let origin = syntax.type_references.generic_application_origin(supplied);
        if origin.is_valid() {
            syntax
                .type_references
                .retain_generic_application_origin(handle, origin);
        }
        syntax
            .type_references
            .retain_copied_operand_owner(supplied, handle);
        if matches!(
            syntax.type_references.supplied_operand_owner(supplied),
            Ok(owner) if !owner.is_valid()
        ) {
            syntax
                .type_references
                .retain_supplied_operand_owner(handle, supplying_declaration);
        }
    }
    // Type nodes above cover signatures. A default body also names a telescope
    // binder as a static call argument (`Reading::floor<Reader>()`). That
    // argument is a declaration path, not a type node, so the same substitution
    // has to rewrite it or the copy still spells the trait's binder.
    substitute_static_binder_arguments(syntax, copied.default_body, substitution);
    copied
}

/// Replace trait-telescope binders used as static arguments with the
/// conformance's actuals. A named declaration, an application, or an integer
/// const becomes that actual. Any other actual keeps the authored path, and
/// resolution reports it.
fn substitute_static_binder_arguments(
    syntax: &mut SyntaxTrees,
    statements: HandleSpan<syntax_trees::statement::StatementHandle>,
    substitution: &HashMap<String, TypeReferenceHandle>,
) {
    if substitution.is_empty() || statements.is_empty() {
        return;
    }
    let handles = syntax.items.statements(statements).to_vec();
    let mut seen = HashSet::new();
    for handle in handles {
        substitute_statement_static_arguments(syntax, handle, substitution, &mut seen);
    }
}

fn substitute_statement_static_arguments(
    syntax: &mut SyntaxTrees,
    statement: syntax_trees::statement::StatementHandle,
    substitution: &HashMap<String, TypeReferenceHandle>,
    seen: &mut HashSet<ExpressionHandle>,
) {
    use syntax_trees::statement::{TransitionGuardNode, TransitionTargetNode};
    let node = syntax.statements.statement(statement).clone();
    match node {
        StatementNode::Call(mut call) => {
            if let Some(arguments) =
                rewritten_static_arguments(syntax, &call.machine_arguments, substitution)
            {
                call.machine_arguments = arguments.into_boxed_slice();
                syntax
                    .statements
                    .replace_statement(statement, StatementNode::Call(call.clone()));
            }
            for argument in syntax
                .statements
                .expression_handles(call.arguments)
                .to_vec()
            {
                substitute_expression_static_arguments(syntax, argument, substitution, seen);
            }
        }
        StatementNode::Expression(expression) => {
            substitute_expression_static_arguments(syntax, expression, substitution, seen);
        }
        StatementNode::Assignment(assignment) => {
            substitute_expression_static_arguments(syntax, assignment.target, substitution, seen);
            substitute_expression_static_arguments(syntax, assignment.value, substitution, seen);
        }
        StatementNode::LocalData(local) => {
            substitute_expression_static_arguments(syntax, local.initial_value, substitution, seen);
        }
        StatementNode::ProofOutputBindingStatement(binding) => {
            substitute_expression_static_arguments(syntax, binding.call, substitution, seen);
        }
        StatementNode::AssemblyFact(fact) => {
            substitute_expression_static_arguments(syntax, fact.expression, substitution, seen);
        }
        StatementNode::RootBinding(binding) => {
            substitute_expression_static_arguments(syntax, binding.receiver, substitution, seen);
            if binding.implementation_operand.is_valid() {
                substitute_expression_static_arguments(
                    syntax,
                    binding.implementation_operand,
                    substitution,
                    seen,
                );
            }
        }
        StatementNode::Transition(transition) => {
            if let TransitionGuardNode::When(guard) = transition.guard {
                substitute_expression_static_arguments(syntax, guard, substitution, seen);
            }
            for target in transition.targets() {
                match syntax.statements.transition_target(target).clone() {
                    TransitionTargetNode::Named { arguments, .. } => {
                        for argument in syntax.statements.expression_handles(arguments).to_vec() {
                            substitute_expression_static_arguments(
                                syntax,
                                argument,
                                substitution,
                                seen,
                            );
                        }
                    }
                    TransitionTargetNode::Value(value) => {
                        substitute_expression_static_arguments(syntax, value, substitution, seen);
                    }
                    TransitionTargetNode::Terminal => {}
                }
            }
        }
        StatementNode::Assembly(handle) => {
            let instructions = syntax.statements.assembly(handle).blocks;
            let blocks = syntax.statements.assembly_blocks(instructions).to_vec();
            for block in blocks {
                for expression in syntax
                    .statements
                    .assembly_expressions(block.requires)
                    .iter()
                    .chain(syntax.statements.assembly_expressions(block.ensures))
                    .copied()
                    .collect::<Vec<_>>()
                {
                    substitute_expression_static_arguments(syntax, expression, substitution, seen);
                }
                for instruction in syntax
                    .statements
                    .assembly_instructions(block.instructions)
                    .to_vec()
                {
                    substitute_statement_static_arguments(syntax, instruction, substitution, seen);
                }
            }
        }
    }
}

fn substitute_expression_static_arguments(
    syntax: &mut SyntaxTrees,
    expression: ExpressionHandle,
    substitution: &HashMap<String, TypeReferenceHandle>,
    seen: &mut HashSet<ExpressionHandle>,
) {
    if !expression.is_valid() || !seen.insert(expression) {
        return;
    }
    let node = syntax.expressions.expression(expression).clone();
    match node {
        ExpressionNode::Call(mut call) => {
            if let Some(arguments) =
                rewritten_static_arguments(syntax, &call.machine_arguments, substitution)
            {
                call.machine_arguments = arguments.into_boxed_slice();
                syntax
                    .expressions
                    .replace_expression(expression, ExpressionNode::Call(call.clone()));
            }
            substitute_expression_static_arguments(syntax, call.receiver, substitution, seen);
            for argument in syntax
                .expressions
                .expression_handles(call.arguments)
                .to_vec()
            {
                substitute_expression_static_arguments(syntax, argument, substitution, seen);
            }
        }
        ExpressionNode::Match(dispatch) => {
            substitute_expression_static_arguments(syntax, dispatch.subject, substitution, seen);
            let arms = syntax.expressions.match_arms(dispatch.arms).to_vec();
            for arm in arms {
                let patterns = syntax
                    .expressions
                    .match_pattern_dependencies(arm.pattern)
                    .collect::<Vec<_>>();
                for pattern in patterns {
                    substitute_expression_static_arguments(syntax, pattern, substitution, seen);
                }
                substitute_expression_static_arguments(syntax, arm.value, substitution, seen);
            }
        }
        ExpressionNode::SavedSubject(saved) => {
            substitute_expression_static_arguments(
                syntax,
                syntax.expressions.match_subject(saved.capture).subject,
                substitution,
                seen,
            );
            substitute_expression_static_arguments(syntax, saved.pattern_head, substitution, seen);
        }
        ExpressionNode::ArrayLiteral(expressions) => {
            for child in syntax.expressions.expression_handles(expressions).to_vec() {
                substitute_expression_static_arguments(syntax, child, substitution, seen);
            }
        }
        ExpressionNode::Atomic(atomic) => {
            substitute_expression_static_arguments(syntax, atomic.value, substitution, seen);
            substitute_expression_static_arguments(syntax, atomic.result, substitution, seen);
        }
        ExpressionNode::Binary(binary) => {
            substitute_expression_static_arguments(syntax, binary.left, substitution, seen);
            substitute_expression_static_arguments(syntax, binary.right, substitution, seen);
        }
        ExpressionNode::Cast(cast) => {
            substitute_expression_static_arguments(syntax, cast.value, substitution, seen);
        }
        ExpressionNode::Indexed(indexed) => {
            substitute_expression_static_arguments(syntax, indexed.collection, substitution, seen);
            substitute_expression_static_arguments(syntax, indexed.index, substitution, seen);
        }
        ExpressionNode::Membership(membership) => {
            substitute_expression_static_arguments(syntax, membership.value, substitution, seen);
        }
        ExpressionNode::Member(member) => {
            substitute_expression_static_arguments(syntax, member.receiver, substitution, seen);
        }
        ExpressionNode::Borrow(inner) => {
            substitute_expression_static_arguments(syntax, inner.target, substitution, seen);
        }
        ExpressionNode::Range(range) => {
            substitute_expression_static_arguments(syntax, range.start, substitution, seen);
            substitute_expression_static_arguments(syntax, range.end, substitution, seen);
        }
        ExpressionNode::StructLiteral(literal) => {
            let fields = syntax.expressions.struct_fields(literal.fields).to_vec();
            for field in fields {
                substitute_expression_static_arguments(syntax, field.value, substitution, seen);
            }
        }
        ExpressionNode::Unary(unary) => {
            substitute_expression_static_arguments(syntax, unary.operand, substitution, seen);
        }
        ExpressionNode::Boolean(_)
        | ExpressionNode::Float(_)
        | ExpressionNode::UndeterminedNaN(_)
        | ExpressionNode::Integer(_)
        | ExpressionNode::Name(_)
        | ExpressionNode::String(_)
        | ExpressionNode::ZeroValue(_)
        | ExpressionNode::TypeExpression(_) => {}
    }
}

fn rewritten_static_arguments(
    syntax: &SyntaxTrees,
    arguments: &[StaticMachineArgument],
    substitution: &HashMap<String, TypeReferenceHandle>,
) -> Option<Vec<StaticMachineArgument>> {
    let mut changed = false;
    let mut output = Vec::with_capacity(arguments.len());
    for argument in arguments {
        output.push(rewritten_static_argument(
            syntax,
            argument,
            substitution,
            &mut changed,
        ));
    }
    changed.then_some(output)
}

fn rewritten_static_argument(
    syntax: &SyntaxTrees,
    argument: &StaticMachineArgument,
    substitution: &HashMap<String, TypeReferenceHandle>,
    changed: &mut bool,
) -> StaticMachineArgument {
    if argument.type_reference.is_valid()
        || argument.const_literal.is_some()
        || argument.evidence_projection.is_some()
        || argument.application.is_some()
    {
        let mut kept = argument.clone();
        if let Some(application) = &argument.application
            && let Some(nested) =
                rewritten_static_arguments(syntax, &application.arguments, substitution)
        {
            kept.application = Some(Box::new(StaticSymbolApplication {
                lifetime_arguments: application.lifetime_arguments.clone(),
                arguments: nested.into_boxed_slice(),
            }));
            *changed = true;
        }
        return kept;
    }
    if let [name] = argument.path.as_ref()
        && let Some(actual) = substitution.get(name.as_str(syntax))
        && let Some(replacement) = static_argument_for_actual(syntax, *actual)
    {
        *changed = true;
        return replacement;
    }
    argument.clone()
}

/// The conformance actual, as a static operand. A path selects a declaration.
/// An application keeps its own telescope. An integer const is a literal.
/// Other type forms are not static operands and stay unreplaced.
fn static_argument_for_actual(
    syntax: &SyntaxTrees,
    actual: TypeReferenceHandle,
) -> Option<StaticMachineArgument> {
    match syntax.type_references.type_reference(actual) {
        TypeReferenceNode::Named(name) => Some(bare_static_argument(name.clone())),
        TypeReferenceNode::Generic {
            base_name,
            lifetime_arguments,
            arguments,
        } => {
            let nested = syntax
                .type_references
                .type_reference_handles(*arguments)
                .iter()
                .map(|argument| static_argument_for_actual(syntax, *argument))
                .collect::<Option<Vec<_>>>()?;
            Some(StaticMachineArgument {
                type_reference: TypeReferenceHandle::invalid(),
                path: Box::new([base_name.clone()]),
                application: Some(Box::new(StaticSymbolApplication {
                    lifetime_arguments: lifetime_arguments.clone().into_boxed_slice(),
                    arguments: nested.into_boxed_slice(),
                })),
                const_literal: None,
                evidence_projection: None,
            })
        }
        TypeReferenceNode::ConstExpression(expression) => {
            match syntax.expressions.expression(*expression) {
                ExpressionNode::Integer(literal) => Some(StaticMachineArgument {
                    type_reference: TypeReferenceHandle::invalid(),
                    path: Box::new([]),
                    application: None,
                    const_literal: Some(literal.clone()),
                    evidence_projection: None,
                }),
                ExpressionNode::Name(path) => {
                    let members = syntax.expressions.identifier_path_members(*path);
                    let [name] = members else {
                        return None;
                    };
                    Some(bare_static_argument(name.clone()))
                }
                _ => None,
            }
        }
        _ => None,
    }
}

fn bare_static_argument(name: Identifier) -> StaticMachineArgument {
    StaticMachineArgument {
        type_reference: TypeReferenceHandle::invalid(),
        path: Box::new([name]),
        application: None,
        const_literal: None,
        evidence_projection: None,
    }
}

/// A requirement edge on a synthesized default only rejoins the declaring
/// trait for an unparameterized trait reference. Parameterized owners resolve
/// through their instantiated generic instead. The returned spelling is the
/// selected trait's logical path: the compiler-derived partition requires a
/// generated spelling, and a qualified path still reaches the exact owner in
/// its package where a bare leaf could select a same-spelled competitor.
fn exact_unargumented_requirement_owner(
    traits: &HashMap<SymbolHandle, TraitDefaultsInput>,
    owner: SymbolHandle,
) -> Option<&str> {
    let definition = traits.get(&owner)?;
    (!definition.has_lifetime_parameters && definition.parameter_names.is_empty())
        .then_some(definition.path.as_str())
}

/// `default_body_trait` names the trait that authored `signature`'s body. The
/// body keeps that trait's lexical scope for bare requirement names; it is
/// empty for compiler-written bodies such as synthesized structural equality.
fn machine_from_signature(
    syntax: &mut SyntaxTrees,
    machine_name: Identifier,
    signature: &StateSignatureNode,
    requirement_owner: Option<&str>,
    default_body_trait: Identifier,
) -> Machine {
    let state = State {
        // This state belongs to the synthesized implementation, not to the
        // foreign trait's authored requirement. Its generated name inherits
        // the machine's conformance-site package provenance. Keep the body
        // and signature operands authored: their names still resolve with
        // the default author's lexical imports and private access.
        name: Identifier::generated(signature.name.as_str(syntax)),
        parameters: signature.parameters,
        return_type: signature.return_type,
        contracts: HandleSpan::empty(),
        statements: signature.default_body,
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
    Machine {
        nested_machines: Vec::new(),
        target_family_interface: false,
        name: machine_name,
        default_body_trait,
        // The member belongs to its implementation's map, not to any data
        // declaration.
        attached_data: None,
        // A synthesized default body is an implementation of the requirement,
        // which owns any token binding (`StateSignature::spelling`); the
        // implementation never redeclares it.
        spelling: None,
        is_public: false,
        bodyless: false,
        target: None,
        authored_target: None,
        boundary: false,
        is_top_level_boundary_requirement: false,
        generic_data_template: Default::default(),
        where_facts: HandleSpan::empty(),
        lifetime_parameters: signature.lifetime_parameters.clone(),
        type_parameters: HandleSpan::empty(),
        satisfies,
        conformance_bounds: Vec::new(),
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
        contracts: signature.contracts,
        states: HandleSpan::from_parts(state, 1),
    }
}
