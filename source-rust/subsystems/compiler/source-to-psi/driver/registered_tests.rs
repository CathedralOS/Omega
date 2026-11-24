//! Each registered satisfier receives its own checked ProgramEntry root.
//! The application and every test start from the same captured source.
use crate::compile_product::ProductSourceInputs;
use crate::driver::published_target;
use crate::source_state::{TypedTrees, machine::resolve_satisfied_declaration};
use crate::*;
use diagnostics::Diagnostic;
use source_admission::RetainedDeclaration;
use source_declarations::source_selections::TestGroupRegistration;

pub struct TestRoot {
    pub group: String,
    pub name: String,
    group_declaration: RetainedDeclaration,
    selecting_machine: RetainedDeclaration,
    declaration: RetainedDeclaration,
}

impl TestRoot {
    pub(super) fn registration<'a>(
        &self,
        typed: &TypedTrees,
        groups: &'a [TestGroupRegistration],
    ) -> Result<&'a TestGroupRegistration, Vec<Diagnostic>> {
        let group = source_symbols::rejoin_declaration(
            &self.group_declaration,
            &typed.symbols,
            groups.iter().map(|g| g.requirement),
        )
        .map_err(|why| vec![Diagnostic::error(why)])?;
        let owner = source_symbols::rejoin_declaration(
            &self.selecting_machine,
            &typed.symbols,
            groups
                .iter()
                .filter(|g| g.requirement == group)
                .map(|g| g.selecting_machine),
        )
        .map_err(|why| vec![Diagnostic::error(why)])?;
        groups
            .iter()
            .find(|g| g.requirement == group && g.selecting_machine == owner)
            .ok_or_else(|| {
                vec![Diagnostic::error(
                    "registered test lost its Build selection",
                )]
            })
    }

    pub(super) fn bind(
        &self,
        typed: &TypedTrees,
        groups: &[TestGroupRegistration],
        target: &str,
    ) -> Result<RootBinding, Vec<Diagnostic>> {
        let registration = self.registration(typed, groups)?;
        let implementation = source_symbols::rejoin_declaration(
            &self.declaration,
            &typed.symbols,
            satisfiers(typed, registration).map(|m| m.symbol),
        )
        .map_err(|why| vec![Diagnostic::error(why)])?;
        Ok(RootBinding {
            slot: format!("{target}::ProgramEntry"),
            implementation: typed.symbols.display_path(implementation, "::"),
            implementation_symbol: implementation,
            source_span: typed.symbols.symbol_source_span(implementation),
        })
    }
}

fn satisfiers<'a>(
    typed: &'a TypedTrees,
    group: &'a TestGroupRegistration,
) -> impl Iterator<Item = &'a crate::source_state::machine::Machine> + 'a {
    let package = typed
        .symbols
        .symbol_package_identity(group.selecting_machine);
    typed.machines().into_iter().filter(move |machine| {
        typed.symbols.symbol_package_identity(machine.symbol) == package
            && typed
                .machine_trait_conformances(machine)
                .iter()
                .filter_map(|edge| resolve_satisfied_declaration(typed, machine, edge))
                .any(|declaration| declaration.symbol() == group.requirement)
    })
}

pub struct RegisteredTests<'a> {
    source: ProductSourceInputs<'a>,
    continuation: SourceTypingCheckpoint,
    pub roots: Vec<TestRoot>,
    pub notes: Vec<String>,
}

impl<'a> RegisteredTests<'a> {
    pub fn discover(
        source: StageSource<'a>,
        target: &str,
        disabled: bool,
    ) -> Result<Option<Self>, Vec<Diagnostic>> {
        if !source.product.policy.executed.iter().any(|event| {
            event.request.operation == declaration_selection::BuildOperation::TestGroupRegistration
        }) {
            return Ok(None);
        }
        if !source.product.calling_targets.contains(&target) {
            return Err(vec![Diagnostic::error(
                "test target is outside its Product deployment",
            )]);
        }
        let mut timings = crate::CompileTimings::default();
        let (source, continuation) = source.resolve(&mut timings)?;
        let checked = source.judge(
            continuation.clone(),
            crate::compile_product::ProductEntry::BuildBindings,
            &mut Default::default(),
            &mut timings,
        )?;
        let typed = &checked.program;
        let retain = |symbol| {
            source_symbols::retain_declaration(&typed.symbols, symbol)
                .map_err(|why| vec![Diagnostic::error(why)])
        };
        let mut roots = Vec::new();
        let mut notes = Vec::new();
        for group in &checked.declarations.test_groups {
            let path = typed.symbols.display_path(group.requirement, "::");
            if disabled || group.configuration.disabled {
                notes.push(format!(
                    "group `{path}` (target `{target}`): disabled; its tests were not run"
                ));
                continue;
            }
            let mut tests = Vec::new();
            for machine in satisfiers(typed, group) {
                let name = typed.symbols.display_path(machine.symbol, "::");
                let refusal = if !typed.machine_type_parameters(machine).is_empty()
                    || !machine.body_is_present
                {
                    Some(
                        "a test must be a concrete machine with a body; wrap a generic satisfier in a concrete test",
                    )
                } else if typed
                    .machine_states(machine)
                    .first()
                    .is_some_and(|entry| entry.parameters.count() != 0)
                {
                    Some(
                        "its entry takes parameters, and no installed runner contract supplies them",
                    )
                } else {
                    None
                };
                if let Some(reason) = refusal {
                    return Err(vec![
                        Diagnostic::error(format!(
                            "test `{name}` in group `{path}` is not runnable: {reason}"
                        ))
                        .with_source_span(group.source_span),
                    ]);
                }
                tests.push(TestRoot {
                    group: path.clone(),
                    name,
                    group_declaration: retain(group.requirement)?,
                    selecting_machine: retain(group.selecting_machine)?,
                    declaration: retain(machine.symbol)?,
                });
            }
            if tests.is_empty() {
                notes.push(format!(
                    "group `{path}` (target `{target}`): registered with no tests; nothing ran"
                ));
            }
            tests.sort_by(|a, b| a.name.cmp(&b.name));
            roots.extend(tests);
        }
        Ok(Some(Self {
            source,
            continuation,
            roots,
            notes,
        }))
    }

    pub fn compile(
        &self,
        root: &TestRoot,
        target: &str,
    ) -> Result<PublishedTargetProgram, Vec<Diagnostic>> {
        let checked = self.source.judge(
            self.continuation.clone(),
            crate::compile_product::ProductEntry::RegisteredTest { root, target },
            &mut Default::default(),
            &mut Default::default(),
        )?;
        let psi = self.source.publish(&checked, &mut Default::default())?;
        published_target(psi, checked, target, self.source.subject)
    }
}
