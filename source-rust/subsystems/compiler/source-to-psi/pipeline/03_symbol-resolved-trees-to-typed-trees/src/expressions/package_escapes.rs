//! A package can cross its opening's boundary. A value whose public type
//! still refers to that opening cannot, including a reference or callable.
use super::package_openings::{bound_subject, package_place, write_replaces};
use crate::lowerer::TypingCohort;
use crate::source_resolved::SymbolResolvedTrees;
use crate::source_state as typed;
use crate::types::typing_queries::scoped_expression_type_reference;
use diagnostics::Diagnostic;
use symbols::SymbolHandle;
use typed::expression::{ExpressionHandle, ExpressionNode};
use typed::package_openings::{PackageSubject, PackageSubjectRoot};
use typed::state::State;
use typed::statement::{StatementNode, StatementRootRole};
use typed::types::{TypeReferenceHandle, TypeReferenceNode};
use typed::{SubtreeEdges, TypedTrees};

pub(crate) fn check_cohort(
    source: &SymbolResolvedTrees,
    program: &TypedTrees,
    cohort: &TypingCohort<'_>,
) -> Result<(), Diagnostic> {
    let named = &source
        .tables
        .declarations
        .child_type_references
        .package_openings;
    if named.is_empty() && program.package_openings.is_empty() {
        return Ok(());
    }
    for (ordinal, machine) in program.machines().iter().enumerate() {
        if !cohort.owns_machine(ordinal, machine.symbol) {
            continue;
        }
        for state in program.machine_states(machine) {
            let mut witnesses = Vec::new();
            for (_, opening) in named
                .iter()
                .filter(|(_, opening)| opening.symbol.is_valid())
            {
                let subject = program.symbols.get(opening.symbol).parent;
                if program.symbols.get(subject).parent == state.symbol {
                    witnesses.push((opening.symbol, bound_subject(subject)));
                }
            }
            for (_, opening) in program
                .package_openings
                .iter()
                .filter(|(_, o)| o.state == state.symbol)
            {
                for &(_, ty) in &opening.witnesses {
                    let TypeReferenceNode::Named { symbol, .. } =
                        program.type_reference_table.type_reference(ty)
                    else {
                        return Err(Diagnostic::error(
                            "a package opening lost the identity of its abstract type",
                        ));
                    };
                    if !witnesses
                        .iter()
                        .any(|(witness, subject)| witness == symbol && subject == &opening.subject)
                    {
                        witnesses.push((*symbol, opening.subject.clone()));
                    }
                }
            }
            for (witness, subject) in witnesses {
                let mut bindings = WitnessBindings::default();
                for parameter in program.state_parameters(state) {
                    if program.type_reference_mentions_any(parameter.type_reference, &[witness]) {
                        bindings.record(parameter.symbol, false);
                    }
                }
                let mut scope = Scope {
                    program,
                    state,
                    witness,
                    subject,
                    bindings,
                };
                if scope.mentions(state.return_type) {
                    return Err(Diagnostic::error(
                        "the declared result exposes a witness outside its package opening",
                    )
                    .with_source_span(
                        program
                            .symbols
                            .symbol_provenance_source_span(witness)
                            .unwrap_or_default(),
                    ));
                }
                scope.statements(state.statement_nodes)?;
                if state.return_type.is_valid()
                    && let Some(StatementNode::Expression(value)) = program
                        .statement_table
                        .statements(state.statement_nodes)
                        .last()
                    && scope.carries(*value)?
                {
                    return Err(scope.refusal(
                        "the returned value exposes a witness outside its package opening",
                        *value,
                    ));
                }
            }
        }
    }
    Ok(())
}

struct Scope<'program> {
    program: &'program TypedTrees,
    state: &'program State,
    witness: SymbolHandle,
    subject: PackageSubject,
    bindings: WitnessBindings,
}

/// Only locals and parameters carrying this opening's witness need a row.
/// Keep full symbol handles so reusing an arena slot cannot revive a binding.
#[derive(Clone, Default)]
struct WitnessBindings(Vec<WitnessBinding>);

#[derive(Clone, Copy)]
struct WitnessBinding {
    symbol: SymbolHandle,
    stale: bool,
}

impl WitnessBindings {
    fn position(&self, symbol: SymbolHandle) -> Result<usize, usize> {
        self.0
            .binary_search_by_key(&(symbol.arena_index(), symbol.generation()), |row| {
                (row.symbol.arena_index(), row.symbol.generation())
            })
    }

    fn get(&self, symbol: SymbolHandle) -> Option<&WitnessBinding> {
        self.position(symbol).ok().map(|index| &self.0[index])
    }

    fn record(&mut self, symbol: SymbolHandle, stale: bool) {
        match self.position(symbol) {
            Ok(index) => self.0[index].stale |= stale,
            Err(index) => self.0.insert(index, WitnessBinding { symbol, stale }),
        }
    }

    fn invalidate(&mut self) {
        for binding in &mut self.0 {
            binding.stale = true;
        }
    }

    fn merge(&mut self, other: Self) {
        for binding in other.0 {
            self.record(binding.symbol, binding.stale);
        }
    }
}

impl Scope<'_> {
    fn mentions(&self, reference: TypeReferenceHandle) -> bool {
        reference.is_valid()
            && self
                .program
                .type_reference_mentions_any(reference, &[self.witness])
    }

    /// Check the type of the value that crosses the boundary. A package
    /// construction retains its abstract field types as private evidence only.
    fn carries(&self, value: ExpressionHandle) -> Result<bool, Diagnostic> {
        let program = self.program;
        if !program.expression_table.expression_is_valid(value) {
            return Ok(false);
        }
        if let Some(projection) = program.package_projection(value) {
            return Ok(self.mentions(projection.result_type));
        }
        let constructed = program
            .expression_table
            .constructed_value_type_reference(value);
        if self.mentions(constructed) {
            return Ok(true);
        }
        let node = program.expression_table.expression(value);
        if let ExpressionNode::Call(call) = node
            && let Some(binding) = call.runtime_call
        {
            return Ok(self.mentions(
                binding
                    .signature(program)
                    .map_err(Diagnostic::error)?
                    .return_type,
            ));
        }
        match node {
            ExpressionNode::Name(path)
                if path
                    .resolved_symbol()
                    .is_some_and(|s| self.bindings.get(s).is_some()) =>
            {
                return Ok(true);
            }
            ExpressionNode::Block(block) => return self.carries(block.tail),
            ExpressionNode::Match(dispatch) => {
                for arm in program.expression_table.match_arms(dispatch.arms) {
                    if self.carries(arm.value)? {
                        return Ok(true);
                    }
                }
                return Ok(false);
            }
            ExpressionNode::ArrayLiteral(elements) => {
                for &element in program.expression_table.expression_handles(*elements) {
                    if self.carries(element)? {
                        return Ok(true);
                    }
                }
                return Ok(false);
            }
            ExpressionNode::StructLiteral(literal) => {
                return Ok(self.mentions(literal.application_type));
            }
            ExpressionNode::Borrow(borrow) => return self.carries(borrow.target),
            _ => {}
        }
        let ty = scoped_expression_type_reference(
            program,
            program.state_parameters(self.state),
            self.state.return_type,
            value,
            Some(self.state),
        )?;
        Ok(ty.is_some_and(|ty| self.mentions(ty)))
    }

    /// Every path that supplies a replacement field must carry the same
    /// witness, including paths whose value is a concrete type.
    fn preserves(&self, value: ExpressionHandle) -> Result<bool, Diagnostic> {
        if !self.program.expression_table.expression_is_valid(value) {
            return Ok(false);
        }
        match self.program.expression_table.expression(value) {
            ExpressionNode::Block(block) => self.preserves(block.tail),
            ExpressionNode::Match(dispatch) => {
                let arms = self.program.expression_table.match_arms(dispatch.arms);
                for arm in arms {
                    if !self.preserves(arm.value)? {
                        return Ok(false);
                    }
                }
                Ok(!arms.is_empty())
            }
            _ => self.carries(value),
        }
    }

    fn refusal(&self, message: &'static str, value: ExpressionHandle) -> Diagnostic {
        Diagnostic::error(message)
            .with_source_span(self.program.expression_table.source_span(value))
    }

    fn statements(
        &mut self,
        statements: arena::HandleSpan<StatementNode>,
    ) -> Result<(), Diagnostic> {
        let program = self.program;
        for handle in statements.handles() {
            let statement = program.statement_table.statement(handle);
            for (role, root) in program.expression_roots(statement) {
                self.blocks(root)?;
                if role == StatementRootRole::TransitionValue && self.carries(root)? {
                    return Err(self.refusal(
                        "the returned value exposes a witness outside its package opening",
                        root,
                    ));
                }
            }
            match statement {
                StatementNode::LocalData(local)
                    if local.initial_value.is_valid() && self.carries(local.initial_value)? =>
                {
                    if !local.type_is_inferred && !self.mentions(local.type_reference) {
                        return Err(self.refusal(
                            "the local type cannot erase the witness of its initializer",
                            local.initial_value,
                        ));
                    }
                    self.bindings.record(local.symbol, false);
                }
                StatementNode::Assignment(assignment) => {
                    let source = self.carries(assignment.value)?;
                    let projected = program.package_projection(assignment.target);
                    let destination =
                        (source || projected.is_some()) && self.carries(assignment.target)?;
                    if source && !destination {
                        return Err(self.refusal("the assignment would store an opened package witness outside its receiving type", assignment.value));
                    }
                    if destination && projected.is_some() && !self.preserves(assignment.value)? {
                        return Err(self.refusal(
                            "a dependent field update must preserve the package's current witness",
                            assignment.value,
                        ));
                    }
                    if source
                        && let ExpressionNode::Name(path) =
                            program.expression_table.expression(assignment.target)
                        && let Some(symbol) = path.resolved_symbol()
                    {
                        self.bindings.record(symbol, false);
                    }
                    let written = package_place(program, assignment.target);
                    if written.as_ref().is_none_or(|place| {
                        write_replaces(program, self.state, place, &self.subject)
                    }) {
                        self.bindings.invalidate();
                    }
                }
                _ => {}
            }
        }
        Ok(())
    }

    fn blocks(&mut self, expression: ExpressionHandle) -> Result<(), Diagnostic> {
        let program = self.program;
        if !program.expression_table.expression_is_valid(expression) {
            return Ok(());
        }
        let node = program.expression_table.expression(expression);
        if let ExpressionNode::Name(path) = node
            && path
                .resolved_symbol()
                .is_some_and(|s| self.bindings.get(s).is_some_and(|binding| binding.stale))
        {
            return Err(self.refusal(
                "replacing the whole package invalidates a live dependent projection",
                expression,
            ));
        }
        if let ExpressionNode::Match(dispatch) = node {
            self.blocks(dispatch.subject)?;
            // A replacement in one arm affects code after the dispatch, but it
            // cannot invalidate a local while a mutually exclusive arm runs.
            let arms = program.expression_table.match_arms(dispatch.arms);
            let mut after = self.bindings.clone();
            for arm in arms {
                for operand in program.expression_table.match_pattern_operands(arm.pattern) {
                    self.blocks(operand)?;
                }
                let before = self.bindings.clone();
                self.blocks(arm.value)?;
                after.merge(std::mem::replace(&mut self.bindings, before));
            }
            self.bindings = after;
        } else if let ExpressionNode::Block(block) = node {
            self.statements(block.statements)?;
            self.blocks(block.tail)?;
            if let PackageSubjectRoot::Binding(subject) = self.subject.root
                && block.statements.handles().any(|h| {
                    program
                        .statement_table
                        .statement(h)
                        .as_local_data()
                        .is_some_and(|local| local.symbol == subject)
                })
                && self.carries(block.tail)?
            {
                return Err(self.refusal(
                    "the block result exposes a witness of a local package opening",
                    block.tail,
                ));
            }
        } else {
            let mut children = Vec::new();
            program.subtree_children(node, SubtreeEdges::VALUES, |child| {
                children.push(child);
                false
            });
            for child in children {
                self.blocks(child)?;
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use crate::qualified_operator_home_tests::checked;
    use crate::source_state::expression::ExpressionNode;

    const PAIR: &str = "data Pair<'a, some S> { first: @'a S; second: @'a S; tag: u8; }";

    #[test]
    fn independent_writes_and_same_witness_updates_keep_one_opening() {
        let source = format!(
            "{PAIR}
            machine update(p: @mutable Pair<some T>, other: @mutable Pair, next: Pair) {{
                let held: @T = p.first;
                p.tag = 1u8;
                $other = next;
                p.first = p.second;
                let retained: @T = held;
                let current: @T = p.first;
            }}"
        );
        let program = checked(&[("main.omg", &source)]).unwrap();
        assert_eq!(program.package_openings.len(), 1);
        assert_eq!(program.package_projections.len(), 4);
    }

    #[test]
    fn whole_replacement_can_change_a_dead_witness_but_cannot_invalidate_a_live_one() {
        let source = format!(
            "{PAIR}
            machine replace(p: @mutable Pair<some T>, a: @u8, b: @u16) {{
                let held: @T = p.first;
                $p = Pair {{ first: a, second: a, tag: 1u8 }};
                p.first;
                $p = Pair {{ first: b, second: b, tag: 2u8 }};
                p.second;
            }}"
        );
        let program = checked(&[("main.omg", &source)]).unwrap();
        let versions: std::collections::HashSet<_> = program
            .package_openings
            .iter()
            .map(|(_, opening)| opening.storage_version)
            .collect();
        assert_eq!(versions.len(), 3);
        let wrong = source.replace("p.second;", "let stale: @T = held;");
        let error = checked(&[("main.omg", &wrong)]).unwrap_err();
        assert!(
            error.contains("invalidates a live dependent projection"),
            "{error}"
        );
    }

    #[test]
    fn one_field_cannot_introduce_a_different_or_concrete_witness() {
        for value in [
            "q.second",
            "concrete",
            "match 0u8 { 0 -> p.second, _ -> concrete, }",
        ] {
            let source = format!(
                "{PAIR}
                machine overwrite(p: @mutable Pair, q: @Pair, concrete: @u8) {{
                    p.first = {value};
                }}"
            );
            let error = checked(&[("main.omg", &source)]).unwrap_err();
            assert!(
                error.contains("must preserve the package's current witness")
                    || error.contains("assignment would store an opened package witness"),
                "{error}"
            );
        }
    }

    #[test]
    fn replacement_in_one_arm_only_expires_a_loan_on_that_path_and_at_the_join() {
        let source = format!(
            "{PAIR}
            machine choose(p: @mutable Pair<some T>, next: Pair, flag: bool) {{
                let held: @T = p.first;
                let tag: u8 = match flag {{
                    true -> {{ $p = next; 1u8 }},
                    false -> {{ let saved: @T = held; let fresh: @T = p.first; 2u8 }},
                }};
            }}"
        );
        checked(&[("main.omg", &source)]).unwrap();
        let wrong = source.replace("};", "}; let stale: @T = held;");
        let error = checked(&[("main.omg", &wrong)]).unwrap_err();
        assert!(
            error.contains("invalidates a live dependent projection"),
            "{error}"
        );
    }

    #[test]
    fn replacement_through_a_local_alias_sees_the_same_live_projection() {
        let source = format!(
            "{PAIR}
            machine update(p: @mutable Pair<some T>, next: Pair) {{
                let alias: @mutable Pair = p;
                let held: @T = p.first;
                $alias = next;
                let stale: @T = held;
            }}"
        );
        let error = checked(&[("main.omg", &source)]).unwrap_err();
        assert!(
            error.contains("invalidates a live dependent projection"),
            "{error}"
        );
    }

    #[test]
    fn a_hidden_reference_cannot_be_returned_or_stored_as_an_unrelated_type() {
        for (body, expected) in [
            (
                "machine leak(p: @Pair<some T>) -> @T { p.first }",
                "declared result exposes a witness",
            ),
            (
                "machine leak(p: @Pair) -> @u8 { p.first }",
                "returned value exposes a witness",
            ),
            (
                "data Slot<'a> { saved: @'a u8; } machine leak(p: @Pair, out: @mutable Slot) { out.saved = p.first; }",
                "assignment would store an opened package witness",
            ),
        ] {
            let source = format!("{PAIR} {body}");
            let error = checked(&[("main.omg", &source)]).unwrap_err();
            assert!(error.contains(expected), "{body}: {error}");
        }
    }

    #[test]
    fn a_block_scoped_witness_cannot_flow_to_an_outer_local() {
        let source = format!(
            "{PAIR}
            machine leak(p: @Pair, ordinary: @u8, code: u32) {{
                let value: @u8 = match code {{
                    0 -> {{ let inner: @Pair = p; inner.first }},
                    _ -> ordinary,
                }};
            }}"
        );
        let error = checked(&[("main.omg", &source)]).unwrap_err();
        assert!(error.contains("block result exposes a witness"), "{error}");
    }

    #[test]
    fn calling_a_stored_operation_can_return_an_abstract_reference() {
        let source = "data Peek<S> = machine() -> @S reaches none crashes none;
            data Source<some S> { peek: Peek<S>; }
            machine leak(p: @Source) -> @u8 { (p.peek)() }";
        let error = checked(&[("main.omg", source)]).unwrap_err();
        assert!(
            error.contains("returned value exposes a witness"),
            "{error}"
        );
    }

    #[test]
    fn one_opening_can_repack_correlated_fields_without_exporting_its_witness() {
        let source = format!("{PAIR}
            machine repack(p: @Pair) -> Pair {{ Pair {{ first: p.first, second: p.second, tag: p.tag }} }}
            machine tag(p: @Pair<some X>) -> u8 {{ let local: @Pair<some Y> = p; local.tag }}");
        let program = checked(&[("main.omg", &source)]).unwrap();
        let package = program
            .expression_table
            .expression_entries()
            .find_map(|(_, expr)| {
                if let ExpressionNode::StructLiteral(literal) = expr {
                    literal.package.as_deref()
                } else {
                    None
                }
            })
            .unwrap();
        let (_, opening) = program.package_openings.iter().next().unwrap();
        assert_eq!(package.witnesses, [opening.witnesses[0].1]);

        let wrong = source
            .replace("repack(p: @Pair)", "repack(p: @Pair, q: @Pair)")
            .replace("second: p.second", "second: q.second");
        let error = checked(&[("main.omg", &wrong)]).unwrap_err();
        assert!(error.contains("conflicting generic arguments"), "{error}");
    }
}
