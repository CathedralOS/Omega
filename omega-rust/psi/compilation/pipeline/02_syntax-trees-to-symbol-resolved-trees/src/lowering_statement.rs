/*
    Statements, including the operand-hoisting rewrites.

    Most statements lower one-to-one. Runtime-indexed operand reads and
    unconditional value calls are hoisted into synthetic `let` bindings ahead
    of the rewritten statement, so later passes see only root-level reads and
    plain locals. A guarded arm's calls are never moved: they keep their
    authored evaluation point inside the arm, and the checking stage evaluates
    them there in operand order.

    This module owns the statement entry point. `statement_nodes` lowers
    each statement node, transition guard and target, `indexed_read_hoisting`
    hoists runtime-indexed operand reads (and pure builtin guard calls),
    `value_call_hoisting` hoists unconditional scalar value calls and
    `match_subject_hoisting` hoists membership and comparison match
    subjects. `place_operand_capture` captures evaluated address operands
    for unconditional reads, writes and loans without extracting their pointee.
    All these owners mint their temps through `hoist_temps`.
*/

mod assembly {
    /* Resolve one assembly scope without flattening its instruction paths. */

    use super::statement_nodes::{lower_statement_expression, lower_statement_node};
    use crate::resolution::lowerer::Lowerer;
    use arena::{Handle, HandleSpan};
    use diagnostics::Diagnostic;
    use symbol_resolved_trees::statement::{AssemblyBlock, AssemblyControlFlow, AssemblyExit};
    use syntax_trees::{SyntaxTrees, statement as syntax};

    pub(super) fn lower(
        lowerer: &mut Lowerer,
        syntax_trees: &SyntaxTrees,
        handle: Handle<syntax::AssemblyControlFlow>,
        statement_index: usize,
    ) -> Result<Handle<AssemblyControlFlow>, Diagnostic> {
        let graph = syntax_trees.statements.assembly(handle);
        let source_blocks = syntax_trees.statements.assembly_blocks(graph.blocks);
        let mut blocks = Vec::with_capacity(source_blocks.len());
        for block in source_blocks {
            let mut instructions = Vec::new();
            for handle in syntax_trees
                .statements
                .assembly_instructions(block.instructions)
            {
                instructions.extend(lower_statement_node(
                    lowerer,
                    syntax_trees,
                    syntax_trees.statements.statement(*handle),
                    statement_index + instructions.len(),
                    false,
                )?);
            }
            let requires = syntax_trees
                .statements
                .assembly_expressions(block.requires)
                .iter()
                .map(|expression| lower_statement_expression(lowerer, syntax_trees, *expression))
                .collect::<Result<Vec<_>, _>>()?;
            let ensures = syntax_trees
                .statements
                .assembly_expressions(block.ensures)
                .iter()
                .map(|expression| lower_statement_expression(lowerer, syntax_trees, *expression))
                .collect::<Result<Vec<_>, _>>()?;
            let declarations = &mut lowerer.symbol_resolved_trees.tables.declarations;
            blocks.push(AssemblyBlock {
                label: crate::lowering::name::lower_name(syntax_trees, &block.label),
                requires: declarations.assembly_expressions.insert_many(requires),
                ensures: declarations.assembly_expressions.insert_many(ensures),
                instructions: declarations.state_statements.insert_many(instructions),
                statement_nodes: HandleSpan::empty(),
                exit: match block.exit {
                    syntax::AssemblyExit::Fallthrough => AssemblyExit::Fallthrough,
                    syntax::AssemblyExit::EnclosingTransition => AssemblyExit::EnclosingTransition,
                    syntax::AssemblyExit::Jump(_) => AssemblyExit::Jump(Handle::invalid()),
                    syntax::AssemblyExit::ZeroFlag(_, when_set) => {
                        AssemblyExit::ZeroFlag(Handle::invalid(), when_set)
                    }
                },
            });
        }
        let primitives = syntax_trees
            .statements
            .assembly_expressions(graph.primitives)
            .iter()
            .map(|expression| lower_statement_expression(lowerer, syntax_trees, *expression))
            .collect::<Result<Vec<_>, _>>()?;
        let mut scratch = [symbol_resolved_trees::expression::ExpressionHandle::invalid();
            language_core::inline_assembly::ASM_SCRATCH_SLOT_COUNT];
        for (target, source) in scratch.iter_mut().zip(graph.scratch) {
            if source.is_valid() {
                *target = lower_statement_expression(lowerer, syntax_trees, source)?;
            }
        }
        let declarations = &mut lowerer.symbol_resolved_trees.tables.declarations;
        let primitives = declarations.assembly_expressions.insert_many(primitives);
        let blocks = declarations.assembly_blocks.insert_many(blocks);
        let handles: Vec<_> = blocks.handles().collect();
        for (handle, source_block) in handles.iter().copied().zip(source_blocks) {
            if let syntax::AssemblyExit::Jump(label) | syntax::AssemblyExit::ZeroFlag(label, _) =
                &source_block.exit
            {
                let Some(index) = source_blocks.iter().position(|block| {
                    block.label.as_str(syntax_trees) == label.as_str(syntax_trees)
                }) else {
                    return Err(Diagnostic::error(format!(
                        "assembly jump has no local label `{}`",
                        label.as_str(syntax_trees)
                    )));
                };
                declarations.assembly_blocks.get_mut(handle).exit =
                    if let syntax::AssemblyExit::ZeroFlag(_, when_set) = source_block.exit {
                        AssemblyExit::ZeroFlag(handles[index], when_set)
                    } else {
                        AssemblyExit::Jump(handles[index])
                    };
            }
        }
        Ok(declarations.assembly_graphs.insert(AssemblyControlFlow {
            target_requirement: graph.target_requirement,
            blocks,
            scratch,
            primitives,
        }))
    }
}
pub(in crate::lowering) mod hoist_temps {
    /*
        The one constructor for synthetic `let __hoist_N` temps.

        Every hoist pushes `let __hoist_N = <initial>;` ahead of the rewritten
        statement and reads the temp back through a fresh one-member `Name`.
        `TypeReference::Unit` is the inference sentinel: the symbol-resolved ->
        typed lowering types such a temp from its initializer
        (`infer_hoist_temp_type`). Any other declared type is the temp's
        annotation.
    */

    use super::place_operand_capture::{SelectorCapture, capture_reference_initializer_selectors};
    use crate::lowering::expression::expression_table;
    use crate::resolution::lowerer::Lowerer;
    use arena::HandleSpan;
    use symbol_resolved_trees::expression::{ExpressionHandle, ExpressionNode, TableNamePath};
    use symbol_resolved_trees::name::DiagnosticName;
    use symbol_resolved_trees::statement::{LocalData, LocalDataStorage, Statement};
    use symbol_resolved_trees::types::TypeReference;
    use symbols::SymbolHandle;

    /*
        Emits `let __hoist_N = <initial>;` with an inferred type and returns a
        `Name` referencing it.
    */
    pub(crate) fn hoist_into_temp(
        lowerer: &mut Lowerer,
        initial: ExpressionHandle,
        hoisted: &mut Vec<Statement>,
    ) -> ExpressionHandle {
        mint_hoist_temp(
            lowerer,
            hoisted,
            TypeReference::Unit,
            None,
            false,
            |_, _| initial,
        )
    }

    /*
        Capture an operand whose consumer determines whether a primitive reference
        supplies its value. Resolution cannot select the operator yet; retain this
        origin on the local rather than guessing from its initializer's spelling.
    */
    pub(crate) fn hoist_operand_into_temp(
        lowerer: &mut Lowerer,
        initial: ExpressionHandle,
        hoisted: &mut Vec<Statement>,
    ) -> ExpressionHandle {
        mint_hoist_temp(lowerer, hoisted, TypeReference::Unit, None, true, |_, _| {
            initial
        })
    }

    /*
        Returns a fresh `Name` referencing a hoist temp, first emitting
        `let __hoist_N: <declared> = <initial>;` unless `shared_subject` already
        has a temp.

        `shared_subject` is the match subject's exact syntax handle, which the parser
        shares across every arm of one match. The first arm mints
        the temp and records it; sibling arms reference that same temp, so
        exhaustiveness still groups them, and their own lowered subjects are left
        orphaned. `initial` runs only when minting and before the temp's name is
        drawn, so any temp it hoists for its own operands numbers first and
        precedes this one in `hoisted`.
    */
    pub(crate) fn mint_hoist_temp(
        lowerer: &mut Lowerer,
        hoisted: &mut Vec<Statement>,
        declared: TypeReference,
        shared_subject: Option<syntax_trees::expression::ExpressionHandle>,
        is_operand_capture: bool,
        initial: impl FnOnce(&mut Lowerer, &mut Vec<Statement>) -> ExpressionHandle,
    ) -> ExpressionHandle {
        let shared_temp = shared_subject.and_then(|subject| lowerer.match_subject_temp(subject));
        let (name, source_span) = match shared_temp {
            Some((existing, source_span)) => (DiagnosticName::generated(existing), source_span),
            None => {
                let initial_value = initial(lowerer, hoisted);
                let source_span = expression_table(lowerer).source_span(initial_value);
                capture_reference_initializer_selectors(
                    lowerer,
                    initial_value,
                    SelectorCapture::Every,
                    hoisted,
                );
                let fresh = lowerer.next_hoist_name();
                if let Some(subject) = shared_subject {
                    lowerer.record_match_subject_temp(subject, fresh.clone(), source_span);
                }
                let name = DiagnosticName::generated(fresh);
                let type_is_inferred = matches!(declared, TypeReference::Unit);
                hoisted.push(Statement::LocalData(LocalData {
                    symbol: SymbolHandle::invalid(),
                    name: name.clone(),
                    storage: LocalDataStorage {
                        role: language_core::LocalBindingRole::Value,
                        type_reference: declared,
                        initial_value,
                        is_mutable: false,
                        type_is_inferred,
                        is_operand_capture,
                        relevance: language_core::BindingRelevance::Relevant,
                        pattern_fields: Default::default(),
                    },
                }));
                (name, source_span)
            }
        };
        let reference = reference_temp(lowerer, name);
        expression_table(lowerer).set_source_span(reference, source_span);
        reference
    }

    /*
        A fresh source occurrence of an already established temporary. Its symbol
        is resolved normally; the syntax occurrence selects the capture, not text
        similarity between two independently authored expressions.
    */
    pub(in crate::lowering) fn reference_temp(
        lowerer: &mut Lowerer,
        name: DiagnosticName,
    ) -> ExpressionHandle {
        let expressions = &mut lowerer.symbol_resolved_trees.tables.bodies.expressions;
        let mut members = HandleSpan::empty();
        expressions.push_name_path_member(&mut members, name);
        let member_symbols = expressions.reserve_name_path_member_symbols(members.count());
        expressions.insert(ExpressionNode::Name(TableNamePath::new(
            members,
            member_symbols,
            SymbolHandle::invalid(),
            SymbolHandle::invalid(),
        )))
    }
}
mod indexed_read_hoisting {
    /* Hoisting runtime-indexed operand reads into synthetic locals. */

    use crate::lowering::expression::expression_table;
    use crate::lowering::statement::hoist_temps::hoist_into_temp;
    use crate::lowering::statement::match_subject_hoisting::{
        borrow_membership_subject, is_reference_struct_parameter_member,
    };
    use crate::lowering::statement::place_operand_capture::capture_address_operands;
    use crate::lowering::statement::statement_nodes::set_expression;
    use crate::resolution::lowerer::Lowerer;
    use symbol_resolved_trees::expression::{
        BinaryOperator, ExpressionHandle, ExpressionNode, TableBinaryExpression,
        TableCallExpression, TableCastExpression, TableMembershipExpression, TableRangeExpression,
        TableUnaryExpression,
    };
    use symbol_resolved_trees::statement::Statement;

    #[derive(Clone, Copy, PartialEq, Eq)]
    pub(crate) enum OperandHoisting {
        Value,
        Guard,
        /* Checked computations retain calls at their evaluation points. */
        Computation,
    }

    /*
        Hoists every runtime-indexed read in OPERAND position out of `value`.

        `value` is the ROOT of an assignment value, local initializer, or return.
        A root-level `Indexed` is left in place (the existing whole-value copy path
        handles it). For every OTHER node, each child that is an `Indexed`
        (transitively) is hoisted into a fresh `let __hoist_N = <indexed>;` appended
        to `hoisted`, and the child is replaced by a name referencing that temp.
        Returns the root handle to use in the rewritten statement.
    */
    pub(crate) fn hoist_operand_indexed_reads(
        lowerer: &mut Lowerer,
        value: ExpressionHandle,
        hoisted: &mut Vec<Statement>,
        mode: OperandHoisting,
    ) -> ExpressionHandle {
        rewrite_children(lowerer, value, hoisted, mode);
        value
    }

    /*
        Rewrites the CHILDREN of `expression` in place, hoisting any child that is an
        operand-position indexed read. Recurses so deeply nested cases work.
    */
    fn rewrite_children(
        lowerer: &mut Lowerer,
        expression: ExpressionHandle,
        hoisted: &mut Vec<Statement>,
        mode: OperandHoisting,
    ) {
        let node = expression_table(lowerer).expression(expression).clone();
        match node {
            ExpressionNode::Binary(binary) => {
                let left = hoist_child(lowerer, binary.left, hoisted, mode);
                let right = if matches!(binary.operator, BinaryOperator::And | BinaryOperator::Or) {
                    binary.right
                } else {
                    hoist_child(lowerer, binary.right, hoisted, mode)
                };
                set_expression(
                    lowerer,
                    expression,
                    ExpressionNode::Binary(TableBinaryExpression {
                        policy: binary.policy,
                        left,
                        operator: binary.operator,
                        right,
                    }),
                );
            }
            ExpressionNode::Unary(unary) => {
                let operand = hoist_child(lowerer, unary.operand, hoisted, mode);
                set_expression(
                    lowerer,
                    expression,
                    ExpressionNode::Unary(TableUnaryExpression {
                        operator: unary.operator,
                        operand,
                    }),
                );
            }
            ExpressionNode::Cast(cast) => {
                let value = if cast.form.is_recast()
                    && matches!(
                        expression_table(lowerer).expression(cast.value),
                        ExpressionNode::Indexed(_)
                    ) {
                    cast.value
                } else if mode == OperandHoisting::Value
                    && !cast.form.is_recast()
                    && is_hoistable_value_cast_call(lowerer, cast.value)
                {
                    hoist_into_temp(lowerer, cast.value, hoisted)
                } else {
                    hoist_child(lowerer, cast.value, hoisted, mode)
                };
                set_expression(
                    lowerer,
                    expression,
                    ExpressionNode::Cast(TableCastExpression {
                        value,
                        target_type: cast.target_type,
                        target_label: cast.target_label,
                        domain: cast.domain,
                        semantic_domain: cast.semantic_domain,
                        semantic_domain_arguments: cast.semantic_domain_arguments,
                        semantic_domain_symbol: cast.semantic_domain_symbol,
                        form: cast.form,
                    }),
                );
            }
            ExpressionNode::Membership(membership) => {
                let value = if is_runtime_indexed_read(lowerer, membership.value) {
                    let observed = borrow_membership_subject(lowerer, membership.value);
                    hoist_into_temp(lowerer, observed, hoisted)
                } else {
                    hoist_child(lowerer, membership.value, hoisted, mode)
                };
                set_expression(
                    lowerer,
                    expression,
                    ExpressionNode::Membership(TableMembershipExpression {
                        is_pattern: membership.is_pattern,
                        value,
                        domain: membership.domain,
                        domain_symbol: membership.domain_symbol,
                        case_type_symbol: membership.case_type_symbol,
                        case_symbol: membership.case_symbol,
                    }),
                );
            }
            ExpressionNode::Borrow(_) => {}
            ExpressionNode::Range(range) => {
                let start = if range.start.is_valid() {
                    hoist_child(lowerer, range.start, hoisted, mode)
                } else {
                    range.start
                };
                let end = if range.end.is_valid() {
                    hoist_child(lowerer, range.end, hoisted, mode)
                } else {
                    range.end
                };
                set_expression(
                    lowerer,
                    expression,
                    ExpressionNode::Range(TableRangeExpression {
                        start,
                        end,
                        end_inclusive: range.end_inclusive,
                    }),
                );
            }
            ExpressionNode::Indexed(_) => {
                capture_address_operands(lowerer, expression, hoisted);
            }
            _ => {}
        }
    }

    /*
        Whether a direct call beneath a non-recast value cast can use the let-bound
        value-call result path. Proof-only integer embedding stays in its proof
        expression because it has no runtime result slot.
    */
    fn is_hoistable_value_cast_call(lowerer: &Lowerer, expression: ExpressionHandle) -> bool {
        let expressions = &lowerer.symbol_resolved_trees.tables.bodies.expressions;
        let Some(call) = expressions.call_node(expression) else {
            return false;
        };
        !call.receiver.is_valid() && !is_integer_embedding_call(lowerer, call)
    }

    /*
        A compiler proof term has no runtime call-result slot to materialize.
        Keep its source expression intact for proof typing and ordered coercion
        checks. Calls are initially unresolved at this normalization site, so the
        reserved syntax is preserved without assigning it semantic authority. The
        later proof-only gate requires the exact builtin symbol and rejects both
        executable uses and any authored same-spelled replacement.
    */
    pub(crate) fn is_integer_embedding_call(lowerer: &Lowerer, call: &TableCallExpression) -> bool {
        !call.receiver.is_valid()
            && call.target.as_str() == symbols::BuiltinFunction::IntegerEmbed.name()
            && (!call.target_symbol.is_valid()
                || lowerer
                    .symbol_resolved_trees
                    .symbols
                    .builtin_function_for_symbol(call.target_symbol)
                    == Some(symbols::BuiltinFunction::IntegerEmbed))
    }

    /*
        Hoists `child` if it is an `Indexed` node, otherwise recurses into it.

        Returns the handle to use in the parent: a `Name` referencing the new temp
        when hoisted, or the original handle (with its children rewritten) otherwise.
    */
    pub(crate) fn hoist_child(
        lowerer: &mut Lowerer,
        child: ExpressionHandle,
        hoisted: &mut Vec<Statement>,
        mode: OperandHoisting,
    ) -> ExpressionHandle {
        if is_reference_struct_parameter_member(lowerer, child) {
            return hoist_into_temp(lowerer, child, hoisted);
        }

        if is_runtime_indexed_read(lowerer, child) {
            rewrite_children(lowerer, child, hoisted, mode);
            return hoist_into_temp(lowerer, child, hoisted);
        }

        if mode != OperandHoisting::Guard && is_member_of_runtime_indexed_read(lowerer, child) {
            rewrite_children(lowerer, child, hoisted, mode);
            return hoist_into_temp(lowerer, child, hoisted);
        }

        rewrite_children(lowerer, child, hoisted, mode);
        child
    }

    /*
        Whether `expression` is an `Indexed` read whose index is NOT a constant
        integer -- the RUNTIME-indexed case that needs operand hoisting. A
        constant-index read (`arr[0]`) lowers as a plain place path and is left
        alone (so existing whole-value copies / borrows are untouched).
        Whether `expression` is a MEMBER chain whose receiver bottoms out at a
        runtime-indexed read of a MACHINE-owned collection (`self.cells[k].v`).
        Restricted to `self.<field>` collections because the hoist temp's type is
        inferred from the machine's attached data (`infer_hoist_temp_type`); a
        LOCAL array's element field would mint an untypeable Unit temp AND break
        the local-array RMW write path that pattern-matches the unhoisted read.
    */
    fn is_member_of_runtime_indexed_read(lowerer: &Lowerer, expression: ExpressionHandle) -> bool {
        let expressions = &lowerer.symbol_resolved_trees.tables.bodies.expressions;
        let Some(member) = expressions.member_node(expression) else {
            return false;
        };
        let mut receiver = member.receiver;
        while let Some(inner) = expressions.member_node(receiver) {
            receiver = inner.receiver;
        }
        if !is_runtime_indexed_read(lowerer, receiver) {
            return false;
        }
        let Some(indexed) = expressions.indexed_node(receiver) else {
            return false;
        };
        match expressions.expression(indexed.collection) {
            ExpressionNode::Member(collection_member) => matches!(
                expressions.expression(collection_member.receiver),
                ExpressionNode::Name(path)
                    if expressions
                        .name_path_members(path.members)
                        .first()
                        .is_some_and(|name| name.is_self_receiver())
            ),
            ExpressionNode::Name(path) => {
                let members = expressions.name_path_members(path.members);
                members.len() == 2 && members[0].is_self_receiver()
            }
            _ => false,
        }
    }

    pub(crate) fn is_runtime_indexed_read(lowerer: &Lowerer, expression: ExpressionHandle) -> bool {
        let expressions = &lowerer.symbol_resolved_trees.tables.bodies.expressions;
        let Some(indexed) = expressions.indexed_node(expression) else {
            return false;
        };
        !matches!(
            expressions.expression(indexed.index),
            ExpressionNode::Integer(_)
        )
    }
}
mod match_subject_hoisting {
    /* Hoisting membership and comparison match subjects. */

    use crate::lowering::expression::expression_table;
    use crate::lowering::statement::hoist_temps::mint_hoist_temp;
    use crate::lowering::statement::indexed_read_hoisting::{
        OperandHoisting, hoist_operand_indexed_reads, is_runtime_indexed_read,
    };
    use crate::lowering::statement::statement_nodes::set_expression;
    use crate::resolution::lowerer::Lowerer;
    use symbol_resolved_trees::expression::{
        BinaryOperator, ExpressionHandle, ExpressionNode, TableBinaryExpression,
        TableBorrowExpression, TableMembershipExpression,
    };
    use symbol_resolved_trees::name::DiagnosticName;
    use symbol_resolved_trees::statement::Statement;
    use symbol_resolved_trees::types::TypeReference;
    use symbols::SymbolHandle;
    use syntax_trees::{self as syntax, SyntaxTrees};

    /*
        Capture a membership place for observation without extracting its value.
        The ordinary borrow preserves address evaluation and loan checking; an
        owned snapshot would illegally move an affine element out of its array.
    */
    pub(crate) fn borrow_membership_subject(
        lowerer: &mut Lowerer,
        subject: ExpressionHandle,
    ) -> ExpressionHandle {
        expression_table(lowerer).insert(ExpressionNode::Borrow(TableBorrowExpression {
            target: subject,
            access: language_semantics::ReferenceAccess::Shared,
        }))
    }

    /*
        Hoists a runtime-indexed ENUM-VARIANT MATCH subject into a SINGLE shared temp so every arm of
        the match tests one plain local. A match `transition self.grid[self.i] { Cell::Wall -> .. }`
        lowers (in the parser) to one `When(subject is Variant)` -- an `ExpressionNode::Membership` --
        per arm, and every arm's Membership references the SAME syntax subject handle. Naively hoisting
        each arm's subject (as the comparison hoist does for `Binary` guards) would mint a DISTINCT temp
        per arm, and the exhaustiveness checker -- which groups the arms by a shared subject -- would then
        report "match does not cover Variant". Instead this keys a memo on the shared syntax subject
        handle: the FIRST arm mints `let __hoist_N = &self.grid[self.i]` and records the name; the siblings
        reuse it. All arms end up testing `__hoist_N`, so exhaustiveness still groups them, and the temp
        retains the original element's address rather than a copied affine value.
        Const-index / field / string-slice subjects are not runtime-indexed and are left
        untouched.
    */
    pub(crate) fn hoist_membership_match_subject(
        lowerer: &mut Lowerer,
        syntax_trees: &SyntaxTrees,
        syntax_guard: syntax::statement::TransitionGuardNode,
        guard_expression: ExpressionHandle,
        hoisted: &mut Vec<Statement>,
    ) {
        let ExpressionNode::Membership(membership) = expression_table(lowerer)
            .expression(guard_expression)
            .clone()
        else {
            return;
        };
        if !is_runtime_indexed_read(lowerer, membership.value) {
            return;
        }
        let syntax::statement::TransitionGuardNode::When(syntax_expression) = syntax_guard else {
            return;
        };
        let syntax::expression::ExpressionNode::Membership(syntax_membership) =
            syntax_trees.expressions.expression(syntax_expression)
        else {
            return;
        };
        let subject_key = syntax_membership.value;

        let name_reference = mint_hoist_temp(
            lowerer,
            hoisted,
            TypeReference::Unit,
            Some(subject_key),
            false,
            |lowerer, _| borrow_membership_subject(lowerer, membership.value),
        );

        set_expression(
            lowerer,
            guard_expression,
            ExpressionNode::Membership(TableMembershipExpression {
                is_pattern: membership.is_pattern,
                value: name_reference,
                domain: membership.domain,
                domain_symbol: membership.domain_symbol,
                case_type_symbol: membership.case_type_symbol,
                case_symbol: membership.case_symbol,
            }),
        );
    }

    /*
        Capture a comparison's hoistable storage reads once for Boolean dispatch.
        The parser shares one syntax subject across its true/false arms, but each
        arm gets its own lowered expression. Independently hoisting `values[index]`
        or a reference-backed field would give the arms different locals and lose
        their common exhaustive subject. Key the shared Boolean local by that syntax
        subject; its initializer owns the storage-read preparation. Ordinary call
        sequencing stays with the common computation owner, not a builtin-name rule.
    */
    pub(crate) fn hoist_comparison_match_subject(
        lowerer: &mut Lowerer,
        syntax_trees: &SyntaxTrees,
        syntax_guard: syntax::statement::TransitionGuardNode,
        guard_expression: ExpressionHandle,
        hoisted: &mut Vec<Statement>,
    ) -> bool {
        let ExpressionNode::Binary(outer) = expression_table(lowerer)
            .expression(guard_expression)
            .clone()
        else {
            return false;
        };
        if !matches!(
            outer.operator,
            BinaryOperator::Equal | BinaryOperator::NotEqual
        ) {
            return false;
        }
        if !matches!(
            expression_table(lowerer).expression(outer.right),
            ExpressionNode::Boolean(_)
        ) {
            return false;
        }
        let subject_is_comparison = matches!(
            expression_table(lowerer).expression(outer.left),
            ExpressionNode::Binary(inner) if inner.operator.is_comparison()
        );
        if !subject_is_comparison || !subject_contains_hoistable(lowerer, outer.left) {
            return false;
        }

        let syntax::statement::TransitionGuardNode::When(syntax_expression) = syntax_guard else {
            return false;
        };
        let syntax::expression::ExpressionNode::Binary(syntax_outer) =
            syntax_trees.expressions.expression(syntax_expression)
        else {
            return false;
        };
        let subject_key = syntax_outer.left;

        let declared_bool = TypeReference::Named {
            supplied_operand_owner: Default::default(),
            symbol: SymbolHandle::invalid(),
            name: DiagnosticName::generated("bool"),
        };
        let name_reference = mint_hoist_temp(
            lowerer,
            hoisted,
            declared_bool,
            Some(subject_key),
            false,
            |lowerer, hoisted| {
                hoist_operand_indexed_reads(
                    lowerer,
                    outer.left,
                    hoisted,
                    OperandHoisting::Computation,
                );
                outer.left
            },
        );

        set_expression(
            lowerer,
            guard_expression,
            ExpressionNode::Binary(TableBinaryExpression {
                policy: outer.policy,
                left: name_reference,
                operator: outer.operator,
                right: outer.right,
            }),
        );
        true
    }

    /*
        Whether `expression` is a member read through a SHARED reference-to-struct
        parameter of the current state (`table.con_out` with `table:
        &EfiSystemTable`) -- the shape whose flat fold reads frame garbage. The
        receiver must be a bare single-segment name matching one of the recorded
        `&Named` params.
    */
    pub(crate) fn is_reference_struct_parameter_member(
        lowerer: &Lowerer,
        expression: ExpressionHandle,
    ) -> bool {
        if lowerer.reference_struct_parameters.is_empty() {
            return false;
        }
        let expressions = &lowerer.symbol_resolved_trees.tables.bodies.expressions;
        let Some(member) = expressions.member_node(expression) else {
            return false;
        };
        let Some(path) = expressions.name_path_node(member.receiver) else {
            return false;
        };
        let members = expressions.name_path_members(path.members);
        let [only] = members else {
            return false;
        };
        lowerer
            .reference_struct_parameters
            .iter()
            .any(|name| name == only.as_str())
    }

    fn subject_contains_hoistable(lowerer: &Lowerer, expression: ExpressionHandle) -> bool {
        if is_runtime_indexed_read(lowerer, expression)
            || is_reference_struct_parameter_member(lowerer, expression)
        {
            return true;
        }
        let node = lowerer
            .symbol_resolved_trees
            .tables
            .bodies
            .expressions
            .expression(expression)
            .clone();
        match node {
            ExpressionNode::Binary(binary) => {
                subject_contains_hoistable(lowerer, binary.left)
                    || subject_contains_hoistable(lowerer, binary.right)
            }
            ExpressionNode::Unary(unary) => subject_contains_hoistable(lowerer, unary.operand),
            ExpressionNode::Cast(cast) => subject_contains_hoistable(lowerer, cast.value),
            _ => false,
        }
    }

    /*
        Whether a `When` guard is a COMPARISON/boolean guard whose runtime-indexed operands should be
        hoisted. A match arm lowers to a `When(subject is Variant)` -- an `ExpressionNode::Membership`
        root -- and hoisting each arm's subject into a distinct temp would break match exhaustiveness
        (all arms of one match must share a single subject). Only a `Binary` root (`arr[i] > 5`,
        `arr[i] == 66`, `a && b`) is hoisted; membership/pattern guards are left for the separate
        shared-subject match rewrite.
    */
    pub(crate) fn guard_hoists_operands(lowerer: &Lowerer, expression: ExpressionHandle) -> bool {
        matches!(
            lowerer
                .symbol_resolved_trees
                .tables
                .bodies
                .expressions
                .expression(expression),
            ExpressionNode::Binary(_)
        )
    }
}
mod place_operand_capture {
    /*
        Ordered operands for an unconditional place evaluation.

        A loan keeps its pointee as storage, but its selectors are evaluated values.
        An address evaluates its collection before its selector and a range's start
        before its end (`wiki/spec/language/expressions.md`). A captured operand is
        evaluated once into an ordinary inferred local. Reads, stores and hoisted
        subjects capture every evaluated operand: later arms, payload receivers and
        right-hand sides read the place again. An authored reference initializer
        forms its loan once where it is written, so it captures an operand only
        when that operand itself may write storage, keeping its effect a statement
        in authored order, or when a later operand of the same address may write
        what it read. Its other operands stay authored, and checking keeps their
        declared ranges, guards and witnesses and names them in diagnostics. This
        owner is intentionally not used for guarded operands: moving their
        producers would change short-circuit order. Anonymous arithmetic keeps its
        contextual landing boundary rather than acquiring a synthetic local's
        scalar carrier.
    */

    use super::value_call_hoisting::expression_contains;
    use crate::lowering::expression::expression_table;
    use crate::lowering::statement::hoist_temps::{hoist_into_temp, hoist_operand_into_temp};
    use crate::lowering::statement::statement_nodes::set_expression;
    use crate::resolution::lowerer::Lowerer;
    use symbol_resolved_trees::expression::{ExpressionHandle, ExpressionNode};
    use symbol_resolved_trees::statement::Statement;

    /* Which evaluated operands of a reference formation receive their own local. */
    #[derive(Clone, Copy, PartialEq, Eq)]
    pub(super) enum SelectorCapture {
        /*
            An authored initializer: operands that may write, or that a later
            operand may write.
        */
        BeforeWrites,
        /* A hoisted subject place read again by later arms: every operand. */
        Every,
    }

    /* Prepare only a reference formation's unconditional address evaluation. */
    pub(super) fn capture_reference_initializer_selectors(
        lowerer: &mut Lowerer,
        initializer: ExpressionHandle,
        capture: SelectorCapture,
        hoisted: &mut Vec<Statement>,
    ) {
        capture_formation(
            lowerer,
            initializer,
            capture == SelectorCapture::Every,
            hoisted,
        );
    }

    /* Retain storage identity while evaluating its address operands in order. */
    pub(super) fn capture_address_operands(
        lowerer: &mut Lowerer,
        address: ExpressionHandle,
        hoisted: &mut Vec<Statement>,
    ) -> ExpressionHandle {
        capture_operands(lowerer, address, true, hoisted)
    }

    /*
        A pending typed subject may become either a value snapshot or a storage
        observation. Prepare the address operands of its projection now, before
        later subjects execute. The typed owner still decides whether to copy the
        completed value or lend the place. A produced root with no projection
        already has that pending value binding and needs no additional local.
    */
    pub(super) fn capture_subject_address_operands(
        lowerer: &mut Lowerer,
        subject: ExpressionHandle,
        hoisted: &mut Vec<Statement>,
    ) -> ExpressionHandle {
        match expression_table(lowerer).expression(subject) {
            ExpressionNode::Member(_) | ExpressionNode::Indexed(_) | ExpressionNode::Borrow(_) => {
                capture_address_operands(lowerer, subject, hoisted)
            }
            ExpressionNode::Cast(cast) if cast.form.is_recast() => {
                capture_address_operands(lowerer, subject, hoisted)
            }
            _ => subject,
        }
    }

    /*
        `capture` says whether an operand evaluated at this point needs its own
        local: the place is read again later, or a later operand of the same
        address may write storage.
    */
    fn capture_formation(
        lowerer: &mut Lowerer,
        formation: ExpressionHandle,
        capture: bool,
        hoisted: &mut Vec<Statement>,
    ) {
        let node = expression_table(lowerer).expression(formation).clone();
        match node {
            ExpressionNode::Borrow(mut borrow) => {
                borrow.target = capture_operands(lowerer, borrow.target, capture, hoisted);
                set_expression(lowerer, formation, ExpressionNode::Borrow(borrow));
            }
            ExpressionNode::Cast(mut cast) if cast.form.is_recast() => {
                cast.value = capture_operands(lowerer, cast.value, capture, hoisted);
                set_expression(lowerer, formation, ExpressionNode::Cast(cast));
            }
            _ => {}
        }
    }

    fn capture_operands(
        lowerer: &mut Lowerer,
        address: ExpressionHandle,
        capture: bool,
        hoisted: &mut Vec<Statement>,
    ) -> ExpressionHandle {
        let node = expression_table(lowerer).expression(address).clone();
        match node {
            ExpressionNode::Name(_) => address,
            ExpressionNode::Member(mut member) => {
                member.receiver = capture_operands(lowerer, member.receiver, capture, hoisted);
                set_expression(lowerer, address, ExpressionNode::Member(member));
                address
            }
            ExpressionNode::Indexed(mut indexed) => {
                let selector_writes = operand_may_write(lowerer, indexed.index);
                indexed.collection = capture_operands(
                    lowerer,
                    indexed.collection,
                    capture || selector_writes,
                    hoisted,
                );
                indexed.index = capture_selector(lowerer, indexed.index, capture, hoisted);
                set_expression(lowerer, address, ExpressionNode::Indexed(indexed));
                address
            }
            ExpressionNode::Borrow(_) => {
                capture_formation(lowerer, address, capture, hoisted);
                address
            }
            ExpressionNode::Cast(mut cast) if cast.form.is_recast() => {
                cast.value = capture_operands(lowerer, cast.value, capture, hoisted);
                set_expression(lowerer, address, ExpressionNode::Cast(cast));
                address
            }
            _ if capture || operand_may_write(lowerer, address) => {
                hoist_into_temp(lowerer, address, hoisted)
            }
            _ => address,
        }
    }

    fn capture_selector(
        lowerer: &mut Lowerer,
        selector: ExpressionHandle,
        capture: bool,
        hoisted: &mut Vec<Statement>,
    ) -> ExpressionHandle {
        if is_numeric_literal_expression(lowerer, selector) {
            return selector;
        }
        let node = expression_table(lowerer).expression(selector).clone();
        match node {
            ExpressionNode::Range(mut range) => {
                let end_writes = range.end.is_valid() && operand_may_write(lowerer, range.end);
                if range.start.is_valid() {
                    range.start =
                        capture_selector(lowerer, range.start, capture || end_writes, hoisted);
                }
                if range.end.is_valid() {
                    range.end = capture_selector(lowerer, range.end, capture, hoisted);
                }
                set_expression(lowerer, selector, ExpressionNode::Range(range));
                selector
            }
            _ if capture || operand_may_write(lowerer, selector) => {
                hoist_operand_into_temp(lowerer, selector, hoisted)
            }
            _ => selector,
        }
    }

    /*
        A call may write through its arguments or a boundary, and an atomic event
        may store. Every other operand form only reads.
    */
    fn operand_may_write(lowerer: &Lowerer, operand: ExpressionHandle) -> bool {
        expression_contains(lowerer, operand, &|node| {
            matches!(node, ExpressionNode::Call(_) | ExpressionNode::Atomic(_))
        })
    }

    fn is_numeric_literal_expression(lowerer: &Lowerer, expression: ExpressionHandle) -> bool {
        let expressions = &lowerer.symbol_resolved_trees.tables.bodies.expressions;
        let mut pending = vec![expression];
        while let Some(expression) = pending.pop() {
            match expressions.expression(expression) {
                ExpressionNode::Integer(_) | ExpressionNode::Float(_) => {}
                ExpressionNode::Unary(unary) => pending.push(unary.operand),
                ExpressionNode::Binary(binary) => {
                    pending.push(binary.right);
                    pending.push(binary.left);
                }
                _ => return false,
            }
        }
        true
    }
}
mod statement_nodes {
    /* One-to-one lowering of statement nodes, transition guards and targets. */

    use super::assembly::lower;
    use crate::lowering::expression::expression_table;
    use crate::lowering::expression::lower_private_expression_into_table;
    use crate::lowering::expression::lower_static_machine_argument;
    use crate::lowering::name::lower_name;
    use crate::lowering::name::lower_names;
    use crate::lowering::statement::hoist_temps::hoist_into_temp;
    use crate::lowering::statement::indexed_read_hoisting::{
        OperandHoisting, hoist_child, hoist_operand_indexed_reads,
    };
    use crate::lowering::statement::match_subject_hoisting::{
        guard_hoists_operands, hoist_comparison_match_subject, hoist_membership_match_subject,
        is_reference_struct_parameter_member,
    };
    use crate::lowering::statement::place_operand_capture::{
        SelectorCapture, capture_address_operands, capture_reference_initializer_selectors,
        capture_subject_address_operands,
    };
    use crate::lowering::statement::value_call_hoisting::hoist_scalar_value_call_comparison;
    use crate::resolution::lowerer::Lowerer;
    use crate::tables::syntax_expressions;
    use arena::HandleSpan;
    use diagnostics::Diagnostic;
    use language_semantics::declaration_selection::BuildOperation;
    use symbol_resolved_trees::expression::{ExpressionHandle, ExpressionNode};
    use symbol_resolved_trees::name::DiagnosticName;
    use symbol_resolved_trees::signature::CrashCause;
    use symbol_resolved_trees::statement::{
        AssemblyFact, AssemblyFactKind, Assignment, Call, CallStorage, EvidenceForwarding,
        LocalData, LocalDataStorage, NamedTransitionTarget, NamedTransitionTargetStorage,
        OutcomeProofSelector, ProofOutputBindingStatement, ProofOutputSelector, RootBinding,
        Statement, Transition, TransitionExit, TransitionGuard, TransitionTarget,
    };
    use symbol_resolved_trees::types::TypeReference;
    use symbols::SymbolHandle;
    use syntax::identifier::Identifier;
    use syntax::statement::StatementNode;
    use syntax::statement::TransitionGuardNode;
    use syntax::statement::TransitionTargetHandle;
    use syntax::statement::TransitionTargetNode;
    use syntax_trees::{self as syntax, SyntaxTrees};

    pub(crate) fn lower_statement_node(
        lowerer: &mut Lowerer,
        syntax_trees: &SyntaxTrees,
        statement: &StatementNode,
        statement_index: usize,
        has_preceding_transition: bool,
    ) -> Result<Vec<Statement>, Diagnostic> {
        match statement {
            StatementNode::Assembly(handle) => {
                lower(lowerer, syntax_trees, *handle, statement_index)
                    .map(|handle| vec![Statement::Assembly(handle)])
            }
            StatementNode::RootBinding(binding) => Ok(vec![Statement::RootBinding(RootBinding {
                receiver: lower_statement_expression(lowerer, syntax_trees, binding.receiver)?,
                slot: lower_names(syntax_trees, &binding.slot).into_boxed_slice(),
                implementation: lower_names(syntax_trees, &binding.implementation)
                    .into_boxed_slice(),
                implementation_operand: if binding.implementation_operand.is_valid() {
                    if !matches!(
                        syntax_expressions(syntax_trees).expression(binding.implementation_operand),
                        syntax::expression::ExpressionNode::Name(_)
                    ) {
                        lower_statement_expression(
                            lowerer,
                            syntax_trees,
                            binding.implementation_operand,
                        )?
                    } else {
                        let exposure = lowerer.current_authored_expression_exposure.take();
                        let lowered = crate::lowering::expression::lower_expression_into_table(
                            lowerer,
                            syntax_trees,
                            binding.implementation_operand,
                        );
                        lowerer.current_authored_expression_exposure = exposure;
                        lowered?
                    }
                } else {
                    symbol_resolved_trees::expression::ExpressionHandle::invalid()
                },
                source_span: binding.source_span,
            })]),
            StatementNode::AssemblyFact(fact) => Ok(vec![Statement::AssemblyFact(AssemblyFact {
                kind: match fact.kind {
                    syntax::statement::AssemblyFactKind::Requires => AssemblyFactKind::Requires,
                    syntax::statement::AssemblyFactKind::Ensures => AssemblyFactKind::Ensures,
                },
                expression: lower_statement_expression(lowerer, syntax_trees, fact.expression)?,
            })]),
            StatementNode::Assignment(assignment) => {
                if let Some((target, source)) = evidence_forwarding_names(
                    lowerer,
                    syntax_trees,
                    assignment.target,
                    assignment.value,
                ) {
                    lowerer
                        .symbol_resolved_trees
                        .evidence_forwardings
                        .push(EvidenceForwarding {
                            machine_root_index: lowerer
                                .current_machine_root_index
                                .unwrap_or(usize::MAX),
                            machine_name: DiagnosticName::generated(
                                lowerer.current_machine_name.as_deref().unwrap_or_default(),
                            ),
                            state_name: DiagnosticName::generated(
                                lowerer.current_state_name.as_deref().unwrap_or_default(),
                            ),
                            machine_symbol: SymbolHandle::invalid(),
                            state_symbol: SymbolHandle::invalid(),
                            statement_index,
                            target: lower_name(syntax_trees, target),
                            source: lower_name(syntax_trees, source),
                            source_conformance: None,
                        });
                    return Ok(Vec::new());
                }
                let target = lower_statement_expression(lowerer, syntax_trees, assignment.target)?;
                let value = lower_statement_expression(lowerer, syntax_trees, assignment.value)?;
                let mut hoisted = Vec::new();
                let target = capture_address_operands(lowerer, target, &mut hoisted);
                let value = hoist_operand_indexed_reads(
                    lowerer,
                    value,
                    &mut hoisted,
                    OperandHoisting::Computation,
                );
                let value = if is_reference_struct_parameter_member(lowerer, value) {
                    hoist_child(lowerer, value, &mut hoisted, OperandHoisting::Value)
                } else {
                    value
                };
                hoisted.push(Statement::Assignment(Assignment { target, value }));
                Ok(hoisted)
            }
            StatementNode::Call(call) => {
                let receiver = lower_statement_path_members(lowerer, syntax_trees, call.receiver);
                if call.target_is_static && !receiver.is_empty() {
                    let mut path = lowerer
                        .symbol_resolved_trees
                        .tables
                        .declarations
                        .statement_path_members
                        .span_or_empty(receiver)
                        .to_vec();
                    path.push(lower_name(syntax_trees, &call.target));
                    lowerer
                        .pending_static_module_statement_calls
                        .push((call.target.source_span(), path));
                }
                let arguments = lower_statement_expressions(lowerer, syntax_trees, call.arguments)?;
                let mut hoisted = Vec::new();
                for offset in 0..arguments.count() {
                    let argument =
                        expression_table(lowerer).expression_handles(arguments)[offset as usize];
                    if is_reference_struct_parameter_member(lowerer, argument) {
                        let temp = hoist_into_temp(lowerer, argument, &mut hoisted);
                        expression_table(lowerer)
                            .set_expression_handle_at_offset(arguments, offset, temp);
                    }
                }
                let lowered_call = Call {
                    receiver_symbol: SymbolHandle::invalid(),
                    target_symbol: SymbolHandle::invalid(),
                    target: lower_name(syntax_trees, &call.target),
                    storage: CallStorage {
                        receiver_root_symbol: SymbolHandle::invalid(),
                        receiver,
                        machine_arguments: call
                            .machine_arguments
                            .iter()
                            .map(|argument| {
                                lower_static_machine_argument(lowerer, syntax_trees, argument)
                            })
                            .collect::<Result<Vec<_>, _>>()?
                            .into_boxed_slice(),
                        arguments,
                        evidence_arguments: lower_names(syntax_trees, &call.evidence_arguments)
                            .into_boxed_slice(),
                        operational_acknowledgement: call.operational_acknowledgement,
                        discards_result: call.discards_result,
                        authored_call_selection: None,
                        authored_static_argument_selections: Box::default(),
                    },
                };
                if matches!(
                    BuildOperation::from_call_target(call.target.as_str(syntax_trees)),
                    Some(
                        BuildOperation::ProviderSelection
                            | BuildOperation::AssumedBindingContract
                            | BuildOperation::ServiceExclusion
                            | BuildOperation::ServiceAudit
                            | BuildOperation::TestGroupRegistration
                            | BuildOperation::BoundaryAcceptance
                            | BuildOperation::WireCompatibilityRequest
                            | BuildOperation::RepresentationSelection
                    )
                ) {
                    let expression =
                        retain_build_declaration_call_expression(lowerer, lowered_call);
                    hoisted.push(Statement::Expression(expression));
                } else {
                    hoisted.push(Statement::Call(lowered_call));
                }
                Ok(hoisted)
            }
            StatementNode::ProofOutputBindingStatement(binding) => {
                let call = lower_statement_expression(lowerer, syntax_trees, binding.call)?;
                let runtime_value = binding.bindings.iter().find(|binding| {
                    binding.output_field.as_str(syntax_trees) == "value"
                        && binding.binding.as_str(syntax_trees) != "_"
                });
                let bindings = binding
                    .bindings
                    .iter()
                    .map(|binding| {
                        if binding.output_field.as_str(syntax_trees) != "value"
                            && binding.binding.as_str(syntax_trees) != "_"
                        {
                            lowerer
                                .current_evidence_term_names
                                .push(binding.binding.as_str(syntax_trees).to_owned());
                        }
                        ProofOutputSelector {
                            output_field: lower_name(syntax_trees, &binding.output_field),
                            binding: lower_name(syntax_trees, &binding.binding),
                        }
                    })
                    .collect::<Vec<_>>()
                    .into_boxed_slice();
                let mut lowered = Vec::with_capacity(usize::from(runtime_value.is_some()) + 1);
                if let Some(runtime_value) = runtime_value {
                    lowered.push(Statement::LocalData(LocalData {
                        symbol: SymbolHandle::invalid(),
                        name: lower_name(syntax_trees, &runtime_value.binding),
                        storage: LocalDataStorage {
                            role: language_core::LocalBindingRole::Value,
                            type_reference: TypeReference::Unit,
                            initial_value: call,
                            is_mutable: false,
                            type_is_inferred: true,
                            is_operand_capture: false,
                            relevance: language_core::BindingRelevance::Relevant,
                            pattern_fields: Default::default(),
                        },
                    }));
                }
                lowered.push(Statement::ProofOutputBindingStatement(
                    ProofOutputBindingStatement {
                        machine_symbol: SymbolHandle::invalid(),
                        state_symbol: SymbolHandle::invalid(),
                        statement_index,
                        bindings,
                        call,
                    },
                ));
                Ok(lowered)
            }
            StatementNode::Expression(expression) => {
                let mut hoisted = Vec::new();
                let expression = lower_statement_expression(lowerer, syntax_trees, *expression)?;
                let expression = hoist_operand_indexed_reads(
                    lowerer,
                    expression,
                    &mut hoisted,
                    OperandHoisting::Computation,
                );
                hoisted.push(Statement::Expression(expression));
                Ok(hoisted)
            }
            StatementNode::LocalData(local_data) => {
                let mut declaration = crate::lowering::state::lower_local_declaration(
                    lowerer,
                    syntax_trees,
                    local_data,
                )?;
                let initial_value = if local_data.initial_value.is_valid() {
                    lower_statement_expression(lowerer, syntax_trees, local_data.initial_value)?
                } else {
                    ExpressionHandle::invalid()
                };
                let mut hoisted = Vec::new();
                let initial_value = if declaration.role
                    == language_core::LocalBindingRole::DispatchSubject
                    && initial_value.is_valid()
                {
                    capture_subject_address_operands(lowerer, initial_value, &mut hoisted)
                } else {
                    initial_value
                };
                let initial_value = if initial_value.is_valid() {
                    hoist_operand_indexed_reads(
                        lowerer,
                        initial_value,
                        &mut hoisted,
                        OperandHoisting::Computation,
                    )
                } else {
                    initial_value
                };
                declaration.initial_value = initial_value;
                if initial_value.is_valid() {
                    capture_reference_initializer_selectors(
                        lowerer,
                        initial_value,
                        SelectorCapture::BeforeWrites,
                        &mut hoisted,
                    );
                }
                hoisted.push(Statement::LocalData(declaration));
                Ok(hoisted)
            }
            StatementNode::Transition(transition) => {
                let mut hoisted = Vec::new();
                let outer_captures = lowerer.expression_match_captures.len();
                let mut patterns = Vec::new();
                for source in syntax_trees
                    .statements
                    .transition_patterns(transition.patterns)
                {
                    let subject =
                        lower_statement_expression(lowerer, syntax_trees, source.subject)?;
                    let capture = expression_table(lowerer).insert_transition_subject(subject);
                    let pattern =
                        crate::lowering::expression::match_dispatch::lower_destructure_pattern(
                            lowerer,
                            syntax_trees,
                            source.pattern,
                            source.capture,
                            capture,
                        )?;
                    let symbol_resolved_trees::expression::MatchPattern::Destructure(pattern) =
                        pattern
                    else {
                        unreachable!()
                    };
                    lowerer.expression_match_captures.push(
                        crate::resolution::lowerer::MatchArmCapture {
                            syntax_capture: source.capture,
                            syntax_head: source.pattern.head,
                            resolved_capture: capture,
                            resolved_head: pattern.head,
                        },
                    );
                    patterns.push(symbol_resolved_trees::statement::TransitionPattern {
                        is_structural: source.is_structural,
                        subject,
                        capture,
                        pattern,
                    });
                }

                let guard = lower_transition_guard_node(lowerer, syntax_trees, transition.guard)?;
                if let TransitionGuard::When(expression) = guard {
                    hoist_membership_match_subject(
                        lowerer,
                        syntax_trees,
                        transition.guard,
                        expression,
                        &mut hoisted,
                    );
                }
                let unconditional = !has_preceding_transition
                    && transition.patterns.is_empty()
                    && matches!(transition.guard, TransitionGuardNode::Always);
                let target = lower_transition_target_node(
                    lowerer,
                    syntax_trees,
                    transition.target,
                    &mut hoisted,
                    unconditional,
                )?;
                let continuation = if transition.continuation.is_valid() {
                    Some(lower_transition_target_node(
                        lowerer,
                        syntax_trees,
                        transition.continuation,
                        &mut hoisted,
                        unconditional,
                    )?)
                } else {
                    None
                };
                if let TransitionGuard::When(expression) = guard {
                    if !has_preceding_transition {
                        hoist_scalar_value_call_comparison(
                            lowerer,
                            syntax_trees,
                            transition.guard,
                            expression,
                            &mut hoisted,
                        );
                    }
                    if guard_hoists_operands(lowerer, expression)
                        && !hoist_comparison_match_subject(
                            lowerer,
                            syntax_trees,
                            transition.guard,
                            expression,
                            &mut hoisted,
                        )
                        && !has_preceding_transition
                    {
                        hoist_operand_indexed_reads(
                            lowerer,
                            expression,
                            &mut hoisted,
                            OperandHoisting::Guard,
                        );
                    }
                }
                lowerer.expression_match_captures.truncate(outer_captures);
                hoisted.push(Statement::Transition(Transition {
                    source_guard: None,
                    patterns: patterns.into_boxed_slice(),
                    target,
                    continuation,
                    guard,
                    proof_selectors: syntax_trees
                        .statements
                        .outcome_proof_selectors(transition.proof_selectors)
                        .iter()
                        .map(|selector| OutcomeProofSelector {
                            output_field: lower_name(syntax_trees, &selector.output_field),
                            binding: lower_name(syntax_trees, &selector.binding),
                        })
                        .collect::<Vec<_>>()
                        .into_boxed_slice(),
                    exit: match transition.exit {
                        syntax::statement::TransitionExit::Ordinary => TransitionExit::Ordinary,
                        syntax::statement::TransitionExit::Crash(cause) => {
                            TransitionExit::Crash(match cause {
                                syntax::item::CrashCause::Trap => CrashCause::Trap,
                                syntax::item::CrashCause::Abort => CrashCause::Abort,
                            })
                        }
                    },
                    ends_dispatch: transition.ends_dispatch,
                    source_span: transition.source_span,
                }));
                Ok(hoisted)
            }
        }
    }

    fn retain_build_declaration_call_expression(
        lowerer: &mut Lowerer,
        call: Call,
    ) -> ExpressionHandle {
        use language_semantics::declaration_selection::AuthoredDeclarationSelectionExposure;
        use symbol_resolved_trees::expression::{TableCallExpression, TableNamePath};

        let receiver_members = lowerer
            .symbol_resolved_trees
            .tables
            .declarations
            .statement_path_members
            .span_or_empty(call.receiver)
            .to_vec();
        let mut receiver_members = receiver_members;
        // `owner.tests.group<Requirement>()` passes the Build's `tests` facet
        // as a value (wiki/spec/build/testing.md), so its receiver keeps the
        // member projection an ordinary `owner.tests` value has instead of the
        // static path a statement call's receiver otherwise becomes.
        let facet = (BuildOperation::from_call_target(call.target.as_str())
            == Some(BuildOperation::TestGroupRegistration)
            && receiver_members.len() >= 2
            && receiver_members
                .last()
                .is_some_and(|member| member.as_str() == "tests"))
        .then(|| receiver_members.pop())
        .flatten();
        let receiver_span = receiver_members.first().map(|member| member.source_span());
        let expression_table = &mut lowerer.symbol_resolved_trees.tables.bodies.expressions;
        let mut members = HandleSpan::empty();
        for member in receiver_members {
            expression_table.push_name_path_member(&mut members, member);
        }
        let receiver = if members.is_empty() {
            ExpressionHandle::invalid()
        } else {
            expression_table.insert(ExpressionNode::Name(TableNamePath {
                members,
                ..Default::default()
            }))
        };
        let (owner, receiver, facet_span) = match facet {
            Some(facet) => {
                let span = facet.source_span();
                let projected = expression_table.insert(ExpressionNode::Member(
                    symbol_resolved_trees::expression::TableMemberExpression {
                        receiver,
                        member_symbol: SymbolHandle::invalid(),
                        member: facet,
                        case_variant: None,
                    },
                ));
                (receiver, projected, Some(span))
            }
            None => (ExpressionHandle::invalid(), receiver, None),
        };
        let source_span = call.target.source_span();
        let expression = expression_table.insert(ExpressionNode::Call(TableCallExpression {
            authored_call_binding: None,
            receiver,
            target_symbol: call.target_symbol,
            target: call.target,
            requirement_application: arena::Handle::invalid(),
            machine_arguments: call.storage.machine_arguments,
            arguments: call.storage.arguments,
            evidence_arguments: call.storage.evidence_arguments,
            operational_acknowledgement: call.storage.operational_acknowledgement,
        }));
        let (owner_span, receiver_span) = if owner.is_valid() {
            (receiver_span, facet_span)
        } else {
            (None, receiver_span)
        };
        for (handle, span) in [
            (owner, owner_span),
            (receiver, receiver_span),
            (expression, Some(source_span)),
        ] {
            if !handle.is_valid() {
                continue;
            }
            if let Some(span) = span {
                expression_table.set_source_span(handle, span);
            }
            let exposure = AuthoredDeclarationSelectionExposure::PrivateImplementation;
            expression_table.set_authored_expression_exposure(handle, exposure);
            lowerer.pending_authored_expressions.push(
                crate::resolution::lowerer::PendingAuthoredExpression {
                    expression: handle,
                    exposure,
                },
            );
            if let Some(partition) = lowerer.current_compiler_selection_partition {
                expression_table.set_compiler_selection_partition(handle, partition);
            }
        }
        expression
    }

    fn evidence_forwarding_names<'syntax>(
        lowerer: &Lowerer,
        syntax_trees: &'syntax SyntaxTrees,
        target: syntax::expression::ExpressionHandle,
        source: syntax::expression::ExpressionHandle,
    ) -> Option<(&'syntax Identifier, &'syntax Identifier)> {
        let target = bare_syntax_name(syntax_trees, target)?;
        let source = bare_syntax_name(syntax_trees, source)?;
        (lowerer
            .current_evidence_term_names
            .iter()
            .any(|name| name == target.as_str(syntax_trees) || name == source.as_str(syntax_trees)))
        .then_some((target, source))
    }

    fn bare_syntax_name(
        syntax_trees: &SyntaxTrees,
        expression: syntax::expression::ExpressionHandle,
    ) -> Option<&Identifier> {
        let syntax::expression::ExpressionNode::Name(path) =
            syntax_trees.expressions.expression(expression)
        else {
            return None;
        };
        let [name] = syntax_trees.expressions.identifier_path_members(*path) else {
            return None;
        };
        Some(name)
    }

    pub(crate) fn set_expression(
        lowerer: &mut Lowerer,
        handle: ExpressionHandle,
        node: ExpressionNode,
    ) {
        *expression_table(lowerer).expression_mut(handle) = node;
    }

    pub(super) fn lower_statement_expression(
        lowerer: &mut Lowerer,
        syntax_trees: &SyntaxTrees,
        expression: syntax::expression::ExpressionHandle,
    ) -> Result<symbol_resolved_trees::expression::ExpressionHandle, Diagnostic> {
        lower_private_expression_into_table(lowerer, syntax_trees, expression)
    }

    fn lower_statement_expressions(
        lowerer: &mut Lowerer,
        syntax_trees: &SyntaxTrees,
        expressions: HandleSpan<syntax::expression::ExpressionHandle>,
    ) -> Result<HandleSpan<symbol_resolved_trees::expression::ExpressionHandle>, Diagnostic> {
        let span = expression_table(lowerer).reserve_expression_handles(expressions.count());

        for (offset, expression) in syntax_trees
            .statements
            .expression_handles(expressions)
            .iter()
            .enumerate()
        {
            let expression =
                lower_private_expression_into_table(lowerer, syntax_trees, *expression)?;
            expression_table(lowerer).set_expression_handle_at_offset(
                span,
                offset
                    .try_into()
                    .expect("expression handle span count overflow"),
                expression,
            );
        }

        Ok(span)
    }

    fn lower_transition_guard_node(
        lowerer: &mut Lowerer,
        syntax_trees: &SyntaxTrees,
        guard: TransitionGuardNode,
    ) -> Result<TransitionGuard, Diagnostic> {
        match guard {
            TransitionGuardNode::Always => Ok(TransitionGuard::Always),
            TransitionGuardNode::When(expression) => Ok(TransitionGuard::When(
                lower_statement_expression(lowerer, syntax_trees, expression)?,
            )),
        }
    }

    fn lower_transition_target_node(
        lowerer: &mut Lowerer,
        syntax_trees: &SyntaxTrees,
        target: TransitionTargetHandle,
        hoisted: &mut Vec<Statement>,
        unconditional: bool,
    ) -> Result<TransitionTarget, Diagnostic> {
        match syntax_trees.statements.transition_target(target) {
            TransitionTargetNode::Named {
                path,
                arguments,
                evidence_arguments,
                source_span,
            } => {
                let arguments = lower_statement_expressions(lowerer, syntax_trees, *arguments)?;
                for offset in 0..if unconditional { arguments.count() } else { 0 } {
                    let argument =
                        expression_table(lowerer).expression_handles(arguments)[offset as usize];
                    let rewritten = hoist_operand_indexed_reads(
                        lowerer,
                        argument,
                        hoisted,
                        OperandHoisting::Computation,
                    );
                    if rewritten != argument {
                        expression_table(lowerer)
                            .set_expression_handle_at_offset(arguments, offset, rewritten);
                    }
                }
                Ok(TransitionTarget::Named(NamedTransitionTarget {
                    receiver_symbol: Default::default(),
                    head_symbol: SymbolHandle::invalid(),
                    symbol: SymbolHandle::invalid(),
                    storage: NamedTransitionTargetStorage {
                        operational_acknowledgement: Default::default(),
                        path: lower_statement_path_members(lowerer, syntax_trees, *path),
                        arguments,
                        evidence_arguments: lower_names(syntax_trees, evidence_arguments)
                            .into_boxed_slice(),
                        source_span: *source_span,
                        authored_call_selection: None,
                    },
                }))
            }
            TransitionTargetNode::Value(expression) => {
                let expression = lower_statement_expression(lowerer, syntax_trees, *expression)?;
                let expression = if unconditional {
                    hoist_operand_indexed_reads(
                        lowerer,
                        expression,
                        hoisted,
                        OperandHoisting::Computation,
                    )
                } else {
                    expression
                };
                Ok(TransitionTarget::Value(expression))
            }
            TransitionTargetNode::Terminal => Ok(TransitionTarget::Terminal),
        }
    }

    fn lower_statement_path_members(
        lowerer: &mut Lowerer,
        syntax_trees: &SyntaxTrees,
        members: HandleSpan<Identifier>,
    ) -> HandleSpan<DiagnosticName> {
        let mut span = HandleSpan::empty();

        for member in syntax_trees.statements.identifier_path_members(members) {
            lowerer
                .symbol_resolved_trees
                .tables
                .declarations
                .statement_path_members
                .append_to_span(&mut span, lower_name(syntax_trees, member));
        }

        span
    }
}
mod value_call_hoisting {
    /* Capture a shared guard-call subject once without changing terminal invocation position. */

    use crate::lowering::expression::expression_table;
    use crate::lowering::statement::hoist_temps::mint_hoist_temp;
    use crate::lowering::statement::indexed_read_hoisting::is_integer_embedding_call;
    use crate::lowering::statement::statement_nodes::set_expression;
    use crate::resolution::lowerer::Lowerer;
    use symbol_resolved_trees::expression::{
        BinaryOperator, ExpressionHandle, ExpressionNode, TableBinaryExpression,
    };
    use symbol_resolved_trees::statement::Statement;
    use symbol_resolved_trees::types::TypeReference;
    use syntax_trees::{self as syntax, SyntaxTrees};

    /*
        Hoists a USER value-machine call out of a guard comparison:
        `self.next() == expected` becomes
        `let __hoist_N = self.next(); __hoist_N == expected`. Fires only when the
        guard ROOT is a comparison with one user Call side and one non-user-Call
        side. The
        temp's type is resolved from the callee's DECLARED return by the
        symbol-resolved -> typed lowering
        (`infer_hoist_temp_type`); an inferred-return callee gets a clear
        annotate-or-bind diagnostic there.
    */
    pub(crate) fn hoist_scalar_value_call_comparison(
        lowerer: &mut Lowerer,
        syntax_trees: &SyntaxTrees,
        syntax_guard: syntax::statement::TransitionGuardNode,
        guard_expression: ExpressionHandle,
        hoisted: &mut Vec<Statement>,
    ) {
        let syntax::statement::TransitionGuardNode::When(mut syntax_cmp) = syntax_guard else {
            return;
        };
        let mut resolved_cmp = guard_expression;
        let binary = loop {
            let node = expression_table(lowerer).expression(resolved_cmp).clone();
            let ExpressionNode::Binary(binary) = node else {
                return;
            };
            if matches!(
                binary.operator,
                BinaryOperator::Equal | BinaryOperator::NotEqual
            ) && matches!(
                expression_table(lowerer).expression(binary.right),
                ExpressionNode::Boolean(_)
            ) {
                let syntax::expression::ExpressionNode::Binary(syntax_outer) =
                    syntax_trees.expressions.expression(syntax_cmp)
                else {
                    return;
                };
                resolved_cmp = binary.left;
                syntax_cmp = syntax_outer.left;
                continue;
            }
            break binary;
        };
        if !matches!(
            binary.operator,
            BinaryOperator::Equal
                | BinaryOperator::NotEqual
                | BinaryOperator::Greater
                | BinaryOperator::GreaterOrEqual
                | BinaryOperator::Less
                | BinaryOperator::LessOrEqual
        ) {
            return;
        }
        let expressions = &lowerer.symbol_resolved_trees.tables.bodies.expressions;
        let side_is_user_call = |handle: ExpressionHandle| match expressions.expression(handle) {
            ExpressionNode::Call(call) => !is_integer_embedding_call(lowerer, call),
            _ => false,
        };
        let call_is_left = side_is_user_call(binary.left) && !side_is_user_call(binary.right);
        let call_is_right = side_is_user_call(binary.right) && !side_is_user_call(binary.left);
        if !call_is_left && !call_is_right {
            return;
        }
        let other_side = if call_is_left {
            binary.right
        } else {
            binary.left
        };
        if expression_contains(lowerer, other_side, &|node| {
            matches!(node, ExpressionNode::Call(_))
        }) {
            return;
        }
        if call_is_right
            && !matches!(
                expressions.expression(binary.left),
                ExpressionNode::Boolean(_)
                    | ExpressionNode::Float(_)
                    | ExpressionNode::UndeterminedNaN(_)
                    | ExpressionNode::Integer(_)
            )
        {
            return;
        }
        let call_side = if call_is_left {
            binary.left
        } else {
            binary.right
        };

        let syntax_call = match syntax_trees.expressions.expression(syntax_cmp) {
            syntax::expression::ExpressionNode::Binary(syntax_binary) => {
                if call_is_left {
                    syntax_binary.left
                } else {
                    syntax_binary.right
                }
            }
            _ => syntax_cmp,
        };
        let subject_key = syntax_call;

        let name_reference = mint_hoist_temp(
            lowerer,
            hoisted,
            TypeReference::Unit,
            Some(subject_key),
            false,
            |_, _| call_side,
        );
        let rewritten = TableBinaryExpression {
            policy: binary.policy,
            left: if call_is_left {
                name_reference
            } else {
                binary.left
            },
            operator: binary.operator,
            right: if call_is_right {
                name_reference
            } else {
                binary.right
            },
        };
        set_expression(lowerer, resolved_cmp, ExpressionNode::Binary(rewritten));
    }

    /*
        Whether `expression` evaluates any node `selects`, including nodes inside
        match arms that may run. The walk covers every evaluated child.
    */
    pub(super) fn expression_contains(
        lowerer: &Lowerer,
        expression: ExpressionHandle,
        selects: &dyn Fn(&ExpressionNode) -> bool,
    ) -> bool {
        let expressions = &lowerer.symbol_resolved_trees.tables.bodies.expressions;
        let node = expressions.expression(expression);
        if selects(node) {
            return true;
        }
        let contains = |child| expression_contains(lowerer, child, selects);
        match node {
            ExpressionNode::Match(dispatch) => {
                contains(dispatch.subject)
                    || expressions.match_arms(dispatch.arms).iter().any(|arm| {
                        expressions
                            .match_pattern_dependencies(arm.pattern)
                            .any(contains)
                            || contains(arm.value)
                    })
            }
            ExpressionNode::SavedSubject(_) => false,
            ExpressionNode::Call(call) => {
                (call.receiver.is_valid() && contains(call.receiver))
                    || expressions
                        .expression_handles(call.arguments)
                        .iter()
                        .any(|argument| contains(*argument))
            }
            ExpressionNode::Atomic(atomic) => contains(atomic.value),
            ExpressionNode::ArrayLiteral(values) => expressions
                .expression_handles(*values)
                .iter()
                .any(|value| contains(*value)),
            ExpressionNode::Binary(binary) => contains(binary.left) || contains(binary.right),
            ExpressionNode::Cast(cast) => contains(cast.value),
            ExpressionNode::Indexed(indexed) => {
                contains(indexed.collection) || contains(indexed.index)
            }
            ExpressionNode::Member(member) => contains(member.receiver),
            ExpressionNode::Membership(membership) => contains(membership.value),
            ExpressionNode::Borrow(borrow) => contains(borrow.target),
            ExpressionNode::Range(range) => contains(range.start) || contains(range.end),
            ExpressionNode::StructLiteral(literal) => expressions
                .struct_fields(literal.fields)
                .iter()
                .any(|field| contains(field.value)),
            ExpressionNode::Unary(unary) => contains(unary.operand),
            ExpressionNode::Boolean(_)
            | ExpressionNode::Float(_)
            | ExpressionNode::UndeterminedNaN(_)
            | ExpressionNode::Integer(_)
            | ExpressionNode::Name(_)
            | ExpressionNode::String(_)
            | ExpressionNode::TypeExpression(_)
            | ExpressionNode::ZeroValue(_) => false,
        }
    }
}

use crate::lowering::statement::statement_nodes::lower_statement_node;
use crate::resolution::lowerer::Lowerer;
use diagnostics::Diagnostic;
use symbol_resolved_trees::statement::Statement;
use syntax_trees::{self as syntax, SyntaxTrees};

/*
    Lowers a syntax statement to one or more symbol-resolved statements.

    Most statements lower one-to-one. Assignments and local-data declarations
    can yield EXTRA statements first: every runtime-indexed read `arr[i]` used
    as a SUB-EXPRESSION OPERAND (a child of a binary/cast/etc., not the root of
    the value) is hoisted into a synthetic `let __hoist_N = arr[i];` placed
    before the rewritten statement, and the operand is replaced with a name
    referencing that temp. The hoisted `let` is itself a root-level whole-value
    indexed read, which already lowers natively; the rewritten parent then only
    reads a plain local. The hoisted statements come first so later passes seed
    and resolve the temps' symbols (locals are bound by ORDER + NAME).
*/
pub(crate) fn lower_statement_handle(
    lowerer: &mut Lowerer,
    syntax_trees: &SyntaxTrees,
    statement: syntax::statement::StatementHandle,
    statement_index: usize,
    has_preceding_transition: bool,
) -> Result<Vec<Statement>, Diagnostic> {
    lower_statement_node(
        lowerer,
        syntax_trees,
        syntax_trees.statements.statement(statement),
        statement_index,
        has_preceding_transition,
    )
}
