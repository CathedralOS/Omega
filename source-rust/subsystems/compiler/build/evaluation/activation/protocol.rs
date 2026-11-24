//! A Build host call includes a source-issued action outside the portable instruction.
//! A successor must give every earlier host call the action the original program ran.

use super::{EntryCall, TerminalBuildHost};
use crate::host::bindings::{BuildProviderRequest, CarriedBindings};
use source_compiler::publication::{
    ProducedTerminalProgram, TerminalBuildFilesystemOperation, TerminalBuildRequestOperation,
};
use source_compiler::representations::declaration_selection::BuildSelectionRequest;
use terminal_semantics::continuation::CheckedPrefix;

#[derive(Clone, Copy, PartialEq, Eq)]
enum Action<'a> {
    Binding(&'a source_compiler::publication::TerminalBuildBindingCall),
    Query(&'a source_compiler::publication::TerminalBuildQueryCall),
    Request(TerminalBuildRequestOperation),
    Filesystem(TerminalBuildFilesystemOperation),
}

fn actions(
    program: &ProducedTerminalProgram,
) -> Result<Vec<(EntryCall, Action<'_>)>, &'static str> {
    let view = program.view();
    let requests = program
        .build_requests()
        .calls_for(view)
        .ok_or("the checked program lost its Build request declarations")?;
    let filesystem = program
        .build_filesystem()
        .calls_for(view)
        .ok_or("the checked program lost its filesystem declarations")?;
    let mut actions = Vec::with_capacity(requests.len() + filesystem.len());
    actions.extend(requests.iter().map(|r| {
        (
            (r.site.machine, r.site.operation),
            Action::Request(r.action),
        )
    }));
    actions.extend(filesystem.iter().map(|r| {
        (
            (r.site.machine, r.site.operation),
            Action::Filesystem(r.action),
        )
    }));
    let bindings = program.build_bindings();
    actions.extend(
        bindings
            .calls_for(view)
            .ok_or("unadmitted Build selection")?
            .iter()
            .map(|row| ((row.site.machine, row.site.operation), Action::Binding(row))),
    );
    actions.extend(
        bindings
            .queries_for(view)
            .ok_or("unadmitted Build query")?
            .iter()
            .map(|row| ((row.site.machine, row.site.operation), Action::Query(row))),
    );
    actions.sort_unstable_by_key(|row| row.0);
    Ok(actions)
}

fn find<'a>(actions: &[(EntryCall, Action<'a>)], site: EntryCall) -> Option<Action<'a>> {
    actions
        .binary_search_by_key(&site, |r| r.0)
        .ok()
        .map(|i| actions[i].1)
}

fn boundary(
    host: &TerminalBuildHost,
    site: EntryCall,
) -> Result<semantic_vocabulary::BoundaryMachineId, &'static str> {
    let op = host
        .produced
        .view()
        .module()
        .machine(site.0)
        .and_then(|m| m.operation(site.1))
        .ok_or("missing reached host call")?;
    let terminal_psi::OperationKind::BoundaryCall { boundary, .. } = op.kind else {
        return Err("reached host service is not a boundary call");
    };
    Ok(boundary)
}

/// The companion tables carry source coordinates in their own compilation.
/// An old coordinate can move only to the same bytes in the same captured file.
struct Sources {
    old: std::sync::Arc<source_compiler::representations::source_inputs::SourceMap>,
    new: std::sync::Arc<source_compiler::representations::source_inputs::SourceMap>,
    // A protocol normally mentions only the root and one or two helpers.
    files: Vec<(source_text::SourceId, source_text::SourceId)>,
}

impl Sources {
    fn rejoin(&mut self, span: source_text::SourceSpan) -> Option<source_text::SourceSpan> {
        let file = self.old.file_at(span)?;
        file.source.get(span.span.start..span.span.end)?;
        let next =
            if let Some((_, next)) = self.files.iter().find(|(from, _)| *from == span.source_id) {
                *next
            } else {
                let next = if let Some(id) = self.new.rejoined_source_id(file) {
                    id
                } else {
                    let mut matches = self
                        .new
                        .selected_files()
                        .filter(|candidate| candidate.continues_capture_of(file));
                    let candidate = matches.next()?.source_id;
                    if matches.next().is_some() {
                        return None;
                    }
                    candidate
                };
                self.files.push((span.source_id, next));
                next
            };
        Some(source_text::SourceSpan::new(next, span.span))
    }
}

pub(super) fn check(
    prefix: &CheckedPrefix<'_>,
    previous: &TerminalBuildHost,
    successor: &TerminalBuildHost,
    installed: &CarriedBindings,
    previous_source: &source_compiler::publication::AcceptedTerminalSource,
    next_source: &source_compiler::publication::AcceptedTerminalSource,
) -> Result<Vec<BuildProviderRequest>, &'static str> {
    let mut sources = Sources {
        old: previous_source
            .source()
            .symbols
            .source_map()
            .ok_or("the previous Build lost its source map")?,
        new: next_source
            .source()
            .symbols
            .source_map()
            .ok_or("the successor Build lost its source map")?,
        files: Vec::new(),
    };
    let mut providers = vec![None; installed.installed_providers.len()];
    let mut supplies = Vec::new();
    let old = actions(&previous.produced)?;
    let new = actions(&successor.produced)?;
    let old_intrinsics = previous
        .providers
        .intrinsics
        .operations_for(&previous.produced.view().custody_identity())?;
    let new_intrinsics = successor
        .providers
        .intrinsics
        .operations_for(&successor.produced.view().custody_identity())?;
    for &(left, right) in prefix.boundary_pairs() {
        let mut accounted = true;
        let mut rejoin = |span| {
            sources.rejoin(span).unwrap_or_else(|| {
                accounted = false;
                span
            })
        };
        let same = match (find(&old, left), find(&new, right)) {
            (Some(Action::Binding(a)), Some(Action::Binding(b))) => {
                let mut request = a.request.clone();
                request.remap_source_spans(&mut rejoin);
                if let (
                    BuildSelectionRequest::ProviderReadiness(l),
                    BuildSelectionRequest::ProviderReadiness(r),
                ) = (&mut request, &b.request)
                {
                    let before = &previous.produced.view().module().identities;
                    let after = &successor.produced.view().module().identities;
                    let exact = |a, b| {
                        before
                            .text(a)
                            .is_some_and(|name| after.text(b) == Some(name))
                    };
                    if !exact(
                        l.requirement.declaration.identity(),
                        r.requirement.declaration.identity(),
                    ) || !exact(l.requirement.application, r.requirement.application)
                    {
                        return Err(
                            "the successor changed the previously selected provider requirement",
                        );
                    }
                    // The successor can call additional members. Each reached member
                    // is checked below and each new installation admits the same provider.
                    l.requirement = r.requirement.clone();
                    if a.action == source_compiler::publication::TerminalBuildBindingOperation::AwaitProvider
                        && let BuildSelectionRequest::ProviderReadiness(original) = &a.request
                    {
                        for (i, installed) in installed.installed_providers.iter().enumerate() {
                            if &installed.request == original {
                                providers[i] = Some(r.clone());
                            }
                        }
                    }
                }
                let machine = a.selecting_machine.map(&mut rejoin);
                let environment =
                    a.static_environment
                        .as_deref()
                        .cloned()
                        .map(|mut environment| {
                            environment.remap_source_spans(&mut rejoin);
                            environment
                        });
                use source_compiler::publication::TerminalBuildBindingOperand as Operand;
                let operand = match (&a.operand, &b.operand) {
                    (None, None) => true,
                    (
                        Some(Operand::TestGroup { referent: old }),
                        Some(Operand::TestGroup { referent: new }),
                    ) => prefix.structural_type(*old) == Some(*new),
                    (Some(Operand::NativeLocator(l)), Some(Operand::NativeLocator(r))) => {
                        prefix.structural_type(l.structural_type) == Some(r.structural_type)
                            && l.widths == r.widths
                            && l.shape == r.shape
                    }
                    (
                        Some(Operand::ProviderOption {
                            structural_type: old_type,
                            cases: old,
                        }),
                        Some(Operand::ProviderOption {
                            structural_type: new_type,
                            cases: new,
                        }),
                    ) => {
                        prefix.structural_type(*old_type) == Some(*new_type)
                            && old.len() == new.len()
                            && old.iter().all(|&(case, choice)| {
                                prefix
                                    .case_id(*old_type, case)
                                    .is_some_and(|id| new.contains(&(id, choice)))
                            })
                    }
                    _ => false,
                };
                a.action == b.action
                    && request == b.request
                    && machine == b.selecting_machine
                    && environment.as_ref() == b.static_environment.as_deref()
                    && operand
            }
            (Some(Action::Query(a)), Some(Action::Query(b))) => {
                let request = a.request.as_deref().cloned().map(|mut request| {
                    request.remap_source_spans(&mut rejoin);
                    request
                });
                a.action == b.action
                    && rejoin(a.occurrence) == b.occurrence
                    && request.as_ref() == b.request.as_deref()
                    && a.selecting_machine.map(&mut rejoin) == b.selecting_machine
                    && a.result_shape == b.result_shape
            }
            (Some(a), Some(b)) => a == b,
            (None, None) => {
                let a = boundary(previous, left)?;
                let b = boundary(successor, right)?;
                if let Some((_, expected)) = old_intrinsics.iter().find(|(slot, _)| *slot == a) {
                    new_intrinsics
                        .iter()
                        .any(|(slot, op)| *slot == b && op == expected)
                } else {
                    supplies.push((a, b));
                    true
                }
            }
            _ => false,
        };
        if !same || !accounted {
            return Err("the successor changed the host action of an earlier Build call");
        }
    }
    let providers: Vec<_> = providers
        .into_iter()
        .collect::<Option<_>>()
        .ok_or("a previously installed provider lost its checked readiness call")?;
    for (before, after) in supplies {
        if !installed
            .installed_providers
            .iter()
            .zip(&providers)
            .any(|(old, new)| {
                old.request.requirement.boundaries.contains(&before)
                    && new.requirement.boundaries.contains(&after)
            })
        {
            return Err("a reached host call no longer selects its installed provider");
        }
    }
    Ok(providers)
}
