//! Each registered satisfier receives its own checked ProgramEntry root.
//! The application and every test start from the same captured source.
use crate::compile_product::{ProductRoots, ProductSourceInputs};
use crate::driver::published_target;
use crate::source_state::{
    TypedTrees, machine::resolve_satisfied_declaration, trait_definition::AppliedTraitScope,
};
use crate::*;
use diagnostics::Diagnostic;
use source_admission::RetainedDeclaration;
pub use source_declarations::declaration_selection::{TestEnginePolicy, TestEngineSelection, TestTargetEngines, TestExpectations, TestFileExpectation, TestInvocationInputs};
use source_declarations::source_selections::TestGroupRegistration;

pub struct TestRoot {
    pub group: String,
    pub name: String,
    pub inputs: TestInvocationInputs,
    pub expected: TestExpectations,
    pub engines: TestEngineSelection,
    group_declaration: RetainedDeclaration,
    registration: crate::declaration_selection::BuildSelectionOrigin,
    declaration: RetainedDeclaration,
    adapter: Option<RetainedDeclaration>,
}

impl TestRoot {
    pub(super) fn registration<'a>(
        &self,
        typed: &TypedTrees,
        groups: &'a [TestGroupRegistration],
    ) -> Result<&'a TestGroupRegistration, Vec<Diagnostic>> {
        let registration = groups
            .iter()
            .find(|group| group.origin == self.registration)
            .ok_or_else(|| {
                vec![Diagnostic::error(
                    "registered test lost its Build selection",
                )]
            })?;
        source_symbols::rejoin_declaration(
            &self.group_declaration,
            &typed.symbols,
            [registration.requirement],
        )
        .map_err(|why| vec![Diagnostic::error(why)])?;
        Ok(registration)
    }

    pub(super) fn bind(
        &self,
        typed: &mut TypedTrees,
        groups: &[TestGroupRegistration],
        target: &str,
    ) -> Result<ProductRoots, Vec<Diagnostic>> {
        let registration = self.registration(typed, groups)?;
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
            .lexical_owner = registration.selecting_machine;
        Ok(ProductRoots::Applied {
            test,
            application: typed_trees_to_prepared_typed_trees::EntryApplication::new(
                typed,
                application,
            ),
        })
    }
}

pub(crate) fn satisfiers<'a>(
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
                .any(|edge| {
                    resolve_satisfied_declaration(typed, machine, edge)
                        .is_some_and(|declaration| declaration.symbol() == group.requirement)
                        && group.closed_requirement.is_none_or(|closed| {
                            AppliedTraitScope::from_machine_conformance(typed, machine, edge)
                                .is_some_and(|scope| {
                                    typed.requirement_application_matches_scope(closed, &scope)
                                })
                        })
                })
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
        let mut refusals = Vec::new();
        for group in &checked.declarations.test_groups {
            let path = if let Some(closed) = group.closed_requirement {
                let application =
                    typed
                        .closed_requirement_application(closed)
                        .ok_or_else(|| {
                            vec![Diagnostic::error("test group lost its closed requirement")]
                        })?;
                let owner = typed
                    .symbols
                    .display_path(application.declaring_trait, "::");
                let arguments = application
                    .trait_arguments
                    .iter()
                    .map(|&reference| typed.display_type_reference(reference))
                    .collect::<Vec<_>>()
                    .join(", ");
                format!(
                    "{owner}<{arguments}>::{}",
                    typed.symbols.name(group.requirement)
                )
            } else {
                typed.symbols.display_path(group.requirement, "::")
            };
            if disabled || group.configuration.disabled {
                notes.push(format!(
                    "group `{path}` (target `{target}`): disabled; its tests were not run"
                ));
                continue;
            }
            let mut candidates: Vec<_> = satisfiers(typed, group)
                .map(|machine| (typed.symbols.display_path(machine.symbol, "::"), machine))
                .collect();
            if candidates.is_empty() {
                notes.push(format!(
                    "group `{path}` (target `{target}`): registered with no tests; nothing ran"
                ));
            }
            candidates.sort_by(|(left, _), (right, _)| left.cmp(right));
            for (name, machine) in candidates {
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
                    let mut row = Diagnostic::error(format!(
                        "test `{name}` in group `{path}` is not runnable: {reason}"
                    ));
                    row.source_span = typed.symbols.symbol_provenance_source_span(machine.symbol);
                    row.build_context =
                        Some(diagnostics::BuildDiagnosticContext::RunningRegisteredTests);
                    refusals.push(row);
                    continue;
                }
                roots.push(TestRoot {
                    group: path.clone(),
                    name,
                    inputs: group.configuration.inputs.clone(),
                    expected: group.configuration.expected.clone(),
                    engines: group.configuration.engines.iter()
                        .find(|row| row.target.canonical_name() == target)
                        .map(|row| row.selection)
                        .unwrap_or_default(),
                    group_declaration: retain(group.requirement)?,
                    registration: group.origin.clone(),
                    declaration: retain(machine.symbol)?,
                    adapter: group.adapter.map(retain).transpose()?,
                });
            }
        }
        if !refusals.is_empty() {
            return Err(refusals);
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
        published_target(psi, &checked, target, self.source.subject)
    }
}
