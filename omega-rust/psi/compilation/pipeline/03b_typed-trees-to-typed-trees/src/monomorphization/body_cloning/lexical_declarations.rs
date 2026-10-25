//! Apply declaration context without applying a helper's invocation telescope.
use super::*;

pub(super) struct LexicalClone<'plan> {
    pub instance: SymbolHandle,
    pub parent_template: SymbolHandle,
    pub parent_instance: SymbolHandle,
    pub parent_application: typed_trees::static_applications::StaticMachineApplicationHandle,
    pub symbols: &'plan [(SymbolHandle, SymbolHandle)],
    pub capture: Option<(
        &'plan typed_trees::typed_trees::ClosedConformanceApplication,
        typed_trees::static_applications::StaticMachineApplicationHandle,
    )>,
    pub invocation: Option<(
        &'plan Candidate<'plan>,
        &'plan [(SymbolHandle, SymbolHandle)],
    )>,
}
impl LexicalClone<'_> {
    pub fn remapped(&self, symbol: SymbolHandle) -> SymbolHandle {
        super::remapped_symbol(symbol, self.symbols)
    }
}

pub(super) struct LexicalPlan {
    pub symbols: Vec<(SymbolHandle, SymbolHandle)>,
    root_states: Vec<(SymbolHandle, SymbolHandle)>,
    declarations: Vec<(
        usize,
        SymbolHandle,
        SymbolHandle,
        SymbolHandle,
        SymbolHandle,
    )>,
}

pub(super) fn direct_children(
    program: &dyn validation::CheckingInput,
    parent: SymbolHandle,
) -> Vec<typed_trees::machine::Machine> {
    program
        .symbols
        .child_handles(parent)
        .into_iter()
        .flatten()
        .filter(|symbol| program.symbols.get(*symbol).kind == SymbolKind::Machine)
        .filter_map(|symbol| program.machine_by_symbol(symbol))
        .cloned()
        .collect()
}

impl LexicalPlan {
    pub fn allocate(
        source: Option<&dyn validation::CheckingInput>,
        program: &mut TypedTrees,
        parent: SymbolHandle,
        instance: SymbolHandle,
        states: &[(SymbolHandle, SymbolHandle)],
        roots: &[(SymbolHandle, SymbolHandle)],
        static_symbols: &[(SymbolHandle, SymbolHandle)],
    ) -> Result<Self, Vec<Diagnostic>> {
        let mut plan = Self {
            symbols: vec![(parent, instance)],
            declarations: Vec::new(),
            root_states: states.to_vec(),
        };
        plan.symbols.extend_from_slice(states);
        let declaration = source
            .unwrap_or(program)
            .machine_by_symbol(parent)
            .ok_or_else(|| {
                vec![Diagnostic::error(
                    "lexical context lost its declaring machine",
                )]
            })?;
        let static_roots = source
            .unwrap_or(program)
            .machine_type_parameters(declaration)
            .iter()
            .map(|parameter| parameter.symbol)
            .chain(
                declaration
                    .conformance_bounds
                    .iter()
                    .map(|bound| bound.binder),
            )
            .collect::<Vec<_>>();
        plan.symbols
            .extend(static_symbols.iter().copied().filter(|(symbol, _)| {
                let mut owner = *symbol;
                while owner.is_valid() && owner != parent {
                    if static_roots.contains(&owner) {
                        return true;
                    }
                    owner = source.unwrap_or(program).symbols.get(owner).parent;
                }
                false
            }));
        plan.symbols.extend_from_slice(roots);
        let mut pending = roots.iter().copied().rev().collect::<Vec<_>>();
        while let Some((template, applied)) = pending.pop() {
            let original = source.unwrap_or(program).symbols.get(template);
            if original.kind == SymbolKind::Machine {
                let parent_template = original.parent;
                let parent_instance = remapped_symbol(parent_template, &plan.symbols);
                if parent_instance == parent_template {
                    return Err(vec![Diagnostic::error(
                        "nested declaration lost its exact applied parent",
                    )]);
                }
                let index = source
                    .unwrap_or(program)
                    .machine_position_by_symbol(template)
                    .ok_or_else(|| {
                        vec![Diagnostic::error(
                            "nested declaration lost its source machine",
                        )]
                    })?;
                plan.declarations.push((
                    index,
                    template,
                    applied,
                    parent_template,
                    parent_instance,
                ));
            }
            let children = source
                .unwrap_or(program)
                .symbols
                .child_handles(template)
                .into_iter()
                .flatten()
                .map(|child| {
                    (
                        child,
                        source.unwrap_or(program).symbols.get(child).kind,
                        source.unwrap_or(program).symbols.name(child).to_owned(),
                    )
                })
                .collect::<Vec<_>>();
            if children.is_empty() {
                continue;
            }
            let copied = program.symbols.insert_generated_children_from(
                applied,
                children
                    .iter()
                    .map(|(child, kind, name)| (*child, *kind, name.as_str())),
            );
            let mappings = children
                .iter()
                .map(|(child, _, _)| *child)
                .zip(symbols::SymbolTableBuilder::child_handles(copied))
                .collect::<Vec<_>>();
            plan.symbols.extend_from_slice(&mappings);
            pending.extend(mappings.into_iter().rev());
        }
        Ok(plan)
    }

    pub fn materialize(
        self,
        source: Option<&dyn validation::CheckingInput>,
        program: &mut TypedTrees,
        enclosing: &Candidate,
        application: typed_trees::static_applications::StaticMachineApplicationHandle,
        ordinal: usize,
        preserve_enclosing_invocation: bool,
        capture: Option<(
            &typed_trees::typed_trees::ClosedConformanceApplication,
            typed_trees::static_applications::StaticMachineApplicationHandle,
        )>,
    ) -> Result<(), Vec<Diagnostic>> {
        let families = source
            .unwrap_or(program)
            .target_machine_families
            .iter()
            .filter(|family| {
                self.declarations.iter().any(|(_, template, _, _, _)| {
                    family.canonical == *template
                        || family
                            .members
                            .iter()
                            .any(|member| member.machine == *template)
                })
            })
            .cloned()
            .collect::<Vec<_>>();
        for (index, template, instance, parent_template, parent_instance) in &self.declarations {
            let original = &source.unwrap_or(program).machines()[*index];
            let mut declaration = enclosing.template.as_ref().clone();
            declaration.machine_index = *index;
            declaration.template_symbol = *template;
            declaration.template_name = original.name.as_str().to_owned();
            declaration.state_symbols = source
                .unwrap_or(program)
                .machine_states(original)
                .iter()
                .map(|state| state.symbol)
                .collect();
            let mut candidate = enclosing.clone();
            candidate.template = std::borrow::Cow::Owned(declaration);
            candidate.binding_application = application;
            let identity = validation::contract_identities::normalized_machine_identity(
                source.unwrap_or(program),
                original,
            )
            .ok_or_else(|| {
                vec![Diagnostic::error(
                    "nested declaration has no exact callable identity",
                )]
            })?;
            let context = LexicalClone {
                instance: *instance,
                parent_template: *parent_template,
                parent_instance: *parent_instance,
                parent_application: application,
                symbols: &self.symbols,
                capture,
                invocation: (!preserve_enclosing_invocation)
                    .then_some((enclosing, &self.root_states)),
            };
            super::clone_machine_in_context(
                source,
                program,
                &candidate,
                ordinal,
                identity,
                None,
                None,
                Some(&context),
            )?;
        }
        for mut family in families {
            let applied = |symbol| {
                self.declarations
                    .iter()
                    .find(|(_, template, _, _, _)| *template == symbol)
                    .map(|(_, _, instance, _, _)| *instance)
                    .ok_or_else(|| {
                        vec![Diagnostic::error(
                            "lexical declaration only partially covers a target family",
                        )]
                    })
            };
            family.canonical = applied(family.canonical)?;
            for member in &mut family.members {
                member.machine = applied(member.machine)?;
            }
            if let Some(existing) = program
                .target_machine_families
                .iter()
                .find(|row| row.canonical == family.canonical)
            {
                if existing != &family {
                    return Err(vec![Diagnostic::error(
                        "lexical target family has conflicting applied membership",
                    )]);
                }
            } else {
                program.target_machine_families.push(family);
            }
        }
        Ok(())
    }
}
