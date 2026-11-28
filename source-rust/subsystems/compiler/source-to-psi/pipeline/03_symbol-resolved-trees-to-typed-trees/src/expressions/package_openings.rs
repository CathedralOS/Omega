//! Open hidden data parameters at typed places before any runtime call binds
//! the signature of its callee. Ordinary data projections take no new row.
use crate::lowerer::TypingCohort;
use crate::source_resolved as resolved;
use crate::source_state as typed;
use diagnostics::Diagnostic;
use symbols::{SymbolHandle, SymbolKind};
use typed::expression::{ExpressionHandle, ExpressionNode, MemberSelection, UnaryOperator};
use typed::package_openings::*;
use typed::state::State;
use typed::statement::StatementNode;
use typed::types::TypeReferenceNode;
use typed::{SubtreeEdges, TypedTrees};

mod calls;
mod copies;
use copies::LiveOpenings;
use typed::callable_values::RuntimeCallBinding;

#[derive(Clone, Copy)]
enum Write {
    Place(ExpressionHandle),
    StatementCall(
        Option<RuntimeCallBinding>,
        arena::HandleSpan<ExpressionHandle>,
    ),
    ExpressionCall(
        Option<RuntimeCallBinding>,
        arena::HandleSpan<ExpressionHandle>,
    ),
    Unknown,
}

pub(crate) fn open_cohort(
    source: &resolved::SymbolResolvedTrees,
    program: &mut TypedTrees,
    cohort: &TypingCohort<'_>,
) -> Result<(), Diagnostic> {
    if !program
        .data_definitions()
        .iter()
        .any(|data| !data.package_parameters.is_empty())
    {
        return Ok(());
    }
    let names = NamedOpenings::from_source(source)?;
    let states: Vec<_> = program
        .machines()
        .iter()
        .enumerate()
        .filter(|(ordinal, machine)| cohort.owns_machine(*ordinal, machine.symbol))
        .flat_map(|(_, machine)| program.machine_states(machine).iter().cloned())
        .collect();
    for state in &states {
        let mut scope = OpeningScope {
            state,
            source,
            names: &names,
            facts: LiveOpenings {
                names: program
                .state_parameters(state)
                .iter()
                .filter(|formal| !names.for_binding(formal.symbol).is_empty())
                .map(|formal| formal.symbol)
                .collect(),
                ..LiveOpenings::default()
            },
            copies: copies::CopyHistory::default(),
            version: 0,
            active: Vec::new(),
        };
        scope.statements(program, state.statement_nodes)?;
    }
    Ok(())
}

/// Authored type names belong to one annotated value. Their positions refer
/// to the declaration's full parameter telescope, before hiding parameters.
#[derive(Default)]
struct NamedOpenings(Vec<AnnotatedBinding>);

struct AnnotatedBinding {
    owner: SymbolHandle,
    witnesses: Vec<(usize, SymbolHandle)>,
}

fn symbol_order(symbol: SymbolHandle) -> (u32, u32) {
    (symbol.arena_index(), symbol.generation())
}

impl NamedOpenings {
    fn for_binding(&self, owner: SymbolHandle) -> &[(usize, SymbolHandle)] {
        self.0
            .binary_search_by_key(&symbol_order(owner), |binding| symbol_order(binding.owner))
            .map_or(&[], |index| self.0[index].witnesses.as_slice())
    }

    fn from_source(source: &resolved::SymbolResolvedTrees) -> Result<Self, Diagnostic> {
        let tables = &source.tables.declarations;
        let types = &tables.child_type_references;
        let mut result = Self::default();
        if types.package_openings.is_empty() {
            return Ok(result);
        }
        let bindings =
            tables
                .state_parameters
                .iter()
                .map(|(_, p)| (p.symbol, &p.type_reference))
                .chain(tables.state_statements.iter().filter_map(
                    |(_, statement)| match statement {
                        resolved::statement::Statement::LocalData(local) => {
                            Some((local.symbol, &local.type_reference))
                        }
                        _ => None,
                    },
                ));
        for (owner, ty) in bindings {
            // Unbound parameter rows carry type syntax for callable signatures.
            // Only a named value can own a lexical package opening.
            if !owner.is_valid() {
                continue;
            }
            let mut names = Vec::new();
            for binding in types.annotation_openings(ty) {
                if !source.symbols.symbols().nodes().is_valid(binding.symbol)
                    || source.symbols.get(binding.symbol).parent != owner
                {
                    return Err(Diagnostic::error("named opening lost its annotated value")
                        .with_source_span(binding.name.source_span()));
                }
                names.push((binding.position as usize, binding.symbol));
            }
            if !names.is_empty() {
                result.0.push(AnnotatedBinding {
                    owner,
                    witnesses: names,
                });
            }
        }
        result
            .0
            .sort_unstable_by_key(|binding| symbol_order(binding.owner));
        if result
            .0
            .windows(2)
            .any(|pair| pair[0].owner == pair[1].owner && pair[0].witnesses != pair[1].witnesses)
        {
            return Err(Diagnostic::error(
                "one annotated value has conflicting package openings",
            ));
        }
        result.0.dedup_by_key(|binding| binding.owner);
        Ok(result)
    }
}

struct OpeningScope<'a> {
    state: &'a State,
    source: &'a resolved::SymbolResolvedTrees,
    names: &'a NamedOpenings,
    facts: LiveOpenings,
    copies: copies::CopyHistory,
    version: u32,
    active: Vec<ExpressionHandle>,
}

impl OpeningScope<'_> {
    fn invalidate(
        &mut self,
        program: &TypedTrees,
        write: Write,
        copied: Option<(PackageSubject, PackageSubject)>,
    ) -> Result<(), Diagnostic> {
        let call = match write {
            Write::StatementCall(Some(binding), operands) => Some((
                binding.callee,
                program.statement_table.expression_handles(operands),
            )),
            Write::ExpressionCall(Some(binding), operands) => Some((
                binding.callee,
                program.expression_table.expression_handles(operands),
            )),
            _ => None,
        };
        let spared = match call {
            Some((callee, arguments)) => self.reborrowed_opening(program, callee, arguments)?,
            None => None,
        }
        .map(|id| &program.package_opening(id).subject);
        let written = match write {
            Write::Place(target) => package_place(program, target),
            _ => None,
        };
        let survives = |subject: &PackageSubject| {
            spared == Some(subject)
                || written.as_ref().is_some_and(|place| {
                    !write_replaces(program, self.state, place, subject)
                        && !subject.steps.iter().any(|step| match step {
                            PackageSubjectStep::IndexBinding(index) => {
                                write_replaces(program, self.state, place, &bound_subject(*index))
                            }
                            _ => false,
                        })
                })
        };
        self.facts
            .openings
            .retain(|&handle| survives(&program.package_opening(handle).subject));
        self.facts.copies = match copied {
            Some((source, destination)) => {
                self.copies.transfer(self.facts.copies, source, destination)
            }
            None => self.copies.written(self.facts.copies, written.as_ref()),
        };
        self.facts.names.retain(|&symbol| {
            let subject = bound_subject(symbol);
            spared == Some(&subject)
                || written
                    .as_ref()
                    .is_some_and(|place| !write_replaces(program, self.state, place, &subject))
        });
        self.version = self
            .version
            .checked_add(1)
            .ok_or_else(|| Diagnostic::error("package storage version exhausted"))?;
        Ok(())
    }

    fn statements(
        &mut self,
        program: &mut TypedTrees,
        statements: arena::HandleSpan<StatementNode>,
    ) -> Result<(), Diagnostic> {
        for handle in statements.handles() {
            let statement = program.statement_table.statement(handle);
            let write = match statement {
                StatementNode::Assignment(assignment) => Some(Write::Place(assignment.target)),
                StatementNode::Call(call) => {
                    Some(Write::StatementCall(call.runtime_call, call.arguments))
                }
                StatementNode::Assembly(_) => Some(Write::Unknown),
                _ => None,
            };
            let inferred_initializer = match statement {
                StatementNode::LocalData(local) if local.type_is_inferred => {
                    local.ordinary_value_initializer()
                }
                _ => None,
            };
            let named_local = match statement {
                StatementNode::LocalData(local)
                    if !self.names.for_binding(local.symbol).is_empty() =>
                {
                    Some(local.symbol)
                }
                _ => None,
            };
            for (_, root) in program.expression_roots(statement) {
                self.visit(program, root)?;
            }
            if let Some(ty) = inferred_initializer
                .and_then(|initializer| program.package_projection(initializer))
                .map(|projection| projection.result_type)
            {
                // A consuming pattern binds each field as an inferred local.
                // The local carries its subject's opened type into later uses.
                let StatementNode::LocalData(local) = program.statement_table.statement_mut(handle)
                else {
                    unreachable!("the initializer comes from this local")
                };
                local.type_reference = ty;
            }
            if let Some(local) = named_local
                && !self.facts.names.contains(&local)
            {
                self.facts.names.push(local);
            }
            let copied = copies::value_copy(
                program,
                self.state,
                program.statement_table.statement(handle),
            );
            if let Some(target) = write {
                self.invalidate(program, target, copied)?;
            } else if let Some((source, destination)) = copied {
                self.facts.copies = self.copies.transfer(self.facts.copies, source, destination);
            }
        }
        Ok(())
    }

    fn visit(
        &mut self,
        program: &mut TypedTrees,
        expression: ExpressionHandle,
    ) -> Result<(), Diagnostic> {
        if !expression.is_valid() {
            return Ok(());
        }
        if self.active.contains(&expression) {
            return Err(Diagnostic::error(
                "cyclic expression while opening a package",
            ));
        }
        self.active.push(expression);
        let node = program.expression_table.expression(expression);
        if let ExpressionNode::Block(block) = *node {
            self.statements(program, block.statements)?;
            self.visit(program, block.tail)?;
        } else if let ExpressionNode::Match(dispatch) = *node {
            self.visit(program, dispatch.subject)?;
            // Guard expressions are evaluated in order along the fall-through
            // path. A taken arm cannot invalidate an opening on another arm.
            let arms = program.expression_table.match_arms(dispatch.arms).to_vec();
            let mut common: Option<LiveOpenings> = None;
            for arm in arms {
                let operands: Vec<_> = program
                    .expression_table
                    .match_pattern_operands(arm.pattern)
                    .collect();
                for operand in operands {
                    self.visit(program, operand)?;
                }
                let fall_through = self.facts.clone();
                self.visit(program, arm.value)?;
                if let Some(joined) = &mut common {
                    joined.retain_common(&self.facts, &mut self.copies);
                } else {
                    common = Some(self.facts.clone());
                }
                self.facts = fall_through;
            }
            if let Some(joined) = common {
                self.facts = joined;
            }
        } else {
            let projection = match node {
                ExpressionNode::Member(member) => match member.selection {
                    MemberSelection::Field(field) => Some((field, member.receiver)),
                    _ => None,
                },
                _ => None,
            };
            let write = match node {
                ExpressionNode::Call(call) => {
                    Some(Write::ExpressionCall(call.runtime_call, call.arguments))
                }
                ExpressionNode::Atomic(_) => Some(Write::Unknown),
                _ => None,
            };
            let mut children = Vec::new();
            program.subtree_children(node, SubtreeEdges::VALUES, |child| {
                children.push(child);
                false
            });
            for child in children {
                self.visit(program, child)?;
            }
            if let Some((field, receiver)) = projection {
                self.project(program, expression, field, receiver)?;
            }
            if let Some(write) = write {
                self.invalidate(program, write, None)?;
            }
        }
        self.active.pop();
        Ok(())
    }

    fn named_type(
        &self,
        subject: &PackageSubject,
        declaration: SymbolHandle,
        binder: SymbolHandle,
    ) -> Option<SymbolHandle> {
        let PackageSubjectRoot::Binding(owner) = subject.root else {
            return None;
        };
        if !subject.steps.is_empty() || !self.facts.names.contains(&owner) {
            return None;
        }
        let data = self.source.data_definition_by_symbol(declaration)?;
        let ordinal = self
            .source
            .data_type_parameters(data.type_parameters)
            .iter()
            .position(|parameter| parameter.symbol == binder)?;
        self.names
            .for_binding(owner)
            .iter()
            .find_map(|(position, symbol)| (*position == ordinal).then_some(*symbol))
    }

    /// Two immutable reference bindings can denote the same storage. An
    /// indexed loan captures its index at formation; re-reading the name of
    /// a mutable index cannot prove which element that loan holds.
    fn same_subject(
        &self,
        program: &TypedTrees,
        open: &PackageOpening,
        subject: &PackageSubject,
    ) -> bool {
        if open.subject == *subject {
            return true;
        }
        let Some(anchor) = backing(program, self.state, &open.subject) else {
            return false;
        };
        if anchor
            .steps
            .iter()
            .any(|step| matches!(step, PackageSubjectStep::IndexBinding(_)))
            || backing(program, self.state, subject).as_ref() != Some(&anchor)
        {
            return false;
        }
        self.names_match(program, open, subject)
    }

    fn names_match(
        &self,
        program: &TypedTrees,
        open: &PackageOpening,
        subject: &PackageSubject,
    ) -> bool {
        // An explicitly named witness remains bound to its lexical declaration.
        open.witnesses.iter().all(|&(binder, ty)| {
            self.named_type(subject, open.declaration, binder)
                .is_none_or(|name| {
                    program.type_reference_table.unconstrained_named_symbol(ty) == Some(name)
                })
        })
    }

    fn project(
        &mut self,
        program: &mut TypedTrees,
        expression: ExpressionHandle,
        field: SymbolHandle,
        receiver: ExpressionHandle,
    ) -> Result<(), Diagnostic> {
        let declaration = program.symbols.get(field).parent;
        let Some(data) = program.unique_data_definition_by_symbol(declaration) else {
            return Ok(());
        };
        if data.package_parameters.is_empty() {
            return Ok(());
        }
        let binders: Vec<_> = program
            .data_type_parameters
            .span_or_empty(data.package_parameters)
            .iter()
            .map(|parameter| parameter.symbol)
            .collect();
        let state = self.state;
        let receiver_type = crate::types::typing_queries::scoped_expression_type_reference(
            program,
            program.state_parameters(state),
            state.return_type,
            receiver,
            Some(state),
        )?
        .ok_or_else(|| Diagnostic::error("a package projection needs its receiver type"))?;
        let mut substitutions = Vec::new();
        let field_type = typed::type_system::source_projections::field_type_reference(
            program,
            receiver_type,
            field,
            &mut substitutions,
        )
        .ok_or_else(|| {
            Diagnostic::error("the selected package field does not belong to its receiver")
        })?;
        if !program.type_reference_mentions_any(field_type, &binders) {
            return Ok(());
        }

        let subject = package_place(program, receiver).unwrap_or(PackageSubject {
            root: PackageSubjectRoot::Evaluation(receiver),
            steps: Vec::new(),
        });
        if let Some(previous) = program.package_projection(expression) {
            let handle = previous.opening;
            let opening = program.package_opening(handle);
            if previous.field != field
                || opening.declaration != declaration
                || opening.state != state.symbol
                || !self.same_subject(program, opening, &subject)
                || (opening.storage_version != self.version
                    && !self.facts.openings.contains(&handle))
            {
                return Err(Diagnostic::error(
                    "one projected value is used under incompatible package openings",
                ));
            }
            if !self.facts.openings.contains(&handle) {
                self.facts.openings.push(handle);
            }
            return Ok(());
        }
        let opening = if let Some(handle) = self.facts.openings.iter().copied().find(|handle| {
            let open = program.package_opening(*handle);
            open.declaration == declaration && self.same_subject(program, open, &subject)
        }) {
            handle
        } else {
            // A copy has its own storage opening. Only its abstract types can
            // be reused from a value with the same still-live witness origin.
            let copied = copies::stable_subject(program, self.state, &subject).and_then(|target| {
                self.facts.openings.iter().find_map(|&handle| {
                    let origin = program.package_opening(handle);
                    let source = copies::stable_subject(program, self.state, &origin.subject)?;
                    (origin.declaration == declaration
                        && self.copies.related(
                            program,
                            self.state,
                            self.facts.copies,
                            &source,
                            &target,
                        )
                        && self.names_match(program, origin, &subject))
                    .then(|| origin.witnesses.clone())
                })
            });
            let witnesses = copied.unwrap_or_else(|| {
                binders
                    .into_iter()
                    .map(|binder| {
                        let symbol = self
                            .named_type(&subject, declaration, binder)
                            .unwrap_or_else(|| {
                                let spelling = program.symbols.name(binder);
                                let name = format!(
                                    "{spelling}@{}.{}",
                                    state.symbol.arena_index(),
                                    expression.arena_index()
                                );
                                program.symbols.insert_generated_root_from(
                                    binder,
                                    SymbolKind::TypeParameter,
                                    &name,
                                )
                            });
                        let witness = program
                            .type_reference_table
                            .find_named_type_reference(symbol)
                            .unwrap_or_else(|| {
                                let name = typed::name::Identifier::generated(
                                    program.symbols.name(symbol),
                                );
                                program
                                    .type_reference_table
                                    .insert(TypeReferenceNode::Named { symbol, name })
                            });
                        (binder, witness)
                    })
                    .collect()
            });
            let handle = program.package_openings.insert(PackageOpening {
                state: state.symbol,
                declaration,
                subject,
                storage_version: self.version,
                witnesses,
            });
            self.facts.openings.push(handle);
            handle
        };
        substitutions.extend_from_slice(&program.package_opening(opening).witnesses);
        let result_type = program
            .substitute_type_parameters(field_type, state.symbol, &substitutions)
            .map_err(Diagnostic::error)?;
        program
            .retain_package_projection(OpenedPackageField {
                expression,
                field,
                opening,
                result_type,
            })
            .map_err(Diagnostic::error)
    }
}

fn binding(program: &TypedTrees, expression: ExpressionHandle) -> Option<SymbolHandle> {
    let ExpressionNode::Name(path) = program.expression_table.expression(expression) else {
        return None;
    };
    let symbol = path.resolved_symbol()?;
    (path.members.len() == 1
        && matches!(
            program.symbols.get(symbol).kind,
            SymbolKind::Parameter | SymbolKind::Local
        ))
    .then_some(symbol)
}

pub(super) fn package_place(
    program: &TypedTrees,
    expression: ExpressionHandle,
) -> Option<PackageSubject> {
    let (base, step) = match program.expression_table.expression(expression) {
        ExpressionNode::Name(_) => {
            return Some(PackageSubject {
                root: PackageSubjectRoot::Binding(binding(program, expression)?),
                steps: Vec::new(),
            });
        }
        ExpressionNode::Member(member) => {
            let MemberSelection::Field(field) = member.selection else {
                return None;
            };
            (member.receiver, PackageSubjectStep::Field(field))
        }
        ExpressionNode::Unary(unary) if unary.operator == UnaryOperator::Dereference => {
            (unary.operand, PackageSubjectStep::Dereference)
        }
        ExpressionNode::Indexed(indexed) => {
            let step = match program.expression_table.expression(indexed.index) {
                ExpressionNode::Integer(literal) if !literal.is_negative() => {
                    PackageSubjectStep::ConstantIndex(literal.magnitude_u128()?.try_into().ok()?)
                }
                _ => PackageSubjectStep::IndexBinding(binding(program, indexed.index)?),
            };
            (indexed.collection, step)
        }
        _ => return None,
    };
    let mut subject = package_place(program, base)?;
    subject.steps.push(step);
    Some(subject)
}

pub(super) fn bound_subject(symbol: SymbolHandle) -> PackageSubject {
    PackageSubject {
        root: PackageSubjectRoot::Binding(symbol),
        steps: Vec::new(),
    }
}

/// A write to an ancestor replaces the observed package. A write to one
/// of its fields leaves the other fields and the package witness in place.
pub(super) fn write_replaces(
    program: &TypedTrees,
    state: &State,
    written: &PackageSubject,
    subject: &PackageSubject,
) -> bool {
    if written.root == subject.root {
        return path_replaces(&written.steps, &subject.steps);
    }
    let (Some(written), Some(subject)) = (
        backing(program, state, written),
        backing(program, state, subject),
    ) else {
        // An opaque reference may point to the observed storage.
        return true;
    };
    written.root == subject.root && path_replaces(&written.steps, &subject.steps)
}

fn path_replaces(written: &[PackageSubjectStep], subject: &[PackageSubjectStep]) -> bool {
    use PackageSubjectStep::*;
    // An implicit receiver dereference and an explicit one address the same fields.
    let mut subject = subject.iter().filter(|step| !matches!(step, Dereference));
    for target in written.iter().filter(|step| !matches!(step, Dereference)) {
        let Some(observed) = subject.next() else {
            return false;
        };
        match (target, observed) {
            (IndexBinding(_), IndexBinding(_) | ConstantIndex(_))
            | (ConstantIndex(_), IndexBinding(_)) => {}
            _ if target == observed => {}
            _ => return false,
        }
    }
    true
}

fn backing(program: &TypedTrees, state: &State, place: &PackageSubject) -> Option<PackageSubject> {
    let mut place = place.clone();
    let mut visited = Vec::new();
    while let PackageSubjectRoot::Binding(symbol) = place.root {
        if program.symbols.get(symbol).kind != SymbolKind::Local {
            return Some(place);
        }
        let (_, local) = program.state_local_declaration(state, symbol)?;
        if !program
            .type_reference_table
            .contains_type_reference(local.type_reference)
        {
            return None;
        }
        if !matches!(
            program
                .type_reference_table
                .type_reference(local.type_reference),
            TypeReferenceNode::Reference { .. }
        ) {
            return Some(place);
        }
        if local.is_mutable || visited.contains(&symbol) {
            return None;
        }
        visited.push(symbol);
        let initializer = local.ordinary_value_initializer()?;
        let source = match program.expression_table.expression(initializer) {
            ExpressionNode::Borrow(borrow) => borrow.target,
            _ => initializer,
        };
        let mut backing = package_place(program, source)?;
        backing.steps.append(&mut place.steps);
        place = backing;
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn destructuring_binds_one_opened_type_to_state_and_operation() {
        let resolved = crate::resolve_fixture(
            r#"
            data Owner<T> { receipt: @T; }
            data Finish<T> = machine(owner: Owner<T>) -> u8 reaches none crashes none;
            data Job<some S> [linear] { owner: Owner<S>; finish: Finish<S>; }
            machine settle(job: Job) -> u8 {
                let { owner, finish } = move job;
                (finish)(move owner)
            }
        "#,
        );
        let program = crate::lower_symbol_resolved_trees(&resolved).unwrap();
        let state = &program.machine_states(&program.machines()[0])[0];
        let mut opened = Vec::new();
        for handle in state.statement_nodes.handles() {
            if let StatementNode::LocalData(local) = program.statement_table.statement(handle)
                && let Some(projection) = program.package_projection(local.initial_value)
            {
                assert_eq!(local.type_reference, projection.result_type);
                opened.push(projection.opening);
            }
        }
        assert_eq!(opened.len(), 2);
        assert_eq!(opened[0], opened[1]);
    }

    #[test]
    fn a_value_copy_has_its_own_storage_opening_and_keeps_the_original_witness() {
        let source = r#"
            data Read<S> = machine(state: @S) -> u8 reaches none crashes none;
            data Pair<some S> [copy] { left: Read<S>; right: Read<S>; }
            machine repack(p: Pair, q: Pair) -> Pair {
                let copied: Pair = p;
                Pair { left: p.left, right: copied.right }
            }
        "#;
        let checked =
            |source: &str| crate::qualified_operator_home_tests::checked(&[("main.omg", source)]);
        let trees = checked(source).unwrap();
        let [left, right] = trees.package_projections.as_slice() else {
            panic!("two callback reads")
        };
        let left = trees.package_opening(left.opening);
        let right = trees.package_opening(right.opening);
        assert_ne!(left.subject, right.subject);
        assert_eq!(left.witnesses, right.witnesses);
        let reassigned = source.replace(
            "let copied: Pair = p;",
            "let mutable copied: Pair = q; copied = p;",
        );
        let trees = checked(&reassigned).expect("assignment transfers the replacing witness");
        let [left, right] = trees.package_projections.as_slice() else {
            panic!("two assigned callbacks")
        };
        assert_eq!(
            trees.package_opening(left.opening).witnesses,
            trees.package_opening(right.opening).witnesses
        );
        let stale = reassigned.replace("left: p.left", "left: q.left");
        assert!(
            checked(&stale)
                .unwrap_err()
                .contains("conflicting generic arguments")
        );
        let survivors = source.replace(
            "let copied: Pair = p;",
            "let mutable slot: Pair = p; let original: Pair = slot; let copied: Pair = slot; slot = q;",
        ).replace("left: p.left", "left: original.left");
        let trees = checked(&survivors)
            .expect("two saved copies remain related after replacing the original slot");
        let [left, right] = trees.package_projections.as_slice() else {
            panic!("two saved callbacks")
        };
        assert_eq!(
            trees.package_opening(left.opening).witnesses,
            trees.package_opening(right.opening).witnesses
        );
        let replaced = survivors.replace("right: copied.right", "right: slot.right");
        assert!(
            checked(&replaced)
                .unwrap_err()
                .contains("conflicting generic arguments")
        );
        let other = source.replace("= p;", "= q;");
        assert!(
            checked(&other)
                .unwrap_err()
                .contains("conflicting generic arguments")
        );
    }

    #[test]
    fn shared_reference_aliases_repack_fields_of_one_subject() {
        let source = r#"
            data Pair<'a, some S> { first: @'a S; second: @'a S; }
            machine repack<'a>(p: @Pair<'a>, q: @Pair<'a>) -> Pair<'a> {
                let alias: @Pair<'a> = p;
                Pair { first: p.first, second: alias.second }
            }
        "#;
        let checked =
            |source: &str| crate::qualified_operator_home_tests::checked(&[("main.omg", source)]);
        let trees = checked(source).unwrap();
        let [first, second] = trees.package_projections.as_slice() else {
            panic!("two field reads")
        };
        assert_eq!(first.opening, second.opening);
        let crossed = source.replace("= p;", "= q;");
        let error = checked(&crossed).unwrap_err();
        assert!(error.contains("conflicting generic arguments"), "{error}");
    }

    #[test]
    fn paired_projections_open_one_abstract_type_per_subject() {
        let resolved = crate::resolve_fixture(
            r#"
data Read<S> = machine(state: @mutable S) -> u8 reaches none crashes none;
data Reader<'a, some S> { state: @'a mutable S; read: Read<S>; }
machine read_two(left: @mutable Reader, right: @mutable Reader) -> u8 {
    let first: u8 = (left.read)(left.state);
    (right.read)(right.state)
}
"#,
        );
        let program = crate::lower_symbol_resolved_trees(&resolved).unwrap();
        let openings: Vec<_> = program.package_openings.iter().collect();
        assert_eq!(openings.len(), 2, "each reader opens once");
        let mut identities = Vec::new();
        for (handle, opening) in openings {
            let projections: Vec<_> = program
                .expression_table
                .iter_expressions()
                .filter_map(|(e, _)| program.package_projection(e))
                .filter(|p| p.opening == handle)
                .collect();
            assert_eq!(projections.len(), 2);
            assert_ne!(projections[0].field, projections[1].field);
            let [(binder, witness)] = opening.witnesses.as_slice() else {
                panic!("one hidden binder")
            };
            let TypeReferenceNode::Named { symbol, .. } =
                program.type_reference_table.type_reference(*witness)
            else {
                panic!("abstract nominal type")
            };
            for p in projections {
                assert!(program.type_reference_mentions_any(p.result_type, &[*symbol]));
                assert!(!program.type_reference_mentions_any(p.result_type, &[*binder]));
            }
            identities.push(program.normalized_type_identity(*witness));
        }
        assert_ne!(identities[0], identities[1]);
        let limits = record_storage::RecordLimits::default();
        let bytes = record_storage::encode(&program, limits).unwrap();
        let restored: TypedTrees = record_storage::decode(&bytes, limits).unwrap();
        for row in &program.package_projections {
            assert_eq!(restored.package_projection(row.expression), Some(row));
            assert_eq!(
                restored.package_opening(row.opening),
                program.package_opening(row.opening)
            );
        }
    }

    #[test]
    fn crossed_actual_and_callable_formal_retain_distinct_types_before_erasure() {
        let resolved = crate::resolve_fixture(
            r#"
data Read<S> = machine(state: @mutable S) -> u8 reaches none crashes none;
data Reader<'a, some S> { state: @'a mutable S; read: Read<S>; }
machine crossed(left: @mutable Reader, right: @mutable Reader) -> u8 {
    (left.read)(right.state)
}
"#,
        );
        let p = crate::lower_symbol_resolved_trees(&resolved).unwrap();
        let (call, binding) = p
            .expression_table
            .iter_expressions()
            .find_map(|(_, node)| {
                let ExpressionNode::Call(call) = node else {
                    return None;
                };
                Some((call, call.runtime_call?))
            })
            .expect("runtime call through the projected descriptor");
        let signature = binding.signature(&p).unwrap();
        let formal = p.state_parameters.span_or_empty(signature.parameters)[0].type_reference;
        let actual = p.expression_table.expression_handles(call.arguments)[0];
        let actual = p.package_projection(actual).expect("opened state argument");
        let callee = p
            .package_projection(binding.callee)
            .expect("opened runtime descriptor");
        assert_ne!(actual.opening, callee.opening);
        let referent = |reference| {
            let TypeReferenceNode::Reference { referee, .. } =
                p.type_reference_table.type_reference(reference)
            else {
                panic!("state is passed by reference")
            };
            p.normalized_type_identity(*referee)
        };
        assert_ne!(referent(actual.result_type), referent(formal));
    }

    #[test]
    fn named_formal_and_local_openings_are_their_annotated_subjects_abstract_types() {
        let source = r#"
data Read<S> = machine(state: @mutable S) -> u8 reaches none crashes none;
data Reader<'a, some S> { state: @'a mutable S; read: Read<S>; }
machine use_openings(left: @mutable Reader<some A>, right: @mutable Reader) -> u8 {
    let stored: Read<A> = left.read;
    let local: @mutable Reader<some B> = right;
    let selected: Read<B> = local.read;
    (selected)(local.state)
}
"#;
        let early = source.replace("let stored: Read<A>", "let stored: Read<B>");
        let early = crate::try_resolve_fixture(&early).and_then(|trees| {
            crate::lower_symbol_resolved_trees(&trees).map(|_| ()).map_err(|error| vec![error])
        });
        assert!(early.is_err(), "the later local cannot supply a callable's type operand");
        let p = crate::lower_symbol_resolved_trees(&crate::resolve_fixture(source)).unwrap();
        let mut actual = Vec::new();
        for (_, opening) in p.package_openings.iter() {
            let PackageSubjectRoot::Binding(owner) = opening.subject.root else {
                panic!("bound subject");
            };
            let [(_, witness)] = opening.witnesses.as_slice() else {
                panic!("one binder");
            };
            let TypeReferenceNode::Named { symbol, .. } =
                p.type_reference_table.type_reference(*witness)
            else {
                panic!("named witness");
            };
            assert_eq!(p.symbols.get(*symbol).parent, owner);
            actual.push((
                p.symbols.name(owner).to_string(),
                p.symbols.name(*symbol).to_string(),
            ));
        }
        actual.sort();
        assert_eq!(
            actual,
            vec![("left".into(), "A".into()), ("local".into(), "B".into())]
        );
    }

    #[test]
    fn reborrowing_the_hidden_state_preserves_the_saved_callbacks_opening() {
        let program = crate::qualified_operator_home_tests::checked(&[(
            "main.omg",
            r#"
data Read<S> = machine(state: @mutable S, output: @mutable [u8]) -> u8 reaches none crashes none;
data Reader<'a, some S> { state: @'a mutable S; read: Read<S>; }
machine repeated(reader: @mutable Reader<some S>, output: @mutable [u8]) -> u8 {
    let read: Read<S> = reader.read;
    let first: u8 = (read)(reader.state, output);
    (read)(reader.state, output)
}
"#,
        )])
        .unwrap();
        let projections: Vec<_> = program
            .package_projections
            .iter()
            .map(|row| row.opening)
            .collect();
        assert_eq!(
            projections.len(),
            3,
            "the saved callback and its two state arguments"
        );
        assert!(projections.iter().all(|id| id == &projections[0]));
    }

    #[test]
    fn lending_the_whole_reader_to_a_callback_expires_its_opening() {
        let source = r#"
data Read<S> = machine(state: @mutable S) -> u8 reaches none crashes none;
data Reader<'a, some S> { state: @'a mutable S; read: Read<S>; }
data Replace = machine(holder: @mutable Reader, next: Reader) reaches none crashes none;
machine observe(reader: @mutable Reader<some S>, next: Reader, replace: Replace) {
    reader.read;
    (replace)(reader, next);
    reader.read;
}
"#;
        let program =
            crate::qualified_operator_home_tests::checked(&[("main.omg", source)]).unwrap();
        let projections: Vec<_> = program
            .package_projections
            .iter()
            .map(|row| row.opening)
            .collect();
        assert_eq!(projections.len(), 2);
        assert_ne!(projections[0], projections[1]);
    }

    #[test]
    fn indexed_replacement_distinguishes_separate_elements_from_may_alias_indices() {
        let source = r#"
data Pair<'a, some S> { first: @'a S; second: @'a S; }
machine observe(items: @mutable [Pair; 2], first: Pair, second: Pair, index: u64) {
    items[0].first;
    items[1] = first;
    items[0].second;
    items[index] = second;
    items[0].first;
}
"#;
        let p = crate::lower_symbol_resolved_trees(&crate::resolve_fixture(source)).unwrap();
        let mut rows: Vec<_> = p.package_projections.iter().collect();
        rows.sort_by_key(|row| p.expression_table.source_span(row.expression).span.start);
        let [before, disjoint, overlap] = rows.as_slice() else {
            panic!("three observed fields")
        };
        assert_eq!(before.opening, disjoint.opening);
        assert_ne!(before.opening, overlap.opening);
    }

    #[test]
    fn indexed_subjects_require_stable_element_and_index_versions() {
        let source = r#"
data Pair<'a, some S> { first: @'a S; second: @'a S; }
machine observe(items: @mutable [Pair; 2], other: @mutable Pair) {
    let mutable index: u64 = 0u64;
    items[0].first;
    items[0].second;
    items[1].second;
    other.first;
    items[index].first;
    items[index].second;
    let index_alias: @mutable u64 = @mutable index;
    $index_alias = 1u64;
    items[index].first;
    index = 0u64;
    items[index].second;
}
"#;
        let p = crate::lower_symbol_resolved_trees(&crate::resolve_fixture(source)).unwrap();
        let mut rows: Vec<_> = p
            .expression_table
            .iter_expressions()
            .filter_map(|(handle, _)| p.package_projection(handle))
            .collect();
        rows.sort_by_key(|row| p.expression_table.source_span(row.expression).span.start);
        let [a, b, c, d, e, f, g, h] = rows.as_slice() else {
            panic!("eight package projections: {rows:?}")
        };
        assert_eq!(a.opening, b.opening);
        assert_ne!(a.opening, c.opening);
        assert_ne!(a.opening, d.opening);
        assert_eq!(e.opening, f.opening);
        assert_ne!(f.opening, g.opening);
        assert_ne!(g.opening, h.opening);
        assert_eq!(
            p.package_opening(e.opening).subject,
            p.package_opening(g.opening).subject
        );
        assert_ne!(
            p.package_opening(e.opening).storage_version,
            p.package_opening(g.opening).storage_version
        );
    }
}
