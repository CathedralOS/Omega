/*
    Boundary-facing proof contract descriptors and obligation summaries.

    We project declared boundary precondition, postcondition, and bounded-type
    contracts into canonical surface descriptions for verification and review.
*/

use typed_trees::name::Identifier;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum BoundaryKind {
    #[default]
    Host,
    CorePrimitive,
    TargetIntrinsic,
}

impl BoundaryKind {
    pub fn label(self) -> &'static str {
        match self {
            BoundaryKind::Host => "host boundary",
            BoundaryKind::CorePrimitive => "core primitive boundary",
            BoundaryKind::TargetIntrinsic => "target intrinsic boundary",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum BoundaryObligationMode {
    #[default]
    Establish,
    Preserve,
}

impl BoundaryObligationMode {
    pub fn label(self) -> &'static str {
        match self {
            BoundaryObligationMode::Establish => "establish",
            BoundaryObligationMode::Preserve => "preserve",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BoundaryProofObligation {
    pub kind: BoundaryKind,
    pub mode: BoundaryObligationMode,
    pub fact: Identifier,
    pub operation: Identifier,
}

impl BoundaryProofObligation {
    pub fn establishes(kind: BoundaryKind, operation: Identifier, fact: Identifier) -> Self {
        Self {
            kind,
            mode: BoundaryObligationMode::Establish,
            fact,
            operation,
        }
    }

    pub fn preserves(kind: BoundaryKind, operation: Identifier, fact: Identifier) -> Self {
        Self {
            kind,
            mode: BoundaryObligationMode::Preserve,
            fact,
            operation,
        }
    }

    pub fn describe(&self) -> String {
        format!(
            "{} must {} `{}` for `{}`",
            self.kind.label(),
            self.mode.label(),
            self.fact,
            self.operation
        )
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct BoundaryObligationSet {
    pub obligations: Vec<BoundaryProofObligation>,
}

impl BoundaryObligationSet {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn push(&mut self, obligation: BoundaryProofObligation) {
        self.obligations.push(obligation);
    }

    pub fn established(&self) -> impl Iterator<Item = &BoundaryProofObligation> {
        self.obligations
            .iter()
            .filter(|obligation| obligation.mode == BoundaryObligationMode::Establish)
    }

    pub fn preserved(&self) -> impl Iterator<Item = &BoundaryProofObligation> {
        self.obligations
            .iter()
            .filter(|obligation| obligation.mode == BoundaryObligationMode::Preserve)
    }

    pub fn is_empty(&self) -> bool {
        self.obligations.is_empty()
    }

    pub fn unmet_preservations<'a>(
        &'a self,
        available: &'a [Identifier],
    ) -> impl Iterator<Item = &'a BoundaryProofObligation> + 'a {
        self.preserved()
            .filter(move |obligation| !available.contains(&obligation.fact))
    }

    pub fn missing_fact_diagnostics(&self, available: &[Identifier]) -> Vec<String> {
        self.unmet_preservations(available)
            .map(|obligation| {
                format!(
                    "{}: needs fact `{}` established before the call so the {} can \
                     preserve it across `{}`",
                    obligation.describe(),
                    obligation.fact,
                    obligation.kind.label(),
                    obligation.operation
                )
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::{BoundaryKind, BoundaryObligationSet, BoundaryProofObligation, Identifier};

    fn ident(name: &str) -> Identifier {
        Identifier::generated(name)
    }

    #[test]
    fn establishes_and_preserves_partition_the_set() {
        let mut set = BoundaryObligationSet::new();
        set.push(BoundaryProofObligation::establishes(
            BoundaryKind::CorePrimitive,
            ident("Slice::index"),
            ident("InBounds"),
        ));
        set.push(BoundaryProofObligation::preserves(
            BoundaryKind::CorePrimitive,
            ident("Slice::index"),
            ident("Slice::Length"),
        ));

        assert_eq!(set.established().count(), 1);
        assert_eq!(set.preserved().count(), 1);
        assert!(!set.is_empty());
    }

    #[test]
    fn describe_mentions_mode_and_fact() {
        let obligation = BoundaryProofObligation::establishes(
            BoundaryKind::Host,
            ident("write_all"),
            ident("String::Utf8"),
        );
        let described = obligation.describe();
        assert!(described.contains("establish"));
        assert!(described.contains("String::Utf8"));
        assert!(described.contains("write_all"));
        assert!(described.contains("host boundary"));
    }

    #[test]
    fn preserve_obligation_reports_missing_fact() {
        let mut set = BoundaryObligationSet::new();
        set.push(BoundaryProofObligation::establishes(
            BoundaryKind::CorePrimitive,
            ident("Slice::index"),
            ident("InBounds"),
        ));
        set.push(BoundaryProofObligation::preserves(
            BoundaryKind::CorePrimitive,
            ident("Slice::index"),
            ident("Slice::Length"),
        ));

        let diagnostics = set.missing_fact_diagnostics(&[]);
        assert_eq!(diagnostics.len(), 1);
        assert!(diagnostics[0].contains("Slice::Length"));
        assert!(diagnostics[0].contains("needs fact"));

        assert!(
            set.missing_fact_diagnostics(&[ident("Slice::Length")])
                .is_empty()
        );
    }

    #[test]
    fn establish_only_boundary_never_reports_missing_facts() {
        let mut set = BoundaryObligationSet::new();
        set.push(BoundaryProofObligation::establishes(
            BoundaryKind::Host,
            ident("from_utf8"),
            ident("String::Utf8"),
        ));
        assert!(set.missing_fact_diagnostics(&[]).is_empty());
        assert_eq!(set.unmet_preservations(&[]).count(), 0);
    }
}
