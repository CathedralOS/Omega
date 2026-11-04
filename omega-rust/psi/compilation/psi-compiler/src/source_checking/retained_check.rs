//! A retained language judgment, and the layout evaluation that produced its input.
//!
//! A product program that declares fixed-layout storage is evaluated under
//! every target scope before it can be matched against an earlier judgment.
//! For a package that depends on `std` that evaluation alone takes seconds, and
//! an equal request repeated it only to find the judgment it already had.
//! Evaluation reads nothing but the completed typed program, the target scopes
//! and the declaration-selection predicate, so equal values of those three
//! produce the program the judgment was made for. While a judgment stays in
//! memory it keeps them, and an equal request takes it before evaluating.
//!
//! The comparison data never authorizes anything: the predicate is compared
//! through its exported scope (`RecordedDeclarationAuthority`), and the request
//! still admits its own representations, declarations and access lifecycles.

use build_time_evaluation::FixedLayoutTargetScope;
use std::sync::Arc;

/// One language judgment with the layout evaluation that produced its input,
/// when that evaluation is still in memory.
#[derive(Debug, Clone)]
pub struct RetainedCheck {
    pub(super) judgment: typed_trees_to_checked_trees::RetainedCheck,
    pub(super) layout_evaluation: Option<Arc<LayoutEvaluationInput>>,
}

/// Everything fixed-layout evaluation reads.
pub(super) struct LayoutEvaluationInput {
    pub(super) program: typed_trees::TypedTrees,
    pub(super) scopes: Vec<FixedLayoutTargetScope>,
    pub(super) authority: validation::RecordedDeclarationAuthority,
}

impl std::fmt::Debug for LayoutEvaluationInput {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("LayoutEvaluationInput")
            .field("scopes", &self.scopes.len())
            .finish_non_exhaustive()
    }
}

impl LayoutEvaluationInput {
    /// The evaluation input of `program` under these scopes, or `None` when
    /// the predicate exports no complete comparison scope.
    pub(super) fn capture(
        program: &typed_trees::TypedTrees,
        scopes: &[FixedLayoutTargetScope],
        authority: Option<&dyn validation::DeclarationSelectionAuthority>,
    ) -> Option<Self> {
        Some(Self {
            program: program.clone(),
            scopes: scopes.to_vec(),
            authority: validation::RecordedDeclarationAuthority::capture(authority).ok()?,
        })
    }

    /// Whether evaluating `program` under `scopes` and `authority` reproduces
    /// this input's evaluated program. Cheap comparisons run first.
    pub(super) fn matches(
        &self,
        program: &typed_trees::TypedTrees,
        scopes: &[FixedLayoutTargetScope],
        authority: Option<&dyn validation::DeclarationSelectionAuthority>,
    ) -> bool {
        self.scopes == scopes
            && validation::RecordedDeclarationAuthority::capture(authority)
                .is_ok_and(|current| current == self.authority)
            && self.program == *program
    }
}

impl RetainedCheck {
    /// Identity of an already-issued language judgment; see the Psi judgment.
    pub fn same_judgment(&self, other: &Self) -> bool {
        self.judgment.same_judgment(&other.judgment)
    }
}

#[cfg(test)]
mod tests {
    use super::LayoutEvaluationInput;
    use build_time_evaluation::FixedLayoutTargetScope;
    use semantic_vocabulary::PackageKeyIdentity;
    use std::collections::BTreeMap;

    struct Scoped(BTreeMap<PackageKeyIdentity, Vec<PackageKeyIdentity>>);

    impl validation::DeclarationSelectionAuthority for Scoped {
        fn allows_declaration_selection(
            &self,
            requester: PackageKeyIdentity,
            owner: PackageKeyIdentity,
        ) -> bool {
            requester == owner
                || self
                    .0
                    .get(&requester)
                    .is_some_and(|owners| owners.contains(&owner))
        }
        fn package_label(&self, _identity: PackageKeyIdentity) -> String {
            String::new()
        }
        fn declaration_selection_scope(
            &self,
        ) -> Option<BTreeMap<PackageKeyIdentity, Vec<PackageKeyIdentity>>> {
            Some(self.0.clone())
        }
    }

    /// A predicate that answers but exports nothing to compare.
    struct Opaque;

    impl validation::DeclarationSelectionAuthority for Opaque {
        fn allows_declaration_selection(
            &self,
            _requester: PackageKeyIdentity,
            _owner: PackageKeyIdentity,
        ) -> bool {
            true
        }
        fn package_label(&self, _identity: PackageKeyIdentity) -> String {
            String::new()
        }
    }

    fn scope(target: &str, pointer_size: u64) -> FixedLayoutTargetScope {
        FixedLayoutTargetScope {
            target: target.to_owned(),
            facts: Some(layout_plans::LayoutTargetFacts {
                profile: target.to_owned(),
                pointer_size,
                pointer_alignment: pointer_size,
                little_endian: true,
            }),
            entry_declarations: None,
            provider_declarations: Vec::new(),
        }
    }

    fn package(marker: u8) -> PackageKeyIdentity {
        PackageKeyIdentity::from_digest([marker; 32]).expect("nonzero digest")
    }

    #[test]
    fn layout_evaluation_input_matches_only_the_same_targets_and_authority() {
        let program = typed_trees::TypedTrees::default();
        let scopes = [scope("linux_x86_64", 8), scope("macos_arm64", 8)];
        let authority = Scoped(BTreeMap::from([(package(1), vec![package(1), package(2)])]));
        let input = LayoutEvaluationInput::capture(&program, &scopes, Some(&authority))
            .expect("a predicate with an exported scope is comparable");
        assert!(input.matches(&program, &scopes, Some(&authority)));

        let another_target = [scope("linux_x86_64", 8), scope("linux_arm64", 8)];
        assert!(!input.matches(&program, &another_target, Some(&authority)));
        let other_facts = [scope("linux_x86_64", 8), scope("macos_arm64", 4)];
        assert!(!input.matches(&program, &other_facts, Some(&authority)));
        assert!(!input.matches(&program, &scopes[..1], Some(&authority)));

        let wider = Scoped(BTreeMap::from([(
            package(1),
            vec![package(1), package(2), package(3)],
        )]));
        assert!(!input.matches(&program, &scopes, Some(&wider)));
        assert!(!input.matches(&program, &scopes, None));
        assert!(!input.matches(&program, &scopes, Some(&Opaque)));
    }

    #[test]
    fn another_program_under_the_same_targets_and_authority_misses() {
        let program = typed_trees::TypedTrees::default();
        let scopes = [scope("linux_x86_64", 8)];
        let authority = Scoped(BTreeMap::from([(package(1), vec![package(1)])]));
        let input = LayoutEvaluationInput::capture(&program, &scopes, Some(&authority))
            .expect("a predicate with an exported scope is comparable");
        let mut other = typed_trees::TypedTrees::default();
        other.push_data_definition(Default::default());
        assert!(!input.matches(&other, &scopes, Some(&authority)));
        assert!(input.matches(&program, &scopes, Some(&authority)));
    }

    #[test]
    fn a_predicate_without_a_comparison_scope_is_never_captured() {
        let program = typed_trees::TypedTrees::default();
        let scopes = [scope("linux_x86_64", 8)];
        assert!(LayoutEvaluationInput::capture(&program, &scopes, Some(&Opaque)).is_none());
        let absent = LayoutEvaluationInput::capture(&program, &scopes, None)
            .expect("an absent predicate is comparable");
        assert!(absent.matches(&program, &scopes, None));
        assert!(!absent.matches(&program, &scopes, Some(&Opaque)));
    }
}
