//! Header-level coverage keeps declaration selection independent of the current
//! source normalization fence on module-owned domain declarations.
use super::*;
use source::{SourceId, SourceMap, SourceSpan, Span};
use std::{path::PathBuf, sync::Arc};
use symbols::{SymbolNameRef, SymbolTableBuilder};

#[test]
fn membership_domain_and_case_candidates_share_one_unambiguous_namespace() {
    for (domains, has_case) in [(0, true), (1, false), (1, true), (2, false), (2, true)] {
        let mut sources = SourceMap::default();
        for name in ["root", "left", "right"] {
            sources.add(
                PathBuf::from(format!("{name}.omg")),
                "Owner::Case".to_owned(),
            );
        }
        let reference = |source| SourceSpan::new(SourceId(source), Span::new(0, 5));
        let mut builder = SymbolTableBuilder::with_sources_and_top_level_bindings(
            Some(Arc::new(sources)),
            Vec::new(),
        );
        let root = builder.insert_root(SymbolKind::Root, SymbolNameRef::Static("root"));
        let mut declarations = vec![(
            SymbolKind::Data,
            SymbolNameRef::OwnedSource {
                value: "Owner",
                source_span: reference(0),
            },
        )];
        for index in 0..domains {
            declarations.push((
                SymbolKind::Domain,
                SymbolNameRef::OwnedSource {
                    value: "Owner::Case",
                    source_span: reference(index + 1),
                },
            ));
        }
        let handles =
            SymbolTableBuilder::child_handles(builder.insert_children(root, declarations))
                .collect::<Vec<_>>();
        let owner = handles[0];
        let case = SymbolTableBuilder::child_handles(builder.insert_children(
            owner,
            [(
                if has_case {
                    SymbolKind::Variant
                } else {
                    SymbolKind::Field
                },
                SymbolNameRef::Static("Case"),
            )],
        ))
        .next()
        .unwrap();
        let mut symbols = builder.finish();
        for (index, name) in ["left", "right"].into_iter().enumerate().take(domains) {
            symbols
                .register_source_module(SourceId(index + 1), [(name, reference(index + 1))])
                .unwrap();
            symbols.register_source_import(SourceId(0), &format!("{name}::Owner"));
        }
        let result = membership_selection(&symbols, "Owner::Case", reference(0));
        match (domains, has_case) {
            (0, true) => assert!(
                matches!(result, Ok(Some(MembershipSelection::Case { owner: selected_owner, case: selected_case })) if selected_owner == owner && selected_case == case)
            ),
            (1, false) => assert!(
                matches!(result, Ok(Some(MembershipSelection::Domain(selected))) if selected == handles[1])
            ),
            _ => {
                let Err(error) = result else {
                    panic!("competing meanings must reject: domains={domains}, case={has_case}");
                };
                assert!(
                    error.contains("ambiguous membership") && error.contains("left::Owner::Case"),
                    "{error}"
                );
                if domains == 2 {
                    assert!(error.contains("right::Owner::Case"), "{error}");
                }
            }
        }
    }
}
