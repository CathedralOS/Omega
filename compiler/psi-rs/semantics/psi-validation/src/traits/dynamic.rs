use psi_diagnostics::Diagnostic;
use psi_typed_trees::TypedTrees;
use psi_typed_trees::expression::{ExpressionHandle, ExpressionNode};
use psi_typed_trees::name::Identifier;
use psi_typed_trees::statement::StatementNode;
use psi_typed_trees::trait_definition::DynamicSignatureIneligibility;
use psi_typed_trees::types::{TypeReferenceHandle, TypeReferenceNode};

/// One local dynamic coercion whose complete nominal conformance is fixed in
/// the checked artifact. Runtime descriptor lowering consumes this exact
/// selection rather than rediscovering implementations from names.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DynamicConformanceSelection {
    pub occurrence: ExpressionHandle,
    pub binding: psi_symbols::SymbolHandle,
    pub binding_name: Identifier,
    pub machine: psi_symbols::SymbolHandle,
    pub state: psi_symbols::SymbolHandle,
    pub statement_index: usize,
    /// Exact source place repackaged by this coercion. Whole-artifact
    /// devirtualization uses it as the selected realization's receiver instead
    /// of treating the two-word dynamic descriptor as the concrete `self`.
    pub source_symbol: psi_symbols::SymbolHandle,
    pub source_name: Identifier,
    pub source_path: Vec<Identifier>,
    pub source_data: psi_symbols::SymbolHandle,
    pub target_trait: psi_symbols::SymbolHandle,
    /// Stable child symbol for a named conformance. `None` denotes the unique
    /// unnamed conformance identified by `source_data + target_trait`.
    pub conformance: Option<psi_symbols::SymbolHandle>,
}

/// Select complete nominal conformances for the currently admitted local
/// coercion form: a direct place cast bound to a reference-typed local. Bare
/// `dyn Trait` requires a unique conformance; `dyn Type::Conformance` selects
/// the named declaration exactly.
pub fn collect_dynamic_conformance_selections(
    program: &TypedTrees,
) -> Result<Vec<DynamicConformanceSelection>, Vec<Diagnostic>> {
    let mut selections = Vec::new();
    let mut diagnostics = Vec::new();

    for machine in program.machines() {
        for state in program.machine_states(machine) {
            for (statement_index, statement) in program
                .statement_table
                .statements(state.statement_nodes)
                .iter()
                .enumerate()
            {
                let StatementNode::LocalData(local) = statement else {
                    continue;
                };
                let occurrence = strip_mutable(program, local.initial_value);
                let ExpressionNode::Cast(cast) = program.expression_table.expression(occurrence)
                else {
                    continue;
                };
                let Some(dynamic_target) = dynamic_trait_reference(program, cast.target_type)
                else {
                    continue;
                };
                let TypeReferenceNode::DynamicTrait {
                    symbol: target_trait,
                    conformance: exact_conformance,
                    conformance_carrier,
                    conformance_name,
                    ..
                } = dynamic_target
                else {
                    unreachable!("dynamic_trait_reference returns a dynamic-trait node")
                };
                let target_trait = *target_trait;
                if local.is_mutable
                    && dynamic_binding_is_reassigned(program, machine, local.symbol, &local.name)
                {
                    diagnostics.push(Diagnostic::error(format!(
                        "local dynamic binding `{}` is reassigned; physical descriptor lowering is required before mutable dynamic bindings can dispatch",
                        local.name
                    )));
                    continue;
                }
                let Some(source_place) = dynamic_source_place(program, cast.value) else {
                    diagnostics.push(Diagnostic::error(format!(
                        "local dynamic coercion `{}` requires a direct named or member source place",
                        cast.display_name(&program.expression_table)
                    )));
                    continue;
                };
                let Some(source_type) = crate::places::declared_place_type_raw(
                    program,
                    machine,
                    Some(state),
                    cast.value,
                ) else {
                    diagnostics.push(Diagnostic::error(format!(
                        "local dynamic coercion `{}` has no statically resolved source place type",
                        cast.display_name(&program.expression_table)
                    )));
                    continue;
                };
                let Some((source_data, source_name)) = nominal_data_type(program, source_type)
                else {
                    diagnostics.push(Diagnostic::error(format!(
                        "local dynamic coercion `{}` requires a concrete nominal data source",
                        cast.display_name(&program.expression_table)
                    )));
                    continue;
                };
                let Some(trait_definition) = program
                    .traits()
                    .iter()
                    .find(|definition| definition.symbol == target_trait)
                else {
                    continue;
                };
                let matches = program
                    .data_conformances()
                    .iter()
                    .filter(|conformance| {
                        conformance.type_name.as_str() == source_name
                            && conformance.trait_name == trait_definition.name
                            && conformance.arguments.is_empty()
                    })
                    .collect::<Vec<_>>();
                if conformance_name.is_some() {
                    let selection_name = conformance_carrier
                        .as_ref()
                        .zip(conformance_name.as_ref())
                        .map(|(carrier, conformance)| format!("{carrier}::{conformance}"))
                        .unwrap_or_else(|| "<invalid named conformance>".to_owned());
                    let Some(exact_symbol) = exact_conformance else {
                        diagnostics.push(Diagnostic::error(format!(
                            "local dynamic coercion selects unresolved named conformance `{selection_name}`"
                        )));
                        continue;
                    };
                    let Some(selected) = matches
                        .iter()
                        .find(|candidate| candidate.symbol == *exact_symbol)
                    else {
                        diagnostics.push(Diagnostic::error(format!(
                            "local dynamic coercion from `{source_name}` to `dyn {}` cannot use named conformance `{selection_name}`",
                            trait_definition.name
                        )));
                        continue;
                    };
                    let selection = DynamicConformanceSelection {
                        occurrence,
                        binding: local.symbol,
                        binding_name: local.name.clone(),
                        machine: machine.symbol,
                        state: state.symbol,
                        statement_index,
                        source_symbol: source_place.symbol,
                        source_name: source_place.name.clone(),
                        source_path: source_place.path.clone(),
                        source_data,
                        target_trait,
                        conformance: Some(selected.symbol),
                    };
                    if !selections.contains(&selection) {
                        selections.push(selection);
                    }
                    continue;
                }
                match matches.as_slice() {
                    [conformance] => {
                        let selection = DynamicConformanceSelection {
                            occurrence,
                            binding: local.symbol,
                            binding_name: local.name.clone(),
                            machine: machine.symbol,
                            state: state.symbol,
                            statement_index,
                            source_symbol: source_place.symbol,
                            source_name: source_place.name.clone(),
                            source_path: source_place.path.clone(),
                            source_data,
                            target_trait,
                            conformance: conformance.symbol.is_valid().then_some(conformance.symbol),
                        };
                        if !selections.contains(&selection) {
                            selections.push(selection);
                        }
                    }
                    [] => diagnostics.push(Diagnostic::error(format!(
                        "local dynamic coercion from `{source_name}` to `dyn {}` has no complete nominal conformance",
                        trait_definition.name
                    ))),
                    many => diagnostics.push(Diagnostic::error(format!(
                        "local dynamic coercion from `{source_name}` to `dyn {}` has {} complete nominal conformances; select one exact named conformance",
                        trait_definition.name,
                        many.len()
                    ))),
                }
            }
        }
    }

    if diagnostics.is_empty() {
        Ok(selections)
    } else {
        Err(diagnostics)
    }
}

#[derive(Debug, Clone)]
struct DynamicSourcePlace {
    symbol: psi_symbols::SymbolHandle,
    name: Identifier,
    path: Vec<Identifier>,
}

fn dynamic_source_place(
    program: &TypedTrees,
    expression: ExpressionHandle,
) -> Option<DynamicSourcePlace> {
    match program.expression_table.expression(expression) {
        ExpressionNode::Atomic(atomic) => dynamic_source_place(program, atomic.value),
        ExpressionNode::Mutable(inner) => dynamic_source_place(program, *inner),
        ExpressionNode::Name(name) => {
            let leaf = program
                .expression_table
                .name_path_members(name.members)
                .last()?
                .clone();
            Some(DynamicSourcePlace {
                symbol: name.symbol,
                name: leaf.clone(),
                path: vec![leaf],
            })
        }
        ExpressionNode::Member(member) => {
            let mut source = dynamic_source_place(program, member.receiver)?;
            source.symbol = member.member_symbol;
            source.name = member.member.clone();
            source.path.push(member.member.clone());
            Some(source)
        }
        _ => None,
    }
}

fn dynamic_binding_is_reassigned(
    program: &TypedTrees,
    machine: &psi_typed_trees::machine::Machine,
    binding: psi_symbols::SymbolHandle,
    binding_name: &Identifier,
) -> bool {
    program.machine_states(machine).iter().any(|state| {
        program
            .statement_table
            .statements(state.statement_nodes)
            .iter()
            .any(|statement| {
                let StatementNode::Assignment(assignment) = statement else {
                    return false;
                };
                let Some(target) = dynamic_source_place(program, assignment.target) else {
                    return false;
                };
                target.path.len() == 1
                    && if binding.is_valid() && target.symbol.is_valid() {
                        target.symbol == binding
                    } else {
                        target.name == *binding_name
                    }
            })
    })
}

/// Explain why one requirement is absent from a local `dyn Trait` surface.
/// Eligibility is intentionally per requirement: an ineligible sibling does
/// not invalidate calls to the rest of the trait.
pub(crate) fn dynamic_requirement_call_error(
    program: &TypedTrees,
    receiver_type: TypeReferenceHandle,
    target: &str,
) -> Option<String> {
    let trait_symbol = dynamic_trait_symbol(program, receiver_type)?;
    let trait_definition = program
        .traits()
        .iter()
        .find(|definition| definition.symbol == trait_symbol)?;
    let requirement = program
        .trait_machine_signatures(trait_definition)
        .iter()
        .find(|signature| signature.name.as_str() == target)?;

    let reason = match program
        .dynamic_signature_eligibility(trait_definition, requirement)
        .err()?
    {
        DynamicSignatureIneligibility::BoundaryRequirement => {
            "boundary-machine requirements are not local dynamic calls"
        }
        DynamicSignatureIneligibility::RequirementLocalGenerics => {
            "the requirement has requirement-local generic parameters"
        }
        DynamicSignatureIneligibility::MissingBorrowedReceiver => {
            "the requirement has no `&self` or `&mut self` receiver"
        }
        DynamicSignatureIneligibility::ByValueReceiver => {
            "the receiver is by value rather than `&self` or `&mut self`"
        }
        DynamicSignatureIneligibility::MultipleReceivers => {
            "the requirement has more than one receiver"
        }
        DynamicSignatureIneligibility::SelfOutsideReceiver => {
            "`Self` appears outside the borrowed receiver"
        }
        DynamicSignatureIneligibility::SelfResult => "`Self` appears in the result type",
    };

    Some(format!(
        "requirement `{}::{}` is absent from `dyn {}`: {reason}",
        trait_definition.name, requirement.name, trait_definition.name
    ))
}

pub(crate) fn dynamic_trait_symbol(
    program: &TypedTrees,
    type_reference: TypeReferenceHandle,
) -> Option<psi_symbols::SymbolHandle> {
    match program.type_reference_table.type_reference(type_reference) {
        TypeReferenceNode::Reference { referee, .. } => dynamic_trait_symbol(program, *referee),
        TypeReferenceNode::Constrained { base_type, .. } => {
            dynamic_trait_symbol(program, *base_type)
        }
        TypeReferenceNode::DynamicTrait { symbol, .. } => Some(*symbol),
        _ => None,
    }
}

fn dynamic_trait_reference(
    program: &TypedTrees,
    type_reference: TypeReferenceHandle,
) -> Option<&TypeReferenceNode> {
    match program.type_reference_table.type_reference(type_reference) {
        TypeReferenceNode::Reference { referee, .. } => dynamic_trait_reference(program, *referee),
        TypeReferenceNode::Constrained { base_type, .. } => {
            dynamic_trait_reference(program, *base_type)
        }
        dynamic @ TypeReferenceNode::DynamicTrait { .. } => Some(dynamic),
        _ => None,
    }
}

fn nominal_data_type(
    program: &TypedTrees,
    type_reference: TypeReferenceHandle,
) -> Option<(psi_symbols::SymbolHandle, &str)> {
    match program.type_reference_table.type_reference(type_reference) {
        TypeReferenceNode::Reference { referee, .. } => nominal_data_type(program, *referee),
        TypeReferenceNode::Constrained { base_type, .. } => nominal_data_type(program, *base_type),
        TypeReferenceNode::Named { symbol, name } => program
            .data_definitions()
            .iter()
            .find(|definition| definition.symbol == *symbol)
            .map(|definition| (definition.symbol, name.as_str())),
        _ => None,
    }
}

fn strip_mutable(program: &TypedTrees, expression: ExpressionHandle) -> ExpressionHandle {
    match program.expression_table.expression(expression) {
        ExpressionNode::Mutable(inner) => *inner,
        _ => expression,
    }
}
