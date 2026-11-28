//! Open hidden data parameters at typed places before any runtime call binds
//! the signature of its callee. Ordinary data projections take no new row.
use crate::lowerer::TypingCohort;
use crate::source_resolved as resolved;
use crate::source_state as typed;
use diagnostics::Diagnostic;
use std::collections::HashMap;
use symbols::{SymbolHandle, SymbolKind};
use typed::expression::{ExpressionHandle, ExpressionNode, MemberSelection, UnaryOperator};
use typed::package_openings::*;
use typed::state::State;
use typed::statement::StatementNode;
use typed::types::TypeReferenceNode;
use typed::{SubtreeEdges, TypedTrees};

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
            named_versions: program
                .state_parameters(state)
                .iter()
                .filter(|formal| names.0.contains_key(&formal.symbol))
                .map(|formal| (formal.symbol, 0))
                .collect(),
            version: 0,
            live: Vec::new(),
            active: Vec::new(),
        };
        scope.statements(program, state.statement_nodes)?;
    }
    Ok(())
}

/// Authored type names belong to one annotated value. Their positions refer
/// to the declaration's full parameter telescope, before hiding parameters.
#[derive(Default)]
struct NamedOpenings(HashMap<SymbolHandle, Vec<(usize, SymbolHandle)>>);

impl NamedOpenings {
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
            let mut names = Vec::new();
            for binding in types.annotation_openings(ty) {
                if !source.symbols.symbols().nodes().is_valid(binding.symbol)
                    || source.symbols.get(binding.symbol).parent != owner
                {
                    return Err(Diagnostic::error("named opening lost its annotated value"));
                }
                names.push((binding.position as usize, binding.symbol));
            }
            if !names.is_empty()
                && result
                    .0
                    .insert(owner, names.clone())
                    .is_some_and(|old| old != names)
            {
                return Err(Diagnostic::error(
                    "one annotated value has conflicting package openings",
                ));
            }
        }
        Ok(result)
    }
}

struct OpeningScope<'a> {
    state: &'a State,
    source: &'a resolved::SymbolResolvedTrees,
    names: &'a NamedOpenings,
    named_versions: HashMap<SymbolHandle, u32>,
    version: u32,
    live: Vec<PackageOpeningHandle>,
    active: Vec<ExpressionHandle>,
}

impl OpeningScope<'_> {
    // A write or opaque call can replace a package, reassign an index, or
    // reach either through an alias. Retain equality only through pure reads.
    fn invalidate(&mut self) -> Result<(), Diagnostic> {
        self.live.clear();
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
            let writes = matches!(
                statement,
                StatementNode::Assignment(_) | StatementNode::Call(_) | StatementNode::Assembly(_)
            );
            let inferred_initializer = match statement {
                StatementNode::LocalData(local) if local.type_is_inferred => local.ordinary_value_initializer(),
                _ => None,
            };
            let named_local = match statement {
                StatementNode::LocalData(local) if self.names.0.contains_key(&local.symbol) => Some(local.symbol),
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
            if let Some(local) = named_local {
                self.named_versions.insert(local, self.version);
            }
            if writes {
                self.invalidate()?;
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
        } else {
            let projection = match node {
                ExpressionNode::Member(member) => match member.selection {
                    MemberSelection::Field(field) => Some((field, member.receiver)),
                    _ => None,
                },
                _ => None,
            };
            let effect = matches!(node, ExpressionNode::Call(_) | ExpressionNode::Atomic(_));
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
            if effect {
                self.invalidate()?;
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
        if !subject.steps.is_empty() || self.named_versions.get(&owner) != Some(&self.version) {
            return None;
        }
        let data = self.source.data_definition_by_symbol(declaration)?;
        let ordinal = self
            .source
            .data_type_parameters(data.type_parameters)
            .iter()
            .position(|parameter| parameter.symbol == binder)?;
        self.names
            .0
            .get(&owner)?
            .iter()
            .find_map(|(position, symbol)| (*position == ordinal).then_some(*symbol))
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
                || opening.subject != subject
                || opening.storage_version != self.version
            {
                return Err(Diagnostic::error(
                    "one projected value is used under incompatible package openings",
                ));
            }
            if !self.live.contains(&handle) {
                self.live.push(handle);
            }
            return Ok(());
        }
        let opening = if let Some(handle) = self.live.iter().copied().find(|handle| {
            let open = program.package_opening(*handle);
            open.subject == subject && open.declaration == declaration
        }) {
            handle
        } else {
            let mut witnesses = Vec::new();
            for binder in binders {
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
                        let name = typed::name::Identifier::generated(program.symbols.name(symbol));
                        program
                            .type_reference_table
                            .insert(TypeReferenceNode::Named { symbol, name })
                    });
                witnesses.push((binder, witness));
            }
            let handle = program.package_openings.insert(PackageOpening {
                state: state.symbol,
                declaration,
                subject,
                storage_version: self.version,
                witnesses,
            });
            self.live.push(handle);
            handle
        };
        substitutions.extend_from_slice(&program.package_opening(opening).witnesses);
        let result_type = program
            .substitute_type_parameters(field_type, state.symbol, &substitutions)
            .map_err(Diagnostic::error)?;
        program
            .retain_package_projection(PackageProjection {
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

fn package_place(program: &TypedTrees, expression: ExpressionHandle) -> Option<PackageSubject> {
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
    index = 1u64;
    items[index].first;
}
"#;
        let p = crate::lower_symbol_resolved_trees(&crate::resolve_fixture(source)).unwrap();
        let mut rows: Vec<_> = p
            .expression_table
            .iter_expressions()
            .filter_map(|(handle, _)| p.package_projection(handle))
            .collect();
        rows.sort_by_key(|row| p.expression_table.source_span(row.expression).span.start);
        let [a, b, c, d, e, f, g] = rows.as_slice() else {
            panic!("seven package projections: {rows:?}")
        };
        assert_eq!(a.opening, b.opening);
        assert_ne!(a.opening, c.opening);
        assert_ne!(a.opening, d.opening);
        assert_eq!(e.opening, f.opening);
        assert_ne!(f.opening, g.opening);
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
