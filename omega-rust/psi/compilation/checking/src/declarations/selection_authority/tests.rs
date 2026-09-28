use super::{DeclarationSelectionAuthority, validate_authored_declaration_selections};
use checked_trees::CheckedTrees;
use language_semantics::declaration_selection::{
    AuthoredDeclarationSelectionExposure, AuthoredDeclarationSelectionKind,
    AuthoredDeclarationSelectionLateBinding, AuthoredDeclarationSelections,
};
use semantic_vocabulary::PackageKeyIdentity;
use source::{SourceId, SourceMap, SourceOrigin, SourceSpan, Span};
use std::collections::HashSet;
use std::path::PathBuf;

fn identity(value: u8) -> PackageKeyIdentity {
    PackageKeyIdentity::from_digest([value; 32]).unwrap()
}

struct Authority {
    direct_dependency: bool,
}

impl DeclarationSelectionAuthority for Authority {
    fn allows_declaration_selection(
        &self,
        requester: PackageKeyIdentity,
        owner: PackageKeyIdentity,
    ) -> bool {
        requester == owner
            || (self.direct_dependency && requester == identity(1) && owner == identity(2))
    }

    fn package_label(&self, package: PackageKeyIdentity) -> String {
        format!("package-{}", package.digest()[0])
    }
}

fn fixture(
    requester: Option<PackageKeyIdentity>,
    origin: SourceOrigin,
) -> (CheckedTrees, SourceId) {
    let mut sources = SourceMap::default();
    let owner_text = "pub data Visible {} data Hidden {}";
    let owner = sources
        .add_with_metadata(
            PathBuf::from("owner/main.omg"),
            owner_text.to_owned(),
            PathBuf::from("owner"),
            Some(identity(2)),
            SourceOrigin::User,
        )
        .source_id;
    let requester_text = "machine consumer() -> u64 { 0 }";
    let requester = sources
        .add_with_metadata(
            PathBuf::from("requester/main.omg"),
            requester_text.to_owned(),
            PathBuf::from("requester"),
            requester,
            origin,
        )
        .source_id;
    let mut typed = crate::front_end::typed_program_from_source_map(
        sources,
        &[(owner, owner_text), (requester, requester_text)],
    );
    // Pin this gate's individual retained selections, independently of which
    // additional builtin selections the frontend records.
    typed.retain_authored_declaration_selections(AuthoredDeclarationSelections::default());
    (
        CheckedTrees::with_roots(typed, Default::default()),
        requester,
    )
}

fn select(program: &mut CheckedTrees, source: SourceId, name: &str, offset: usize) {
    let symbol = program
        .data_definitions()
        .iter()
        .find(|definition| definition.name.as_str() == name)
        .unwrap()
        .symbol;
    program
        .typed
        .record_resolved_authored_declaration_selection_once(
            SourceSpan::new(source, Span::new(offset, offset + 1)),
            AuthoredDeclarationSelectionExposure::PrivateImplementation,
            AuthoredDeclarationSelectionKind::TypeReference,
            symbol,
        )
        .unwrap();
}

#[test]
fn loaded_public_owner_still_requires_direct_dependency_authority() {
    let (mut program, requester) = fixture(Some(identity(1)), SourceOrigin::User);
    select(&mut program, requester, "Visible", 0);
    let denied = validate_authored_declaration_selections(
        &program,
        &Authority {
            direct_dependency: false,
        },
        &HashSet::new(),
    )
    .unwrap_err();
    assert_eq!(denied.len(), 1);
    assert!(
        denied[0]
            .message
            .contains("without declaring that package as a direct dependency")
    );
    assert!(denied[0].message.contains("package-1"));
    assert!(denied[0].message.contains("package-2"));
    validate_authored_declaration_selections(
        &program,
        &Authority {
            direct_dependency: true,
        },
        &HashSet::new(),
    )
    .expect("exact declared edge admits the public selection");
}

#[test]
fn private_owner_rejects_even_with_direct_dependency_but_owner_access_admits() {
    let (mut program, requester) = fixture(Some(identity(1)), SourceOrigin::User);
    select(&mut program, requester, "Hidden", 0);
    let denied = validate_authored_declaration_selections(
        &program,
        &Authority {
            direct_dependency: true,
        },
        &HashSet::new(),
    )
    .unwrap_err();
    assert_eq!(denied.len(), 1);
    assert!(denied[0].message.contains("selects private"));

    let (mut owner_program, requester) = fixture(Some(identity(2)), SourceOrigin::User);
    select(&mut owner_program, requester, "Hidden", 0);
    validate_authored_declaration_selections(
        &owner_program,
        &Authority {
            direct_dependency: false,
        },
        &HashSet::new(),
    )
    .expect("own private declaration is accessible");
}

#[test]
fn target_scope_excludes_only_exact_occurrences_and_keeps_diagnostic_order() {
    let (mut program, requester) = fixture(Some(identity(1)), SourceOrigin::User);
    select(&mut program, requester, "Hidden", 0);
    select(&mut program, requester, "Visible", 2);
    let denied = validate_authored_declaration_selections(
        &program,
        &Authority {
            direct_dependency: false,
        },
        &HashSet::new(),
    )
    .unwrap_err();
    assert_eq!(denied.len(), 2);
    assert!(denied[0].message.contains("selects private"));
    assert!(denied[1].message.contains("without declaring"));
    let first = program
        .authored_declaration_selections()
        .iter()
        .next()
        .unwrap()
        .occurrence_id();
    let denied = validate_authored_declaration_selections(
        &program,
        &Authority {
            direct_dependency: false,
        },
        &HashSet::from([first]),
    )
    .unwrap_err();
    assert_eq!(denied.len(), 1);
    assert!(denied[0].message.contains("without declaring"));
}

#[test]
fn missing_requester_custody_rejects_but_toolchain_is_not_a_package_requester() {
    let (mut program, requester) = fixture(None, SourceOrigin::User);
    select(&mut program, requester, "Visible", 0);
    let denied = validate_authored_declaration_selections(
        &program,
        &Authority {
            direct_dependency: true,
        },
        &HashSet::new(),
    )
    .unwrap_err();
    assert_eq!(denied.len(), 1);
    assert!(
        denied[0]
            .message
            .contains("no reconciled requesting package identity")
    );

    let (mut program, requester) = fixture(None, SourceOrigin::Toolchain);
    select(&mut program, requester, "Hidden", 0);
    validate_authored_declaration_selections(
        &program,
        &Authority {
            direct_dependency: false,
        },
        &HashSet::new(),
    )
    .expect("toolchain authority is not projected as package authority");
}

#[test]
fn unresolved_selection_and_absent_source_remain_distinct_rejections() {
    let (mut program, requester) = fixture(Some(identity(1)), SourceOrigin::User);
    let mut selections = AuthoredDeclarationSelections::default();
    selections
        .record_late_bound(
            SourceSpan::new(requester, Span::new(0, 1)),
            AuthoredDeclarationSelectionExposure::PrivateImplementation,
            AuthoredDeclarationSelectionKind::MemberAccess,
            AuthoredDeclarationSelectionLateBinding::CheckedCall,
        )
        .unwrap();
    program
        .typed
        .retain_authored_declaration_selections(selections);
    select(&mut program, SourceId(9999), "Visible", 0);
    let denied = validate_authored_declaration_selections(
        &program,
        &Authority {
            direct_dependency: true,
        },
        &HashSet::new(),
    )
    .unwrap_err();
    assert_eq!(denied.len(), 2);
    assert!(
        denied[0]
            .message
            .contains("remained unresolved after successful checking")
    );
    assert_eq!(
        denied[1].message,
        "authored declaration selection has no compiler-owned source custody"
    );
}
