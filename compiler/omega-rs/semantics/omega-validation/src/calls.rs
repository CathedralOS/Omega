use crate::arithmetic_domains::{self, ValueEnv};
use crate::expression_types::{
    argument_matches_type_reference_handle, expression_type_name_handle, report_cross_class_store,
    report_data_type_conflict,
};
use crate::locals::WritableRoots;
use crate::places::declared_place_type;
use crate::properties::{
    declared_property_requirements, referenced_type_parameter, type_satisfies_declared_property,
};
use crate::struct_literals::data_declares_field;
use crate::symbols::{MachineSymbols, TopLevelSymbols};
use crate::type_references::type_reference_label;
use omega_core::arena::HandleSpan;
use omega_core::diagnostics::Diagnostic;
use omega_core::symbols::SymbolHandle;
use omega_facts::NormalizedWriteFrame;
use omega_typed_trees::TypedTrees;
use omega_typed_trees::data::{DataMember, TypeParameterKind};
use omega_typed_trees::expression::{ExpressionHandle, ExpressionNode, TableCallExpression};
use omega_typed_trees::machine::Machine;
use omega_typed_trees::name::Identifier;
use omega_typed_trees::signature::StateParameter;
use omega_typed_trees::state::State;
use omega_typed_trees::statement::{
    StatementNode, TableCall, TransitionGuardNode, TransitionTargetNode,
};
use omega_typed_trees::types::{PrimitiveType, TypeReferenceHandle, TypeReferenceNode};

/// Shared conservative call-frame resolver. A complete result is the set of
/// caller-visible places the call may write; `None` is deliberately opaque and
/// requires consumers to invalidate every fact they cannot otherwise prove.
///
/// The resolver owns the top-level symbol cache so validation, proof, recast,
/// and invariant consumers share one resolution law instead of reimplementing
/// call identity. Per-machine caches are built at the query boundary and fail
/// closed if the program's symbols are already invalid.
pub struct CallFrameResolver<'program> {
    program: &'program TypedTrees,
    symbols: TopLevelSymbols<'program>,
}

impl<'program> CallFrameResolver<'program> {
    pub fn new(program: &'program TypedTrees) -> Option<Self> {
        let mut diagnostics = Vec::new();
        let symbols = TopLevelSymbols::build(program, &mut diagnostics);
        diagnostics.is_empty().then_some(Self { program, symbols })
    }

    pub fn may_write_paths(
        &self,
        current_machine: &'program Machine,
        call: &TableCall,
    ) -> Option<Vec<String>> {
        self.may_write_frame(current_machine, call)
            .into_complete_paths()
    }

    pub fn may_write_frame(
        &self,
        current_machine: &'program Machine,
        call: &TableCall,
    ) -> NormalizedWriteFrame {
        let mut diagnostics = Vec::new();
        let machine_symbols =
            MachineSymbols::build(self.program, current_machine, &mut diagnostics);
        if !diagnostics.is_empty() {
            return NormalizedWriteFrame::opaque();
        }
        let paths = known_call_written_paths(
            self.program,
            call,
            current_machine,
            &machine_symbols,
            &self.symbols,
        )
        .or_else(|| {
            known_boundary_call_written_paths(self.program, &machine_symbols, &self.symbols, call)
        });
        paths.map_or_else(NormalizedWriteFrame::opaque, NormalizedWriteFrame::complete)
    }

    /// Conservative aggregate frame of every value-position call nested in
    /// `expression`. `Some([])` means the expression is call-free; `None`
    /// means at least one call is opaque, so consumers must fail closed.
    pub fn expression_may_write_paths(
        &self,
        current_machine: &'program Machine,
        expression: ExpressionHandle,
    ) -> Option<Vec<String>> {
        self.expression_write_frame(current_machine, expression)
            .into_complete_paths()
    }

    pub fn expression_write_frame(
        &self,
        current_machine: &'program Machine,
        expression: ExpressionHandle,
    ) -> NormalizedWriteFrame {
        let mut diagnostics = Vec::new();
        let machine_symbols =
            MachineSymbols::build(self.program, current_machine, &mut diagnostics);
        if !diagnostics.is_empty() {
            return NormalizedWriteFrame::opaque();
        }
        let mut written = Vec::new();
        let complete = collect_expression_call_written_paths(
            self.program,
            expression,
            current_machine,
            &machine_symbols,
            &self.symbols,
            &mut written,
        )
        .is_some();
        if complete {
            NormalizedWriteFrame::complete(written)
        } else {
            NormalizedWriteFrame::opaque()
        }
    }

    /// Aggregate only the value-position calls embedded in a statement. The
    /// statement-position call itself is handled separately by
    /// `may_write_paths`; its receiver is a path, not an evaluated expression.
    pub fn statement_value_may_write_paths(
        &self,
        current_machine: &'program Machine,
        statement: &StatementNode,
    ) -> Option<Vec<String>> {
        self.statement_value_write_frame(current_machine, statement)
            .into_complete_paths()
    }

    pub fn statement_value_write_frame(
        &self,
        current_machine: &'program Machine,
        statement: &StatementNode,
    ) -> NormalizedWriteFrame {
        let mut diagnostics = Vec::new();
        let machine_symbols =
            MachineSymbols::build(self.program, current_machine, &mut diagnostics);
        if !diagnostics.is_empty() {
            return NormalizedWriteFrame::opaque();
        }
        let mut written = Vec::new();
        for expression in statement_value_expression_roots(self.program, statement) {
            if collect_expression_call_written_paths(
                self.program,
                expression,
                current_machine,
                &machine_symbols,
                &self.symbols,
                &mut written,
            )
            .is_none()
            {
                return NormalizedWriteFrame::opaque();
            }
        }
        NormalizedWriteFrame::complete(written)
    }

    /// Body-derived frame in the target state's own namespace. `self` remains
    /// `self`; non-self state parameters normalize positionally as `$P<N>`, so
    /// source renames and discovery order do not perturb implementation identity.
    pub fn inferred_state_write_frame(
        &self,
        machine: &'program Machine,
        state: &'program State,
    ) -> NormalizedWriteFrame {
        if !self
            .program
            .machine_states(machine)
            .iter()
            .any(|candidate| candidate.symbol == state.symbol)
        {
            return NormalizedWriteFrame::opaque();
        }
        let mut active_states = vec![state.symbol];
        let Some(relative_paths) = summarize_state_written_paths(
            self.program,
            machine,
            state,
            &self.symbols,
            &mut active_states,
        ) else {
            return NormalizedWriteFrame::opaque();
        };
        let mut normalized = Vec::new();
        for relative in relative_paths {
            match normalize_state_relative_path(self.program, state, &relative) {
                Some(Some(path)) => normalized.push(path),
                Some(None) => {}
                None => return NormalizedWriteFrame::opaque(),
            }
        }
        NormalizedWriteFrame::complete(normalized)
    }
}

/// Parent/child places overlap in both directions: writing `self.item` kills a
/// fact about `self.item.len`, and writing the child kills a whole-value fact.
pub fn frame_paths_overlap(left: &str, right: &str) -> bool {
    left == right
        || left
            .strip_prefix(right)
            .is_some_and(|suffix| suffix.starts_with('.') || suffix.starts_with('['))
        || right
            .strip_prefix(left)
            .is_some_and(|suffix| suffix.starts_with('.') || suffix.starts_with('['))
}

fn statement_value_expression_roots(
    program: &TypedTrees,
    statement: &StatementNode,
) -> Vec<ExpressionHandle> {
    let mut roots = Vec::new();
    match statement {
        StatementNode::AssemblyFact(fact) => roots.push(fact.expression),
        StatementNode::Assignment(assignment) => {
            roots.push(assignment.target);
            roots.push(assignment.value);
        }
        StatementNode::Call(call) => roots.extend(
            program
                .statement_table
                .expression_handles(call.arguments)
                .iter()
                .copied(),
        ),
        StatementNode::Expression(expression) => roots.push(*expression),
        StatementNode::LocalData(local) => roots.push(local.initial_value),
        StatementNode::Transition(transition) => {
            if let TransitionGuardNode::When(guard) = transition.guard {
                roots.push(guard);
            }
            for target in [transition.target, transition.continuation] {
                if !target.is_valid() {
                    continue;
                }
                match program.statement_table.transition_target(target) {
                    TransitionTargetNode::Named { arguments, .. } => roots.extend(
                        program
                            .statement_table
                            .expression_handles(*arguments)
                            .iter()
                            .copied(),
                    ),
                    TransitionTargetNode::Value(value) => roots.push(*value),
                    TransitionTargetNode::SelfTarget | TransitionTargetNode::Terminal => {}
                }
            }
        }
    }
    roots
}

#[allow(clippy::too_many_arguments)]
fn collect_expression_call_written_paths(
    program: &TypedTrees,
    expression: ExpressionHandle,
    current_machine: &Machine,
    machine_symbols: &MachineSymbols<'_>,
    symbols: &TopLevelSymbols<'_>,
    written: &mut Vec<String>,
) -> Option<()> {
    if !expression.is_valid() {
        return Some(());
    }
    let mut visit = |child| {
        collect_expression_call_written_paths(
            program,
            child,
            current_machine,
            machine_symbols,
            symbols,
            written,
        )
    };
    match program.expression_table.expression(expression) {
        ExpressionNode::Atomic(atomic) => visit(atomic.value)?,
        ExpressionNode::Call(call) => {
            if call.receiver.is_valid() {
                visit(call.receiver)?;
            }
            for argument in program.expression_table.expression_handles(call.arguments) {
                visit(*argument)?;
            }
            // Reserved value/view builtins are operand operations, not machine
            // calls. They may read their operands or create a view, but they do
            // not write caller storage. Keep this list aligned with the value
            // call validation exemptions below so frame consumers do not turn
            // `min`/`max` reductions into opaque whole-receiver clobbers.
            if value_builtin_has_empty_write_frame(call.target.as_str()) {
                return Some(());
            }
            let receiver_members = if call.receiver.is_valid() {
                receiver_member_chain(program, call.receiver)?
            } else {
                Vec::new()
            };
            let arguments = program.expression_table.expression_handles(call.arguments);
            let paths = known_call_written_paths_for_parts(
                program,
                call.target_symbol,
                call.target.as_str(),
                &receiver_members,
                arguments,
                current_machine,
                machine_symbols,
                symbols,
                &mut Vec::new(),
            )
            .or_else(|| {
                known_boundary_call_written_paths_for_parts(
                    program,
                    machine_symbols,
                    symbols,
                    &receiver_members,
                    call.target.as_str(),
                    arguments,
                )
            })
            // Even when the callee body is opaque (transitioning, cyclic,
            // static-machine, or unresolved), ownership still gives a sound
            // caller-visible floor: it cannot mutate an unpassed caller local.
            // Conservatively poison the whole receiver (`self` for an implicit
            // receiver) plus every explicit mutable argument.
            .or_else(|| syntactic_call_written_paths(program, &receiver_members, arguments))?;
            for path in paths {
                if !written.contains(&path) {
                    written.push(path);
                }
            }
        }
        ExpressionNode::Binary(binary) => {
            visit(binary.left)?;
            visit(binary.right)?;
        }
        ExpressionNode::Unary(unary) => visit(unary.operand)?,
        ExpressionNode::Cast(cast) => visit(cast.value)?,
        ExpressionNode::Indexed(indexed) => {
            visit(indexed.collection)?;
            visit(indexed.index)?;
        }
        ExpressionNode::Member(member) => visit(member.receiver)?,
        ExpressionNode::Mutable(inner) => visit(*inner)?,
        ExpressionNode::ArrayLiteral(elements) => {
            for element in program.expression_table.expression_handles(*elements) {
                visit(*element)?;
            }
        }
        ExpressionNode::StructLiteral(literal) => {
            for field in program.expression_table.struct_fields(literal.fields) {
                visit(field.value)?;
            }
        }
        ExpressionNode::Range(range) => {
            visit(range.start)?;
            visit(range.end)?;
        }
        ExpressionNode::Boolean(_)
        | ExpressionNode::Float(_)
        | ExpressionNode::Integer(_)
        | ExpressionNode::Name(_)
        | ExpressionNode::String(_) => {}
    }
    Some(())
}

fn value_builtin_has_empty_write_frame(target: &str) -> bool {
    matches!(
        target,
        "min" | "max" | "sqrt" | "as_slice" | "as_mut_slice" | "as_view" | "bytes"
    )
}

fn syntactic_call_written_paths(
    program: &TypedTrees,
    receiver_members: &[String],
    arguments: &[ExpressionHandle],
) -> Option<Vec<String>> {
    let mut written = vec![if receiver_members.is_empty() {
        "self".to_owned()
    } else {
        receiver_members.join(".")
    }];
    for argument in arguments {
        let ExpressionNode::Mutable(place) = program.expression_table.expression(*argument) else {
            continue;
        };
        let path = coarse_place_path(program, *place)?;
        if !written.contains(&path) {
            written.push(path);
        }
    }
    Some(written)
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn validate_call_node(
    program: &TypedTrees,
    call: &TableCall,
    current_machine: &omega_typed_trees::machine::Machine,
    state_name: &str,
    machine_symbols: &MachineSymbols<'_>,
    symbols: &TopLevelSymbols<'_>,
    writable_roots: &WritableRoots<'_, '_>,
    value_env: &ValueEnv,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let receiver_members = program.statement_table.name_path_members(call.receiver);
    let arguments = program.statement_table.expression_handles(call.arguments);

    // `Schema::encode(...)` / `Schema::decode(...)`: the wire
    // module owns the synthesized encoder/decoder calls' diagnostics
    // (chapter 20, wire stage 2).
    if crate::wire::validate_wire_schema_call(
        program,
        call,
        current_machine,
        machine_symbols.state(state_name),
        diagnostics,
    ) {
        return;
    }

    // Asm intrinsic statements (`asm { hlt }`, `asm { out port, value }`)
    // desugar to calls on unnameable `asm#...` targets -- known-contract
    // instructions with FIXED shapes, validated here instead of against a
    // state signature. (`asm { in dest, port }` is an assignment whose value
    // is the `asm#port_in` call; the value-call path owns it.)
    if receiver_members.is_empty() && call.target.as_str().starts_with("asm#") {
        let control_write =
            omega_core::inline_assembly::AsmControlRegister::from_write_intrinsic_name(
                call.target.as_str(),
            );
        let (source_mnemonic, expected_arguments) = match control_write {
            Some(register) => (
                register
                    .write_mnemonic()
                    .expect("writable control-register intrinsic"),
                1,
            ),
            None => match call.target.as_str() {
                "asm#hlt" => ("hlt", 0),
                "asm#port_out" => ("out", 2),
                "asm#lfence" => ("lfence", 0),
                "asm#sfence" => ("sfence", 0),
                "asm#mfence" => ("mfence", 0),
                "asm#cli" => ("cli", 0),
                "asm#sti" => ("sti", 0),
                "asm#popfq" => ("popfq", 1),
                "asm#wrmsr" => ("wrmsr", 2),
                other => {
                    diagnostics.push(Diagnostic::error(format!(
                        "asm intrinsic `{other}` is not a statement form"
                    )));
                    return;
                }
            },
        };
        if arguments.len() != expected_arguments {
            diagnostics.push(Diagnostic::error(format!(
                "asm intrinsic `{}` takes {} operand(s), found {}",
                call.target,
                expected_arguments,
                arguments.len()
            )));
            return;
        }
        if control_write.is_some() || matches!(source_mnemonic, "out" | "popfq" | "wrmsr") {
            let contract = user_asm_contract(source_mnemonic);
            for (operand, constraint) in arguments.iter().zip(contract.operands.iter()) {
                validate_asm_operand_constraint(
                    program,
                    current_machine,
                    machine_symbols.state(state_name),
                    source_mnemonic,
                    *operand,
                    *constraint,
                    diagnostics,
                );
            }
        }
        return;
    }

    // Q7 ruling (2026-07-13): a STATEMENT-position call to the enclosing
    // machine's OWN ENTRY (`self.drip(n - 1);` as a trailing statement) is
    // tail recursion spelled as a call -- it lowered as a Nested-transition
    // loop and slipped the transition-arm fence. "Banned, if it reads as
    // recursion... go write this as states": repetition is a state
    // transition (`-> target(..)`), never a self-call statement.
    if matches!(receiver_members, [receiver] if receiver.as_str() == "self") {
        let machine_entry_name = current_machine
            .name
            .as_str()
            .rsplit("::")
            .next()
            .unwrap_or(current_machine.name.as_str());
        if call.target.as_str() == machine_entry_name {
            diagnostics.push(Diagnostic::error(format!(
                "`self.{}(..)` as a STATEMENT calls the enclosing machine's own entry -- tail recursion spelled as a call, which Omega does not support (machine call cycles are banned; stack size must be predictable). Write the repetition as states: transition to a sub-state or loop back with a bare `-> {}(..)` arm",
                call.target.as_str(),
                call.target.as_str(),
            )));
            return;
        }
    }

    if receiver_members.is_empty()
        || matches!(receiver_members, [receiver] if receiver.as_str() == "self")
    {
        if let Some(signature) =
            program.machine_parameter_signature_in(current_machine, call.target_symbol)
        {
            validate_result_use(
                program,
                call,
                signature.name.as_str(),
                signature.return_type,
                diagnostics,
            );
            validate_call_arguments_handles(
                program,
                current_machine,
                machine_symbols.state(state_name),
                value_env,
                arguments,
                signature.name.as_str(),
                program.state_signature_parameters(signature),
                writable_roots,
                diagnostics,
            );
            return;
        }

        // MP4 specializes `F(args)` to the selected concrete ENTRY symbol.
        // It remains receiverless because the whole callable parameter list
        // (including any explicit data argument) is already present.
        if let Some((callee_machine, state)) = machine_state_by_symbol(program, call.target_symbol)
            && callee_machine.symbol != current_machine.symbol
        {
            validate_result_use(
                program,
                call,
                state.name.as_str(),
                state.return_type,
                diagnostics,
            );
            validate_call_arguments_handles(
                program,
                current_machine,
                machine_symbols.state(state_name),
                value_env,
                arguments,
                state.name.as_str(),
                program.state_parameters(state),
                writable_roots,
                diagnostics,
            );
            validate_machine_call_type_parameter_bounds(
                program,
                symbols,
                callee_machine,
                state,
                state.name.as_str(),
                arguments,
                current_machine,
                machine_symbols.state(state_name),
                diagnostics,
            );
            return;
        }

        if let Some(state) = machine_symbols.state(&call.target) {
            validate_result_use(
                program,
                call,
                state.name.as_str(),
                state.return_type,
                diagnostics,
            );
            validate_call_arguments_handles(
                program,
                current_machine,
                machine_symbols.state(state_name),
                value_env,
                arguments,
                state.name.as_str(),
                program.state_parameters(state),
                writable_roots,
                diagnostics,
            );
            validate_machine_call_type_parameter_bounds(
                program,
                symbols,
                current_machine,
                state,
                state.name.as_str(),
                arguments,
                current_machine,
                machine_symbols.state(state_name),
                diagnostics,
            );
            return;
        }

        let attached_state = current_machine
            .attached_data
            .as_ref()
            .and_then(|attached_data| {
                symbols.attached_machine_state(
                    program,
                    attached_data.as_str(),
                    call.target.as_str(),
                )
            });
        // A receiverless call can also target a FREE top-level machine
        // (`machine compute(item: &Item) -> i32`, called as `compute(item)`);
        // its implicit entry state carries the parameters and return type.
        let Some((callee_machine, state)) = attached_state
            .or_else(|| free_machine_entry_state(program, symbols, call.target.as_str()))
        else {
            diagnostics.push(Diagnostic::error(format!(
                "machine `{}` has no local state `{}`",
                current_machine.name, call.target
            )));
            return;
        };

        // Diagnostics name the call as spelled (`compute`), not the free
        // machine's generated entry-state name (`entry`).
        validate_result_use(
            program,
            call,
            call.target.as_str(),
            state.return_type,
            diagnostics,
        );
        validate_call_arguments_handles(
            program,
            current_machine,
            machine_symbols.state(state_name),
            value_env,
            arguments,
            call.target.as_str(),
            program.state_parameters(state),
            writable_roots,
            diagnostics,
        );
        validate_machine_call_type_parameter_bounds(
            program,
            symbols,
            callee_machine,
            state,
            call.target.as_str(),
            arguments,
            current_machine,
            machine_symbols.state(state_name),
            diagnostics,
        );
        return;
    }

    let receiver = receiver_members
        .last()
        .map(|member| member.as_str())
        .unwrap_or_default();
    let receiver_type = machine_symbols.contained_type(receiver);

    if let Some(machine) = receiver_type
        .and_then(|type_name| symbols.machine(type_name))
        .or_else(|| symbols.machine(receiver))
    {
        if let Some(state) = program
            .machine_states(machine)
            .iter()
            .find(|state| state.name == call.target)
        {
            validate_result_use(program, call, &state.name, state.return_type, diagnostics);
            validate_call_arguments_handles(
                program,
                current_machine,
                machine_symbols.state(state_name),
                value_env,
                arguments,
                &state.name,
                program.state_parameters(state),
                writable_roots,
                diagnostics,
            );
            validate_machine_call_type_parameter_bounds(
                program,
                symbols,
                machine,
                state,
                state.name.as_str(),
                arguments,
                current_machine,
                machine_symbols.state(state_name),
                diagnostics,
            );
            return;
        };

        diagnostics.push(Diagnostic::error(format!(
            "machine `{}` has no state `{}`",
            machine.name, call.target
        )));
        return;
    }

    if let Some((callee_machine, state)) = receiver_type.and_then(|type_name| {
        symbols.attached_machine_state(program, type_name, call.target.as_str())
    }) {
        validate_result_use(program, call, &state.name, state.return_type, diagnostics);
        validate_call_arguments_handles(
            program,
            current_machine,
            machine_symbols.state(state_name),
            value_env,
            arguments,
            &state.name,
            program.state_parameters(state),
            writable_roots,
            diagnostics,
        );
        validate_machine_call_type_parameter_bounds(
            program,
            symbols,
            callee_machine,
            state,
            state.name.as_str(),
            arguments,
            current_machine,
            machine_symbols.state(state_name),
            diagnostics,
        );
        return;
    }

    // Boundary/trait receivers (e.g. `self.console.exit_process(0)`) resolve to a
    // trait machine signature. Strict result use plus argument validation apply
    // here -- a boundary is still a typed call, and a cross-class argument
    // (`exit_process(self.bool_field)`) would otherwise reach the host encoder as
    // a raw byte and be read as garbage with no frontend error.
    if let Some(signature) = receiver_type
        .and_then(|type_name| symbols.trait_definition(type_name))
        .and_then(|trait_definition| {
            program
                .trait_machine_signatures(trait_definition)
                .iter()
                .find(|signature| signature.name == call.target)
        })
    {
        validate_result_use(
            program,
            call,
            &signature.name,
            signature.return_type,
            diagnostics,
        );
        validate_call_arguments_handles(
            program,
            current_machine,
            machine_symbols.state(state_name),
            value_env,
            arguments,
            &signature.name,
            program.state_signature_parameters(signature),
            writable_roots,
            diagnostics,
        );
        return;
    }

    let _ = diagnostics;
}

fn user_asm_contract(mnemonic: &str) -> omega_core::inline_assembly::AsmInstructionContract {
    let Some(omega_core::inline_assembly::AsmCatalogEntry::Contract(contract)) =
        omega_core::inline_assembly::asm_catalog_entry(mnemonic)
    else {
        panic!("accepted asm intrinsic `{mnemonic}` is absent from the shared catalog");
    };
    assert_eq!(
        contract.availability,
        omega_core::inline_assembly::AsmInstructionAvailability::UserChecked,
        "source asm intrinsic `{mnemonic}` must be user-checked"
    );
    contract
}

fn validate_asm_operand_constraint(
    program: &TypedTrees,
    machine: &Machine,
    state: Option<&State>,
    instruction: &str,
    operand: ExpressionHandle,
    constraint: omega_core::inline_assembly::AsmOperandConstraint,
    diagnostics: &mut Vec<Diagnostic>,
) {
    if let ExpressionNode::Integer(literal) = program.expression_table.expression(operand) {
        if let Some(maximum) = constraint.maximum_literal()
            && literal.value_u64().is_some_and(|value| value <= maximum)
        {
            return;
        }
        diagnostics.push(Diagnostic::error(format!(
            "asm instruction `{instruction}` operand `{}` requires target register `{}` \
             constraint `{}`{}; integer literal `{}` is outside that operand class",
            constraint.role,
            constraint.target_register,
            constraint.expected_type_name(),
            constraint
                .maximum_literal()
                .map(|maximum| format!(" or a literal in 0..={maximum}"))
                .unwrap_or_default(),
            literal.text(),
        )));
        return;
    }

    let actual = if constraint.requires_place() {
        crate::places::declared_place_type(program, machine, state, operand)
            .and_then(|type_reference| program.primitive_type_reference(type_reference))
    } else {
        asm_operand_primitive_type(program, machine, state, operand)
    };
    let expected = PrimitiveType::from_name(constraint.expected_type_name())
        .expect("asm operand constraint must name a primitive type");
    if actual == Some(expected) {
        return;
    }

    let actual = actual
        .map(|primitive| format!("`{}`", primitive.name()))
        .unwrap_or_else(|| expression_type_name_handle(program, operand).to_owned());
    let place_requirement = constraint
        .requires_writable_place()
        .then_some(" writable place")
        .unwrap_or("");
    diagnostics.push(Diagnostic::error(format!(
        "asm instruction `{instruction}` operand `{}` requires an exact `{}`{place_requirement} \
         for target register `{}`, found {actual}",
        constraint.role,
        constraint.expected_type_name(),
        constraint.target_register,
    )));
}

fn asm_operand_primitive_type(
    program: &TypedTrees,
    machine: &Machine,
    state: Option<&State>,
    operand: ExpressionHandle,
) -> Option<PrimitiveType> {
    match program.expression_table.expression(operand) {
        ExpressionNode::Mutable(inner) => {
            asm_operand_primitive_type(program, machine, state, *inner)
        }
        ExpressionNode::Cast(cast) => program
            .expression_table
            .name_path_members(cast.target_type)
            .last()
            .and_then(|name| PrimitiveType::from_name(name.as_str())),
        _ => crate::places::declared_place_type(program, machine, state, operand)
            .and_then(|type_reference| program.primitive_type_reference(type_reference)),
    }
}

pub(crate) fn validate_asm_value_destination(
    program: &TypedTrees,
    machine: &Machine,
    state: Option<&State>,
    assignment: &omega_typed_trees::statement::TableAssignment,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let ExpressionNode::Call(call) = program.expression_table.expression(assignment.value) else {
        return;
    };
    let instruction =
        match omega_core::inline_assembly::AsmControlRegister::from_read_intrinsic_name(
            call.target.as_str(),
        ) {
            Some(register) => register.read_mnemonic(),
            None => match call.target.as_str() {
                "asm#port_in" => "in",
                "asm#pushfq" => "pushfq",
                "asm#rdmsr" => "rdmsr",
                _ => return,
            },
        };
    let contract = user_asm_contract(instruction);
    validate_asm_operand_constraint(
        program,
        machine,
        state,
        instruction,
        assignment.target,
        contract.operands[0],
        diagnostics,
    );
}

/// The boundary-trait signature a call statement resolves to (`self.fw.
/// get_size(..)` -> trait `Firmware`'s `get_size`), or None for every other
/// receiver class. Mirrors `validate_call_node`'s trait branch; used by the
/// R4 witness mint (out-param ensures seeding the value env). Kept
/// cache-based (vs the shared `omega_typed_trees::boundary` chain the
/// checker/proof consumers use) because `contained_type` also resolves
/// `contains`-clause receivers, not just attached-data fields.
pub(crate) fn boundary_trait_signature<'program>(
    program: &'program TypedTrees,
    machine_symbols: &MachineSymbols<'_>,
    symbols: &TopLevelSymbols<'program>,
    call: &TableCall,
) -> Option<&'program omega_typed_trees::signature::StateSignature> {
    let receiver_members = program
        .statement_table
        .name_path_members(call.receiver)
        .iter()
        .map(|member| member.as_str().to_owned())
        .collect::<Vec<_>>();
    boundary_trait_signature_for_parts(
        program,
        machine_symbols,
        symbols,
        &receiver_members,
        call.target.as_str(),
    )
}

fn boundary_trait_signature_for_parts<'program>(
    program: &'program TypedTrees,
    machine_symbols: &MachineSymbols<'_>,
    symbols: &TopLevelSymbols<'program>,
    receiver_members: &[String],
    target: &str,
) -> Option<&'program omega_typed_trees::signature::StateSignature> {
    let receiver = receiver_members.last()?.as_str();
    let receiver_type = machine_symbols.contained_type(receiver)?;
    let trait_definition = symbols.trait_definition(receiver_type)?;
    program
        .trait_machine_signatures(trait_definition)
        .iter()
        .find(|signature| signature.name.as_str() == target)
}

/// The program-place frame of a resolved boundary call before authored
/// `stores` lands. Boundary code may mutate its receiver and every explicit
/// exclusive argument; it cannot manufacture reach to unrelated caller
/// fields. An exclusive parameter not represented by a direct `&mut place`
/// remains opaque and returns `None`, preserving the fail-closed fallback.
pub(crate) fn known_boundary_call_written_paths(
    program: &TypedTrees,
    machine_symbols: &MachineSymbols<'_>,
    symbols: &TopLevelSymbols<'_>,
    call: &TableCall,
) -> Option<Vec<String>> {
    let receiver = program
        .statement_table
        .name_path_members(call.receiver)
        .iter()
        .map(|member| member.as_str().to_owned())
        .collect::<Vec<_>>();
    known_boundary_call_written_paths_for_parts(
        program,
        machine_symbols,
        symbols,
        &receiver,
        call.target.as_str(),
        program.statement_table.expression_handles(call.arguments),
    )
}

fn known_boundary_call_written_paths_for_parts(
    program: &TypedTrees,
    machine_symbols: &MachineSymbols<'_>,
    symbols: &TopLevelSymbols<'_>,
    receiver: &[String],
    target: &str,
    arguments: &[ExpressionHandle],
) -> Option<Vec<String>> {
    let signature =
        boundary_trait_signature_for_parts(program, machine_symbols, symbols, receiver, target)?;
    if receiver.is_empty() {
        return None;
    }
    let mut written = vec![receiver.join(".")];
    let parameters = program
        .state_signature_parameters(signature)
        .iter()
        .filter(|parameter| !parameter.is_self);

    for (parameter, argument) in parameters.zip(arguments) {
        let TypeReferenceNode::Reference {
            is_mutable: true, ..
        } = program
            .type_reference_table
            .type_reference(parameter.type_reference)
        else {
            continue;
        };
        let ExpressionNode::Mutable(place) = program.expression_table.expression(*argument) else {
            return None;
        };
        let path = coarse_place_path(program, *place)?;
        if !written.contains(&path) {
            written.push(path);
        }
    }

    Some(written)
}

/// The FREE top-level machine named `target` and its entry state (`machine
/// compute(item: &Item) -> i32 { ... }`), or None. The parser names a free
/// machine's implicit entry state `entry`; explicit entry states matching the
/// call target name win first.
pub(crate) fn free_machine_entry_state<'program>(
    program: &'program TypedTrees,
    symbols: &TopLevelSymbols<'program>,
    target: &str,
) -> Option<(&'program Machine, &'program State)> {
    let machine = symbols.machine(target)?;
    if machine.attached_data.is_some() {
        return None;
    }

    let states = program.machine_states(machine);
    states
        .iter()
        .find(|state| state.name.as_str() == target)
        .or_else(|| states.iter().find(|state| state.name.as_str() == "entry"))
        .or_else(|| states.first())
        .map(|state| (machine, state))
}

fn machine_state_by_symbol(
    program: &TypedTrees,
    symbol: SymbolHandle,
) -> Option<(&Machine, &State)> {
    program.machines().iter().find_map(|machine| {
        program
            .machine_states(machine)
            .iter()
            .find(|state| state.symbol == symbol)
            .map(|state| (machine, state))
    })
}

/// Instantiate the conservative may-write set of a resolved internal call in
/// the caller's place namespace. `None` means the summary is not complete and
/// the caller must invalidate every flow fact. Internal acyclic calls compose;
/// transitions, boundaries, machine parameters, and unresolved writes remain
/// deliberately opaque until later R5 rungs add `stores` and state summaries.
pub(crate) fn known_call_written_paths(
    program: &TypedTrees,
    call: &TableCall,
    current_machine: &Machine,
    machine_symbols: &MachineSymbols<'_>,
    symbols: &TopLevelSymbols<'_>,
) -> Option<Vec<String>> {
    let receiver_members = program
        .statement_table
        .name_path_members(call.receiver)
        .iter()
        .map(|member| member.as_str().to_owned())
        .collect::<Vec<_>>();
    known_call_written_paths_for_parts(
        program,
        call.target_symbol,
        call.target.as_str(),
        &receiver_members,
        program.statement_table.expression_handles(call.arguments),
        current_machine,
        machine_symbols,
        symbols,
        &mut Vec::new(),
    )
}

#[allow(clippy::too_many_arguments)]
fn known_call_written_paths_for_parts(
    program: &TypedTrees,
    target_symbol: SymbolHandle,
    target: &str,
    receiver_members: &[String],
    arguments: &[ExpressionHandle],
    current_machine: &Machine,
    machine_symbols: &MachineSymbols<'_>,
    symbols: &TopLevelSymbols<'_>,
    active_states: &mut Vec<SymbolHandle>,
) -> Option<Vec<String>> {
    // A static machine parameter's selected target is a specialization input,
    // not an ordinary receiver binding. Until MP summaries instantiate that
    // binding explicitly, retain the sound all-facts invalidation.
    if program
        .machine_parameter_signature_in(current_machine, target_symbol)
        .is_some()
    {
        return None;
    }
    let (callee_machine, callee_state) = if receiver_members.is_empty()
        || matches!(receiver_members, [receiver] if receiver == "self")
    {
        machine_state_by_symbol(program, target_symbol)
            .filter(|(machine, _)| machine.symbol != current_machine.symbol)
            .or_else(|| {
                machine_symbols
                    .state(target)
                    .map(|state| (current_machine, state))
            })
            .or_else(|| {
                current_machine
                    .attached_data
                    .as_ref()
                    .and_then(|attached_data| {
                        symbols.attached_machine_state(program, attached_data.as_str(), target)
                    })
            })
            .or_else(|| free_machine_entry_state(program, symbols, target))?
    } else {
        let receiver = receiver_members.last()?.as_str();
        let machine = machine_symbols
            .contained_type(receiver)
            .and_then(|type_name| symbols.machine(type_name))
            .or_else(|| symbols.machine(receiver))?;
        let state = program
            .machine_states(machine)
            .iter()
            .find(|state| state.name.as_str() == target)?;
        (machine, state)
    };

    if active_states.contains(&callee_state.symbol) {
        return None;
    }
    active_states.push(callee_state.symbol);
    let result = summarize_resolved_call(
        program,
        arguments,
        callee_machine,
        callee_state,
        receiver_members,
        symbols,
        active_states,
    );
    active_states.pop();
    result
}

#[allow(clippy::too_many_arguments)]
fn summarize_resolved_call(
    program: &TypedTrees,
    arguments: &[ExpressionHandle],
    callee_machine: &Machine,
    callee_state: &State,
    receiver_members: &[String],
    symbols: &TopLevelSymbols<'_>,
    active_states: &mut Vec<SymbolHandle>,
) -> Option<Vec<String>> {
    let receiver_base = (!receiver_members.is_empty())
        .then(|| receiver_members.join("."))
        .or_else(|| {
            callee_machine
                .attached_data
                .as_ref()
                .map(|_| "self".to_owned())
        });
    let parameters = program.state_parameters(callee_state);
    let mut written = Vec::new();

    let relative_paths = summarize_state_written_paths(
        program,
        callee_machine,
        callee_state,
        symbols,
        active_states,
    )?;
    for relative in relative_paths {
        if let Some(instantiated) = instantiate_written_path(
            program,
            &relative,
            receiver_base.as_deref(),
            parameters,
            arguments,
            &[],
        )? && !written.contains(&instantiated)
        {
            written.push(instantiated);
        }
    }

    Some(written)
}

fn summarize_state_written_paths(
    program: &TypedTrees,
    machine: &Machine,
    state: &State,
    symbols: &TopLevelSymbols<'_>,
    active_states: &mut Vec<SymbolHandle>,
) -> Option<Vec<String>> {
    let parameters = program.state_parameters(state);
    let mut locals = Vec::new();
    let mut written = Vec::new();

    let mut nested_diagnostics = Vec::new();
    let machine_symbols = MachineSymbols::build(program, machine, &mut nested_diagnostics);
    if !nested_diagnostics.is_empty() {
        return None;
    }

    for statement in program.statement_table.statements(state.statement_nodes) {
        match statement {
            StatementNode::AssemblyFact(fact) => {
                if !expression_is_call_free(program, fact.expression) {
                    return None;
                }
            }
            StatementNode::Assignment(assignment) => {
                if !expression_is_call_free(program, assignment.target)
                    || !expression_is_call_free(program, assignment.value)
                {
                    return None;
                }
                let relative = coarse_place_path(program, assignment.target)?;
                if relative_state_path_is_visible(&relative, parameters, &locals)?
                    && !written.contains(&relative)
                {
                    written.push(relative);
                }
            }
            StatementNode::Call(nested_call) => {
                let nested_receiver_members = program
                    .statement_table
                    .name_path_members(nested_call.receiver)
                    .iter()
                    .map(|member| member.as_str().to_owned())
                    .collect::<Vec<_>>();
                let nested_writes = known_call_written_paths_for_parts(
                    program,
                    nested_call.target_symbol,
                    nested_call.target.as_str(),
                    &nested_receiver_members,
                    program
                        .statement_table
                        .expression_handles(nested_call.arguments),
                    machine,
                    &machine_symbols,
                    symbols,
                    active_states,
                )?;
                for relative in nested_writes {
                    if relative_state_path_is_visible(&relative, parameters, &locals)?
                        && !written.contains(&relative)
                    {
                        written.push(relative);
                    }
                }
            }
            StatementNode::Transition(_) => return None,
            StatementNode::Expression(expression) => {
                if !expression_is_call_free(program, *expression) {
                    return None;
                }
            }
            StatementNode::LocalData(local) => {
                if !expression_is_call_free(program, local.initial_value) {
                    return None;
                }
                locals.push(local.name.as_str().to_owned());
            }
        }
    }

    Some(written)
}

fn relative_state_path_is_visible(
    relative: &str,
    parameters: &[StateParameter],
    locals: &[String],
) -> Option<bool> {
    let (root, _) = split_place_root(relative);
    if root == "self"
        || parameters
            .iter()
            .any(|parameter| parameter.name.as_str() == root)
    {
        return Some(true);
    }
    if locals.iter().any(|local| local == root) {
        return Some(false);
    }
    None
}

fn normalize_state_relative_path(
    program: &TypedTrees,
    state: &State,
    relative: &str,
) -> Option<Option<String>> {
    let (root, suffix) = split_place_root(relative);
    if root == "self" {
        return Some(Some(append_place_suffix("self", suffix)));
    }
    if let Some(parameter_index) = program
        .state_parameters(state)
        .iter()
        .filter(|parameter| !parameter.is_self)
        .position(|parameter| parameter.name.as_str() == root)
    {
        return Some(Some(append_place_suffix(
            &format!("$P{parameter_index}"),
            suffix,
        )));
    }
    let is_local = program
        .statement_table
        .statements(state.statement_nodes)
        .iter()
        .any(|statement| {
            matches!(statement, StatementNode::LocalData(local) if local.name.as_str() == root)
        });
    is_local.then_some(None)
}

fn instantiate_written_path(
    program: &TypedTrees,
    relative: &str,
    receiver_base: Option<&str>,
    parameters: &[StateParameter],
    arguments: &[ExpressionHandle],
    locals: &[String],
) -> Option<Option<String>> {
    let (root, suffix) = split_place_root(relative);
    if root == "self" {
        return Some(Some(append_place_suffix(receiver_base?, suffix)));
    }
    if let Some(argument_index) = parameters
        .iter()
        .filter(|parameter| !parameter.is_self)
        .position(|parameter| parameter.name.as_str() == root)
    {
        let argument = *arguments.get(argument_index)?;
        let base = coarse_place_path(program, argument)?;
        return Some(Some(append_place_suffix(&base, suffix)));
    }
    if locals.iter().any(|local| local == root) {
        return Some(None);
    }
    // A write whose root is neither local nor a known parameter is externally
    // visible in a way this rung cannot instantiate safely.
    None
}

fn split_place_root(path: &str) -> (&str, &str) {
    let boundary = path.find(['.', '[']).unwrap_or(path.len());
    path.split_at(boundary)
}

fn append_place_suffix(base: &str, suffix: &str) -> String {
    format!("{base}{suffix}")
}

/// Coarsen indexed writes to their collection (`self.cells[i]` writes
/// `self.cells`). The value environment does not track index-sensitive facts.
fn coarse_place_path(program: &TypedTrees, expression: ExpressionHandle) -> Option<String> {
    match program.expression_table.expression(expression) {
        ExpressionNode::Mutable(inner) => coarse_place_path(program, *inner),
        ExpressionNode::Indexed(indexed) => coarse_place_path(program, indexed.collection),
        _ => arithmetic_domains::place_path(program, expression),
    }
}

fn expression_is_call_free(program: &TypedTrees, expression: ExpressionHandle) -> bool {
    if !expression.is_valid() {
        return true;
    }
    match program.expression_table.expression(expression) {
        ExpressionNode::Atomic(atomic) => expression_is_call_free(program, atomic.value),
        ExpressionNode::ArrayLiteral(values) => program
            .expression_table
            .expression_handles(*values)
            .iter()
            .all(|value| expression_is_call_free(program, *value)),
        ExpressionNode::Binary(binary) => {
            expression_is_call_free(program, binary.left)
                && expression_is_call_free(program, binary.right)
        }
        ExpressionNode::Cast(cast) => expression_is_call_free(program, cast.value),
        ExpressionNode::Call(_) => false,
        ExpressionNode::Indexed(indexed) => {
            expression_is_call_free(program, indexed.collection)
                && expression_is_call_free(program, indexed.index)
        }
        ExpressionNode::Member(member) => expression_is_call_free(program, member.receiver),
        ExpressionNode::Mutable(inner) => expression_is_call_free(program, *inner),
        ExpressionNode::Range(range) => {
            expression_is_call_free(program, range.start)
                && expression_is_call_free(program, range.end)
        }
        ExpressionNode::StructLiteral(literal) => program
            .expression_table
            .struct_fields(literal.fields)
            .iter()
            .all(|field| expression_is_call_free(program, field.value)),
        ExpressionNode::Unary(unary) => expression_is_call_free(program, unary.operand),
        ExpressionNode::Boolean(_)
        | ExpressionNode::Float(_)
        | ExpressionNode::Integer(_)
        | ExpressionNode::Name(_)
        | ExpressionNode::String(_) => true,
    }
}

/// FROZEN DECISION 13 residue -- machine-call monomorphization arguments.
/// A bracket bound on a callee type parameter (`machine copy_it<T [copy]>`)
/// must hold for the concrete type the call instantiates `T` with. There is
/// no explicit type-argument list at call sites today: instantiation is
/// positional inference, so each non-self parameter whose declared type names
/// a bounded callee type parameter (`x: &T`, `x: T`, `[T; N]`, constrained
/// forms) pins `T` to the matching argument's declared place type, and that
/// concrete type must satisfy every bound via the same structural check the
/// data-instantiation path uses (`type_satisfies_declared_property`). An
/// in-scope bounded parameter of the CALLER counts as carrying its bound, so
/// a generic caller may forward its own `U [copy]`.
///
/// FRONTIER (stands down silently, like the wire argument checks): arguments
/// the declared-place scope cannot type (call results, indexed elements,
/// literals, nested member chains), parameters whose type buries `T` inside a
/// generic (`Box<T>`) or slice (`&[T]`).
///
/// Both STATEMENT-position calls (via `validate_call_node`) and VALUE-position
/// calls (via `validate_value_position_calls` + `scan_expression_calls`) now
/// reach this function.
#[allow(clippy::too_many_arguments)]
fn validate_machine_call_type_parameter_bounds(
    program: &TypedTrees,
    symbols: &TopLevelSymbols<'_>,
    callee_machine: &Machine,
    callee_state: &State,
    target_name: &str,
    arguments: &[ExpressionHandle],
    current_machine: &Machine,
    current_state: Option<&State>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    // A claim-free bodyless boundary declaration is a SYMBOL for contracts,
    // not an executable provider.  It has neither checked code nor a `via`
    // realization, so allowing an ordinary body call would turn "introduces
    // no fact" into a hidden runtime implementation hole.  Contract
    // expressions are not body call sites and remain free to name the symbol.
    if callee_machine.supply_mode == omega_core::semantics::MachineSupplyMode::Boundary
        && program
            .statement_table
            .statements(callee_state.statement_nodes)
            .is_empty()
    {
        diagnostics.push(Diagnostic::error(format!(
            "bodyless boundary symbol `{target_name}` has no executable realization; use it only in contracts, or satisfy a boundary requirement via an admitted provider"
        )));
    }

    let type_parameters = program.machine_type_parameters(callee_machine);
    if type_parameters.is_empty() {
        return;
    }

    let caller_type_parameters = program.machine_type_parameters(current_machine);

    for (argument, parameter) in arguments.iter().zip(
        program
            .state_parameters(callee_state)
            .iter()
            .filter(|parameter| !parameter.is_self),
    ) {
        let Some(type_parameter) =
            referenced_type_parameter(program, type_parameters, parameter.type_reference)
        else {
            continue;
        };
        let bounds = declared_property_requirements(&type_parameter.bounds);
        if bounds.is_empty() {
            continue;
        }
        let bound_labels = bounds.iter().map(ToString::to_string).collect::<Vec<_>>();
        let Some(argument_type) =
            declared_place_type(program, current_machine, current_state, *argument)
        else {
            continue;
        };
        for property in bounds {
            if type_satisfies_declared_property(
                program,
                symbols,
                caller_type_parameters,
                argument_type,
                property,
            ) {
                continue;
            }
            diagnostics.push(Diagnostic::error(format!(
                "type parameter `{} [{}]` of machine `{target_name}` was instantiated with `{}`, which does not satisfy `[{property}]`",
                type_parameter.name,
                bound_labels.join(", "),
                type_reference_label(program, argument_type)
            )));
        }
    }
}

/// FROZEN DECISION 9 -- STRICT RESULT USE: a statement-position call whose callee
/// returns a non-unit value must not silently drop that value. Intentional
/// discards are spelled `_ = call();` (which sets `discards_result`). "Non-unit"
/// means the resolved callee declares a return type (`-> T`) that is not `()`.
///
/// PROOF-MACHINE callees are exempt (owner, 2026-07-12): a bare statement
/// call to a proof machine is a CITATION (ch10 "Citing Proofs") -- the
/// lemma is invoked for its ensures and erases at codegen, so there is no
/// runtime result to drop. The exemption is a property of the callee's
/// (computed) classification, visible at its declaration -- never of the
/// call site's context.
fn validate_result_use(
    program: &TypedTrees,
    call: &TableCall,
    target_name: &str,
    return_type: TypeReferenceHandle,
    diagnostics: &mut Vec<Diagnostic>,
) {
    if call.discards_result || !return_type.is_valid() {
        return;
    }

    if matches!(
        program.type_reference_table.type_reference(return_type),
        TypeReferenceNode::Unit
    ) {
        return;
    }

    if call.receiver.is_empty() {
        let classification = omega_typed_trees::proof_only::classify(program);
        let is_citation = program
            .machines()
            .iter()
            .find(|candidate| {
                candidate.attached_data.is_none() && candidate.name.as_str() == call.target.as_str()
            })
            .is_some_and(|callee| classification.is_proof_machine(program, callee));
        if is_citation {
            return;
        }
    }

    diagnostics.push(Diagnostic::error(format!(
        "call to `{target_name}` discards its non-unit `{}` result; consume the value or discard it explicitly with `_ = {target_name}(...);`",
        program.display_type_reference_with_constraints(return_type)
    )));
}

/// Reports the "state `X` expects N argument(s), got M" error when `arguments`
/// does not match the callee's callable (non-`self`) parameter count, returning
/// `true` on a mismatch so callers skip the per-argument checks (which zip the
/// two and would misalign). SINGLE SOURCE OF TRUTH for call arity across the
/// statement-position (`validate_call_arguments_handles`) and value-position
/// (`validate_value_call_argument_classes`) paths.
pub(crate) fn report_argument_count_mismatch(
    target_name: &str,
    parameters: &[StateParameter],
    arguments: &[ExpressionHandle],
    diagnostics: &mut Vec<Diagnostic>,
) -> bool {
    let callable_parameter_count = parameters
        .iter()
        .filter(|parameter| !parameter.is_self)
        .count();

    if arguments.len() != callable_parameter_count {
        diagnostics.push(Diagnostic::error(format!(
            "state `{}` expects {} argument(s), got {}",
            target_name,
            callable_parameter_count,
            arguments.len()
        )));
        return true;
    }
    false
}

/// Whether an argument that is NOT spelled `&mut ...` still DELIVERS a
/// mutable reference: a bare name forwarding a `&mut` parameter, or a local
/// that is itself a `&mut` reference (declared `&mut T`, or bound to a
/// `&mut place` initializer). Everything else lends immutable access -- a
/// shared `&` vanishes at parse time, so a bare place expression IS the
/// immutable-lend spelling. Bindings resolve at WHOLE-MACHINE scope (a
/// sub-state legitimately reads the entry state's params and locals), so
/// every state of the current machine is consulted.
fn argument_forwards_mutable_reference(
    program: &TypedTrees,
    current_machine: &Machine,
    argument: ExpressionHandle,
) -> bool {
    let ExpressionNode::Name(path) = program.expression_table.expression(argument) else {
        return false;
    };
    let [name] = program.expression_table.name_path_members(path.members) else {
        return false;
    };
    program.machine_states(current_machine).iter().any(|state| {
        if program
            .state_parameters(state)
            .iter()
            .any(|parameter| parameter.is_mutable && parameter.name == *name)
        {
            return true;
        }
        program
            .statement_table
            .statements(state.statement_nodes)
            .iter()
            .any(|statement| {
                let StatementNode::LocalData(local_data) = statement else {
                    return false;
                };
                if local_data.name != *name {
                    return false;
                }
                crate::locals::local_is_mutable_reference(program, local_data)
                    || (local_data.initial_value.is_valid()
                        && matches!(
                            program
                                .expression_table
                                .expression(local_data.initial_value),
                            ExpressionNode::Mutable(_)
                        ))
            })
    })
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn validate_call_arguments_handles(
    program: &TypedTrees,
    current_machine: &Machine,
    current_state: Option<&State>,
    value_env: &ValueEnv,
    arguments: &[ExpressionHandle],
    target_name: &str,
    parameters: &[StateParameter],
    writable_roots: &WritableRoots<'_, '_>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    if report_argument_count_mismatch(target_name, parameters, arguments, diagnostics) {
        return;
    }

    for (argument, parameter) in arguments
        .iter()
        .zip(parameters.iter().filter(|parameter| !parameter.is_self))
    {
        let is_mutable = matches!(
            program.expression_table.expression(*argument),
            ExpressionNode::Mutable(_)
        );

        if parameter.is_mutable && !is_mutable {
            // Not spelled `&mut ...`. The only legitimate remaining shape is
            // a FORWARD: a bare name that is itself already a `&mut`
            // reference (a `&mut` parameter passed onward, or a local bound
            // to a `&mut` borrow). Anything else lends IMMUTABLE access to a
            // parameter that may write through it -- the borrow-safety hole
            // this arm used to skip silently (a shared `&` vanishes at parse
            // time, so a bare place expression IS the immutable-lend
            // spelling; the unenforced write segfaulted natively).
            if !argument_forwards_mutable_reference(program, current_machine, *argument) {
                diagnostics.push(Diagnostic::error(format!(
                    "argument `{}` for state `{}` is declared `&mut` (`{}`), but the \
                     caller lends only immutable access -- pass `&mut ...` or forward a \
                     `&mut` binding",
                    parameter.name,
                    target_name,
                    program.display_type_reference_with_constraints(parameter.type_reference),
                )));
            }
            continue;
        }

        if !parameter.is_mutable && is_mutable {
            continue;
        }

        let expected_type =
            program.display_type_reference_with_constraints(parameter.type_reference);

        if !argument_matches_type_reference_handle(program, *argument, parameter.type_reference) {
            diagnostics.push(Diagnostic::error(format!(
                "argument `{}` for state `{}` expects `{}`, got `{}`",
                parameter.name,
                target_name,
                expected_type,
                expression_type_name_handle(program, *argument)
            )));
        } else if !report_cross_class_argument(
            program,
            current_machine,
            current_state,
            *argument,
            parameter,
            target_name,
            diagnostics,
        ) {
            // The shape gate blanket-accepts place/name arguments (`self.field`,
            // a local) against ANY primitive parameter, so a `bool` field passed
            // for an `i32` parameter slips through and the backend silently reads
            // it as garbage. Resolve the argument's scalar class and reject a
            // cross-class store, exactly as the assignment path does. Only args
            // that PASSED the shape gate reach here, so cross-class LITERALS (which
            // the shape gate already rejects above) are not double-reported. When
            // the classes DO agree (a same-class numeric arg), check the narrowing
            // obligation -- `take_i8(self.i64_field)` would silently truncate.
            report_narrowing_argument(
                program,
                current_machine,
                current_state,
                value_env,
                *argument,
                parameter,
                target_name,
                diagnostics,
            );
        }
        // An array-literal argument (`sink([300, ..])`) is checked element-wise
        // against the parameter's `[T; N]` element type -- the scalar guards above
        // no-op on a non-primitive (array) parameter.
        if let Some(state) = current_state {
            crate::struct_literals::validate_array_literal_elements(
                program,
                current_machine,
                state,
                *argument,
                parameter.type_reference,
                diagnostics,
            );
        }
        // Nominal guard: the shape gate blanket-accepts a place/name argument
        // against ANY `Named` parameter, so `take_foo(&self.bar)` (a `&Bar` for a
        // `&Foo` parameter) is silently accepted and reads the wrong storage.
        // Reject when both parameter and argument resolve to concrete data types
        // that differ (every non-data form is skipped, so no false positive on
        // trait/generic parameters or computed arguments).
        let slot_context = format!("argument `{}` for state `{target_name}`", parameter.name);
        report_data_type_conflict(
            program,
            current_machine,
            current_state,
            *argument,
            parameter.type_reference,
            &slot_context,
            "argument",
            diagnostics,
        );
        // Scalar-vs-data shape guard: `take_struct(5)` (a scalar for a struct param)
        // or `take_int(self.struct)` (a struct for a scalar param). Unlike the
        // array/scalar check below, this is SAFE at the argument position -- it fires
        // only on scalar-vs-DATA-type crossings, and `&buffer`/`addr`/text args
        // involve no data type on either side, so they never trigger it.
        crate::expression_types::report_scalar_data_shape_mismatch(
            program,
            current_machine,
            current_state,
            *argument,
            parameter.type_reference,
            &slot_context,
            "argument",
            diagnostics,
        );
        // NOTE: an array/scalar SHAPE check does NOT belong at the argument position
        // -- `&self.msg` (address-of a `[u8; N]` buffer) passed to an `addr`/pointer
        // param is a valid array-value-into-scalar-target flow, and boundary/host
        // text params (`addr`, byte slices) accept text/byte values freely. The
        // reference/`addr` and text representations make args a false-positive
        // minefield; a wrong-count/type arg is already caught here + at the backend.
    }

    let _ = (writable_roots, diagnostics);
}

/// Reject a single ARGUMENT whose scalar class conflicts with its `parameter`'s
/// primitive type -- a `bool`/text value passed where a numeric parameter is
/// expected (or vice versa), which the backend would otherwise read as garbage.
/// Shared by the statement/transition path (`validate_call_arguments_handles`)
/// and the value-position path (`validate_value_call_argument_classes`). Returns
/// `true` if it reported. A non-primitive parameter (a data reference, a struct)
/// or an unresolvable argument class yields `false` -- no report.
fn report_cross_class_argument(
    program: &TypedTrees,
    current_machine: &Machine,
    current_state: Option<&State>,
    argument: ExpressionHandle,
    parameter: &StateParameter,
    target_name: &str,
    diagnostics: &mut Vec<Diagnostic>,
) -> bool {
    let Some(parameter_primitive) = program.primitive_type_reference(parameter.type_reference)
    else {
        return false;
    };
    let slot_context = format!("argument `{}` for state `{target_name}`", parameter.name);
    report_cross_class_store(
        program,
        Some(current_machine),
        current_state,
        argument,
        parameter_primitive,
        &slot_context,
        "parameter",
        diagnostics,
    )
}

/// Reject a single numeric ARGUMENT that NARROWS into its `parameter` -- a wider
/// value (`self.big: i64 = 300`) passed where a narrower integer parameter is
/// expected (`x: i8`), which the backend would otherwise silently truncate
/// (300 -> 44). Decision-17's narrowing proof obligation, applied at the call
/// boundary exactly as `check_narrowing_assignment` applies it at the assignment
/// boundary. Honors dominating guards via the flow-sensitive `value_env`, so a
/// guarded-in-range argument is not flagged. The argument's OWN arithmetic is
/// analyzed into a THROWAWAY buffer, so only the narrowing check contributes a
/// diagnostic here (an arg's exact-overflow obligation is not this gate's job).
fn report_narrowing_argument(
    program: &TypedTrees,
    current_machine: &Machine,
    current_state: Option<&State>,
    value_env: &ValueEnv,
    argument: ExpressionHandle,
    parameter: &StateParameter,
    target_name: &str,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let Some(parameter_primitive) = program.primitive_type_reference(parameter.type_reference)
    else {
        return;
    };
    let owner = format!(
        "machine `{}` state `{target_name}` argument `{}`",
        current_machine.name, parameter.name,
    );
    arithmetic_domains::check_value_narrowing(
        program,
        current_machine,
        current_state,
        argument,
        parameter_primitive,
        value_env,
        &owner,
        diagnostics,
    );
}

/// Reject cross-class scalar ARGUMENTS at a VALUE-position call site
/// (`let r = self.f(self.bool_field)`). The value-position path validates only
/// type-parameter bounds, so the same cross-class hole the statement/transition
/// paths had applies here. Unlike `validate_call_arguments_handles` there is no
/// shape gate ahead of this, so it also covers literal arguments.
fn validate_value_call_argument_classes(
    program: &TypedTrees,
    current_machine: &Machine,
    current_state: &State,
    value_env: &ValueEnv,
    arguments: &[ExpressionHandle],
    callee_state: &State,
    diagnostics: &mut Vec<Diagnostic>,
) {
    // (The void-callee-in-value-position check lives in report_void_value_callee:
    // it consults the resolved state's return type AND the callee machine's
    // transition VALUE arms, which is what keying off `callee_state.return_type`
    // alone could not do.)

    // Arity: value-position calls (`let r = self.pick(1)`) reach only this path,
    // never `validate_call_arguments_handles`, so without this a wrong argument
    // count compiled silently (a missing arg then read its ZII default). Safe here
    // because this function runs only on a RESOLVED callee -- the resolver's blind
    // spots fall through earlier without reaching it.
    if report_argument_count_mismatch(
        callee_state.name.as_str(),
        program.state_parameters(callee_state),
        arguments,
        diagnostics,
    ) {
        return;
    }

    for (argument, parameter) in arguments.iter().zip(
        program
            .state_parameters(callee_state)
            .iter()
            .filter(|parameter| !parameter.is_self),
    ) {
        // Class check first; narrowing only when the classes agree (a same-class
        // numeric arg), so a cross-class arg is not double-reported. Mirrors the
        // statement/transition path in `validate_call_arguments_handles`.
        if !report_cross_class_argument(
            program,
            current_machine,
            Some(current_state),
            *argument,
            parameter,
            callee_state.name.as_str(),
            diagnostics,
        ) {
            report_narrowing_argument(
                program,
                current_machine,
                Some(current_state),
                value_env,
                *argument,
                parameter,
                callee_state.name.as_str(),
                diagnostics,
            );
        }
        // Nominal guard (value-position complement): `let r = self.take_foo(&self.bar)`
        // with a `&Foo` parameter is silently accepted -- the same wrong-data-type
        // hole the statement/transition path has.
        let slot_context = format!(
            "argument `{}` for state `{}`",
            parameter.name,
            callee_state.name.as_str()
        );
        report_data_type_conflict(
            program,
            current_machine,
            Some(current_state),
            *argument,
            parameter.type_reference,
            &slot_context,
            "argument",
            diagnostics,
        );
        // Scalar-vs-data shape guard -- safe at the argument position (see the twin
        // call in `validate_call_arguments_handles`): fires only on scalar-vs-DATA
        // crossings, which `&buffer`/`addr`/text args never are.
        crate::expression_types::report_scalar_data_shape_mismatch(
            program,
            current_machine,
            Some(current_state),
            *argument,
            parameter.type_reference,
            &slot_context,
            "argument",
            diagnostics,
        );
        // (No array/scalar shape check here -- see the note in
        // `validate_call_arguments_handles`: `&buffer`-into-`addr` and text/byte args
        // make the argument position a false-positive minefield.)
    }
}

/// FROZEN DECISION 13 residue (value-position complement of `validate_call_node`).
///
/// Walk every expression in every statement of `state` and enforce
/// machine-call type-parameter bounds for VALUE-position calls
/// (`let r = self.pick(&self.h)`).  These never reach `validate_call_node`
/// because they appear as `ExpressionNode::Call` inside expression trees,
/// not as top-level `StatementNode::Call` nodes.
///
/// Scope: covers all expression positions that feed into statements
/// (LocalData initializers, assignment values/targets, guard expressions,
/// transition arguments, terminal expressions) and recurses into nested
/// call arguments.  Enforces the type-parameter BOUND check plus, for a
/// RESOLVED callee, argument arity (`report_argument_count_mismatch`) and the
/// per-argument class/narrowing/nominal checks (`validate_value_call_argument_classes`).
/// The remaining frontier is the UNRESOLVED callee: a value call whose target
/// no branch resolves falls through silently (the nonexistent-value-call gap),
/// which needs the complete value-call target resolver.
#[allow(clippy::too_many_arguments)]
pub(crate) fn validate_value_position_calls(
    program: &TypedTrees,
    machine: &Machine,
    state: &State,
    statement: &StatementNode,
    machine_symbols: &MachineSymbols<'_>,
    symbols: &TopLevelSymbols<'_>,
    writable_roots: &WritableRoots<'_, '_>,
    value_env: &ValueEnv,
    diagnostics: &mut Vec<Diagnostic>,
) {
    match statement {
        StatementNode::AssemblyFact(_) => {}
        StatementNode::Assignment(assignment) => {
            scan_expression_calls(
                program,
                machine,
                state,
                machine_symbols,
                symbols,
                writable_roots,
                value_env,
                assignment.value,
                diagnostics,
            );
            // target is a place (Name/Member/Indexed), no calls to validate
        }
        StatementNode::Call(call) => {
            // Statement-position call arguments may themselves be value calls.
            for argument in program.statement_table.expression_handles(call.arguments) {
                scan_expression_calls(
                    program,
                    machine,
                    state,
                    machine_symbols,
                    symbols,
                    writable_roots,
                    value_env,
                    *argument,
                    diagnostics,
                );
            }
        }
        StatementNode::Expression(expression) => {
            scan_expression_calls(
                program,
                machine,
                state,
                machine_symbols,
                symbols,
                writable_roots,
                value_env,
                *expression,
                diagnostics,
            );
        }
        StatementNode::LocalData(local_data) => {
            scan_expression_calls(
                program,
                machine,
                state,
                machine_symbols,
                symbols,
                writable_roots,
                value_env,
                local_data.initial_value,
                diagnostics,
            );
        }
        StatementNode::Transition(transition) => {
            if let TransitionGuardNode::When(guard) = transition.guard {
                scan_expression_calls(
                    program,
                    machine,
                    state,
                    machine_symbols,
                    symbols,
                    writable_roots,
                    value_env,
                    guard,
                    diagnostics,
                );
            }
            for target_handle in [transition.target, transition.continuation] {
                if !target_handle.is_valid() {
                    continue;
                }
                let target = program.statement_table.transition_target(target_handle);
                match target {
                    TransitionTargetNode::Named { arguments, .. } => {
                        for argument in program.statement_table.expression_handles(*arguments) {
                            scan_expression_calls(
                                program,
                                machine,
                                state,
                                machine_symbols,
                                symbols,
                                writable_roots,
                                value_env,
                                *argument,
                                diagnostics,
                            );
                        }
                    }
                    TransitionTargetNode::Value(expression) => {
                        scan_expression_calls(
                            program,
                            machine,
                            state,
                            machine_symbols,
                            symbols,
                            writable_roots,
                            value_env,
                            *expression,
                            diagnostics,
                        );
                    }
                    TransitionTargetNode::SelfTarget | TransitionTargetNode::Terminal => {}
                }
            }
        }
    }
}

/// Measured recursion MR3 (2026-07-18 ruling): runtime recursion is
/// TAIL-ONLY, and the only tail position is the transition ARM TARGET
/// (`-> self.f(..)` on a measured machine, resolved onto the loop-back
/// edge). This walk names every OTHER self-recursive call spelling with
/// why it is not tail:
/// - embedded in a larger expression (`3 * self.f(n - 1)`, an argument, a
///   guard, an initializer): the frame must survive the call to finish the
///   surrounding computation -- non-tail, CUT by the amendment (depth
///   lives in explicit storage the author sizes);
/// - a state's bare terminal expression (`{ self.sum(n - 1, acc) }`): tail
///   in shape, but its loop-back rewrite is the MR2 rung -- refused with
///   that pointer until the lowering lands.
pub(crate) fn validate_self_recursive_call_positions(
    program: &TypedTrees,
    machine: &Machine,
    state: &State,
    statement: &StatementNode,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let entry_name = machine
        .name
        .as_str()
        .rsplit("::")
        .next()
        .unwrap_or(machine.name.as_str());
    match statement {
        StatementNode::AssemblyFact(_) => {}
        // The statement-position fence in `validate_call_node` owns
        // StatementNode::Call; transition ARM TARGETS are the legal tail
        // spelling (planned by the state graph). Everything else that can
        // hold an expression walks below.
        StatementNode::Call(call) => {
            for argument in program.statement_table.expression_handles(call.arguments) {
                reject_embedded_self_calls(program, machine, entry_name, *argument, diagnostics);
            }
        }
        StatementNode::Assignment(assignment) => {
            reject_embedded_self_calls(program, machine, entry_name, assignment.value, diagnostics);
        }
        StatementNode::LocalData(local_data) => {
            reject_embedded_self_calls(
                program,
                machine,
                entry_name,
                local_data.initial_value,
                diagnostics,
            );
        }
        StatementNode::Expression(expression) => {
            // A bare terminal self-call is TAIL in shape; the whole-expression
            // case gets the MR2 pointer, anything nested is non-tail.
            // A terminal self-call surviving to validation means the machine
            // is UNMEASURED: the parser rewrites measured machines' terminal
            // tail calls onto the loop-back edge (MR2).
            if let Some(call_display) = whole_expression_self_call(program, entry_name, *expression)
            {
                diagnostics.push(Diagnostic::error(format!(
                    "`{call_display}` in terminal position is TAIL self-recursion on an \
                     UNMEASURED machine. Recursive call spellings are legal only when \
                     measured: declare `terminates by ...;` and the terminal \
                     call rewrites onto the loop-back edge; unmeasured repetition spells \
                     as the bare loop `-> {entry_name}(..)` (constant stack, may diverge).",
                )));
                return;
            }
            reject_embedded_self_calls(program, machine, entry_name, *expression, diagnostics);
        }
        StatementNode::Transition(transition) => {
            if let TransitionGuardNode::When(guard) = transition.guard {
                reject_embedded_self_calls(program, machine, entry_name, guard, diagnostics);
            }
            for target_handle in [transition.target, transition.continuation] {
                if !target_handle.is_valid() {
                    continue;
                }
                match program.statement_table.transition_target(target_handle) {
                    // Arm-target arguments are evaluated AT the jump; a
                    // self-call inside one still needs its own frame first.
                    TransitionTargetNode::Named { arguments, .. } => {
                        for argument in program.statement_table.expression_handles(*arguments) {
                            reject_embedded_self_calls(
                                program,
                                machine,
                                entry_name,
                                *argument,
                                diagnostics,
                            );
                        }
                    }
                    TransitionTargetNode::Value(expression) => {
                        reject_embedded_self_calls(
                            program,
                            machine,
                            entry_name,
                            *expression,
                            diagnostics,
                        );
                    }
                    TransitionTargetNode::SelfTarget | TransitionTargetNode::Terminal => {}
                }
            }
        }
    }
    let _ = state;
}

/// PROOF-MACHINE recursion legality (math roster N2d gateway). A free
/// machine over proof-only data emits no runtime code, so the tail-only
/// rule does not apply (no frame survives anything) -- structural recursion
/// in ANY position is the induction the measure licenses. What DOES apply,
/// both strata: a cycle without a measure is an unproven termination claim.
/// Every self-call must be measured, and rung 1 proves the decrease
/// STRUCTURALLY: the argument in the measure's parameter position is a
/// case-payload SUBTERM of the measure -- a pattern binding like
/// `transition n { Nat::Succ { prev } -> .. double(prev) .. }` lowers
/// `prev` to the case-tagged member read `n.prev`, so the test is "a Member
/// chain (>= 1 hop) rooted at the measure parameter". Anything else refuses
/// with the shape named; the arithmetic bridge (n > 0 => n == Succ(n - 1))
/// is the recorded follow-on.
/// COMPUTED-SUBJECT strict decrease by CITATION (N4 order rung, slice a2,
/// design-ruled 2026-07-17): a recursive proof machine whose measure
/// argument is an application (`mod(sub(a, b), b)` at measure `a`) proves
/// the strict edge by citing a lemma in the SAME state whose instantiated
/// ensures is EXACTLY the monus-order strict fact
/// `sub(Succ(ARG), MEASURE) == Zero` (`ARG < MEASURE`). The cited lemma's
/// REQUIRES discharge syntactically at the site against (i) the citing
/// machine's own requires and (ii) the incoming-arm case equations (every
/// transition arm targeting this state whose guard cases subject S into
/// constructor C contributes the fact `S == C` -- the mod shape's Zero arm
/// over `sub(b, a)` contributes exactly `sub(b, a) == Zero`, the `b <= a`
/// premise). Everything is structural expression equality -- no arithmetic
/// is re-derived here; the lemma carries the mathematics.
fn cited_strict_decrease(
    program: &TypedTrees,
    machine: &Machine,
    state: &omega_typed_trees::state::State,
    argument: ExpressionHandle,
    measure_name: Option<&omega_typed_trees::name::Identifier>,
) -> bool {
    let Some(measure_name) = measure_name else {
        return false;
    };
    // A let-bound edge argument (`let next = sub(a, b); .. mod(next, b)` --
    // the value-call face forces the hoist) resolves through its
    // initializer before matching.
    let argument = resolve_state_local(program, state, argument);
    // Site facts: the citing machine's requires + incoming-arm equations.
    let mut site_facts: Vec<SiteFact> = Vec::new();
    for contract in program.machine_contracts(machine) {
        if !matches!(
            contract.kind,
            omega_typed_trees::signature::SignatureContractKind::Requires
        ) {
            continue;
        }
        for fact in program.proof_facts.span_or_empty(contract.facts) {
            if let omega_typed_trees::domain::ProofFact::Expression(expression) = fact
                && let ExpressionNode::Binary(binary) =
                    program.expression_table.expression(*expression)
                && binary.operator == omega_typed_trees::expression::BinaryOperator::Equal
            {
                site_facts.push(SiteFact {
                    left: binary.left,
                    right: binary.right,
                });
            }
        }
    }
    for other in program.machine_states(machine) {
        for statement in program.statement_table.statements(other.statement_nodes) {
            let StatementNode::Transition(transition) = statement else {
                continue;
            };
            let omega_typed_trees::statement::TransitionGuardNode::When(guard) = transition.guard
            else {
                continue;
            };
            if !transition.target.is_valid() {
                continue;
            }
            let TransitionTargetNode::Named { path, .. } =
                program.statement_table.transition_target(transition.target)
            else {
                continue;
            };
            let targets_state = program
                .statement_table
                .name_path_members(path.members)
                .last()
                .is_some_and(|name| name.as_str() == state.name.as_str());
            if !targets_state {
                continue;
            }
            if let ExpressionNode::Binary(binary) = program.expression_table.expression(guard)
                && binary.operator == omega_typed_trees::expression::BinaryOperator::Equal
            {
                site_facts.push(SiteFact {
                    left: binary.left,
                    right: binary.right,
                });
            }
        }
    }

    // Each citation in THIS state: a bare statement call to a free machine.
    for statement in program.statement_table.statements(state.statement_nodes) {
        let StatementNode::Call(call) = statement else {
            continue;
        };
        let receiver_members = program.statement_table.name_path_members(call.receiver);
        if !receiver_members.is_empty() {
            continue;
        }
        let Some(callee) = program.machines().iter().find(|candidate| {
            candidate.attached_data.is_none()
                && candidate
                    .name
                    .as_str()
                    .rsplit("::")
                    .next()
                    .unwrap_or(candidate.name.as_str())
                    == call.target.as_str()
        }) else {
            continue;
        };
        let Some(entry) = program.machine_states(callee).first() else {
            continue;
        };
        let parameters = program.state_parameters(entry);
        let citation_arguments = program.statement_table.expression_handles(call.arguments);
        if parameters.len() != citation_arguments.len() {
            continue;
        }
        let map: Vec<(&str, ExpressionHandle)> = parameters
            .iter()
            .zip(citation_arguments)
            .map(|(parameter, argument)| (parameter.name.as_str(), *argument))
            .collect();

        // The callee's requires must all discharge against the site facts.
        let mut requires_ok = true;
        let mut ensures_matches = false;
        for contract in program.machine_contracts(callee) {
            match contract.kind {
                omega_typed_trees::signature::SignatureContractKind::Requires => {
                    for fact in program.proof_facts.span_or_empty(contract.facts) {
                        let omega_typed_trees::domain::ProofFact::Expression(expression) = fact
                        else {
                            requires_ok = false;
                            continue;
                        };
                        let ExpressionNode::Binary(binary) =
                            program.expression_table.expression(*expression)
                        else {
                            requires_ok = false;
                            continue;
                        };
                        if binary.operator != omega_typed_trees::expression::BinaryOperator::Equal {
                            requires_ok = false;
                            continue;
                        }
                        let discharged = site_facts.iter().any(|site| {
                            substituted_expression_equals(program, binary.left, &map, site.left)
                                && substituted_expression_equals(
                                    program,
                                    binary.right,
                                    &map,
                                    site.right,
                                )
                        });
                        if !discharged {
                            requires_ok = false;
                        }
                    }
                }
                omega_typed_trees::signature::SignatureContractKind::Ensures => {
                    for fact in program.proof_facts.span_or_empty(contract.facts) {
                        let omega_typed_trees::domain::ProofFact::Expression(expression) = fact
                        else {
                            continue;
                        };
                        if ensures_is_strict_decrease(
                            program,
                            *expression,
                            &map,
                            argument,
                            measure_name,
                        ) {
                            ensures_matches = true;
                        }
                    }
                }
                _ => {}
            }
        }
        if requires_ok && ensures_matches {
            return true;
        }
    }
    false
}

/// Resolve a single-name expression through a same-state `let` binding to
/// its initializer (one hop; anything else returns the input unchanged).
fn resolve_state_local(
    program: &TypedTrees,
    state: &omega_typed_trees::state::State,
    expression: ExpressionHandle,
) -> ExpressionHandle {
    let ExpressionNode::Name(path) = program.expression_table.expression(expression) else {
        return expression;
    };
    let [only] = program.expression_table.name_path_members(path.members) else {
        return expression;
    };
    for statement in program.statement_table.statements(state.statement_nodes) {
        if let StatementNode::LocalData(local) = statement
            && local.name.as_str() == only.as_str()
            && local.initial_value.is_valid()
        {
            return local.initial_value;
        }
    }
    expression
}

struct SiteFact {
    left: ExpressionHandle,
    right: ExpressionHandle,
}

/// The callee's ensures fact, instantiated at the citation's arguments,
/// must be exactly `sub(Succ { prev: ARG }, MEASURE) == Nat::Zero`.
fn ensures_is_strict_decrease(
    program: &TypedTrees,
    fact: ExpressionHandle,
    map: &[(&str, ExpressionHandle)],
    edge_argument: ExpressionHandle,
    measure_name: &omega_typed_trees::name::Identifier,
) -> bool {
    let ExpressionNode::Binary(binary) = program.expression_table.expression(fact) else {
        return false;
    };
    if binary.operator != omega_typed_trees::expression::BinaryOperator::Equal {
        return false;
    }
    // RHS: the Zero constructor.
    if !expression_is_nat_zero(program, binary.right) {
        return false;
    }
    // LHS: sub(Succ { prev: X }, M).
    let ExpressionNode::Call(sub_call) = program.expression_table.expression(binary.left) else {
        return false;
    };
    if sub_call.target.as_str() != "sub" {
        return false;
    }
    let sub_arguments = program
        .expression_table
        .expression_handles(sub_call.arguments);
    let [succ_side, measure_side] = sub_arguments else {
        return false;
    };
    let ExpressionNode::StructLiteral(literal) = program.expression_table.expression(*succ_side)
    else {
        return false;
    };
    if literal.case_name.as_ref().map(|name| name.as_str()) != Some("Succ") {
        return false;
    }
    let fields = program.expression_table.struct_fields(literal.fields);
    let [field] = fields else {
        return false;
    };
    if field.name.as_str() != "prev" {
        return false;
    }
    substituted_expression_equals(program, field.value, map, edge_argument)
        && substituted_name_is(program, *measure_side, map, measure_name.as_str())
}

fn expression_is_nat_zero(program: &TypedTrees, handle: ExpressionHandle) -> bool {
    match program.expression_table.expression(handle) {
        ExpressionNode::StructLiteral(literal) => {
            literal.case_name.as_ref().map(|name| name.as_str()) == Some("Zero")
                && program
                    .expression_table
                    .struct_fields(literal.fields)
                    .is_empty()
        }
        ExpressionNode::Name(path) => program
            .expression_table
            .name_path_members(path.members)
            .last()
            .is_some_and(|member| member.as_str() == "Zero"),
        _ => false,
    }
}

/// Does the callee-side expression, with callee parameters substituted by
/// the citation's argument expressions, resolve to the single NAME `name`?
fn substituted_name_is(
    program: &TypedTrees,
    callee_side: ExpressionHandle,
    map: &[(&str, ExpressionHandle)],
    name: &str,
) -> bool {
    if let ExpressionNode::Name(path) = program.expression_table.expression(callee_side)
        && let [only] = program.expression_table.name_path_members(path.members)
    {
        if let Some((_, substituted)) = map.iter().find(|(param, _)| *param == only.as_str()) {
            return expression_is_single_name(program, *substituted, name);
        }
        return only.as_str() == name;
    }
    false
}

fn expression_is_single_name(program: &TypedTrees, handle: ExpressionHandle, name: &str) -> bool {
    matches!(
        program.expression_table.expression(handle),
        ExpressionNode::Name(path)
            if matches!(
                program.expression_table.name_path_members(path.members),
                [only] if only.as_str() == name
            )
    )
}

/// Structural equality: callee-side expression under the citation's
/// parameter substitution vs a caller-side expression. Names compare by
/// their (single) member spelling; parenthesization is transparent in the
/// table form. Conservative: unhandled node kinds compare false.
fn substituted_expression_equals(
    program: &TypedTrees,
    callee_side: ExpressionHandle,
    map: &[(&str, ExpressionHandle)],
    caller_side: ExpressionHandle,
) -> bool {
    // A callee-side parameter name substitutes to its citation argument
    // and the comparison continues caller-vs-caller.
    if let ExpressionNode::Name(path) = program.expression_table.expression(callee_side)
        && let [only] = program.expression_table.name_path_members(path.members)
        && let Some((_, substituted)) = map.iter().find(|(param, _)| *param == only.as_str())
    {
        return caller_expressions_equal(program, *substituted, caller_side);
    }
    match (
        program.expression_table.expression(callee_side),
        program.expression_table.expression(caller_side),
    ) {
        (ExpressionNode::Name(left), ExpressionNode::Name(right)) => {
            let left = program.expression_table.name_path_members(left.members);
            let right = program.expression_table.name_path_members(right.members);
            left.len() == right.len()
                && left
                    .iter()
                    .zip(right)
                    .all(|(l, r)| l.as_str() == r.as_str())
        }
        (ExpressionNode::Call(left), ExpressionNode::Call(right)) => {
            if left.target.as_str() != right.target.as_str() {
                return false;
            }
            let left_arguments = program.expression_table.expression_handles(left.arguments);
            let right_arguments = program.expression_table.expression_handles(right.arguments);
            left_arguments.len() == right_arguments.len()
                && left_arguments
                    .iter()
                    .zip(right_arguments)
                    .all(|(l, r)| substituted_expression_equals(program, *l, map, *r))
        }
        (ExpressionNode::StructLiteral(left), ExpressionNode::StructLiteral(right)) => {
            if left.type_name.as_str() != right.type_name.as_str()
                || left.case_name.as_ref().map(|name| name.as_str())
                    != right.case_name.as_ref().map(|name| name.as_str())
            {
                return false;
            }
            let left_fields = program.expression_table.struct_fields(left.fields);
            let right_fields = program.expression_table.struct_fields(right.fields);
            left_fields.len() == right_fields.len()
                && left_fields.iter().zip(right_fields).all(|(l, r)| {
                    l.name.as_str() == r.name.as_str()
                        && substituted_expression_equals(program, l.value, map, r.value)
                })
        }
        (ExpressionNode::Integer(left), ExpressionNode::Integer(right)) => {
            left.value_i64() == right.value_i64()
        }
        _ => false,
    }
}

/// Caller-space structural equality (no substitution on either side).
fn caller_expressions_equal(
    program: &TypedTrees,
    left: ExpressionHandle,
    right: ExpressionHandle,
) -> bool {
    substituted_expression_equals(program, left, &[], right)
}

/// The arithmetic predecessor bridge for proof recursion over an integer
/// measure.  A call nested in the TAKEN value of `transition n > 0` may pass
/// `n - 1`: the guard proves subtraction cannot underflow and the result is
/// strictly below `n`.  Keep this association syntactic and local -- an
/// unrelated positive guard elsewhere in the state must not license the
/// call.
fn guarded_integer_predecessor_call(
    program: &TypedTrees,
    state: &omega_typed_trees::state::State,
    entry_name: &str,
    measure_position: usize,
    argument: ExpressionHandle,
    measure_symbol: omega_core::symbols::SymbolHandle,
    measure_name: Option<&omega_typed_trees::name::Identifier>,
) -> bool {
    let ExpressionNode::Binary(predecessor) = program.expression_table.expression(argument) else {
        return false;
    };
    if predecessor.operator != omega_typed_trees::expression::BinaryOperator::Subtract
        || !expression_names_measure(program, predecessor.left, measure_symbol, measure_name)
        || !matches!(
            program.expression_table.expression(predecessor.right),
            ExpressionNode::Integer(literal) if literal.value_i64() == Some(1)
        )
    {
        return false;
    }

    program
        .statement_table
        .statements(state.statement_nodes)
        .iter()
        .any(|statement| {
            let StatementNode::Transition(transition) = statement else {
                return false;
            };
            let TransitionGuardNode::When(guard) = transition.guard else {
                return false;
            };
            let guard = match program.expression_table.expression(guard) {
                ExpressionNode::Binary(wrapper)
                    if wrapper.operator == omega_typed_trees::expression::BinaryOperator::Equal
                        && matches!(
                            program.expression_table.expression(wrapper.right),
                            ExpressionNode::Boolean(true)
                        ) =>
                {
                    wrapper.left
                }
                _ => guard,
            };
            let ExpressionNode::Binary(positive) = program.expression_table.expression(guard)
            else {
                return false;
            };
            if positive.operator != omega_typed_trees::expression::BinaryOperator::Greater
                || !expression_names_measure(program, positive.left, measure_symbol, measure_name)
                || !matches!(
                    program.expression_table.expression(positive.right),
                    ExpressionNode::Integer(literal) if literal.value_i64() == Some(0)
                )
            {
                return false;
            }

            let TransitionTargetNode::Value(value) =
                program.statement_table.transition_target(transition.target)
            else {
                return false;
            };
            let mut calls = Vec::new();
            collect_self_entry_call_arguments(program, entry_name, *value, &mut calls);
            calls.into_iter().any(|arguments| {
                program
                    .expression_table
                    .expression_handles(arguments)
                    .get(measure_position)
                    .is_some_and(|candidate| *candidate == argument)
            })
        })
}

fn expression_names_measure(
    program: &TypedTrees,
    expression: ExpressionHandle,
    measure_symbol: omega_core::symbols::SymbolHandle,
    measure_name: Option<&omega_typed_trees::name::Identifier>,
) -> bool {
    let ExpressionNode::Name(path) = program.expression_table.expression(expression) else {
        return false;
    };
    path.symbol == measure_symbol
        || measure_name.is_some_and(|name| {
            program
                .expression_table
                .name_path_members(path.members)
                .last()
                .is_some_and(|member| member.as_str() == name.as_str())
        })
}

pub(crate) fn validate_proof_machine_recursion(
    program: &TypedTrees,
    machine: &Machine,
    state: &omega_typed_trees::state::State,
    statement: &StatementNode,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let entry_name = machine
        .name
        .as_str()
        .rsplit("::")
        .next()
        .unwrap_or(machine.name.as_str());
    let mut self_calls: Vec<HandleSpan<ExpressionHandle>> = Vec::new();
    for root in statement_expression_roots(program, statement) {
        collect_self_entry_call_arguments(program, entry_name, root, &mut self_calls);
    }
    if self_calls.is_empty() {
        return;
    }

    let subjects = program
        .expression_table
        .expression_handles(machine.decreases);
    let [subject] = subjects else {
        diagnostics.push(Diagnostic::error(format!(
            "recursive proof machine `{}` needs a single structural measure: declare \
             `terminates by <param>;` naming one proof-data parameter -- a \
             cycle without a measure is an unproven termination claim (measured \
             recursion, both strata)",
            machine.name,
        )));
        return;
    };
    let ExpressionNode::Name(measure_path) = program.expression_table.expression(*subject) else {
        diagnostics.push(Diagnostic::error(format!(
            "recursive proof machine `{}`: the structural measure must be a bare \
             parameter name (rung 1); compound measures over proof data are not \
             proven yet",
            machine.name,
        )));
        return;
    };
    let measure_symbol = measure_path.symbol;
    let measure_name = program
        .expression_table
        .name_path_members(measure_path.members)
        .first()
        .cloned();
    // The measure names an ENTRY parameter; its POSITION is where every
    // self-call's argument must descend.
    let Some(measure_position) = program.machine_states(machine).first().and_then(|entry| {
        program
            .state_parameters(entry)
            .iter()
            .position(|parameter| {
                (parameter.symbol.is_valid() && parameter.symbol == measure_symbol)
                    || measure_name
                        .as_ref()
                        .is_some_and(|name| parameter.name.as_str() == name.as_str())
            })
    }) else {
        diagnostics.push(Diagnostic::error(format!(
            "recursive proof machine `{}`: the measure must name an entry parameter",
            machine.name,
        )));
        return;
    };

    for arguments in self_calls {
        let argument = program
            .expression_table
            .expression_handles(arguments)
            .get(measure_position)
            .copied();
        let descends = argument.is_some_and(|argument| {
            strict_subterm_of_measure(program, argument, measure_symbol, measure_name.as_ref())
                || substate_parameter_descends(
                    program,
                    machine,
                    argument,
                    measure_symbol,
                    measure_name.as_ref(),
                )
                || cited_strict_decrease(program, machine, state, argument, measure_name.as_ref())
                || measure_name.as_ref().is_some_and(|name| {
                    crate::contract_entailment::proof_edge_strict_decrease_judged(
                        program,
                        machine,
                        state,
                        argument,
                        name.as_str(),
                    )
                })
                || guarded_integer_predecessor_call(
                    program,
                    state,
                    entry_name,
                    measure_position,
                    argument,
                    measure_symbol,
                    measure_name.as_ref(),
                )
        });
        if !descends {
            diagnostics.push(Diagnostic::error(format!(
                "`{entry_name}(..)` cannot prove the measure `{}` structurally \
                 decreases at this self-call: the call does not prove a strict \
                 predecessor of ranking subject `{}`. Pass a case-payload subterm \
                 (`Nat::Succ {{ prev }} -> .. {entry_name}(prev)`) or, for an integer \
                 measure, `n - 1` in the taken value of its dominating `n > 0` arm",
                measure_name
                    .as_ref()
                    .map(|name| name.as_str())
                    .unwrap_or("<measure>"),
                measure_name
                    .as_ref()
                    .map(|name| name.as_str())
                    .unwrap_or("<measure>"),
            )));
        }
    }
}

/// The root expression handles a statement can carry (guard subjects, arm
/// arguments, terminal values, initializers, call arguments).
fn statement_expression_roots(
    program: &TypedTrees,
    statement: &StatementNode,
) -> Vec<ExpressionHandle> {
    match statement {
        StatementNode::AssemblyFact(_) => Vec::new(),
        StatementNode::Call(call) => program
            .statement_table
            .expression_handles(call.arguments)
            .to_vec(),
        StatementNode::Assignment(assignment) => vec![assignment.target, assignment.value],
        StatementNode::LocalData(local_data) => vec![local_data.initial_value],
        StatementNode::Expression(expression) => vec![*expression],
        StatementNode::Transition(transition) => {
            let mut roots = Vec::new();
            if let TransitionGuardNode::When(guard) = transition.guard {
                roots.push(guard);
            }
            for target_handle in [transition.target, transition.continuation] {
                if !target_handle.is_valid() {
                    continue;
                }
                match program.statement_table.transition_target(target_handle) {
                    TransitionTargetNode::Named { arguments, .. } => {
                        roots.extend(
                            program
                                .statement_table
                                .expression_handles(*arguments)
                                .iter()
                                .copied(),
                        );
                    }
                    TransitionTargetNode::Value(expression) => roots.push(*expression),
                    TransitionTargetNode::SelfTarget | TransitionTargetNode::Terminal => {}
                }
            }
            roots
        }
    }
}

/// Collect the ARGUMENT spans of every self-entry call in this tree.
fn collect_self_entry_call_arguments(
    program: &TypedTrees,
    entry_name: &str,
    expression: ExpressionHandle,
    found: &mut Vec<HandleSpan<ExpressionHandle>>,
) {
    if !expression.is_valid() {
        return;
    }
    let mut recurse = |handle: ExpressionHandle, found: &mut Vec<HandleSpan<ExpressionHandle>>| {
        collect_self_entry_call_arguments(program, entry_name, handle, found);
    };
    match program.expression_table.expression(expression) {
        ExpressionNode::Atomic(atomic) => recurse(atomic.value, found),
        ExpressionNode::Call(call) => {
            if is_self_entry_call(program, entry_name, call) {
                found.push(call.arguments);
            }
            recurse(call.receiver, found);
            for argument in program.expression_table.expression_handles(call.arguments) {
                recurse(*argument, found);
            }
        }
        ExpressionNode::Binary(binary) => {
            recurse(binary.left, found);
            recurse(binary.right, found);
        }
        ExpressionNode::Cast(cast) => recurse(cast.value, found),
        ExpressionNode::Indexed(indexed) => {
            recurse(indexed.collection, found);
            recurse(indexed.index, found);
        }
        ExpressionNode::Member(member) => recurse(member.receiver, found),
        ExpressionNode::Mutable(inner) => recurse(*inner, found),
        ExpressionNode::Range(range) => {
            recurse(range.start, found);
            recurse(range.end, found);
        }
        ExpressionNode::Unary(unary) => recurse(unary.operand, found),
        ExpressionNode::ArrayLiteral(items) => {
            for item in program.expression_table.expression_handles(*items) {
                recurse(*item, found);
            }
        }
        ExpressionNode::StructLiteral(struct_literal) => {
            for field in program
                .expression_table
                .struct_fields(struct_literal.fields)
            {
                recurse(field.value, found);
            }
        }
        ExpressionNode::Boolean(_)
        | ExpressionNode::Float(_)
        | ExpressionNode::Integer(_)
        | ExpressionNode::Name(_)
        | ExpressionNode::String(_) => {}
    }
}

/// A STRICT subterm of the measure: a Member chain of one or more hops whose
/// root Name is the measure parameter (symbol match, name fallback). One hop
/// = one constructor consumed (`n.prev`); depth composes for free.
/// N3 rung 2 -- descent THROUGH a sub-state parameter: a self-call inside a
/// per-arm sub-proof passes the sub-state's own parameter (`step_case(prev,
/// b)` bound it at the entry arm from the measure's case payload, so inside
/// `step_case` the recursion argument is the bare name `prev`). The
/// parameter counts as strictly descending iff EVERY Named transition into
/// its state passes a strict-subterm Member read of the measure at that
/// position -- provenance over ALL binding sites, so a single
/// non-descending entry poisons the parameter.
///
/// Matching is symbol-first (precise); the name fallback additionally
/// refuses when any local or assignment anywhere in the machine shares the
/// name, so a shadowing binding cannot launder a non-descending value
/// through a descending parameter's name.
fn substate_parameter_descends(
    program: &TypedTrees,
    machine: &Machine,
    argument: ExpressionHandle,
    measure_symbol: omega_core::symbols::SymbolHandle,
    measure_name: Option<&Identifier>,
) -> bool {
    let ExpressionNode::Name(path) = program.expression_table.expression(argument) else {
        return false;
    };
    let [name] = program.expression_table.name_path_members(path.members) else {
        return false;
    };
    let states = program.machine_states(machine);
    if states.len() < 2 {
        return false;
    }
    // Every sub-state parameter this name could denote (symbol-first).
    let mut candidates: Vec<(&omega_typed_trees::state::State, usize)> = Vec::new();
    let mut symbol_matched = false;
    for state in &states[1..] {
        for (position, parameter) in program.state_parameters(state).iter().enumerate() {
            let by_symbol = path.symbol.is_valid()
                && parameter.symbol.is_valid()
                && parameter.symbol == path.symbol;
            let by_name = parameter.name.as_str() == name.as_str();
            if by_symbol {
                symbol_matched = true;
                candidates.push((state, position));
            } else if by_name {
                candidates.push((state, position));
            }
        }
    }
    if candidates.is_empty() {
        return false;
    }
    if !symbol_matched {
        // Name-only matching: refuse if anything else in the machine binds
        // this name.
        for state in states {
            for statement in program.statement_table.statements(state.statement_nodes) {
                match statement {
                    StatementNode::LocalData(local_data)
                        if local_data.name.as_str() == name.as_str() =>
                    {
                        return false;
                    }
                    StatementNode::Assignment(_) => return false,
                    _ => {}
                }
            }
        }
    }
    // Every candidate parameter must descend at EVERY Named transition into
    // its state.
    candidates.iter().all(|(sub_state, position)| {
        let mut binding_sites = 0usize;
        for source in states {
            for statement in program.statement_table.statements(source.statement_nodes) {
                let StatementNode::Transition(transition) = statement else {
                    continue;
                };
                for target_handle in [transition.target, transition.continuation] {
                    if !target_handle.is_valid() {
                        continue;
                    }
                    let TransitionTargetNode::Named { path, arguments } =
                        program.statement_table.transition_target(target_handle)
                    else {
                        continue;
                    };
                    let [target_name] = program.statement_table.name_path_members(path.members)
                    else {
                        return false;
                    };
                    if target_name.as_str() != sub_state.name.as_str() {
                        continue;
                    }
                    binding_sites += 1;
                    let handles = program.statement_table.expression_handles(*arguments);
                    let Some(bound) = handles.get(*position) else {
                        return false;
                    };
                    if !strict_subterm_of_measure(program, *bound, measure_symbol, measure_name) {
                        return false;
                    }
                }
            }
        }
        binding_sites > 0
    })
}

fn strict_subterm_of_measure(
    program: &TypedTrees,
    expression: ExpressionHandle,
    measure_symbol: omega_core::symbols::SymbolHandle,
    measure_name: Option<&Identifier>,
) -> bool {
    let ExpressionNode::Member(member) = program.expression_table.expression(expression) else {
        return false;
    };
    let mut root = member.receiver;
    loop {
        match program.expression_table.expression(root) {
            ExpressionNode::Member(inner) => root = inner.receiver,
            ExpressionNode::Name(path) => {
                return (path.symbol.is_valid() && path.symbol == measure_symbol)
                    || measure_name.is_some_and(|name| {
                        matches!(
                            program.expression_table.name_path_members(path.members),
                            [only] if only.as_str() == name.as_str()
                        )
                    });
            }
            _ => return false,
        }
    }
}

/// The rendered call when `expression` IS a self-call to the machine's own
/// entry (the terminal-position tail shape); None otherwise.
fn whole_expression_self_call(
    program: &TypedTrees,
    entry_name: &str,
    expression: ExpressionHandle,
) -> Option<String> {
    let ExpressionNode::Call(call) = program.expression_table.expression(expression) else {
        return None;
    };
    is_self_entry_call(program, entry_name, call).then(|| format!("self.{entry_name}(..)"))
}

fn is_self_entry_call(program: &TypedTrees, entry_name: &str, call: &TableCallExpression) -> bool {
    if call.target.as_str() != entry_name {
        return false;
    }
    !call.receiver.is_valid()
        || matches!(
            program.expression_table.expression(call.receiver),
            ExpressionNode::Name(path)
                if matches!(
                    program.expression_table.name_path_members(path.members),
                    [only] if only.as_str() == "self"
                )
        )
}

/// Reject every self-entry call in this expression tree: any hit here is
/// embedded in a larger computation, so the frame outlives the call.
fn reject_embedded_self_calls(
    program: &TypedTrees,
    machine: &Machine,
    entry_name: &str,
    expression: ExpressionHandle,
    diagnostics: &mut Vec<Diagnostic>,
) {
    if !expression.is_valid() {
        return;
    }
    let recurse = |handle: ExpressionHandle, diagnostics: &mut Vec<Diagnostic>| {
        reject_embedded_self_calls(program, machine, entry_name, handle, diagnostics);
    };
    match program.expression_table.expression(expression) {
        ExpressionNode::Atomic(atomic) => recurse(atomic.value, diagnostics),
        ExpressionNode::Call(call) => {
            if is_self_entry_call(program, entry_name, call) {
                diagnostics.push(Diagnostic::error(format!(
                    "`self.{entry_name}(..)` here is NON-TAIL self-recursion: the \
                     result feeds the surrounding computation, so the frame must \
                     survive the call. Runtime recursion is TAIL-ONLY (measured \
                     recursion amendment) -- recursion depth lives in explicit \
                     storage you size. Restructure so the recursive step is the \
                     transition arm `-> self.{entry_name}(..)` on a measured \
                     machine, or iterate with an explicit stack.",
                )));
            }
            recurse(call.receiver, diagnostics);
            for argument in program.expression_table.expression_handles(call.arguments) {
                recurse(*argument, diagnostics);
            }
        }
        ExpressionNode::Binary(binary) => {
            recurse(binary.left, diagnostics);
            recurse(binary.right, diagnostics);
        }
        ExpressionNode::Cast(cast) => recurse(cast.value, diagnostics),
        ExpressionNode::Indexed(indexed) => {
            recurse(indexed.collection, diagnostics);
            recurse(indexed.index, diagnostics);
        }
        ExpressionNode::Member(member) => recurse(member.receiver, diagnostics),
        ExpressionNode::Mutable(inner) => recurse(*inner, diagnostics),
        ExpressionNode::Range(range) => {
            recurse(range.start, diagnostics);
            recurse(range.end, diagnostics);
        }
        ExpressionNode::Unary(unary) => recurse(unary.operand, diagnostics),
        ExpressionNode::ArrayLiteral(items) => {
            for item in program.expression_table.expression_handles(*items) {
                recurse(*item, diagnostics);
            }
        }
        ExpressionNode::StructLiteral(struct_literal) => {
            for field in program
                .expression_table
                .struct_fields(struct_literal.fields)
            {
                recurse(field.value, diagnostics);
            }
        }
        ExpressionNode::Boolean(_)
        | ExpressionNode::Float(_)
        | ExpressionNode::Integer(_)
        | ExpressionNode::Name(_)
        | ExpressionNode::String(_) => {}
    }
}

/// Is `name` a legal single-segment BARE name in the body of `machine`?
///
/// A bare `Name` resolves to a field (implicit `self`), a top-level symbol (type
/// / machine / platform / trait), an enum case constant (`Red`), or a local/
/// parameter. The binding scope for locals and parameters is the WHOLE machine,
/// not one state: a sub-state legitimately reads a parameter or `let` declared on
/// the machine's entry (or an ancestor) state (`state nonpos` reading the entry
/// state's `n`). We therefore scan every state's parameters and `LocalData`.
///
/// The allow-list is deliberately GENEROUS -- scanning ALL states over-approximates
/// the true lexical scope, so an out-of-scope-but-declared name is accepted (an
/// UNDER-rejection, never a false rejection of a real name). The sole goal is to
/// catch a name that exists NOWHERE (a typo reading as 0/garbage).
fn is_known_bare_name(
    program: &TypedTrees,
    machine: &Machine,
    machine_symbols: &MachineSymbols<'_>,
    symbols: &TopLevelSymbols<'_>,
    name: &str,
) -> bool {
    // Field of the receiver data (bare `fld` == `self.fld`), owned data, or a
    // contained object.
    if machine_symbols.has_member(name)
        || machine_symbols.has_owned_data(name)
        || machine_symbols.contained_type(name).is_some()
    {
        return true;
    }
    // Top-level symbol: a type, machine, or trait spelled bare.
    if symbols.has_type(name)
        || symbols.machine(name).is_some()
        || symbols.trait_definition(name).is_some()
    {
        return true;
    }
    // A generic attached method may use its container's const parameter as a
    // value in the authored template. Instance desugaring replaces this name
    // with the concrete integer before the executable clone is validated.
    if program
        .machine_type_parameters(machine)
        .iter()
        .any(|parameter| {
            parameter.name.as_str() == name
                && matches!(&parameter.kind, TypeParameterKind::Const { .. })
        })
    {
        return true;
    }
    if let Some(attached_data) = &machine.attached_data
        && program.data_definitions().iter().any(|definition| {
            definition.name == *attached_data
                && program
                    .data_type_parameters(definition)
                    .iter()
                    .any(|parameter| {
                        parameter.name.as_str() == name
                            && matches!(&parameter.kind, TypeParameterKind::Const { .. })
                    })
        })
    {
        return true;
    }
    // Enum case constant used bare (`let s: Signal = Red`).
    for definition in program.data_definitions() {
        for member in program.data_members(definition) {
            if let DataMember::Variant(variant) = member
                && variant.name.as_str() == name
            {
                return true;
            }
        }
    }
    // Parameter or local declared on ANY state of this machine (whole-machine
    // scope -- see the doc comment).
    for other in program.machine_states(machine) {
        for parameter in program.state_parameters(other) {
            if parameter.name.as_str() == name {
                return true;
            }
        }
        for statement in program.statement_table.statements(other.statement_nodes) {
            if let StatementNode::LocalData(local) = statement
                && local.name.as_str() == name
            {
                return true;
            }
        }
    }
    false
}

/// Reject READING an array element with a RUNTIME SCALAR index whose collection is
/// itself reached THROUGH a RUNTIME array index -- `grid[i][j]` with BOTH indices
/// runtime. The lowering op carries ONE runtime index, so a runtime index above
/// another runtime index has no computable offset: the backend would resolve it to
/// the collection BASE and SILENTLY READ 0 instead of the element. Lowerable shapes
/// are untouched: any CONST index level (`grid[i][0]` -- suffix walk; `grid[1][j]`,
/// `rows[2].data[j]` -- const levels fold into the collection resolution's fixed
/// offset, landed 2026-07-07), a single index over a non-indexed base (`arr[i]`,
/// `self.field[j]`), and a RANGE index (`sub[1..][1..]`, a nested subslice). The
/// real fix for both-runtime is a two-index lowering op (backend feature).
fn report_nested_runtime_indexed_read(
    program: &TypedTrees,
    machine: &Machine,
    state: &State,
    expression: ExpressionHandle,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let ExpressionNode::Indexed(indexed) = program.expression_table.expression(expression) else {
        return;
    };
    // The index must be a RUNTIME SCALAR: not a constant integer (a fixed offset,
    // lowerable) and not a RANGE (`sub[1..][1..]` is a nested SUBSLICE that lowers).
    let mut index = indexed.index;
    while let ExpressionNode::Mutable(inner) = program.expression_table.expression(index) {
        index = *inner;
    }
    if matches!(
        program.expression_table.expression(index),
        ExpressionNode::Integer(_) | ExpressionNode::Range(_)
    ) {
        return;
    }
    // The collection's place chain (through Member receivers and Mutable) must reach
    // a RUNTIME array index -- `grid[c][j]` with BOTH indices runtime. A base with no
    // index in its chain (`arr`, `self.field`) is a plain place whose element offset
    // IS computable; a CONST-indexed base (`grid[1][j]`, `rows[2].data[j]`) folds into
    // the machine-owned collection resolution's fixed offset and lowers (2026-07-07:
    // walk-boundary threading in instruction selection + the root-element-index
    // ordering fix in resolve_machine_owned_collection_in_table; value-validated
    // differential probes). Only a runtime index ABOVE another RUNTIME index remains
    // unlowerable -- the op carries one runtime index.
    let mut collection = indexed.collection;
    let base_is_runtime_indexed = loop {
        match program.expression_table.expression(collection) {
            ExpressionNode::Indexed(inner) => {
                let mut inner_index = inner.index;
                while let ExpressionNode::Mutable(next) =
                    program.expression_table.expression(inner_index)
                {
                    inner_index = *next;
                }
                if !matches!(
                    program.expression_table.expression(inner_index),
                    ExpressionNode::Integer(_) | ExpressionNode::Range(_)
                ) {
                    break true;
                }
                collection = inner.collection;
            }
            ExpressionNode::Member(member) => collection = member.receiver,
            ExpressionNode::Mutable(inner) => collection = *inner,
            _ => break false,
        }
    };
    if !base_is_runtime_indexed {
        return;
    }
    // The DIRECTLY-nested two-level machine-owned read (`self.grid[i][j]`,
    // both indices runtime, the base a `self` field behind only const-indexed
    // /member links) lowers since 2026-07-07 via the double-indexed copy op
    // (resolve_runtime_machine_double_indexed_source_in_table -- this
    // predicate mirrors its shape gates exactly). Everything else stays
    // fenced: 3+ runtime levels, a member BETWEEN the two indices
    // (`rows[i].data[j]`), and frame/local/param collections. Faces the op's
    // consumers do not cover (writes, member suffixes above the element,
    // oversized elements) reject LOUDLY downstream -- the write classifier
    // refuses the fast path and the storage blockers report unlowered
    // statements, so nothing falls back to the legacy silent paths.
    if double_indexed_machine_read_is_lowerable(program, indexed) {
        return;
    }
    diagnostics.push(Diagnostic::error(format!(
        "machine `{}` state `{}` reads an array element with a runtime index whose base is itself \
         indexed by another RUNTIME value (`grid[i][j]`, both indices runtime), which the backend \
         cannot lower yet -- it silently reads 0 instead of the element. Make one index a constant, \
         or use a flat array with a computed index",
        machine.name.as_str(),
        state.name.as_str(),
    )));
}

/// Whether a both-runtime nested read matches the DOUBLE-INDEXED lowering's
/// shape: `<self-field base>[i][j]` -- the outer collection (Mutable-peeled)
/// is DIRECTLY another `Indexed` with a runtime index, and that inner
/// collection's chain reaches a `self` member through only CONST-indexed /
/// member / mutable links (the const-prefix peel folds those). Mirrors
/// `resolve_runtime_machine_double_indexed_source_in_table`.
fn double_indexed_machine_read_is_lowerable(
    program: &TypedTrees,
    indexed: &omega_typed_trees::expression::TableIndexedExpression,
) -> bool {
    // A member chain BETWEEN the two indices (`rows[i].data[j]`) is a fixed
    // offset the resolver folds into the op's field_byte_offset (2026-07-07),
    // so peel Member links too.
    let mut inner = indexed.collection;
    loop {
        match program.expression_table.expression(inner) {
            ExpressionNode::Mutable(next) => inner = *next,
            ExpressionNode::Member(member) => inner = member.receiver,
            _ => break,
        }
    }
    let ExpressionNode::Indexed(inner_indexed) = program.expression_table.expression(inner) else {
        return false;
    };
    let mut inner_index = inner_indexed.index;
    while let ExpressionNode::Mutable(next) = program.expression_table.expression(inner_index) {
        inner_index = *next;
    }
    if matches!(
        program.expression_table.expression(inner_index),
        ExpressionNode::Integer(_) | ExpressionNode::Range(_)
    ) {
        return false;
    }
    // Below the two runtime levels: only const-indexed / member / mutable
    // links, ending at a `self.<field>` head (machine-owned storage; a bare
    // local/param array stays fenced -- the frame op family does not cover
    // both-runtime yet).
    let mut place = inner_indexed.collection;
    loop {
        match program.expression_table.expression(place) {
            ExpressionNode::Indexed(below) => {
                let mut below_index = below.index;
                while let ExpressionNode::Mutable(next) =
                    program.expression_table.expression(below_index)
                {
                    below_index = *next;
                }
                if !matches!(
                    program.expression_table.expression(below_index),
                    ExpressionNode::Integer(_)
                ) {
                    return false;
                }
                place = below.collection;
            }
            ExpressionNode::Member(member) => {
                let receiver = member.receiver;
                if matches!(
                    program.expression_table.expression(receiver),
                    ExpressionNode::Name(path)
                        if program
                            .expression_table
                            .name_path_members(path.members)
                            .first()
                            .is_some_and(|name| name.as_str() == "self")
                ) {
                    return true;
                }
                place = receiver;
            }
            ExpressionNode::Mutable(next) => place = *next,
            ExpressionNode::Name(path) => {
                // A multi-member Name path starting at `self` (`self.grid`
                // resolved as one path) is machine-owned. A BARE single name
                // (a by-value param or local 2D array, `g[i][j]`) is the
                // FRAME flavor, lowered since 2026-07-07 by
                // CopyRuntimeFrameBaseDoubleIndexedToRuntimeStorage -- but
                // only for the DIRECTLY-nested member-free shape, which is
                // exactly what reaching this arm through the loop implies
                // for the frame case (member links between the indices stay
                // fenced by the resolver and report loudly downstream).
                let members = program.expression_table.name_path_members(path.members);
                return members.first().is_some_and(|name| name.as_str() == "self")
                    || members.len() == 1;
            }
            _ => return false,
        }
    }
}

/// Recursively scan `expression` for `ExpressionNode::Call` nodes and
/// validate machine-call type-parameter bounds for each one found.
#[allow(clippy::too_many_arguments)]
fn scan_expression_calls(
    program: &TypedTrees,
    machine: &Machine,
    state: &State,
    machine_symbols: &MachineSymbols<'_>,
    symbols: &TopLevelSymbols<'_>,
    writable_roots: &WritableRoots<'_, '_>,
    value_env: &ValueEnv,
    expression: ExpressionHandle,
    diagnostics: &mut Vec<Diagnostic>,
) {
    if !expression.is_valid() {
        return;
    }
    // Unknown-field READ: a direct `self.<field>` read of a nonexistent field (a typo)
    // gets a clear error instead of silently passing type-check. Mirrors the
    // assignment-target write check (places.rs): scoped to a direct `self.<field>`
    // against the machine's top-level data fields, versioned data excluded. Nested
    // `self.a.b` (checked at `a` when the recursion reaches the receiver) and non-self
    // members are left alone. The recursion continues afterward.
    if let Some(field_name) = crate::places::direct_self_field_member(program, expression)
        && let Some(data) = crate::places::machine_attached_data(program, machine)
        && !data_declares_field(program, data, field_name)
    {
        diagnostics.push(Diagnostic::error(format!(
            "machine `{}` state `{}` reads `self.{field_name}`, but data `{}` has no field \
             `{field_name}` (check the spelling of the field name)",
            machine.name.as_str(),
            state.name.as_str(),
            data.name.as_str()
        )));
    }
    // Unknown NESTED-field READ (3+ segments): `self.o.inner.nonexistent` (final
    // missing) and `self.o.bogus.value` (intermediate missing) used to compile
    // and silently read a ZII 0. The walker reports only a provably-missing
    // member on a provably-plain container -- versioned containers (fields in
    // wire version blocks), contained-machine/owned-data roots, and non-data
    // hops all skip. The recursion below revisits inner Member receivers, which
    // resolve to the SAME missing hop (`self.o.bogus.value` then `self.o.bogus`),
    // so an identical message is deduplicated rather than reported per level.
    if matches!(
        program.expression_table.expression(expression),
        ExpressionNode::Member(_) | ExpressionNode::Name(_)
    ) && let Some((container, member)) =
        crate::places::first_unknown_nested_field(program, machine, Some(state), expression)
    {
        let message = format!(
            "machine `{}` state `{}` reads a nested member `{member}`, but data `{container}` \
             has no field `{member}` (check the spelling of the field name)",
            machine.name.as_str(),
            state.name.as_str(),
        );
        if !diagnostics
            .iter()
            .any(|diagnostic| diagnostic.to_string().contains(&message))
        {
            diagnostics.push(Diagnostic::error(message));
        }
    }
    // Member / index access on a PRIMITIVE receiver. A number or bool has no fields,
    // no `.len`, and is not indexable, so `x.field` / `x.len` / `x[0]` on an `i32`
    // local silently reads a ZII 0 -- reject any such access. `String` is the one
    // exception, and only for MEMBER access: text carries a `.len` view, so `s.len`
    // is legal. An UNRESOLVED receiver or a struct / array / slice receiver is
    // left alone (those accesses are separate).
    let primitive_access = match program.expression_table.expression(expression) {
        ExpressionNode::Member(member) => Some((
            member.receiver,
            format!("member `{}`", member.member.as_str()),
        )),
        ExpressionNode::Indexed(indexed) => Some((indexed.collection, "an index".to_owned())),
        _ => None,
    };
    if let Some((receiver, access)) = primitive_access
        && let Some(receiver_type) =
            crate::places::declared_place_type(program, machine, Some(state), receiver)
        && let Some(primitive) = program.primitive_type_reference(receiver_type)
    {
        diagnostics.push(Diagnostic::error(format!(
            "machine `{}` state `{}` accesses {access} of a `{}` value, but a primitive \
             scalar has no members or elements",
            machine.name.as_str(),
            state.name.as_str(),
            primitive.name(),
        )));
    }
    // Unknown BARE NAME: a SINGLE-segment name (`undeclared_var`, not `self.x` or
    // `Type::Case`) that resolves to nothing is otherwise silently accepted and reads
    // as 0/garbage. Reject it when it is none of the legal bare-name forms. The scope
    // is the whole MACHINE (a sub-state may read the entry state's params/locals), and
    // `true`/`false` are single-segment `Name` nodes here, so they are skipped. The
    // allow-list is deliberately GENEROUS -- an unrecognised valid form only
    // UNDER-rejects (misses a typo), never falsely rejects a real name.
    if let ExpressionNode::Name(path) = program.expression_table.expression(expression)
        && let [only] = program.expression_table.name_path_members(path.members)
    {
        let name = only.as_str();
        if name != "self"
            && name != "true"
            && name != "false"
            && !is_known_bare_name(program, machine, machine_symbols, symbols, name)
        {
            diagnostics.push(Diagnostic::error(format!(
                "machine `{}` state `{}` uses `{name}`, which is not a declared local, \
                 parameter, field, or type (check the spelling)",
                machine.name.as_str(),
                state.name.as_str(),
            )));
        }
    }
    // Unknown TWO-segment path (`Type::Case`, `Trait::NAME`): the sibling of the
    // bare-name check above. A LEGITIMATE qualified case (`Signal::Green`) or a
    // substituted const resolves to a valid symbol before this stage; an
    // unresolved `Scope::tail` -- a bogus case (`Signal::Blue`) or a typo -- keeps BOTH the
    // head and leaf symbols invalid and would silently read 0 (ZII) in value
    // position (native AND interpreter agree, so the differential cannot catch
    // it). Reject exactly that shape: both symbols unresolved.
    if let ExpressionNode::Name(path) = program.expression_table.expression(expression)
        && let [scope, tail] = program.expression_table.name_path_members(path.members)
        && !path.head_symbol.is_valid()
        && !path.symbol.is_valid()
    {
        diagnostics.push(Diagnostic::error(format!(
            "machine `{}` state `{}` uses `{}::{}`, which resolves to no case, \
             constant, or per-target value -- it would silently read 0 (ZII). \
             Check the spelling (and, for a per-target value, that the selected \
             target's provides table declares it)",
            machine.name.as_str(),
            state.name.as_str(),
            scope.as_str(),
            tail.as_str(),
        )));
    }
    match program.expression_table.expression(expression) {
        ExpressionNode::Atomic(atomic) => scan_expression_calls(
            program,
            machine,
            state,
            machine_symbols,
            symbols,
            writable_roots,
            value_env,
            atomic.value,
            diagnostics,
        ),
        ExpressionNode::Call(call) => {
            let call = call.clone();
            validate_expression_call_bounds(
                program,
                machine,
                state,
                machine_symbols,
                symbols,
                writable_roots,
                value_env,
                &call,
                diagnostics,
            );
            // Recurse into the receiver and arguments (nested calls).
            if call.receiver.is_valid() {
                scan_expression_calls(
                    program,
                    machine,
                    state,
                    machine_symbols,
                    symbols,
                    writable_roots,
                    value_env,
                    call.receiver,
                    diagnostics,
                );
            }
            for argument in program.expression_table.expression_handles(call.arguments) {
                scan_expression_calls(
                    program,
                    machine,
                    state,
                    machine_symbols,
                    symbols,
                    writable_roots,
                    value_env,
                    *argument,
                    diagnostics,
                );
            }
        }
        ExpressionNode::Binary(binary) => {
            // Every binary-operand TYPE check (operator applied to operands it is
            // not defined for) lives behind one dispatcher.
            crate::expression_types::validate_binary_operand_types(
                program,
                machine,
                Some(state),
                binary.operator,
                binary.left,
                binary.right,
                diagnostics,
            );
            scan_expression_calls(
                program,
                machine,
                state,
                machine_symbols,
                symbols,
                writable_roots,
                value_env,
                binary.left,
                diagnostics,
            );
            scan_expression_calls(
                program,
                machine,
                state,
                machine_symbols,
                symbols,
                writable_roots,
                value_env,
                binary.right,
                diagnostics,
            );
        }
        ExpressionNode::Cast(cast) => {
            // All cast TARGET/SOURCE type validation lives behind one dispatcher.
            crate::expression_types::validate_cast_types(
                program,
                machine,
                Some(state),
                cast,
                diagnostics,
            );
            scan_expression_calls(
                program,
                machine,
                state,
                machine_symbols,
                symbols,
                writable_roots,
                value_env,
                cast.value,
                diagnostics,
            );
        }
        ExpressionNode::Indexed(indexed) => {
            // Reading a nested array element with a RUNTIME COLUMN index (`grid[..][j]`)
            // silently reads 0 (the backend cannot lower it yet); the sibling WRITE is
            // already fenced, this fences the READ to match.
            report_nested_runtime_indexed_read(program, machine, state, expression, diagnostics);
            scan_expression_calls(
                program,
                machine,
                state,
                machine_symbols,
                symbols,
                writable_roots,
                value_env,
                indexed.collection,
                diagnostics,
            );
            scan_expression_calls(
                program,
                machine,
                state,
                machine_symbols,
                symbols,
                writable_roots,
                value_env,
                indexed.index,
                diagnostics,
            );
        }
        ExpressionNode::Member(member) => {
            scan_expression_calls(
                program,
                machine,
                state,
                machine_symbols,
                symbols,
                writable_roots,
                value_env,
                member.receiver,
                diagnostics,
            );
        }
        ExpressionNode::Mutable(inner) => {
            scan_expression_calls(
                program,
                machine,
                state,
                machine_symbols,
                symbols,
                writable_roots,
                value_env,
                *inner,
                diagnostics,
            );
        }
        ExpressionNode::Unary(unary) => {
            // Logical `!` is bool-only (bitwise-not is `~`); reject `!<non-bool>`.
            crate::expression_types::report_non_bool_logical_not(
                program,
                machine,
                Some(state),
                unary.operand,
                diagnostics,
            );
            scan_expression_calls(
                program,
                machine,
                state,
                machine_symbols,
                symbols,
                writable_roots,
                value_env,
                unary.operand,
                diagnostics,
            );
        }
        ExpressionNode::ArrayLiteral(elements) => {
            let elements = *elements;
            for element in program.expression_table.expression_handles(elements) {
                scan_expression_calls(
                    program,
                    machine,
                    state,
                    machine_symbols,
                    symbols,
                    writable_roots,
                    value_env,
                    *element,
                    diagnostics,
                );
            }
        }
        ExpressionNode::StructLiteral(literal) => {
            let fields = literal.fields;
            for field in program.expression_table.struct_fields(fields) {
                scan_expression_calls(
                    program,
                    machine,
                    state,
                    machine_symbols,
                    symbols,
                    writable_roots,
                    value_env,
                    field.value,
                    diagnostics,
                );
            }
        }
        ExpressionNode::Range(range) => {
            scan_expression_calls(
                program,
                machine,
                state,
                machine_symbols,
                symbols,
                writable_roots,
                value_env,
                range.start,
                diagnostics,
            );
            scan_expression_calls(
                program,
                machine,
                state,
                machine_symbols,
                symbols,
                writable_roots,
                value_env,
                range.end,
                diagnostics,
            );
        }
        ExpressionNode::Boolean(_)
        | ExpressionNode::Float(_)
        | ExpressionNode::Integer(_)
        | ExpressionNode::Name(_)
        | ExpressionNode::String(_) => {}
    }
}

/// Enforce machine-call type-parameter bounds for a single VALUE-position
/// `ExpressionNode::Call`.  The receiver name path is extracted from the
/// receiver expression (must be a `Name` node with identifier segments).
/// Other receiver shapes (member chains, indexed, etc.) are beyond this
/// scope and stand down silently, consistent with the statement-path's
/// handling of unrecognised receivers.
/// A VALUE-position call from emitted concrete code may not retain a GENERIC
/// callee: its result slot has no concrete layout. Uninstantiated generic
/// templates are different—they are checked modularly but never emitted, and
/// their symbolic calls are resolved by fixed-point specialization once a
/// concrete outer call selects them.
fn fence_generic_value_callee(
    program: &TypedTrees,
    caller_machine: &Machine,
    callee_machine: &Machine,
    target: &str,
    diagnostics: &mut Vec<Diagnostic>,
) {
    // An uninstantiated generic template is checked modularly but is never
    // emitted. Its symbolic value calls become concrete when an outer call
    // specializes the template. Keep the fence for concrete callers whose
    // selected callee somehow remains generic: that is still an incomplete
    // lowering and must fail loudly.
    if !program.machine_type_parameters(caller_machine).is_empty()
        || program.machine_type_parameters(callee_machine).is_empty()
    {
        return;
    }
    diagnostics.push(Diagnostic::error(format!(
        "a value call to the generic machine `{target}` is not supported natively yet (the \
         monomorphized result is never materialized): wrap it in a concrete machine, or use a \
         statement call",
    )));
}

/// The receiver's spelled member chain, root -> leaf (`["self", "p",
/// "second"]` for `self.p.second.stored()`). `None` for non-place receivers
/// (calls, literals). Mirrors the state-call plan's `append_receiver_path`
/// walk at the typed layer.
fn receiver_member_chain(
    program: &TypedTrees,
    receiver: omega_typed_trees::expression::ExpressionHandle,
) -> Option<Vec<String>> {
    if !receiver.is_valid() {
        return None;
    }
    match program.expression_table.expression(receiver) {
        ExpressionNode::Name(path) => {
            let members = program.expression_table.name_path_members(path.members);
            (!members.is_empty()).then(|| {
                members
                    .iter()
                    .map(|member| member.as_str().to_string())
                    .collect()
            })
        }
        ExpressionNode::Member(member) => {
            let mut chain = receiver_member_chain(program, member.receiver)?;
            chain.push(member.member.as_str().to_string());
            Some(chain)
        }
        ExpressionNode::Mutable(inner) => receiver_member_chain(program, *inner),
        _ => None,
    }
}

#[allow(clippy::too_many_arguments)]
fn validate_expression_call_bounds(
    program: &TypedTrees,
    current_machine: &Machine,
    current_state: &State,
    machine_symbols: &MachineSymbols<'_>,
    symbols: &TopLevelSymbols<'_>,
    writable_roots: &WritableRoots<'_, '_>,
    value_env: &ValueEnv,
    call: &TableCallExpression,
    diagnostics: &mut Vec<Diagnostic>,
) {
    if (call.target.as_str() == "asm#pushfq"
        || omega_core::inline_assembly::AsmControlRegister::from_read_intrinsic_name(
            call.target.as_str(),
        )
        .is_some())
        && !call.receiver.is_valid()
    {
        let arguments = program.expression_table.expression_handles(call.arguments);
        if !arguments.is_empty() {
            diagnostics.push(Diagnostic::error(format!(
                "asm intrinsic `{}` takes 0 operands, found {}",
                call.target,
                arguments.len()
            )));
        }
        return;
    }

    if matches!(call.target.as_str(), "asm#port_in" | "asm#rdmsr") && !call.receiver.is_valid() {
        let (intrinsic, instruction, operand_index) = if call.target.as_str() == "asm#port_in" {
            ("asm#port_in", "in", 1)
        } else {
            ("asm#rdmsr", "rdmsr", 1)
        };
        let arguments = program.expression_table.expression_handles(call.arguments);
        if arguments.len() != 1 {
            diagnostics.push(Diagnostic::error(format!(
                "asm intrinsic `{intrinsic}` takes 1 operand, found {}",
                arguments.len()
            )));
            return;
        }
        let contract = user_asm_contract(instruction);
        validate_asm_operand_constraint(
            program,
            current_machine,
            Some(current_state),
            instruction,
            arguments[0],
            contract.operands[operand_index],
            diagnostics,
        );
        return;
    }

    // Resolve the receiver: is this a self-call, and if not, the name of the
    // receiver object (field/local). A self-call has no receiver (`call.receiver`
    // invalid) or an explicit `self`. A `Name`-path receiver names the object via
    // its last member; a `Member` receiver (`self.host.method(..)`, where
    // `self.host` is a member access, NOT a name path) names it via that member —
    // WITHOUT this, a member receiver fell through as an empty path and the call
    // was misrouted into the self-call branch (resolving to a same-named sibling
    // state instead of the field's boundary/machine type).
    let (call_is_self, external_receiver_name): (bool, Option<&str>) = if !call.receiver.is_valid()
    {
        (true, None)
    } else {
        match program.expression_table.expression(call.receiver) {
            ExpressionNode::Name(path) => {
                let members = program.expression_table.name_path_members(path.members);
                if members.is_empty() || matches!(members, [r] if r.as_str() == "self") {
                    (true, None)
                } else {
                    (false, members.last().map(Identifier::as_str))
                }
            }
            ExpressionNode::Member(member) => (false, Some(member.member.as_str())),
            _ => (true, None),
        }
    };

    let arguments = program.expression_table.expression_handles(call.arguments);

    // Self-call or `self`-prefixed call: the callee is a state of the
    // current machine, an attached-data sibling machine, or a free machine.
    // Mirrors the same three-way fallback in `validate_call_node`.
    if call_is_self {
        if let Some(signature) =
            program.machine_parameter_signature_in(current_machine, call.target_symbol)
        {
            if !signature.return_type.is_valid() {
                diagnostics.push(Diagnostic::error(format!(
                    "machine `{}` state `{}`: machine parameter `{}` does not return a value but is used in a VALUE position",
                    current_machine.name,
                    current_state.name,
                    signature.name,
                )));
            }
            validate_call_arguments_handles(
                program,
                current_machine,
                Some(current_state),
                value_env,
                arguments,
                signature.name.as_str(),
                program.state_signature_parameters(signature),
                writable_roots,
                diagnostics,
            );
            return;
        }

        if let Some((callee_machine, callee_state)) =
            machine_state_by_symbol(program, call.target_symbol)
            && callee_machine.symbol != current_machine.symbol
        {
            report_void_value_callee(
                program,
                callee_machine,
                current_machine,
                current_state,
                callee_state,
                call.target.as_str(),
                diagnostics,
            );
            fence_generic_value_callee(
                program,
                current_machine,
                callee_machine,
                call.target.as_str(),
                diagnostics,
            );
            validate_machine_call_type_parameter_bounds(
                program,
                symbols,
                callee_machine,
                callee_state,
                call.target.as_str(),
                arguments,
                current_machine,
                Some(current_state),
                diagnostics,
            );
            validate_value_call_argument_classes(
                program,
                current_machine,
                current_state,
                value_env,
                arguments,
                callee_state,
                diagnostics,
            );
            return;
        }

        if let Some(callee_state) = machine_symbols.state(call.target.as_str()) {
            report_void_value_callee(
                program,
                current_machine,
                current_machine,
                current_state,
                callee_state,
                call.target.as_str(),
                diagnostics,
            );
            validate_machine_call_type_parameter_bounds(
                program,
                symbols,
                current_machine,
                callee_state,
                callee_state.name.as_str(),
                arguments,
                current_machine,
                Some(current_state),
                diagnostics,
            );
            validate_value_call_argument_classes(
                program,
                current_machine,
                current_state,
                value_env,
                arguments,
                callee_state,
                diagnostics,
            );
            return;
        }

        // A self-call can also target a SIBLING machine that shares the same
        // attached data (`machine Main::pick<T [copy]>` called from
        // `machine Main::main`). The statement-position path uses
        // `symbols.attached_machine_state(program, attached_data, call.target)`.
        let attached_state = current_machine
            .attached_data
            .as_ref()
            .and_then(|attached_data| {
                symbols.attached_machine_state(
                    program,
                    attached_data.as_str(),
                    call.target.as_str(),
                )
            });

        if let Some((callee_machine, callee_state)) = attached_state {
            report_void_value_callee(
                program,
                callee_machine,
                current_machine,
                current_state,
                callee_state,
                call.target.as_str(),
                diagnostics,
            );
            fence_generic_value_callee(
                program,
                current_machine,
                callee_machine,
                call.target.as_str(),
                diagnostics,
            );
            validate_machine_call_type_parameter_bounds(
                program,
                symbols,
                callee_machine,
                callee_state,
                call.target.as_str(),
                arguments,
                current_machine,
                Some(current_state),
                diagnostics,
            );
            validate_value_call_argument_classes(
                program,
                current_machine,
                current_state,
                value_env,
                arguments,
                callee_state,
                diagnostics,
            );
            return;
        }

        // Free machine call (`compute(item)` -- no `self.`, no receiver).
        if let Some((callee_machine, callee_state)) =
            free_machine_entry_state(program, symbols, call.target.as_str())
        {
            report_void_value_callee(
                program,
                callee_machine,
                current_machine,
                current_state,
                callee_state,
                call.target.as_str(),
                diagnostics,
            );
            fence_generic_value_callee(
                program,
                current_machine,
                callee_machine,
                call.target.as_str(),
                diagnostics,
            );
            validate_machine_call_type_parameter_bounds(
                program,
                symbols,
                callee_machine,
                callee_state,
                call.target.as_str(),
                arguments,
                current_machine,
                Some(current_state),
                diagnostics,
            );
            validate_value_call_argument_classes(
                program,
                current_machine,
                current_state,
                value_env,
                arguments,
                callee_state,
                diagnostics,
            );
            return;
        }
        report_unresolved_value_call(
            program,
            current_machine,
            current_state,
            symbols,
            None,
            None,
            call,
            diagnostics,
        );
        return;
    }

    let receiver_name = external_receiver_name.unwrap_or_default();
    // Direct field/local receivers resolve by bare name. A NESTED self-rooted
    // VALUE-position member chain (`self.p.a.get()`) resolves by walking the
    // chain's declared field types to the leaf type (receiver-place staircase,
    // rung 3). The full arc is now sound: symbol resolution stamps the nested
    // symbols (rung 2b) so the state-call plan records the call; the backend
    // storage walk descends plain-DATA intermediates (rung 2a/D1) so the
    // callee's `self` base resolves; and the emission-planning
    // contained-receiver blocker rejects an ambiguous nested receiver (a
    // same-type sibling that the by-type walk would misresolve) loudly instead
    // of binding 0. STATEMENT-position nested calls are validated separately
    // (`validate_call_node`) and remain unsupported -- see TASKS D2.
    let receiver_type = machine_symbols.contained_type(receiver_name).or_else(|| {
        let chain = receiver_member_chain(program, call.receiver)?;
        if chain.len() < 3 || chain.first().map(String::as_str) != Some("self") {
            return None;
        }
        crate::places::nested_receiver_type_name(
            program,
            current_machine,
            Some(current_state),
            &chain,
        )
    });

    // External machine receiver.
    if let Some(callee_machine) = receiver_type
        .and_then(|type_name| symbols.machine(type_name))
        .or_else(|| symbols.machine(receiver_name))
    {
        if let Some(callee_state) = program
            .machine_states(callee_machine)
            .iter()
            .find(|s| s.name == call.target)
        {
            report_void_value_callee(
                program,
                callee_machine,
                current_machine,
                current_state,
                callee_state,
                call.target.as_str(),
                diagnostics,
            );
            fence_generic_value_callee(
                program,
                current_machine,
                callee_machine,
                call.target.as_str(),
                diagnostics,
            );
            validate_machine_call_type_parameter_bounds(
                program,
                symbols,
                callee_machine,
                callee_state,
                callee_state.name.as_str(),
                arguments,
                current_machine,
                Some(current_state),
                diagnostics,
            );
            validate_value_call_argument_classes(
                program,
                current_machine,
                current_state,
                value_env,
                arguments,
                callee_state,
                diagnostics,
            );
            return;
        }
        report_unresolved_value_call(
            program,
            current_machine,
            current_state,
            symbols,
            Some(receiver_name),
            receiver_type,
            call,
            diagnostics,
        );
        return;
    }

    // Attached-data machine receiver.
    if let Some((callee_machine, callee_state)) = receiver_type.and_then(|type_name| {
        symbols.attached_machine_state(program, type_name, call.target.as_str())
    }) {
        report_void_value_callee(
            program,
            callee_machine,
            current_machine,
            current_state,
            callee_state,
            call.target.as_str(),
            diagnostics,
        );
        fence_generic_value_callee(
            program,
            current_machine,
            callee_machine,
            call.target.as_str(),
            diagnostics,
        );
        validate_machine_call_type_parameter_bounds(
            program,
            symbols,
            callee_machine,
            callee_state,
            callee_state.name.as_str(),
            arguments,
            current_machine,
            Some(current_state),
            diagnostics,
        );
        validate_value_call_argument_classes(
            program,
            current_machine,
            current_state,
            value_env,
            arguments,
            callee_state,
            diagnostics,
        );
        let _ = writable_roots;
        return;
    }
    report_unresolved_value_call(
        program,
        current_machine,
        current_state,
        symbols,
        Some(receiver_name),
        receiver_type,
        call,
        diagnostics,
    );

    let _ = writable_roots;
}

/// A value call on a LET-BOUND LOCAL receiver (`let p: Pair = ..; p.total()`)
/// reads ZII natively: receiver resolution reaches machine FIELDS and state
/// PARAMETERS only, so the callee's `self.field` reads bind to nothing and
/// the result silently zeroes when the caller is itself an inlined value
/// callee (Main-state spellings hit the emission backstop instead). Fence it
/// loudly until local receiver resolution lands (TASKS.md "local-receiver
/// value calls"). Field receivers, `self`, and state-parameter receivers are
/// the supported (canaried) forms.
pub(crate) fn report_local_receiver_value_call(
    program: &TypedTrees,
    machine: &Machine,
    state_name: &str,
    value: ExpressionHandle,
    diagnostics: &mut Vec<Diagnostic>,
) {
    if !value.is_valid() {
        return;
    }
    let ExpressionNode::Call(call) = program.expression_table.expression(value) else {
        return;
    };
    if !call.receiver.is_valid() {
        return;
    }
    // BUILTIN view/operand methods (`view.bytes()`, `arr.as_slice()`, min/max
    // shapes) compose on locals through the operand machinery -- the same
    // exemption list as the nested-argument fence above.
    if matches!(
        call.target.as_str(),
        "min" | "max" | "sqrt" | "as_slice" | "as_mut_slice" | "as_view" | "bytes"
    ) {
        return;
    }
    // Only a BARE single-member NAME receiver can be a local; `self.x` and
    // deeper member paths route through the supported field machinery.
    let ExpressionNode::Name(path) = program.expression_table.expression(call.receiver) else {
        return;
    };
    let members = program.expression_table.name_path_members(path.members);
    let [receiver_name] = members else {
        return;
    };
    let receiver = receiver_name.as_str();
    if receiver == "self" {
        return;
    }
    // A state PARAMETER (whole-machine scope) is a supported receiver.
    let is_parameter = program.machine_states(machine).iter().any(|state| {
        program
            .state_parameters(state)
            .iter()
            .any(|parameter| parameter.name.as_str() == receiver)
    });
    if is_parameter {
        return;
    }
    // A machine FIELD read as a bare name cannot happen (fields spell
    // `self.x`), but keep the check total: owned-data names pass through.
    let is_field = program
        .machine_owned_data(machine)
        .iter()
        .any(|owned| owned.name.as_str() == receiver);
    if is_field {
        return;
    }
    // A LOCAL-DATA binding anywhere in the machine (bindings are
    // whole-machine scope).
    let is_local = program.machine_states(machine).iter().any(|state| {
        program
            .statement_table
            .statements(state.statement_nodes)
            .iter()
            .any(|statement| {
                matches!(
                    statement,
                    StatementNode::LocalData(local) if local.name.as_str() == receiver
                )
            })
    });
    if !is_local {
        return;
    }
    diagnostics.push(Diagnostic::error(format!(
        "machine `{}` state `{state_name}`: value call `{}.{}(..)` uses a LET-bound          local as its receiver, which reads ZII (zeroes) natively -- receiver          resolution reaches machine fields and state parameters only. Store the          value in a data field (`self.{} = {}; self.{}.{}(..)`) or pass it as a          state parameter.",
        machine.name,
        receiver,
        call.target.as_str(),
        receiver,
        receiver,
        receiver,
        call.target.as_str(),
    )));
}

/// A LET/ASSIGNMENT-bound value call whose ARGUMENT nests another machine call
/// (`let out = self.double(self.inc(3))`) reads a garbage inner result: the
/// inner callee's frame locals cannot materialize inside the outer call's
/// argument context in the VALUE sink (some consumer shapes fence loudly with
/// "needs stack/local storage lowering", but the guard-consumer shape slipped
/// through and natively bound 0). STATEMENT-call arguments take a different
/// materialization path and legitimately nest (the dungeon's
/// `self.append_exit(.., self.direction_command(self.opposite(d)), ..)`), so
/// this check runs ONLY on the value expression of a local/assignment.
pub(crate) fn report_nested_call_in_bound_value_call(
    program: &TypedTrees,
    machine: &Machine,
    state_name: &str,
    value: ExpressionHandle,
    diagnostics: &mut Vec<Diagnostic>,
) {
    if !value.is_valid() {
        return;
    }
    let ExpressionNode::Call(call) = program.expression_table.expression(value) else {
        return;
    };
    // A BUILTIN outer call (`let v = max(self.range(..), floor)`) composes:
    // builtin arguments materialize as operands through the call-result-local
    // machinery (canaried). Only a MACHINE outer call's argument context is
    // broken.
    if matches!(
        call.target.as_str(),
        "min" | "max" | "sqrt" | "as_slice" | "as_mut_slice" | "as_view" | "bytes"
    ) {
        return;
    }
    for argument in program.expression_table.expression_handles(call.arguments) {
        if let Some(inner) = first_non_builtin_call(program, *argument) {
            diagnostics.push(Diagnostic::error(format!(
                "machine `{}` state `{state_name}`: a value-call argument cannot itself \
                 be a machine call yet (`{inner}(..)` nested in `{}(..)` would read a \
                 garbage result) -- bind the inner call to a local first, then pass \
                 the local.",
                machine.name,
                call.target.as_str(),
            )));
            return;
        }
    }
}

/// The first NON-BUILTIN machine call nested anywhere inside `expression`
/// (its target name, for the diagnostic), or None. Reserved value builtins
/// (`min`/`max`/`sqrt`) and the view builtins (`as_slice`/`as_mut_slice`/
/// `as_view`/`bytes`) compose in arguments and are exempt.
fn first_non_builtin_call(
    program: &TypedTrees,
    expression: ExpressionHandle,
) -> Option<Identifier> {
    if !expression.is_valid() {
        return None;
    }
    match program.expression_table.expression(expression) {
        ExpressionNode::Atomic(atomic) => first_non_builtin_call(program, atomic.value),
        ExpressionNode::Call(call) => {
            if !matches!(
                call.target.as_str(),
                "min" | "max" | "sqrt" | "as_slice" | "as_mut_slice" | "as_view" | "bytes"
            ) {
                return Some(call.target.clone());
            }
            program
                .expression_table
                .expression_handles(call.arguments)
                .iter()
                .find_map(|argument| first_non_builtin_call(program, *argument))
        }
        ExpressionNode::Binary(binary) => first_non_builtin_call(program, binary.left)
            .or_else(|| first_non_builtin_call(program, binary.right)),
        ExpressionNode::Unary(unary) => first_non_builtin_call(program, unary.operand),
        ExpressionNode::Cast(cast) => first_non_builtin_call(program, cast.value),
        ExpressionNode::Mutable(inner) => first_non_builtin_call(program, *inner),
        ExpressionNode::Indexed(indexed) => first_non_builtin_call(program, indexed.collection)
            .or_else(|| first_non_builtin_call(program, indexed.index)),
        ExpressionNode::Member(member) => first_non_builtin_call(program, member.receiver),
        _ => None,
    }
}

/// A VOID callee in VALUE position used to compile and silently bind 0 (ZII)
/// -- and native/interp DIVERGED on the bound value. "Void" means: no declared
/// return type on the resolved state (the parser now lands `-> T` written
/// after the machine clauses too; it used to be silently dropped) AND no state
/// of the callee machine produces a value through a transition VALUE arm --
/// undeclared-return value machines (`transition r > 0 { true -> self.f(r-1)
/// false -> 0 }`, the termination-canary surface) stay callable.
fn report_void_value_callee(
    program: &TypedTrees,
    callee_machine: &Machine,
    current_machine: &Machine,
    current_state: &State,
    callee_state: &State,
    callee_display: &str,
    diagnostics: &mut Vec<Diagnostic>,
) {
    if callee_state.return_type.is_valid() {
        return;
    }
    let produces_value = program.machine_states(callee_machine).iter().any(|state| {
        state.return_type.is_valid()
            || program
                .statement_table
                .statements(state.statement_nodes)
                .iter()
                .any(|statement| {
                    let StatementNode::Transition(transition) = statement else {
                        return false;
                    };
                    [transition.target, transition.continuation]
                        .iter()
                        .any(|handle| {
                            handle.is_valid()
                                && matches!(
                                    program.statement_table.transition_target(*handle),
                                    TransitionTargetNode::Value(_)
                                )
                        })
                })
    });
    if produces_value {
        return;
    }
    diagnostics.push(Diagnostic::error(format!(
        "machine `{}` state `{}`: `{callee_display}(..)` does not return a value but is \
         used in a VALUE position -- it would silently bind 0 (ZII) at runtime. Declare \
         a return type on the callee (`-> T`) or call it as a statement.",
        current_machine.name,
        current_state.name.as_str(),
    )));
}

/// The declared TYPE NAME of a receiver that is a state param, state local,
/// whole-machine param, or machine-owned field -- walked through reference/
/// constraint shells to the Named/Generic/DynamicTrait head. `None` for
/// primitives, arrays, slices, and unknown names.
fn receiver_declared_type_name<'program>(
    program: &'program TypedTrees,
    machine: &Machine,
    state: &State,
    receiver: &str,
) -> Option<&'program str> {
    let handle = program
        .state_parameters(state)
        .iter()
        .find(|parameter| parameter.name.as_str() == receiver)
        .map(|parameter| parameter.type_reference)
        .or_else(|| {
            program
                .statement_table
                .statements(state.statement_nodes)
                .iter()
                .find_map(|statement| {
                    let StatementNode::LocalData(local) = statement else {
                        return None;
                    };
                    (local.name.as_str() == receiver).then_some(local.type_reference)
                })
        })
        .or_else(|| {
            program
                .machine_owned_data(machine)
                .iter()
                .find(|owned| owned.name.as_str() == receiver)
                .map(|owned| owned.type_reference)
        })
        .or_else(|| {
            // State bindings are whole-machine scope: a param declared on any
            // state of this machine is readable everywhere in it.
            program.machine_states(machine).iter().find_map(|other| {
                program
                    .state_parameters(other)
                    .iter()
                    .find(|parameter| parameter.name.as_str() == receiver)
                    .map(|parameter| parameter.type_reference)
            })
        })?;
    named_type_reference_name(program, handle)
}

fn named_type_reference_name<'program>(
    program: &'program TypedTrees,
    handle: TypeReferenceHandle,
) -> Option<&'program str> {
    match program.type_reference_table.type_reference(handle) {
        TypeReferenceNode::Reference { referee, .. } => {
            named_type_reference_name(program, *referee)
        }
        TypeReferenceNode::Constrained { base_type, .. } => {
            named_type_reference_name(program, *base_type)
        }
        TypeReferenceNode::Named { name, .. }
        | TypeReferenceNode::Generic {
            base_name: name, ..
        }
        | TypeReferenceNode::DynamicTrait { name, .. } => Some(name.as_str()),
        _ => None,
    }
}

/// True when `type_name` resolves the value-call target through any of the
/// channels the LOWERING understands: a boundary-trait machine signature, a
/// machine's local state, or a machine attached to that data type.
fn type_name_resolves_value_call(
    program: &TypedTrees,
    symbols: &TopLevelSymbols<'_>,
    type_name: &str,
    target: &str,
) -> bool {
    if let Some(trait_definition) = symbols.trait_definition(type_name)
        && program
            .trait_machine_signatures(trait_definition)
            .iter()
            .any(|signature| signature.name.as_str() == target)
    {
        return true;
    }
    if let Some(machine) = symbols.machine(type_name)
        && program
            .machine_states(machine)
            .iter()
            .any(|state| state.name.as_str() == target)
    {
        return true;
    }
    symbols
        .attached_machine_state(program, type_name, target)
        .is_some()
}

/// Decision layer for the value-call fall-through: everything the partial
/// bounds resolver above recognizes has already returned; anything the
/// LOWERING can still resolve is checked here (builtins, platform/trait
/// receivers, declared receiver types, type-name receivers). A target that
/// resolves through NONE of these names nothing anywhere -- it would silently
/// bind a ZII 0 at runtime, so it is a compile error.
#[allow(clippy::too_many_arguments)]
fn report_unresolved_value_call(
    program: &TypedTrees,
    current_machine: &Machine,
    current_state: &State,
    symbols: &TopLevelSymbols<'_>,
    receiver_name: Option<&str>,
    receiver_type: Option<&str>,
    call: &TableCallExpression,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let target = call.target.as_str();
    let Some(receiver) = receiver_name else {
        // Receiverless: the three machine channels missed; only the reserved
        // value builtins remain. `asm#port_in` is the value-position asm
        // intrinsic (`asm { in dest, port }` desugars to `dest =
        // asm#port_in(port)`); the name is unnameable from source.
        if matches!(
            target,
            "min" | "max" | "sqrt" | "asm#port_in" | "asm#pushfq" | "asm#rdmsr"
        ) || omega_core::inline_assembly::AsmControlRegister::from_read_intrinsic_name(target)
            .is_some()
        {
            return;
        }
        diagnostics.push(Diagnostic::error(format!(
            "machine `{}` state `{}`: value call `{target}(..)` does not resolve to a \
             state of this machine, an attached sibling machine, or a free machine -- \
             it would silently bind 0 (ZII) at runtime. Check the name.",
            current_machine.name,
            current_state.name.as_str(),
        )));
        return;
    };

    // Collection/text view builtins: `arr.as_slice()` / `.as_mut_slice()`,
    // the text view `text.as_view()` (the borrow layer's own builtin list,
    // borrow/loans.rs), and the view byte accessor `view.bytes()`.
    if matches!(target, "as_slice" | "as_mut_slice" | "as_view" | "bytes") {
        return;
    }
    // Wire-schema synthesized codecs (`Schema::encode(..)` / `::decode(..)`)
    // are not user machines; a data-definition receiver resolves them.
    if matches!(target, "encode" | "decode")
        && program
            .data_definitions()
            .iter()
            .any(|definition| definition.name.as_str() == receiver)
    {
        return;
    }

    let declared_type =
        receiver_declared_type_name(program, current_machine, current_state, receiver);
    let resolves = receiver_type
        .is_some_and(|type_name| type_name_resolves_value_call(program, symbols, type_name, target))
        || declared_type
            .is_some_and(|type_name| type_name_resolves_value_call(program, symbols, type_name, target))
        // The receiver may BE a type name (`Real.from(..)`, `Worker.run(..)`).
        || type_name_resolves_value_call(program, symbols, receiver, target);
    if resolves {
        return;
    }

    diagnostics.push(Diagnostic::error(format!(
        "machine `{}` state `{}`: value call `{receiver}.{target}(..)` does not resolve \
         to any machine state, attached machine, platform state, or boundary-trait \
         method -- it would silently bind 0 (ZII) at runtime. Check the receiver and \
         method names.",
        current_machine.name,
        current_state.name.as_str(),
    )));
}
