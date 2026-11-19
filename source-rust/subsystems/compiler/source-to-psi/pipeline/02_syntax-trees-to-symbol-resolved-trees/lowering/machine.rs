/*
    Machine definitions.

    `lower_machine_into` sets the lowerer's current-machine context, then lowers
    the ranking view and termination plan, trait conformances, external
    binding identity, and every state. `lower_generic_conformance_bounds` is
    shared with trait definitions.
*/

use crate::resolution::lowering::expression::expression_table;
use crate::resolution::tables::syntax_expressions;
use crate::resolution::tables::syntax_items;
use syntax_trees::identifier::JoinedSyntaxPath;
mod type_equations {
    /*
        Preserve the source classifier's type-role equations for generic body
        checking. Application preparation still discharges every requires equation;
        current callers never create assumptions for a declaration's body.
    */

    use crate::resolution::lowering::lowerer::Lowerer;
    use crate::resolution::lowering::type_reference;
    use crate::source_resolved::machine::DeclaredTypeEquation;
    use crate::source_resolved::types::{
        ReferenceTypeReference, ReferenceTypeReferenceStorage, SliceTypeReference,
        SliceTypeReferenceStorage, TypeReference,
    };
    use arena::HandleSpan;
    use diagnostics::Diagnostic;
    use syntax_trees::expression::{ExpressionHandle, ExpressionNode};
    use syntax_trees::identifier::JoinedSyntaxPath;
    use syntax_trees::{SyntaxTrees, item::Machine};

    pub(super) struct MachineTypeEquations {
        pub(super) definitions: HandleSpan<DeclaredTypeEquation>,
        pub(super) requires_argument_recovery: bool,
        pub(super) declares_equations: bool,
    }

    pub(super) fn lower(
        lowerer: &mut Lowerer,
        syntax: &SyntaxTrees,
        machine: &Machine,
    ) -> Result<MachineTypeEquations, Diagnostic> {
        let parameters = syntax.items.type_parameters(machine.type_parameters);
        let equations = crate::resolution::declarations::machine_equations::machine_type_equations(
            syntax, machine,
        );
        let declares_equations = !equations.is_empty();
        let requires_argument_recovery = equations
            .iter()
            .any(crate::resolution::declarations::type_equations::TypeEquation::requires_argument_recovery);
        let binders = parameters
            .iter()
            .map(|parameter| &parameter.name)
            .chain(&machine.lifetime_parameters)
            .map(|name| name.as_str(syntax).to_owned())
            .collect::<Vec<_>>();
        let mut definitions = Vec::with_capacity(equations.len());
        for equation in equations {
            equation.validate_kind("machine")?;
            equation.structural_definitions(syntax, &binders, &mut definitions);
        }
        let mut lowered = Vec::with_capacity(definitions.len());
        for (parameter, definition) in definitions {
            lowered.push(DeclaredTypeEquation {
                parameter: TypeReference::Named {
                    supplied_operand_owner: Default::default(),
                    symbol: symbols::SymbolHandle::invalid(),
                    name: crate::resolution::lowering::name::lower_name(
                        syntax,
                        &parameters[parameter].name,
                    ),
                },
                definition: lower_type_operand(lowerer, syntax, definition)?,
            });
        }
        Ok(MachineTypeEquations {
            definitions: lowerer
                .symbol_resolved_trees
                .tables
                .declarations
                .declared_type_equations
                .insert_many(lowered),
            requires_argument_recovery,
            declares_equations,
        })
    }

    /*
        These expression forms have type role only after the equation classifier
        selected an opposite type binder. Their children join ordinary type storage.
    */
    fn lower_type_operand(
        lowerer: &mut Lowerer,
        syntax: &SyntaxTrees,
        expression: ExpressionHandle,
    ) -> Result<TypeReference, Diagnostic> {
        Ok(match syntax.expressions.expression(expression) {
            ExpressionNode::TypeExpression(reference) => {
                type_reference::lower_type_reference_handle(lowerer, syntax, *reference)?
            }
            ExpressionNode::Name {
                members: path,
                application: path_application,
            } if path_application.is_none() => {
                let members = syntax.expressions.identifier_path_members(path.to_owned());
                TypeReference::Named {
                    supplied_operand_owner: Default::default(),
                    symbol: symbols::SymbolHandle::invalid(),
                    name: crate::source_resolved::name::DiagnosticName::new(
                        members.joined_path_in(syntax),
                        syntax.expressions.source_span(expression),
                    ),
                }
            }
            ExpressionNode::Borrow(borrow) => {
                let referee = lower_type_operand(lowerer, syntax, borrow.target)?;
                let referee = lowerer.declaration_child_type_references().append(referee);
                TypeReference::Reference(ReferenceTypeReference {
                    storage: ReferenceTypeReferenceStorage {
                        supplied_operand_owner: Default::default(),
                        referee,
                        access: borrow.access,
                        lifetime: None,
                    },
                })
            }
            ExpressionNode::ArrayLiteral(elements) => {
                let [element] = syntax.expressions.expression_handles(*elements) else {
                    return Err(Diagnostic::error(
                        "type equation requires exactly one slice element type",
                    )
                    .with_source_span(syntax.expressions.source_span(expression)));
                };
                let element = lower_type_operand(lowerer, syntax, *element)?;
                let element_type = lowerer.declaration_child_type_references().append(element);
                TypeReference::Slice(SliceTypeReference {
                    storage: SliceTypeReferenceStorage {
                        supplied_operand_owner: Default::default(),
                        element_type,
                    },
                })
            }

            _ => {
                return Err(Diagnostic::error(
                    "structural type equation lost its classified type operand",
                )
                .with_source_span(syntax.expressions.source_span(expression)));
            }
        })
    }
}

use crate::declaration_selection::AuthoredDeclarationSelectionExposure;
use crate::resolution::lowering::data::lower_type_parameters;
use crate::resolution::lowering::expression::lower_expression_into_table;
use crate::resolution::lowering::lowerer::Lowerer;
use crate::resolution::lowering::name::lower_name;
use crate::resolution::lowering::name::lower_names;
use crate::resolution::lowering::state::{
    lower_machine_signature_contracts, lower_service_reach_names, lower_signature_invokes,
    lower_state_body, lower_state_node,
};
use crate::resolution::lowering::type_reference::lower_child_type_references;
use crate::source_resolved::expression::ExpressionHandle;
use crate::source_resolved::machine::{
    GenericConformanceBound, GenericDataMachineOrigin, Machine, MachineBodyStatus, MachineStorage,
    TraitConformance, TraitDefaultOrigin,
};
use crate::source_resolved::name::DiagnosticName;
use crate::source_resolved::state::State;
use crate::source_selections::CompilerDerivedSelectionPartition;
use arena::{Handle, HandleSpan};
use diagnostics::Diagnostic;
use semantic_vocabulary::RankingViewId;
use std::mem::replace;
use symbols::SymbolHandle;
use syntax::expression::BinaryOperator;
use syntax::expression::ExpressionNode;
use syntax::identifier::Identifier;
use syntax::item::ExternalBinding;
use syntax::item::Item;
use syntax::item::SatisfiesClause;
use syntax::item::StateHandle;
use syntax_trees::item::CapabilityContractKind;
use syntax_trees::{self as syntax, SyntaxTrees};

// Only statements inside a machine can forward its evidence and use its boundary parameters.
pub(crate) struct MachineBodyScope {
    pub(crate) ordinal: usize,
    pub(crate) name: String,
    pub(crate) is_boundary: bool,
    pub(crate) evidence_names: Vec<String>,
}

fn with_machine_body_scope<T>(
    lowerer: &mut Lowerer,
    syntax: &SyntaxTrees,
    machine: &syntax::item::Machine,
    ordinal: usize,
    operation: impl FnOnce(&mut Lowerer) -> Result<T, Diagnostic>,
) -> Result<T, Diagnostic> {
    let previous = lowerer.machine_body.replace(MachineBodyScope {
        ordinal,
        name: machine.name.as_str(syntax).to_owned(),
        is_boundary: machine.boundary,
        evidence_names: syntax_items(syntax)
            .capability_contracts(machine.contracts)
            .iter()
            .filter_map(|contract| contract.binding.as_ref())
            .map(|name| name.as_str(syntax).to_owned())
            .collect(),
    });
    let result = lowerer.with_authored_expression_exposure(
        AuthoredDeclarationSelectionExposure::PrivateImplementation,
        operation,
    );
    lowerer.machine_body = previous;
    result
}

/*
    Prepared syntax bound to the one declaration roster that indexed it.
    Compiler-derived machines can rewrite a template's name/attachment; retain
    that normalized owner rather than rediscovering it by an authored spelling.
*/
#[derive(Debug, Clone, record_storage::Record)]
pub(crate) struct PendingMachineBody {
    pub(crate) machine_ordinal: usize,
    source: syntax::item::Machine,
    states: Vec<(Handle<State>, StateHandle)>,
}

impl PendingMachineBody {
    pub(crate) fn same_syntax(
        &self,
        other: &Self,
        left: &syntax::SyntaxTextOwner,
        right: &syntax::SyntaxTextOwner,
    ) -> bool {
        use syntax::SyntaxEquivalent;
        let Self {
            machine_ordinal,
            source,
            states,
        } = self;
        *machine_ordinal == other.machine_ordinal
            && *states == other.states
            && source.same_syntax(&other.source, left, right)
    }
}

/*
    Structural exposure is decided before supply classification erases whether
    an external leaf used the boundary introducer. Keep it separate from `public`:
    exported contracts do not make a private declaration source-nameable.
*/
pub(crate) fn machine_interface_exposure(
    machine: &syntax::item::Machine,
) -> AuthoredDeclarationSelectionExposure {
    use crate::declaration_selection::AuthoredDeclarationSelectionExposure;
    if machine.is_public
        || (machine.boundary
            && !crate::resolution::lowering::machine_supply::is_token_bearing_boundary_signature(
                machine,
            ))
    {
        AuthoredDeclarationSelectionExposure::PublicInterface
    } else {
        AuthoredDeclarationSelectionExposure::PrivateImplementation
    }
}

pub(crate) fn lower_machine_into(
    lowerer: &mut Lowerer,
    syntax_trees: &SyntaxTrees,
    machine: &syntax::item::Machine,
) -> Result<(), Diagnostic> {
    lower_lexical_machine_into(lowerer, syntax_trees, machine, Handle::invalid())
}

fn lower_lexical_machine_into(
    lowerer: &mut Lowerer,
    syntax_trees: &SyntaxTrees,
    machine: &syntax::item::Machine,
    lexical_parent: Handle<Machine>,
) -> Result<(), Diagnostic> {
    let owner = crate::resolution::names::runtime_callable_scopes::CallableScopeOwner::Machine(
        lowerer.symbol_resolved_trees.machines.len(),
    );
    lowerer.with_runtime_callable_scope(owner, |lowerer| {
        lowerer.with_authored_expression_exposure(machine_interface_exposure(machine), |lowerer| {
            lower_lexical_machine_parts(lowerer, syntax_trees, machine, lexical_parent)
        })
    })
}

fn lower_lexical_machine_parts(
    lowerer: &mut Lowerer,
    syntax_trees: &SyntaxTrees,
    machine: &syntax::item::Machine,
    lexical_parent: Handle<Machine>,
) -> Result<(), Diagnostic> {
    if lexical_parent.is_valid()
        && (machine.is_public
            || machine.attached_data.is_some()
            || machine.boundary
            || machine.spelling.is_some())
    {
        return Err(Diagnostic::error(
            "a nested machine is a private independent lexical declaration",
        ));
    }
    let compiler_provided = machine.target.is_none()
        && !machine.target_family_interface
        && crate::resolution::lowering::machine_supply::is_bare_bodyless_signature(machine);
    if compiler_provided {
        crate::resolution::lowering::machine_supply::require_catalog_signature(
            lowerer,
            syntax_trees,
            machine,
        )?;
    }
    let compiler_selection_partition = compiler_selection_partition(
        lowerer.symbol_resolved_trees.machines.len(),
        syntax_trees,
        machine,
    );
    lowerer.current_compiler_selection_partition = compiler_selection_partition;
    let interface_exposure = machine_interface_exposure(machine);
    let states = with_machine_body_scope(
        lowerer,
        syntax_trees,
        machine,
        lowerer.symbol_resolved_trees.machines.len(),
        |lowerer| {
            lower_machine_states(
                lowerer,
                syntax_trees,
                machine.states,
                interface_exposure == AuthoredDeclarationSelectionExposure::PublicInterface,
            )
        },
    )?;
    let body_status = if lowerer.defer_machine_bodies && !machine.bodyless {
        lowerer.pending_machine_bodies.push(PendingMachineBody {
            machine_ordinal: lowerer.symbol_resolved_trees.machines.len(),
            source: machine.clone(),
            states: lowerer
                .symbol_resolved_trees
                .machine_state_handles(states)
                .iter()
                .copied()
                .zip(
                    syntax_items(syntax_trees)
                        .state_handles(machine.states)
                        .iter()
                        .copied(),
                )
                .collect(),
        });
        MachineBodyStatus::Deferred
    } else {
        MachineBodyStatus::Complete
    };
    let type_parameters = lower_type_parameters(lowerer, syntax_trees, machine.type_parameters)?;
    let type_equations::MachineTypeEquations {
        definitions: declared_type_equations,
        requires_argument_recovery,
        declares_equations,
    } = type_equations::lower(lowerer, syntax_trees, machine)?;

    let sibling_target = machine
        .target
        .as_ref()
        .map(|name| lower_name(syntax_trees, name));
    let satisfies = lower_machine_trait_conformances(lowerer, syntax_trees, machine.satisfies)?;
    let inferred_float_binding = if is_sealed_float_provider_member(lowerer, syntax_trees, machine)
    {
        let binding = lowerer
            .symbol_resolved_trees
            .external_bindings
            .intern(semantic_vocabulary::ExternalBindingIdentity::CompilerIntrinsic);
        for edge in lowerer
            .symbol_resolved_trees
            .tables
            .declarations
            .machine_trait_conformances
            .span_mut_or_empty(satisfies)
        {
            edge.external_binding = Some(binding);
        }
        Some(binding)
    } else {
        None
    };
    let conformance_bounds =
        lower_generic_conformance_bounds(lowerer, syntax_trees, &machine.conformance_bounds)?;
    let ranking_subjects =
        lower_ranking_expressions(lowerer, syntax_trees, machine.ranking_subjects)?;
    let ranking_view = lower_machine_ranking_view(lowerer, syntax_trees, machine.ranking_view);
    let ranking_view_arguments =
        lower_ranking_expressions(lowerer, syntax_trees, machine.ranking_view_arguments)?;
    let ranking_range = if machine.ranking_range.is_valid() {
        lower_expression_into_table(lowerer, syntax_trees, machine.ranking_range)?
    } else {
        ExpressionHandle::invalid()
    };
    let service_reaches = lower_service_reach_names(syntax_trees, machine.service_reaches);
    let invokes = lower_signature_invokes(lowerer, syntax_trees, machine.invokes);
    let contracts =
        lower_machine_signature_contracts(lowerer, syntax_trees, machine.contracts, states)?;
    let machine_name = lower_name(syntax_trees, &machine.name);
    let target_symbol_name = match &sibling_target {
        Some(target) => DiagnosticName::new(
            format!("{}::{}", machine.name.as_str(syntax_trees), target.as_str()),
            machine.name.source_span(),
        ),
        None => DiagnosticName::default(),
    };
    let attached_data = machine
        .attached_data
        .as_ref()
        .map(|name| lower_name(syntax_trees, name));
    let termination_plan = build_termination_plan(syntax_trees, machine);

    let supply_mode = {
        let via_binding = syntax_items(syntax_trees)
            .satisfies_clauses(machine.satisfies)
            .iter()
            .find_map(|clause| clause.via.as_ref())
            .map(|value| external_binding_identity(syntax_trees, value));
        let has_via_expression = syntax_items(syntax_trees)
            .satisfies_clauses(machine.satisfies)
            .iter()
            .any(|clause| clause.via_expression.is_valid());
        if compiler_provided {
            declaration_contracts::MachineSupplyMode::CompilerProvided
        } else if machine.target_family_interface {
            declaration_contracts::MachineSupplyMode::Requirement
        } else if crate::resolution::lowering::machine_supply::is_token_bearing_boundary_signature(
            machine,
        ) || (machine.bodyless
            && machine.boundary
            && sealed_intrinsic(lowerer, syntax_trees, machine)
                == Some(
                    language_core::sealed_intrinsics::SealedIntrinsic::FloatOperationRequirement,
                ))
        {
            // Named and token-bound catalog declarations have the same supply
            // meaning. Exact sealed source custody nominates the declaration;
            // typing and receiving still authenticate its full numeric contract.
            declaration_contracts::MachineSupplyMode::TopLevelRequirement
        } else if let Some(binding) = inferred_float_binding {
            declaration_contracts::MachineSupplyMode::ExternalRealization {
                binding: Some(binding),
                mechanism: Some(semantic_vocabulary::ExternalBindingMechanism::CompilerIntrinsic),
            }
        } else if machine.bodyless && (via_binding.is_some() || has_via_expression) {
            let (binding, mechanism) = via_binding
                .map(|identity| {
                    let mechanism = identity.mechanism();
                    let binding = lowerer
                        .symbol_resolved_trees
                        .external_bindings
                        .intern(identity);
                    (Some(binding), Some(mechanism))
                })
                .unwrap_or((None, None));
            declaration_contracts::MachineSupplyMode::ExternalRealization { binding, mechanism }
        } else if machine.bodyless && machine.boundary {
            let authors_fact = syntax_items(syntax_trees)
                .capability_contracts(machine.contracts)
                .iter()
                .any(|contract| {
                    matches!(contract.kind, CapabilityContractKind::Ensures)
                        && !contract.facts.is_empty()
                });
            if compiler_owned_boundary(lowerer, syntax_trees, machine) {
                declaration_contracts::MachineSupplyMode::Boundary
            } else if authors_fact {
                declaration_contracts::MachineSupplyMode::AdmissionClaim
            } else {
                declaration_contracts::MachineSupplyMode::Boundary
            }
        } else if machine.boundary {
            declaration_contracts::MachineSupplyMode::Boundary
        } else {
            declaration_contracts::MachineSupplyMode::CheckedBody
        }
    };
    lowerer.pending_machine_service_reaches.push(
        crate::resolution::selections::service_reaches::PendingAuthoredServiceReach::Authored {
            keyword_source_spans: machine.service_reach_keyword_source_spans.clone(),
            authored: service_reaches,
        },
    );
    if let Some(sources) = &mut lowerer.equation_sources
        && !machine.type_parameters.is_empty()
    {
        sources.declarations.push((
            lowerer.symbol_resolved_trees.machines.len(),
            machine.clone(),
        ));
    }
    let declaration = lowerer.symbol_resolved_trees.machines.push(Machine {
        symbol: SymbolHandle::invalid(),
        lexical_parent,
        name: machine_name,
        generic_data_origin: GenericDataMachineOrigin {
            template_source: lower_name(syntax_trees, &machine.generic_data_template),
            ..Default::default()
        },
        trait_default_origin: TraitDefaultOrigin {
            trait_source: lower_name(syntax_trees, &machine.default_body_trait),
            ..Default::default()
        },
        attached_data,
        attached_data_symbol: SymbolHandle::invalid(),
        operator_home_domain: SymbolHandle::invalid(),
        attached_data_application: None,
        spelling: machine.spelling,
        is_public: machine.is_public,
        interface_exposure,
        supply_mode,
        body_is_present: !machine.bodyless,
        target: sibling_target,
        authored_target: machine
            .authored_target
            .as_ref()
            .map(|name| lower_name(syntax_trees, name)),
        target_family: SymbolHandle::invalid(),
        target_family_interface: machine.target_family_interface,
        target_family_contracts_prepared: false,
        target_symbol_name,
        has_structural_type_equations: requires_argument_recovery,
        structural_type_equations_pending: declares_equations
            && (lowerer.structural_type_equations_pending || !requires_argument_recovery),
        termination_plan,
        service_reach_row: semantic_vocabulary::ServiceReachRowId::NULL,
        service_reach_is_installation_bound: machine.service_reach_is_installation_bound,
        admitted_same_stack_bytes: machine.admitted_same_stack_bytes,
        declares_no_crash_route: machine.declares_no_crash_route,
        suspends_keyword_source_spans: machine.suspends_keyword_source_spans.clone(),
        blocks_keyword_source_spans: machine.blocks_keyword_source_spans.clone(),
        compiler_selection_partition,
        body_status,
        storage: MachineStorage {
            lifetime_parameters: lower_names(syntax_trees, &machine.lifetime_parameters),
            type_parameters,
            declared_type_equations,
            owned_data: HandleSpan::empty(),
            satisfies,
            conformance_bounds,
            ranking_subjects,
            ranking_view,
            selected_ranking_view: Default::default(),
            ranking_view_arguments,
            ranking_range,
            invokes,
            suspends: machine.suspends,
            blocks: machine.blocks,
            contracts,
            states,
        },
    });
    lowerer.current_compiler_selection_partition = None;
    for child in machine.nested_machines.handles() {
        let Item::Machine(child) = syntax_trees.items.item(child) else {
            return Err(Diagnostic::error(
                "a machine lexical scope contains a non-machine declaration",
            ));
        };
        lower_lexical_machine_into(lowerer, syntax_trees, child, declaration)?;
    }
    Ok(())
}

/*
    Lower a selected implementation without recreating its declaration or its
    state parameters. Symbols for new locals are appended by the symbol owner.
*/
pub(crate) fn complete_machine_body(
    lowerer: &mut Lowerer,
    syntax: &SyntaxTrees,
    pending: &PendingMachineBody,
) -> Result<(), Diagnostic> {
    let inputs = crate::resolution::lowering::type_inputs::TypeInputScope::new(lowerer);
    with_pending_machine_scope(lowerer, syntax, pending, |lowerer| {
        for &(state_handle, syntax_state) in &pending.states {
            let state = lowerer
                .symbol_resolved_trees
                .machine_state(state_handle)
                .clone();
            let statements = lower_state_body(
                lowerer,
                syntax,
                &state,
                syntax.items.state(syntax_state).statements,
            )?;
            lowerer
                .symbol_resolved_trees
                .tables
                .declarations
                .machine_states
                .get_mut(state_handle)
                .set_construction_statements(statements);
        }
        lowerer.symbol_resolved_trees.machines[pending.machine_ordinal].body_status =
            MachineBodyStatus::Complete;
        Ok(())
    })?;
    let owner = lowerer.symbol_resolved_trees.machines[pending.machine_ordinal].symbol;
    inputs.finish(lowerer, owner);
    lowerer.completed_machines.push(owner);
    Ok(())
}

#[cfg(any(test, feature = "test-support"))]
pub(crate) fn lower_pending_lexical_inputs(
    lowerer: &mut Lowerer,
    syntax: &SyntaxTrees,
    demand: crate::resolution::lowering::state::LexicalInputDemand,
) -> Result<Vec<crate::resolution::lowering::state::StateLexicalInputs>, Diagnostic> {
    let pending = lowerer.pending_machine_bodies.clone();
    let mut inputs = Vec::new();
    for body in &pending {
        with_pending_machine_scope(lowerer, syntax, body, |lowerer| {
            let machine = lowerer.symbol_resolved_trees.machines[body.machine_ordinal].symbol;
            for &(state, syntax_state) in &body.states {
                inputs.push(
                    crate::resolution::lowering::state::lower_state_lexical_inputs(
                        lowerer,
                        syntax,
                        machine,
                        state,
                        syntax.items.state(syntax_state).statements,
                        demand,
                    )?,
                );
            }
            Ok(())
        })?;
    }
    Ok(inputs)
}

fn with_pending_machine_scope<T>(
    lowerer: &mut Lowerer,
    syntax: &SyntaxTrees,
    pending: &PendingMachineBody,
    operation: impl FnOnce(&mut Lowerer) -> Result<T, Diagnostic>,
) -> Result<T, Diagnostic> {
    let declaration = lowerer
        .symbol_resolved_trees
        .machines
        .iter()
        .nth(pending.machine_ordinal)
        .ok_or_else(|| Diagnostic::error("deferred body lost its indexed machine"))?;
    if declaration.body_status != MachineBodyStatus::Deferred {
        return Err(Diagnostic::error(
            "machine body completion requires a pending implementation",
        ));
    }
    let partition = declaration.compiler_selection_partition;
    let previous_defer = replace(&mut lowerer.defer_machine_bodies, false);
    let previous_partition = replace(&mut lowerer.current_compiler_selection_partition, partition);
    let result = lowerer.with_runtime_callable_scope(
        crate::resolution::names::runtime_callable_scopes::CallableScopeOwner::Machine(
            pending.machine_ordinal,
        ),
        |lowerer| {
            with_machine_body_scope(
                lowerer,
                syntax,
                &pending.source,
                pending.machine_ordinal,
                operation,
            )
        },
    );
    lowerer.defer_machine_bodies = previous_defer;
    lowerer.current_compiler_selection_partition = previous_partition;
    result
}

fn compiler_selection_partition(
    machine_ordinal: usize,
    syntax_trees: &SyntaxTrees,
    machine: &syntax::item::Machine,
) -> Option<CompilerDerivedSelectionPartition> {
    if machine.name.is_source_backed() {
        return None;
    }
    let ordinal = u64::try_from(machine_ordinal).ok()?;
    let [requirement] = syntax_trees.items.satisfies_clauses(machine.satisfies) else {
        return machine
            .attached_data
            .is_some()
            .then(|| CompilerDerivedSelectionPartition::from_compiler_ordinal(ordinal));
    };
    let requirement_name = requirement.requirement.as_ref()?;
    if requirement.trait_name.is_source_backed()
        || requirement_name.is_source_backed()
        || requirement.alias.is_some()
        || requirement.via.is_some()
        || requirement.via_expression.is_valid()
    {
        return None;
    }
    Some(CompilerDerivedSelectionPartition::from_compiler_ordinal(
        ordinal,
    ))
}

pub(crate) fn lower_generic_conformance_bounds(
    lowerer: &mut Lowerer,
    syntax_trees: &SyntaxTrees,
    bounds: &[syntax::item::GenericConformanceBound],
) -> Result<Vec<GenericConformanceBound>, Diagnostic> {
    bounds
        .iter()
        .map(|bound| {
            Ok(GenericConformanceBound {
                binder: SymbolHandle::invalid(),
                binder_name: lower_name(syntax_trees, &bound.binder),
                trait_symbol: SymbolHandle::invalid(),
                trait_name: lower_name(syntax_trees, &bound.trait_name),
                lifetime_arguments: bound
                    .lifetime_arguments
                    .iter()
                    .map(|name| lower_name(syntax_trees, name))
                    .collect(),
                arguments: lower_child_type_references(lowerer, syntax_trees, bound.arguments)?,
            })
        })
        .collect()
}

/*
    Retain the authored promise and witness without guessing a subject's
    carrier from its spelling. Typing selects canonical default views after
    parameter types and collection observations have exact identities.
    Neither stage establishes the checked termination guarantee.
*/
fn build_termination_plan(
    syntax_trees: &SyntaxTrees,
    machine: &syntax::item::Machine,
) -> declaration_contracts::MachineTerminationPlan {
    use declaration_contracts::{
        MachineTerminationPlan, RankingWitness, TerminationGuarantee, TerminationInterface,
    };

    let interface = if machine.terminates_guarantee {
        TerminationInterface::Published(TerminationGuarantee::Terminates {
            premises: Vec::new(),
        })
    } else if machine.boundary || machine.bodyless {
        TerminationInterface::Published(TerminationGuarantee::NoGuarantee)
    } else {
        TerminationInterface::InternalDerived
    };

    let subjects = syntax_expressions(syntax_trees).expression_handles(machine.ranking_subjects);
    let implementation_witness = (!subjects.is_empty()).then(|| {
        let (ranking_view, view_path) = authored_ranking_view(syntax_trees, machine);
        RankingWitness {
            subjects: subjects
                .iter()
                .map(|subject| render_ranked_subject(syntax_trees, *subject))
                .collect(),
            ranking_view,
            view_path,
            view_arguments: syntax_expressions(syntax_trees)
                .expression_handles(machine.ranking_view_arguments)
                .iter()
                .map(|argument| render_ranked_subject(syntax_trees, *argument))
                .collect(),
            rank_range: machine.ranking_range.is_valid().then(|| {
                let ExpressionNode::Range(range) =
                    syntax_trees.expressions.expression(machine.ranking_range)
                else {
                    return declaration_contracts::RankRange::default();
                };
                declaration_contracts::RankRange {
                    floor: render_ranked_subject(syntax_trees, range.start),
                    ceiling: render_ranked_subject(syntax_trees, range.end),
                    ceiling_inclusive: range.end_inclusive,
                }
            }),
        }
    });

    MachineTerminationPlan {
        interface,
        checked_summary: TerminationGuarantee::NoGuarantee,
        implementation_witness,
    }
}

/*
    Render one ranked subject as source-like text for the private witness
    for diagnostics; retained expression handles carry semantic identity.
*/
fn render_ranked_subject(
    syntax_trees: &SyntaxTrees,
    subject: syntax::expression::ExpressionHandle,
) -> String {
    match syntax_trees.expressions.expression(subject) {
        ExpressionNode::Integer(literal) => literal.to_string(),
        ExpressionNode::Name {
            members: path,
            application: path_application,
        } if path_application.is_none() => syntax_expressions(syntax_trees)
            .identifier_path_members(path.to_owned())
            .joined_path_with_in(syntax_trees, "."),
        ExpressionNode::Member(member) => format!(
            "{}.{}",
            render_ranked_subject(syntax_trees, member.receiver),
            member.member.as_str(syntax_trees)
        ),
        ExpressionNode::Binary(binary) if matches!(binary.operator, BinaryOperator::Subtract) => {
            format!(
                "{} - {}",
                render_ranked_subject(syntax_trees, binary.left),
                render_ranked_subject(syntax_trees, binary.right)
            )
        }
        _ => "value".to_string(),
    }
}

/*
    Retain the authored view for name resolution. No spelling grants a
    ranking identity here. An empty view requests typed default selection;
    an unresolved explicit view must never acquire that default instead.
*/
fn authored_ranking_view(
    syntax_trees: &SyntaxTrees,
    machine: &syntax::item::Machine,
) -> (RankingViewId, String) {
    use semantic_vocabulary::RankingViewId;

    let order = syntax_items(syntax_trees).identifier_path_members(machine.ranking_view);
    if !order.is_empty() {
        let path = order.joined_path_in(syntax_trees);
        return (RankingViewId::NULL, path);
    }

    (RankingViewId::NULL, String::new())
}

fn lower_machine_ranking_view(
    lowerer: &mut Lowerer,
    syntax_trees: &SyntaxTrees,
    order: HandleSpan<Identifier>,
) -> HandleSpan<DiagnosticName> {
    crate::resolution::lowering::name::lower_name_members(
        syntax_trees,
        order,
        &mut lowerer
            .symbol_resolved_trees
            .tables
            .declarations
            .ranking_views,
    )
}

fn lower_ranking_expressions(
    lowerer: &mut Lowerer,
    syntax_trees: &SyntaxTrees,
    source_expressions: HandleSpan<syntax::expression::ExpressionHandle>,
) -> Result<HandleSpan<ExpressionHandle>, Diagnostic> {
    let mut lowered = Vec::new();

    for expression in syntax_expressions(syntax_trees).expression_handles(source_expressions) {
        let expression = lower_expression_into_table(lowerer, syntax_trees, *expression)?;
        lowered.push(expression);
    }

    Ok(expression_table(lowerer).insert_expression_handles(lowered))
}

fn lower_machine_trait_conformances(
    lowerer: &mut Lowerer,
    syntax_trees: &SyntaxTrees,
    satisfies: HandleSpan<SatisfiesClause>,
) -> Result<HandleSpan<TraitConformance>, Diagnostic> {
    let mut span = HandleSpan::empty();

    for clause in syntax_trees.items.satisfies_clauses(satisfies) {
        let arguments = lower_child_type_references(lowerer, syntax_trees, clause.arguments)?;
        let external_binding = clause.via.as_ref().map(|binding| {
            lowerer
                .symbol_resolved_trees
                .external_bindings
                .intern(external_binding_identity(syntax_trees, binding))
        });
        let via_expression = clause
            .via_expression
            .is_valid()
            .then(|| lower_expression_into_table(lowerer, syntax_trees, clause.via_expression))
            .transpose()?
            .unwrap_or_else(ExpressionHandle::invalid);
        lowerer
            .symbol_resolved_trees
            .tables
            .declarations
            .machine_trait_conformances
            .append_to_span(
                &mut span,
                TraitConformance {
                    symbol: SymbolHandle::invalid(),
                    name: lower_name(syntax_trees, &clause.trait_name),
                    lifetime_arguments: lower_names(syntax_trees, &clause.lifetime_arguments),
                    arguments,
                    requirement: clause
                        .requirement
                        .as_ref()
                        .map(|name| lower_name(syntax_trees, name)),
                    alias: clause
                        .alias
                        .as_ref()
                        .map(|name| lower_name(syntax_trees, name)),
                    external_binding,
                    via_expression,
                    external_binding_source_span: clause.via_keyword_source_span,
                },
            );
    }

    Ok(span)
}

fn external_binding_identity(
    syntax_trees: &syntax_trees::SyntaxTrees,
    binding: &ExternalBinding,
) -> semantic_vocabulary::ExternalBindingIdentity {
    use semantic_vocabulary::ExternalBindingIdentity;

    match binding {
        ExternalBinding::Syscall { number } => ExternalBindingIdentity::Syscall { number: *number },
        ExternalBinding::CompilerIntrinsic => ExternalBindingIdentity::CompilerIntrinsic,
        ExternalBinding::VtableSlot { index } => {
            ExternalBindingIdentity::VtableSlot { index: *index }
        }
        ExternalBinding::VtableField { field } => ExternalBindingIdentity::VtableField {
            field: field.as_str(syntax_trees).to_owned(),
        },
        ExternalBinding::TableFunction { field } => ExternalBindingIdentity::TableFunction {
            field: field.as_str(syntax_trees).to_owned(),
        },
    }
}

fn lower_machine_states(
    lowerer: &mut Lowerer,
    syntax_trees: &SyntaxTrees,
    states: HandleSpan<StateHandle>,
    is_public: bool,
) -> Result<HandleSpan<Handle<State>>, Diagnostic> {
    let mut span = HandleSpan::empty();

    for (ordinal, state) in syntax_trees.items.state_handles(states).iter().enumerate() {
        let authored_type = syntax_trees.items.state(*state).return_type;
        let state = lower_state_node(
            lowerer,
            syntax_trees,
            syntax_trees.items.state(*state),
            is_public && ordinal == 0,
        )?;
        let state = lowerer
            .symbol_resolved_trees
            .tables
            .declarations
            .machine_states
            .append(state);
        if authored_type.is_valid()
            && let Some(sources) = &mut lowerer.equation_sources
        {
            sources.declared_types.push((
                crate::resolution::declarations::machine_equations::DeclaredTypeOrigin::Result(
                    state,
                ),
                authored_type,
            ));
        }
        lowerer
            .symbol_resolved_trees
            .tables
            .declarations
            .machine_state_handles
            .append_to_span(&mut span, state);
    }

    Ok(span)
}

/*
    The sealed intrinsic catalog row this authored declaration matches, if
    any. Lowering reads the catalog before the machine has a symbol, from the
    same toolchain file and declaration path the symbol table later reports.
*/
fn sealed_intrinsic(
    lowerer: &Lowerer,
    syntax_trees: &SyntaxTrees,
    machine: &syntax::item::Machine,
) -> Option<language_core::sealed_intrinsics::SealedIntrinsic> {
    let file = lowerer
        .sources
        .as_ref()?
        .get(machine.name.source_span().source_id)?;
    crate::source_symbols::sealed_intrinsic_at(file, machine.name.as_str(syntax_trees))
}

/*
    A core `NativeFloats` member declares implicit compiler-intrinsic supply
    only in its exact bodyless single-relation shape. Typed validation must
    still authenticate the exact public relation, complete scalar signature
    and numeric meaning before it can be consumed.
*/
fn is_sealed_float_provider_member(
    lowerer: &Lowerer,
    syntax_trees: &SyntaxTrees,
    machine: &syntax::item::Machine,
) -> bool {
    let [edge] = syntax_trees.items.satisfies_clauses(machine.satisfies) else {
        return false;
    };
    machine.bodyless
        && machine.boundary
        && machine.attached_data.is_none()
        && edge.via.is_none()
        && !edge.via_expression.is_valid()
        && sealed_intrinsic(lowerer, syntax_trees, machine)
            == Some(language_core::sealed_intrinsics::SealedIntrinsic::NativeFloatOperation)
}

/*
    Classify exact bundled compiler operations before deciding whether a
    bodyless guarantee is an authored admission claim. Typed checking and
    execution still validate the signature and discharge its operation duties.
*/
fn compiler_owned_boundary(
    lowerer: &Lowerer,
    syntax: &SyntaxTrees,
    machine: &syntax::item::Machine,
) -> bool {
    matches!(
        sealed_intrinsic(lowerer, syntax, machine),
        Some(language_core::sealed_intrinsics::SealedIntrinsic::StaticInterrupt(_))
    )
}
