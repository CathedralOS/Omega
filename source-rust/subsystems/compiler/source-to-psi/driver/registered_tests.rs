//! Each registered satisfier receives its own checked ProgramEntry root.
//! The application and every test start from the same captured source.
use crate::compile_product::{ProductRoots, ProductSourceInputs};
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
    adapter: Option<RetainedDeclaration>,
}

impl TestRoot {
    pub(super) fn bind(
        &self,
        typed: &mut TypedTrees,
        groups: &[TestGroupRegistration],
        target: &str,
    ) -> Result<ProductRoots, Vec<Diagnostic>> {
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
        let registration = groups
            .iter()
            .find(|g| g.requirement == group && g.selecting_machine == owner)
            .ok_or_else(|| {
                vec![Diagnostic::error(
                    "registered test lost its Build selection",
                )]
            })?;
        let implementation = source_symbols::rejoin_declaration(
            &self.declaration,
            &typed.symbols,
            satisfiers(typed, registration).map(|m| m.symbol),
        )
        .map_err(|why| vec![Diagnostic::error(why)])?;
        let test = RootBinding {
            slot: format!("{target}::ProgramEntry"),
            implementation: typed.symbols.display_path(implementation, "::"),
            implementation_symbol: implementation,
            source_span: typed.symbols.symbol_source_span(implementation),
        };
        let adapter = match (&self.adapter, registration.adapter) {
            (None, None) => return Ok(ProductRoots::Direct(vec![test])),
            (Some(retained), Some(candidate)) => {
                source_symbols::rejoin_declaration(retained, &typed.symbols, [candidate])
                    .map_err(|why| vec![Diagnostic::error(why)])?
            }
            _ => {
                return Err(vec![Diagnostic::error(
                    "registered test lost its selected adapter",
                )]);
            }
        };
        use crate::declaration_selection::{
            AuthoredDeclarationSelectionExposure as Exposure,
            AuthoredDeclarationSelectionKind as Kind,
        };
        use crate::source_state::data::{TypeParameter, TypeParameterKind};
        use crate::source_state::expression::StaticMachineArgument;
        use crate::source_state::name::Identifier;
        let adapter_name = typed
            .machine_by_symbol(adapter)
            .filter(|machine| {
                machine.body_is_present
                    && matches!(
                        typed.machine_callable_parameters(machine).as_slice(),
                        [TypeParameter {
                            kind: TypeParameterKind::Machine { .. },
                            ..
                        }]
                    )
            })
            .map(|machine| machine.name.clone())
            .ok_or_else(|| {
                let path = typed.symbols.display_path(adapter, "::");
                let reason = format!(
                    "test adapter `{path}` must define a body and take one static machine parameter"
                );
                vec![Diagnostic::error(reason).with_source_span(registration.source_span)]
            })?;
        // The executed registration and the package-local satisfaction above
        // select this exact machine. Visibility follows the supplying request.
        let occurrence = typed
            .record_resolved_authored_declaration_selection_once(
                registration.source_span,
                Exposure::PrivateImplementation,
                Kind::InstalledStaticArgument,
                implementation,
            )
            .map_err(|reason| {
                vec![
                    Diagnostic::error(format!(
                        "cannot retain the test adapter's callable argument: {reason:?}"
                    ))
                    .with_source_span(registration.source_span),
                ]
            })?;
        let argument = StaticMachineArgument {
            symbol: implementation,
            path: vec![Identifier::from(typed.symbols.name(implementation))].into_boxed_slice(),
            authored_selection: Some(occurrence),
            ..Default::default()
        };
        let application = typed.expression_table.retain_static_machine_application(
            adapter,
            adapter_name,
            &[argument],
        );
        typed
            .expression_table
            .static_machine_application_mut(application)
            .lexical_owner = owner;
        Ok(ProductRoots::Applied {
            test,
            application: typed_trees_to_prepared_typed_trees::EntryApplication::new(
                typed,
                application,
            ),
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
                } else if group.adapter.is_none()
                    && typed
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
                    adapter: group.adapter.map(retain).transpose()?,
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
