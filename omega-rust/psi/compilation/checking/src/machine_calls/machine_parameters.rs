//! MP2b: admission of compile-time machine-symbol arguments.
//!
//! Static selections are checked at the generic call edge. The selected
//! machine must be concrete, match the authored callable shape, stay within
//! the required service and operational ceilings, and conservatively refine
//! conjunctive requires/ensures facts. This pass never invents a callback
//! contract.

mod callable_shape;
mod selected_dependencies;
pub(crate) use selected_dependencies::machine_selection_dependency_symbols;
mod closed_types;
pub use closed_types::closed_static_call_type_bindings;
pub(crate) mod contract_facts;
mod nominal_admission;
mod type_refinement;
pub(super) use type_refinement::infer_call_type_arguments;

use callable_shape::validate_selected_callable_effects;
use callable_shape::validate_selected_callable_shape;
pub(crate) use callable_shape::validate_trait_callable_parameter_refinement;
pub(crate) use contract_facts::validate_callable_contract_refinement;
use nominal_admission::validate_nominal_machine_selection;

use diagnostics::Diagnostic;
use language_semantics::declaration_selection::BuildOperation;
use symbols::{JoinedPath, SymbolHandle, SymbolKind};
use typed_trees::data::{MachineParameterContract, TypeParameter, TypeParameterKind};
use typed_trees::expression::{ExpressionHandle, ExpressionNode, StaticMachineArgument};
use typed_trees::machine::Machine;
use typed_trees::state::State;
use typed_trees::statement::StatementHandle;
use typed_trees::types::TypeReferenceHandle;

#[derive(Clone, Copy)]
struct MachineSuspensionRow {
    symbol: SymbolHandle,
    transitive_may_suspend: bool,
}

#[derive(Clone, Copy)]
struct MachineBlockingRow {
    symbol: SymbolHandle,
    transitive_may_block: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ValidatedNominalMachineUseSite {
    Statement(StatementHandle),
    Expression(ExpressionHandle),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidatedNominalMachineUse {
    pub site: ValidatedNominalMachineUseSite,
    pub registration_operation: SymbolHandle,
    pub static_machine_ordinal: u32,
    pub selected_machine: SymbolHandle,
    pub selected_entry: SymbolHandle,
    pub satisfaction_trait: SymbolHandle,
    pub satisfaction_requirement: SymbolHandle,
    pub canonical_requirement_overload: String,
}

/// One `Type` generic-parameter substitution MP2b's callable-shape
/// refinement derived while admitting a static machine argument. The
/// nominal satisfaction path replays this judgment through contract
/// envelopes, but a structural selection leaves no such row; this binding
/// is the only retained record of what the call instantiated.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ValidatedRequirementCallTypeBinding {
    /// The callee's generic `Type` parameter.
    pub parameter: SymbolHandle,
    /// The actual type the selected callable's shape bound to it.
    pub actual: TypeReferenceHandle,
}

/// One admitted `machine`-binder selection at a generic call edge, keyed by
/// the same telescope ordinal nominal use rows retain.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ValidatedRequirementCallMachineSelection {
    /// Telescope ordinal of the callee's `machine` parameter the argument
    /// was admitted against.
    pub static_machine_ordinal: u32,
    /// The `machine` parameter's symbol; its retained contract view names
    /// the requirement this selection satisfies (`Target`).
    pub parameter: SymbolHandle,
    /// The concrete machine owning the selected entry state. Invalid when
    /// `selected` forwards an in-scope `machine` binder.
    pub selected_machine: SymbolHandle,
    /// The admitted selection: the selected concrete machine's resolved
    /// entry state, or the forwarded binder's own parameter symbol.
    pub selected: SymbolHandle,
}

/// The specialization MP2b derived at one generic call's admitted static
/// machine selections, which the nominal path used to drop: the
/// substitution over the callee's generic `Type` parameters plus the exact
/// provider evidence — which concrete machine entry, or forwarded in-scope
/// binder, each `machine` parameter bound. A call that names no static
/// machine argument emits no row.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidatedRequirementCallSpecialization {
    /// Exact authored call site, keyed identically to nominal use rows.
    pub site: ValidatedNominalMachineUseSite,
    /// The generic callee the call named: a requirement signature, a machine
    /// parameter contract's call target, or a generic machine's entry state.
    pub registration_operation: SymbolHandle,
    /// Derived `Type` substitutions, in derivation order.
    pub type_bindings: Vec<ValidatedRequirementCallTypeBinding>,
    /// One row per admitted static machine argument, in telescope order.
    pub machine_selections: Vec<ValidatedRequirementCallMachineSelection>,
}

/// The complete static-machine selection evidence one validation pass
/// retained: nominal satisfaction rows plus the specializations generic
/// calls derived at their `machine` arguments.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ValidatedStaticMachineSelections {
    /// ENT4: exact nominal satisfaction selected for each admitted static
    /// machine argument.
    pub nominal_uses: Vec<ValidatedNominalMachineUse>,
    /// The specialization each call's admitted static machine arguments
    /// derived over the callee's generic parameters.
    pub requirement_call_specializations: Vec<ValidatedRequirementCallSpecialization>,
}

/// Invocation-local formation work. Its rows cannot be consumed as admitted
/// selections until concrete effect obligations have also been discharged.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct FormedStaticMachineSelections {
    rows: ValidatedStaticMachineSelections,
    pending_effects: Vec<StaticMachineEffectObligation>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct StaticMachineEffectObligation {
    parameter: SymbolHandle,
    selected: SymbolHandle,
    label: String,
}

fn project_operational_rows(
    operational: &flow_effects::OperationalPlan,
) -> (Vec<MachineSuspensionRow>, Vec<MachineBlockingRow>) {
    let suspensions = operational
        .machines()
        .iter()
        .map(|summary| MachineSuspensionRow {
            symbol: summary.symbol,
            transitive_may_suspend: summary.transitive_may_suspend,
        })
        .collect();
    let blockings = operational
        .machines()
        .iter()
        .map(|summary| MachineBlockingRow {
            symbol: summary.symbol,
            transitive_may_block: summary.transitive_may_block,
        })
        .collect();
    (suspensions, blockings)
}

pub(crate) fn form_static_machine_selections(
    program: &dyn crate::CheckingInput,
    diagnostics: &mut Vec<Diagnostic>,
) -> FormedStaticMachineSelections {
    let mut selections = FormedStaticMachineSelections::default();
    let mathematical_expressions = crate::mathematical_applications::expression_nodes(program);
    // Every call site resolves its machine/state target through this index;
    // the program is frozen for the duration of the pass so one build covers
    // all lookups.
    let index = machine_state_index(program);
    for (handle, expression) in program.expression_table.iter_expressions() {
        if let ExpressionNode::Call(call) = expression {
            // These occurrences belong to mathematical declaration elaboration,
            // whose kernel checks their level and term applications. A runtime
            // occurrence naming the same declaration is still checked here.
            if mathematical_expressions.contains(&handle) {
                continue;
            }
            // Named operators own their complete static telescope in the
            // ordinary value-call validator. MP2b governs executable machine
            // selections and must not reinterpret operator type arguments as
            // machine authority.
            if !typed_trees::operator::named_expression_call_candidates(program, call).is_empty() {
                continue;
            }
            let (target_symbol, target, arguments) = if call.authored_machine_application.is_valid()
            {
                let application = program
                    .expression_table
                    .static_machine_application(call.authored_machine_application);
                (
                    application.registration_operation,
                    application.target.as_str(),
                    program
                        .expression_table
                        .static_machine_application_arguments(call.authored_machine_application),
                )
            } else {
                (
                    call.target_symbol,
                    call.target.as_str(),
                    call.machine_arguments.as_ref(),
                )
            };
            validate_call_selection(
                program,
                &index,
                ValidatedNominalMachineUseSite::Expression(handle),
                target_symbol,
                target,
                arguments,
                diagnostics,
                &mut selections,
            );
        }
    }

    for machine in program.machines() {
        for state in program.machine_states(machine) {
            for (statement_handle, statement) in program
                .statement_table
                .iter_statements(state.statement_nodes)
            {
                if let Some(call) = statement.as_call() {
                    if !typed_trees::operator::named_statement_call_candidates(program, call)
                        .is_empty()
                    {
                        continue;
                    }
                    let (target_symbol, target, arguments) =
                        if call.authored_machine_application.is_valid() {
                            let application = program
                                .expression_table
                                .static_machine_application(call.authored_machine_application);
                            (
                                application.registration_operation,
                                application.target.as_str(),
                                program
                                    .expression_table
                                    .static_machine_application_arguments(
                                        call.authored_machine_application,
                                    ),
                            )
                        } else {
                            (
                                call.target_symbol,
                                call.target.as_str(),
                                call.machine_arguments.as_ref(),
                            )
                        };
                    validate_call_selection(
                        program,
                        &index,
                        ValidatedNominalMachineUseSite::Statement(statement_handle),
                        target_symbol,
                        target,
                        arguments,
                        diagnostics,
                        &mut selections,
                    );
                }
            }
        }
    }
    selections
}

pub(crate) fn admit_static_machine_effects(
    program: &dyn crate::CheckingInput,
    operational: &flow_effects::OperationalPlan,
    formed: FormedStaticMachineSelections,
) -> Result<ValidatedStaticMachineSelections, Vec<Diagnostic>> {
    let mut diagnostics = Vec::new();
    let service_reaches = crate::infer_service_reaches(program, operational);
    let invocations = crate::infer_synchronous_invocations(program);
    let (suspensions, blockings) = project_operational_rows(operational);
    let index = machine_state_index(program);
    for obligation in formed.pending_effects {
        let parameter = program
            .data_type_parameter_by_symbol(obligation.parameter)
            .expect("formed selection retains its original binder");
        let TypeParameterKind::Machine { contract } = &parameter.kind else {
            unreachable!("formed machine selection has a machine binder")
        };
        let requirement = machine_parameter_signature(program, contract);
        let (machine, state) = machine_and_state(program, &index, obligation.selected)
            .expect("formed concrete selection retains its callable entry");
        validate_selected_callable_effects(
            program,
            &suspensions,
            &blockings,
            &service_reaches,
            &invocations,
            &obligation.label,
            parameter,
            requirement,
            machine,
            state,
            &mut diagnostics,
        );
    }
    if diagnostics.is_empty() {
        Ok(formed.rows)
    } else {
        Err(diagnostics)
    }
}

pub fn validate_static_machine_selections_with_facts(
    program: &dyn crate::CheckingInput,
) -> Result<ValidatedStaticMachineSelections, Vec<Diagnostic>> {
    let mut diagnostics = Vec::new();
    let selections = form_static_machine_selections(program, &mut diagnostics);
    if diagnostics.is_empty() {
        let operational = crate::infer_operational_may(program);
        admit_static_machine_effects(program, &operational, selections)
    } else {
        Err(diagnostics)
    }
}

/// The machine selections among a call's `<>` arguments, in authored order.
/// The `n`th selection binds the callee's `n`th `machine` binder; type and
/// const arguments occupy no machine ordinal. Every consumer that joins a
/// binder to its argument (admission here, native callback destinations in
/// receiver-call validation) shares this one ordinal space.
pub(crate) fn machine_selection_arguments<'call>(
    program: &dyn crate::CheckingInput,
    arguments: &'call [StaticMachineArgument],
) -> impl Iterator<Item = &'call StaticMachineArgument> {
    arguments
        .iter()
        .filter(|argument| is_machine_selection_symbol(program, argument.symbol))
}

fn is_machine_selection_symbol(program: &dyn crate::CheckingInput, symbol: SymbolHandle) -> bool {
    symbol.is_valid()
        && matches!(
            program.symbols.get(symbol).kind,
            SymbolKind::State | SymbolKind::MachineParameter
        )
}

fn validate_call_selection(
    program: &dyn crate::CheckingInput,
    index: &MachineStateIndex,
    site: ValidatedNominalMachineUseSite,
    target_symbol: SymbolHandle,
    target_name: &str,
    arguments: &[StaticMachineArgument],
    diagnostics: &mut Vec<Diagnostic>,
    selections: &mut FormedStaticMachineSelections,
) {
    /// Split a type-parameter signature into its `Machine`-kind contract
    /// requirements and its `Type`-kind generic slots, in declaration order.
    fn contract_and_type_parameters<'p>(
        parameters: &'p [TypeParameter],
    ) -> (
        Vec<(&'p TypeParameter, &'p MachineParameterContract)>,
        Vec<&'p TypeParameter>,
    ) {
        (
            parameters
                .iter()
                .filter_map(|parameter| match &parameter.kind {
                    TypeParameterKind::Machine { contract } => Some((parameter, contract)),
                    _ => None,
                })
                .collect(),
            parameters
                .iter()
                .filter(|parameter| matches!(parameter.kind, TypeParameterKind::Type))
                .collect(),
        )
    }

    // The build operations whose static operands name declarations, not
    // machine requirements, carry no requirement calls to validate.
    if !target_symbol.is_valid()
        && matches!(
            BuildOperation::from_call_target(target_name),
            Some(
                BuildOperation::ProviderSelection
                    | BuildOperation::BoundaryAcceptance
                    | BuildOperation::WireCompatibilityRequest
                    | BuildOperation::RepresentationSelection
                    | BuildOperation::ServiceExclusion
            )
        )
    {
        return;
    }
    let (requirements, generic_types): (Vec<_>, Vec<_>) = if let Some((callee, _)) =
        machine_and_state(program, index, target_symbol)
    {
        contract_and_type_parameters(program.machine_type_parameters(callee))
    } else if let Some((declaring_machine, signature)) =
        program.machine_parameter_signature(target_symbol)
    {
        (
            contract_and_type_parameters(program.state_signature_type_parameters(signature)).0,
            contract_and_type_parameters(program.machine_type_parameters(declaring_machine)).1,
        )
    } else if let Some(signature) = program
        .traits()
        .iter()
        .flat_map(|definition| program.trait_machine_signatures(definition))
        .find(|signature| signature.symbol == target_symbol)
    {
        contract_and_type_parameters(program.state_signature_type_parameters(signature))
    } else {
        if !arguments.is_empty() {
            diagnostics.push(Diagnostic::error(format!(
                    "call `{target_name}` supplies static machine arguments, but its generic callee did not resolve"
                )));
        }
        return;
    };

    if let Some(projection) = arguments
        .iter()
        .find_map(|argument| argument.evidence_projection.as_ref())
    {
        diagnostics.push(Diagnostic::error(format!(
            "proof-static evidence projection `{}.{}` cannot select an executable machine parameter; erased evidence cannot eliminate into runtime computation",
            projection.term, projection.member
        )));
        return;
    }

    let machine_arguments = machine_selection_arguments(program, arguments).collect::<Vec<_>>();
    if machine_arguments.len() != requirements.len() {
        diagnostics.push(Diagnostic::error(format!(
            "generic call `{target_name}` requires {} static machine argument(s), got {}",
            requirements.len(),
            machine_arguments.len()
        )));
        return;
    }

    let mut bindings = Vec::new();
    let mut machine_selections = Vec::new();

    for (static_machine_ordinal, ((parameter, requirement), selected)) in
        requirements.into_iter().zip(machine_arguments).enumerate()
    {
        let rendered = selected.path.joined_path();
        if selected.application.is_some() {
            diagnostics.push(Diagnostic::error(format!(
                "static machine argument `{rendered}` for `{}` is a nested machine application; recursive specialization identity is not yet supported",
                parameter.name
            )));
            continue;
        }
        // A recursive generic body may forward its own authored machine
        // parameter (`map<F>(tail)`). This is not a concrete selection yet,
        // but it is already governed by exactly this requirement; the
        // eventual external selection is validated at the concrete call edge.
        if selected.symbol == parameter.symbol {
            continue;
        }
        if !selected.symbol.is_valid() {
            diagnostics.push(Diagnostic::error(format!(
                "static machine argument `{rendered}` for `{}` does not resolve to a concrete machine",
                parameter.name
            )));
            continue;
        }
        let Ok(nominal_selection) = validate_nominal_machine_selection(
            program,
            index,
            target_name,
            parameter,
            requirement,
            selected.symbol,
            &rendered,
            &generic_types,
            &mut bindings,
            diagnostics,
        ) else {
            continue;
        };
        let requirement = machine_parameter_signature(program, requirement);
        validate_selected_callable_shape(
            program,
            index,
            target_name,
            parameter,
            requirement,
            selected.symbol,
            &rendered,
            &generic_types,
            &mut bindings,
            diagnostics,
        );
        // Retain which selection this binder admitted even when no nominal
        // satisfaction row exists: a structural contract's bindings are
        // otherwise discarded here and `build_call_operation` can never
        // recover what `Target` (or `T`, `Arguments`) instantiated to.
        let (selected_machine, selection_symbol) =
            machine_and_state(program, index, selected.symbol)
                .map(|(machine, state)| (machine.symbol, state.symbol))
                .unwrap_or((SymbolHandle::invalid(), selected.symbol));
        if let Some((machine, _)) = machine_and_state(program, index, selected.symbol) {
            selections
                .pending_effects
                .push(StaticMachineEffectObligation {
                    parameter: parameter.symbol,
                    selected: selection_symbol,
                    label: format!("machine argument `{}` for `{target_name}`", machine.name),
                });
        }
        machine_selections.push(ValidatedRequirementCallMachineSelection {
            static_machine_ordinal: u32::try_from(static_machine_ordinal)
                .expect("static machine argument ordinal overflow"),
            parameter: parameter.symbol,
            selected_machine,
            selected: selection_symbol,
        });
        if let Some(selection) = nominal_selection {
            selections
                .rows
                .nominal_uses
                .push(ValidatedNominalMachineUse {
                    site,
                    registration_operation: target_symbol,
                    static_machine_ordinal: u32::try_from(static_machine_ordinal)
                        .expect("static machine argument ordinal overflow"),
                    selected_machine: selection.selected_machine,
                    selected_entry: selection.selected_entry,
                    satisfaction_trait: selection.satisfaction_trait,
                    satisfaction_requirement: selection.satisfaction_requirement,
                    canonical_requirement_overload: selection.canonical_requirement_overload,
                });
        }
    }

    // Retain the call-edge specialization this admission derived. MP4
    // consumes the static argument syntax and the nominal rows only cover
    // `Nominal` contracts, so a structural requirement call like
    // `runtime.start<Worker::run>(token)` otherwise loses `T`, `Arguments`,
    // and `Target` before checked call planning can see them.
    if !machine_selections.is_empty() {
        selections.rows.requirement_call_specializations.push(
            ValidatedRequirementCallSpecialization {
                site,
                registration_operation: target_symbol,
                type_bindings: bindings
                    .iter()
                    .map(|binding| ValidatedRequirementCallTypeBinding {
                        parameter: binding.symbol,
                        actual: binding.actual,
                    })
                    .collect(),
                machine_selections,
            },
        );
    }
}

/// N7 data-family admission uses the same refinement judgment as a generic
/// call, but its selected symbol is carried in a generic type argument rather
/// than an expression call node. Keeping this entry point here prevents proof
/// data from growing a weaker, shape-only callback check.
pub(crate) fn validate_data_machine_selection(
    program: &dyn crate::CheckingInput,
    family_name: &str,
    parameter: &TypeParameter,
    requirement: &MachineParameterContract,
    selected_symbol: SymbolHandle,
    selected_name: &str,
    generic_types: &[&TypeParameter],
    diagnostics: &mut Vec<Diagnostic>,
) {
    let index = machine_state_index(program);
    let mut bindings = Vec::new();
    if validate_nominal_machine_selection(
        program,
        &index,
        family_name,
        parameter,
        requirement,
        selected_symbol,
        selected_name,
        generic_types,
        &mut bindings,
        diagnostics,
    )
    .is_err()
    {
        return;
    }
    let requirement = machine_parameter_signature(program, requirement);
    let (service_reaches, suspensions, blockings) = {
        let operational = crate::infer_operational_may(program);
        let service_reaches = crate::infer_service_reaches(program, &operational);
        let (suspensions, blockings) = project_operational_rows(&operational);
        (service_reaches, suspensions, blockings)
    };
    let invocations = crate::infer_synchronous_invocations(program);
    validate_selected_callable_shape(
        program,
        &index,
        family_name,
        parameter,
        requirement,
        selected_symbol,
        selected_name,
        generic_types,
        &mut bindings,
        diagnostics,
    );
    if let Some((machine, state)) = machine_and_state(program, &index, selected_symbol) {
        let label = format!("machine argument `{}` for `{family_name}`", machine.name);
        validate_selected_callable_effects(
            program,
            &suspensions,
            &blockings,
            &service_reaches,
            &invocations,
            &label,
            parameter,
            requirement,
            machine,
            state,
            diagnostics,
        );
    }
}

fn machine_parameter_contract(
    program: &dyn crate::CheckingInput,
    symbol: SymbolHandle,
) -> Option<(&TypeParameter, &typed_trees::signature::StateSignature)> {
    let (parameter, contract) = machine_parameter_contract_definition(program, symbol)?;
    Some((parameter, machine_parameter_signature(program, contract)))
}

fn machine_parameter_contract_definition(
    program: &dyn crate::CheckingInput,
    symbol: SymbolHandle,
) -> Option<(&TypeParameter, &MachineParameterContract)> {
    for machine in program.machines() {
        if let Some(found) = machine_parameter_contract_definition_in(
            program,
            program.machine_type_parameters(machine),
            symbol,
        ) {
            return Some(found);
        }
    }
    for data in program.data_definitions() {
        if let Some(found) = machine_parameter_contract_definition_in(
            program,
            program.data_type_parameters(data),
            symbol,
        ) {
            return Some(found);
        }
    }
    None
}

fn machine_parameter_contract_definition_in<'program>(
    program: &'program dyn crate::CheckingInput,
    parameters: &'program [TypeParameter],
    symbol: SymbolHandle,
) -> Option<(&'program TypeParameter, &'program MachineParameterContract)> {
    for parameter in parameters {
        let TypeParameterKind::Machine { contract } = &parameter.kind else {
            continue;
        };
        if parameter.symbol == symbol {
            return Some((parameter, contract));
        }
        let signature = machine_parameter_signature(program, contract);
        if let Some(found) = machine_parameter_contract_definition_in(
            program,
            program.state_signature_type_parameters(signature),
            symbol,
        ) {
            return Some(found);
        }
    }
    None
}

fn machine_parameter_signature<'program>(
    program: &'program dyn crate::CheckingInput,
    contract: &'program MachineParameterContract,
) -> &'program typed_trees::signature::StateSignature {
    program
        .machine_parameter_contract_view(contract)
        .expect("typed machine-parameter contract must retain a valid requirement identity")
        .signature()
}

/// State symbol to its `(machine position, state position)`; a machine symbol
/// points at its own first state, matching `machine_and_state` precedence:
/// state names win over the machine name within one machine, and the first
/// machine in declaration order wins on the (impossible in practice) collision.
type MachineStateIndex = symbols::SymbolKeyMap<SymbolHandle, (u32, u32)>;

fn machine_state_index(program: &dyn crate::CheckingInput) -> MachineStateIndex {
    let mut index = MachineStateIndex::default();
    for (machine_position, machine) in program.machines().iter().enumerate() {
        let states = program.machine_states(machine);
        for (state_position, state) in states.iter().enumerate() {
            index.insert(
                state.symbol,
                (
                    u32::try_from(machine_position).expect("machine position overflow"),
                    u32::try_from(state_position).expect("state position overflow"),
                ),
            );
        }
        if !states.is_empty() {
            index.entry(machine.symbol).or_insert((
                u32::try_from(machine_position).expect("machine position overflow"),
                0,
            ));
        }
    }
    index
}

fn machine_and_state<'a>(
    program: &'a dyn crate::CheckingInput,
    index: &MachineStateIndex,
    selected_symbol: SymbolHandle,
) -> Option<(&'a Machine, &'a State)> {
    if !selected_symbol.is_valid() {
        return None;
    }
    let (machine_position, state_position) = index.get(&selected_symbol)?;
    let machine = program
        .machines()
        .get(*machine_position as usize)
        .expect("machine-state index position out of range");
    let state = program
        .machine_states(machine)
        .get(*state_position as usize)
        .expect("machine-state index state out of range");
    Some((machine, state))
}
