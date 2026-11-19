use omega_lexer::SourceSpan;
use omega_resolver::{DeclarationId, ResolvedStage, StageSource, Symbol};

/// Check provenance against the input files and declarations independently of lookup order.
pub fn check_origins(
    stage: &ResolvedStage,
    sources: &[StageSource<'_, '_>],
) {
    let source = |package: &str, file: &str| {
        sources
            .iter()
            .find(|s| s.package == package && s.source.logical_path == file)
            .unwrap()
    };
    let valid_span = |package: &str, file: &str, span: SourceSpan| {
        let text = source(package, file).source.text;
        assert!(span.get_byte_count() > 0);
        assert!(
            text.get(span.get_start_offset()..span.get_end_offset())
                .is_some()
        );
    };
    let declared = |id: &DeclarationId| assert_eq!(stage.declarations.iter().filter(|d| *d == id).count(), 1, "{id:?}");
    assert_eq!(stage.declarations.len(), stage.definition_sites.len());
    for (id, site) in stage.declarations.iter().zip(&stage.definition_sites) {
        valid_span(&id.package, &site.file, site.span);
    }
    for usage in &stage.uses {
        valid_span(&usage.package, &usage.file, usage.span);
        let mut targets = vec![&usage.target];
        while let Some(target) = targets.pop() {
            match target {
                Symbol::Declaration(id) => declared(id),
                Symbol::Candidates(ids) => {
                    assert!(ids.len() > 1);
                    for id in ids {
                        declared(id);
                    }
                }
                Symbol::ByOverload(options) => {
                    assert!(options.len() > 1);
                    for option in options {
                        declared(&option.candidate);
                        targets.extend(option.target.as_deref());
                    }
                }
                Symbol::Binding { span, .. } | Symbol::StaticGuarantee { binder: span } => valid_span(&usage.package, &usage.file, *span),
                Symbol::EvidenceMember { binder, requirements } => {
                    valid_span(&usage.package, &usage.file, *binder);
                    for id in requirements {
                        declared(id);
                    }
                }
                _ => {}
            }
        }
    }
    for operator in &stage.operator_declarations {
        declared(&operator.machine);
        if let Some(home) = &operator.home {
            declared(home);
        }
    }
    for operator in &stage.operators {
        valid_span(&operator.package, &operator.file, operator.span);
        let text = source(&operator.package, &operator.file).source.text;
        let spelling = &text[operator.span.get_start_offset()..operator.span.get_end_offset()];
        if operator.token == "[]" {
            assert!(spelling.starts_with('[') && spelling.ends_with(']'));
        } else {
            assert_eq!(spelling, operator.token);
        }
    }
    for app in &stage.applications {
        assert!(stage.uses.contains(&app.site));
        for &arg in &app.arguments {
            valid_span(&app.site.package, &app.site.file, arg);
        }
    }
}
