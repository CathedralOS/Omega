use omega_lexer::SourceSpan;
use omega_resolver::{DeclarationId, StageSource, Symbol, SymbolResolvedTrees};

/// Check provenance against the input files and declarations independently of lookup order.
pub fn check_origins(
    stage: &SymbolResolvedTrees<'_>,
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
    let definitions = stage
        .trees()
        .flat_map(|tree| tree.declaration_nodes().map(|(_, id)| id));
    assert!(
        definitions.eq(stage.declarations.iter()),
        "source declarations differ from their defining trees"
    );
    for tree in stage.trees() {
        for usage in tree.occurrences() {
            assert_eq!(usage.node, tree.file.node_id(usage.node.node));
            valid_span(tree.file.package, tree.file.path, usage.span);
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
                    Symbol::DomainCandidates { candidates, .. } => {
                        for c in candidates {
                            declared(&c.declaration);
                            if let Some((owner, _)) = &c.type_binder {
                                declared(owner);
                            }
                        }
                    }
                    Symbol::ScopedNames { names, .. } => targets.extend(names),
                    Symbol::Binding { span, .. } | Symbol::StaticMember { binder: span, .. } => valid_span(tree.file.package, tree.file.path, *span),
                    _ => {}
                }
            }
        }
    }
    for operator in &stage.operator_declarations {
        declared(&operator.machine);
        let tree = stage.trees().nth(operator.node.file).unwrap();
        let node = &tree.file.nodes[operator.node.node];
        assert_eq!(node.kind, omega_parser::NodeKind::OperatorToken);
        assert_eq!(tree.declaration_at(node.parent.unwrap()), Some(&operator.machine));
        match &operator.declared_home {
            Some(Symbol::Declaration(id)) => declared(id),
            Some(Symbol::DomainCandidates { candidates, .. }) => {
                for c in candidates {
                    declared(&c.declaration);
                }
            }
            Some(name) => panic!("invalid operator home {name:?}"),
            None => {}
        }
    }
    for tree in stage.trees() {
        for (span, id) in tree.definitions() {
            assert_eq!(id.package, tree.file.package);
            valid_span(tree.file.package, tree.file.path, span);
        }
        for operator in (0..tree.file.nodes.len()).filter_map(|n| tree.file.operator(n)) {
            valid_span(tree.file.package, tree.file.path, operator.span);
            let text = source(tree.file.package, tree.file.path).source.text;
            let spelling = &text[operator.span.get_start_offset()..operator.span.get_end_offset()];
            if operator.token == "[]" {
                assert!(spelling.starts_with('[') && spelling.ends_with(']'));
            } else {
                assert_eq!(spelling, operator.token);
            }
        }
        for (section, name) in tree.applications() {
            assert_eq!(tree.file.nodes[section].parent, Some(name.node.node));
            assert_eq!(tree.application_name(section), Some(name));
            for &argument in &tree.file.nodes[section].children {
                valid_span(tree.file.package, tree.file.path, tree.file.span(argument));
            }
        }
    }
}
