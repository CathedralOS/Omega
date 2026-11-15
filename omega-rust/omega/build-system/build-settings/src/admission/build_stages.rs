/*
Divide a Build entry into the stages its dependency preparations mark
(`select_build_stage`).

A Build may call code from a build dependency only after it prepared that
dependency (`wiki/spec/build/scoped_execution.md`, "Checked Build stages"):
"Execute only a completely checked and admitted prefix", and "After
preparation, the compiler resolves and checks the dependent continuation
before it executes." The names a continuation uses do not exist before the
preparation, so a Build entry cannot be resolved and checked whole. It is
checked in parts.

The parts are read off the entry's own statements. Every top-level statement
`suspend Build::prepare_dependency(builder, registered);` ends one stage, and
the statements after it start the next:

```omega
machine build(builder: &mut Build)
suspends
{
    Build::application(builder, "app");                          // stage 0
    let registered: DependencyRegistration = ...;                // stage 0
    suspend Build::prepare_dependency(builder, registered);      // stage 0 ends
    let x: i32 = kit::v();                                       // stage 1
}
```

`select_build_stage` leaves the entry holding the statements of one stage, so
that stage is resolved, typed, lowered and judged as the complete Build entry
it then is. No stage is special to any of those owners. The entry's nested
machines and its closing `transition` stay with the last stage: they are
written after every statement and may use every prepared dependency.

A preparation that is not a top-level statement of the entry (inside a helper
or a nested machine) divides nothing. It prepares its dependency and code
checked before it gains no names, so availability holds "on every incoming
path where it is used": uses are only ever in a stage after a top-level
preparation.

What is not carried across a preparation. The specification asks that the
continuation "capture exact declarations, substitutions, source scope,
aliases, facts, loans and resource custody across the suspension". Only the
entry's parameters cross here. A later stage that uses a local an earlier
stage bound, and an earlier stage that names a nested machine of the entry,
are therefore refused as not supported. That is decided on the undivided
entry, before any stage is checked: an earlier stage that holds a value for a
later one (an undischarged `RequiredOutput`, say) would otherwise be rejected
by the checker for a reason that is not the program's.

The division is the same for every assembly of one source: it reads only the
parsed statement forms. Whether a dividing statement really calls the
toolchain's `Build::prepare_dependency` is settled where names are: the stage
runs, and the host continues from it only if it ended at a preparation the
host served (`TerminalBuildActivation::carry`).
*/

use diagnostics::Diagnostic;
use products::BuildStage;
use source::SourceId;
use syntax_trees::SyntaxTrees;
use syntax_trees::expression::{
    ExpressionHandle, ExpressionNode, MatchFieldAction, MatchPattern, TableDestructurePattern,
    TableMatchField,
};
use syntax_trees::identifier::Identifier;
use syntax_trees::item::{Item, ItemHandle, Machine, StateHandle};
use syntax_trees::statement::{
    StatementHandle, StatementNode, TransitionGuardNode, TransitionTargetHandle,
    TransitionTargetNode,
};

const BUILD_ENTRY: &str = "build";
const BUILD_TYPE: &str = "Build";
const PREPARATION: &str = "prepare_dependency";

/// Keeps, in the Build entry of `build_source`, the statements of stage
/// `stage` only, and reports how many stages the entry has. An entry with no
/// top-level preparation is left as it is. A source with no Build entry has
/// nothing to divide; entry selection reports that.
pub fn select_build_stage(
    syntax: &mut SyntaxTrees,
    build_source: SourceId,
    stage: usize,
) -> Result<BuildStage, Vec<Diagnostic>> {
    let Some((item, machine, state)) = entry(syntax, build_source) else {
        return Ok(BuildStage::WHOLE);
    };
    let span = syntax.items.state(state).statements;
    let statements = syntax.items.statements(span).to_vec();
    let ends = statements
        .iter()
        .enumerate()
        .filter(|(_, statement)| divides(syntax, **statement))
        .map(|(position, _)| position)
        .collect::<Vec<_>>();
    if ends.is_empty() {
        return if stage == 0 {
            Ok(BuildStage::WHOLE)
        } else {
            Err(vec![Diagnostic::error(
                "Build entry has no dependency preparation to continue after",
            )])
        };
    }
    let selected = BuildStage {
        index: stage,
        count: ends.len() + 1,
    };
    if stage >= selected.count {
        return Err(vec![Diagnostic::error(
            "Build entry has no stage after its last dependency preparation",
        )]);
    }
    refuse_uncarried_names(syntax, &machine, &statements, &ends)?;
    let first = match stage {
        0 => 0,
        stage => ends[stage - 1] + 1,
    };
    let end = ends.get(stage).map_or(statements.len(), |end| end + 1);
    let offset = u32::try_from(first).expect("a statement span is indexed by u32");
    let count = u32::try_from(end - first).expect("a statement span is counted by u32");
    syntax.items.state_mut(state).statements =
        arena::HandleSpan::from_parts_or_empty(span.handle_at(offset), count);
    if !selected.is_last() {
        let mut declaration = machine;
        declaration.nested_machines = arena::HandleSpan::empty();
        syntax.items.replace_item(item, Item::Machine(declaration));
    }
    Ok(selected)
}

/// The Build entry of `build_source` with the state that holds its
/// statements.
fn entry(
    syntax: &SyntaxTrees,
    build_source: SourceId,
) -> Option<(ItemHandle, Machine, StateHandle)> {
    syntax.root_item_handles().iter().find_map(|handle| {
        let machine = syntax.root_item(*handle).as_machine()?;
        if machine.attached_data.is_some()
            || machine.name.as_str(syntax) != BUILD_ENTRY
            || machine.name.source_span().source_id != build_source
        {
            return None;
        }
        let state = *syntax.items.state_handles(machine.states).first()?;
        Some((*handle, machine.clone(), state))
    })
}

/// Whether `statement` is `suspend Build::prepare_dependency(...);`.
fn divides(syntax: &SyntaxTrees, statement: StatementHandle) -> bool {
    match syntax.statements.statement(statement) {
        StatementNode::Call(call) => {
            call.operational_acknowledgement.acknowledges_suspend
                && call.target.as_str(syntax) == PREPARATION
                && matches!(
                    syntax.statements.identifier_path_members(call.receiver),
                    [owner] if owner.as_str(syntax) == BUILD_TYPE
                )
        }
        StatementNode::Expression(expression) => {
            let ExpressionNode::Call(call) = syntax.expressions.expression(*expression) else {
                return false;
            };
            call.operational_acknowledgement.acknowledges_suspend
                && call.target.as_str(syntax) == PREPARATION
                && call.receiver.is_valid()
                && matches!(
                    syntax.expressions.expression(call.receiver),
                    ExpressionNode::Name { members, .. } if matches!(
                        syntax.expressions.identifier_path_members(*members),
                        [owner] if owner.as_str(syntax) == BUILD_TYPE
                    )
                )
        }
        _ => false,
    }
}

/// Refuses the two uses nothing carries across a preparation: a local of an
/// earlier stage used in a later one, and a nested machine of the entry
/// named before the last stage.
fn refuse_uncarried_names(
    syntax: &SyntaxTrees,
    machine: &Machine,
    statements: &[StatementHandle],
    ends: &[usize],
) -> Result<(), Vec<Diagnostic>> {
    let nested = machine
        .nested_machines
        .handles()
        .filter_map(|handle| syntax.items.item(handle).as_machine())
        .map(|machine| machine.name.as_str(syntax))
        .collect::<Vec<_>>();
    let last_end = *ends.last().expect("a divided entry has a preparation");
    // Locals bound by stages already passed, and by the stage being walked.
    let mut earlier: Vec<&str> = Vec::new();
    let mut current: Vec<&str> = Vec::new();
    let mut refusals = Vec::new();
    for (position, statement) in statements.iter().enumerate() {
        let node = syntax.statements.statement(*statement);
        let mut uses = Uses {
            syntax,
            names: Vec::new(),
            machines: Vec::new(),
        };
        uses.statement(node);
        for name in uses.names {
            let text = name.as_str(syntax);
            if !current.contains(&text) && earlier.contains(&text) {
                refusals.push(
                    Diagnostic::error(format!(
                        "not supported yet: a Build statement after `Build::prepare_dependency` that uses `{text}`, a local bound before that preparation; only the entry's parameters are carried across a preparation, so nothing is claimed about this program"
                    ))
                    .with_source_span(name.source_span()),
                );
            }
        }
        if position <= last_end {
            for name in uses.machines {
                let text = name.as_str(syntax);
                if nested.contains(&text) {
                    refusals.push(
                        Diagnostic::error(format!(
                            "not supported yet: a Build statement before `Build::prepare_dependency` that names `{text}`, a nested machine of the Build entry; the entry's nested machines are checked with the statements after its last preparation, so nothing is claimed about this program"
                        ))
                        .with_source_span(name.source_span()),
                    );
                }
            }
        }
        bound(syntax, node, &mut current);
        if ends.contains(&position) {
            earlier.append(&mut current);
        }
    }
    if refusals.is_empty() {
        Ok(())
    } else {
        Err(refusals)
    }
}

/// The locals `statement` binds for the statements after it.
fn bound<'syntax>(
    syntax: &'syntax SyntaxTrees,
    statement: &'syntax StatementNode,
    names: &mut Vec<&'syntax str>,
) {
    match statement {
        StatementNode::LocalData(local) => {
            names.push(local.name.as_str(syntax));
            for field in syntax.expressions.match_fields(local.pattern_fields) {
                if let MatchFieldAction::Bind(binding) = &field.action {
                    names.push(binding.as_str(syntax));
                }
            }
        }
        StatementNode::ProofOutputBindingStatement(binding) => {
            for selector in binding.bindings.iter() {
                names.push(selector.binding.as_str(syntax));
            }
        }
        _ => {}
    }
}

/// The value names and callee names one statement reads. A name is recorded
/// by its first path member: `registered`, or `request` of `request.alias`.
struct Uses<'syntax> {
    syntax: &'syntax SyntaxTrees,
    names: Vec<&'syntax Identifier>,
    machines: Vec<&'syntax Identifier>,
}

impl<'syntax> Uses<'syntax> {
    fn statement(&mut self, statement: &'syntax StatementNode) {
        let syntax = self.syntax;
        match statement {
            // Assembly belongs to target code, which a Build entry is not;
            // its operands are still expressions of this statement.
            StatementNode::Assembly(handle) => {
                let graph = syntax.statements.assembly(*handle);
                for block in syntax.statements.assembly_blocks(graph.blocks) {
                    self.all(syntax.statements.assembly_expressions(block.requires));
                    self.all(syntax.statements.assembly_expressions(block.ensures));
                    for instruction in syntax.statements.assembly_instructions(block.instructions) {
                        self.statement(syntax.statements.statement(*instruction));
                    }
                }
                self.all(syntax.statements.assembly_expressions(graph.primitives));
                self.all(&graph.scratch);
            }
            StatementNode::RootBinding(binding) => {
                self.expression(binding.receiver);
                self.expression(binding.implementation_operand);
            }
            StatementNode::AssemblyFact(fact) => self.expression(fact.expression),
            StatementNode::Assignment(assignment) => {
                self.expression(assignment.target);
                self.expression(assignment.value);
            }
            StatementNode::Call(call) => {
                if syntax
                    .statements
                    .identifier_path_members(call.receiver)
                    .is_empty()
                {
                    self.machines.push(&call.target);
                }
                self.all(syntax.statements.expression_handles(call.arguments));
            }
            StatementNode::ProofOutputBindingStatement(binding) => self.expression(binding.call),
            StatementNode::Expression(value) => self.expression(*value),
            StatementNode::LocalData(local) => {
                self.expression(local.initial_value);
                self.fields(syntax.expressions.match_fields(local.pattern_fields));
            }
            StatementNode::Transition(transition) => {
                for pattern in syntax.statements.transition_patterns(transition.patterns) {
                    self.expression(pattern.subject);
                    self.capture(pattern.capture);
                    self.pattern(&pattern.pattern);
                }
                self.target(transition.target);
                self.target(transition.continuation);
                if let TransitionGuardNode::When(guard) = transition.guard {
                    self.expression(guard);
                }
            }
        }
    }

    fn target(&mut self, target: TransitionTargetHandle) {
        if !target.is_valid() {
            return;
        }
        let syntax = self.syntax;
        match syntax.statements.transition_target(target) {
            TransitionTargetNode::Named {
                path, arguments, ..
            } => {
                if let [machine] = syntax.statements.identifier_path_members(*path) {
                    self.machines.push(machine);
                }
                self.all(syntax.statements.expression_handles(*arguments));
            }
            TransitionTargetNode::Value(value) => self.expression(*value),
            TransitionTargetNode::Terminal => {}
        }
    }

    fn all(&mut self, expressions: &[ExpressionHandle]) {
        for expression in expressions {
            self.expression(*expression);
        }
    }

    fn capture(&mut self, capture: arena::Handle<syntax_trees::expression::MatchSubjectCapture>) {
        if capture.is_valid() {
            self.expression(self.syntax.expressions.match_subject(capture).subject);
        }
    }

    fn fields(&mut self, fields: &[TableMatchField]) {
        for field in fields {
            if let MatchFieldAction::Equal(value) = &field.action {
                self.expression(*value);
            }
        }
    }

    fn pattern(&mut self, pattern: &TableDestructurePattern) {
        self.expression(pattern.head);
        self.fields(self.syntax.expressions.match_fields(pattern.fields));
        self.expression(pattern.guard);
    }

    fn expression(&mut self, handle: ExpressionHandle) {
        if !handle.is_valid() {
            return;
        }
        let syntax = self.syntax;
        match syntax.expressions.expression(handle) {
            ExpressionNode::Match(dispatch) => {
                self.expression(dispatch.subject);
                self.capture(dispatch.capture);
                for arm in syntax.expressions.match_arms(dispatch.arms) {
                    match &arm.pattern {
                        MatchPattern::Value(value) => self.expression(*value),
                        MatchPattern::Wildcard => {}
                        MatchPattern::Destructure(pattern) => self.pattern(pattern),
                    }
                    self.expression(arm.value);
                }
            }
            ExpressionNode::SavedSubject(saved) => {
                self.capture(saved.capture);
                self.expression(saved.pattern_head);
            }
            ExpressionNode::ArrayLiteral(values) => {
                self.all(syntax.expressions.expression_handles(*values));
            }
            ExpressionNode::Atomic(atomic) => {
                self.expression(atomic.value);
                self.expression(atomic.result);
            }
            ExpressionNode::Binary(binary) => {
                self.expression(binary.left);
                self.expression(binary.right);
            }
            ExpressionNode::Cast(cast) => self.expression(cast.value),
            ExpressionNode::Call(call) => {
                if call.receiver.is_valid() {
                    self.expression(call.receiver);
                } else {
                    self.machines.push(&call.target);
                }
                self.all(syntax.expressions.expression_handles(call.arguments));
            }
            ExpressionNode::Indexed(indexed) => {
                self.expression(indexed.collection);
                self.expression(indexed.index);
            }
            ExpressionNode::Membership(membership) => self.expression(membership.value),
            ExpressionNode::Member(member) => self.expression(member.receiver),
            ExpressionNode::Borrow(borrow) => self.expression(borrow.target),
            ExpressionNode::Name { members, .. } => {
                if let Some(first) = syntax.expressions.identifier_path_members(*members).first() {
                    self.names.push(first);
                }
            }
            ExpressionNode::Range(range) => {
                self.expression(range.start);
                self.expression(range.end);
            }
            ExpressionNode::StructLiteral(literal) => {
                for field in syntax.expressions.struct_fields(literal.fields) {
                    self.expression(field.value);
                }
            }
            ExpressionNode::Unary(unary) => self.expression(unary.operand),
            ExpressionNode::Boolean(_)
            | ExpressionNode::Float(_)
            | ExpressionNode::UndeterminedNaN(_)
            | ExpressionNode::Integer(_)
            | ExpressionNode::String(_)
            | ExpressionNode::TypeExpression(_)
            | ExpressionNode::ZeroValue(_) => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SOURCE: SourceId = SourceId(0);

    fn parse(text: &str) -> SyntaxTrees {
        let tokens = source_files_to_tokens::Lexer::new(text).tokenize().unwrap();
        tokens_to_syntax_trees::parse_syntax_trees_with_id(SOURCE, &tokens).unwrap()
    }

    /// The stage `stage` of `text`: how many stages the entry has, how many
    /// statements this one keeps, and how many nested machines.
    fn stage(text: &str, stage: usize) -> Result<(usize, usize, usize), String> {
        let mut syntax = parse(text);
        let selected = select_build_stage(&mut syntax, SOURCE, stage).map_err(|refusals| {
            refusals
                .into_iter()
                .map(|refusal| refusal.message)
                .collect::<Vec<_>>()
                .join("\n")
        })?;
        let (_, machine, state) = entry(&syntax, SOURCE).unwrap();
        Ok((
            selected.count,
            syntax.items.state(state).statements.len(),
            machine.nested_machines.len(),
        ))
    }

    const STAGED: &str = "
machine build(builder: &mut Build) suspends {
    Build::application(builder, \"app\");
    let first: DependencyRegistration = Build::build_dependency(builder, one());
    suspend Build::prepare_dependency(builder, first);
    let second: DependencyRegistration = Build::build_dependency(builder, two());
    suspend Build::prepare_dependency(builder, second);
    let seven: i32 = kit::v();
    transition seven == 7 {
        true -> done(builder)
        _ -> wrong()
    }
    machine done(builder: &mut Build) { }
    machine wrong() { }
}
";

    /// Each top-level wait ends a stage. The nested machines and the closing
    /// dispatch stay with the last one: its `let` and the three statements
    /// the two-armed dispatch is stored as.
    #[test]
    fn an_entry_is_divided_at_each_top_level_preparation() {
        assert_eq!(stage(STAGED, 0), Ok((3, 3, 0)));
        assert_eq!(stage(STAGED, 1), Ok((3, 2, 0)));
        assert_eq!(stage(STAGED, 2), Ok((3, 4, 2)));
        assert!(stage(STAGED, 3).is_err());
    }

    /// The twins: an entry with no wait, and one whose only wait is inside a
    /// nested machine, are one stage and are left whole.
    #[test]
    fn an_entry_with_no_top_level_preparation_is_one_stage() {
        let plain = "
machine build(builder: &mut Build) {
    Build::application(builder, \"app\");
    let seven: i32 = 7;
}
";
        assert_eq!(stage(plain, 0), Ok((1, 2, 0)));
        assert!(stage(plain, 1).is_err());
        let nested = "
machine build(builder: &mut Build) suspends {
    let registered: DependencyRegistration = Build::build_dependency(builder, one());
    suspend wait(builder, registered);
    machine wait(builder: &mut Build, registered: DependencyRegistration) suspends {
        suspend Build::prepare_dependency(builder, registered);
    }
}
";
        assert_eq!(stage(nested, 0), Ok((1, 2, 1)));
    }

    /// Only the entry's parameters cross a wait. A statement after it that
    /// uses a local bound before it is refused by that local's name, whether
    /// it reads the local or a field of it.
    #[test]
    fn a_local_bound_before_a_preparation_is_not_carried_across_it() {
        for used in ["before", "before.alias", "pick(&before)"] {
            let text = format!(
                "
machine build(builder: &mut Build) suspends {{
    let before: DependencyDescription = one();
    let registered: DependencyRegistration = Build::build_dependency(builder, two());
    suspend Build::prepare_dependency(builder, registered);
    let after: i32 = {used};
}}
"
            );
            let refusal = stage(&text, 0).unwrap_err();
            assert!(
                refusal.starts_with("not supported yet:") && refusal.contains("`before`"),
                "{refusal}"
            );
        }
        // The twin: a later stage that binds the same name again uses its
        // own local, and the parameter is carried.
        let rebound = "
machine build(builder: &mut Build) suspends {
    let value: i32 = 1;
    let registered: DependencyRegistration = Build::build_dependency(builder, one());
    suspend Build::prepare_dependency(builder, registered);
    let value: i32 = 2;
    let after: i32 = value;
    Build::application(builder, \"app\");
}
";
        assert_eq!(stage(rebound, 1), Ok((2, 3, 0)));
    }

    /// The entry's nested machines are checked with the last stage, so an
    /// earlier stage cannot name one.
    #[test]
    fn a_stage_before_the_last_cannot_name_a_nested_machine() {
        let text = "
machine build(builder: &mut Build) suspends {
    helper(builder);
    let registered: DependencyRegistration = Build::build_dependency(builder, one());
    suspend Build::prepare_dependency(builder, registered);
    helper(builder);
    machine helper(builder: &mut Build) { }
}
";
        let refusal = stage(text, 0).unwrap_err();
        assert!(
            refusal.starts_with("not supported yet:") && refusal.contains("`helper`"),
            "{refusal}"
        );
        assert_eq!(
            refusal.lines().count(),
            1,
            "the call after the wait is fine"
        );
    }
}
