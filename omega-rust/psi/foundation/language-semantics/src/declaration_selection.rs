/*
We manage build-selected declaration ledgers (`AuthoredDeclarationSelections`),
target and policy predicates (`product_target`, `product_policy`), product
operand bindings (`product_operands`, `product`), and provider choice
resolutions (`provider_choices`).
*/

use source::SourceSpan;
use symbols::SymbolHandle;

#[derive(Debug, Clone, Copy, PartialEq, Eq, compiler_storage::Record)]
pub struct ResolvedAuthoredCallBinding {
    pub selected: SymbolHandle,
    pub original: SymbolHandle,
}

mod build_selection_origin {
    use source::{SourceSpan, Span};
    use symbols::{RetainedDeclaration, SymbolHandle, SymbolKind, SymbolTable};

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct BuildSelectionOrigin {
        activation: RetainedDeclaration,
        author: RetainedDeclaration,
        occurrence: Span,
        event_ordinal: Option<u64>,
    }

    impl BuildSelectionOrigin {
        pub fn capture(
            symbols: &SymbolTable,
            activation: SymbolHandle,
            author: SymbolHandle,
            occurrence: SourceSpan,
            event_ordinal: Option<u64>,
        ) -> Result<Self, &'static str> {
            let activation = RetainedDeclaration::capture(symbols, activation)?;
            let author = RetainedDeclaration::capture(symbols, author)?;
            if author.source().source_id != occurrence.source_id {
                return Err("Build selection occurrence does not belong to its lexical author");
            }
            Self::from_retained_parts(activation, author, occurrence.span, event_ordinal)
        }

        pub fn from_retained_parts(
            activation: RetainedDeclaration,
            author: RetainedDeclaration,
            occurrence: Span,
            event_ordinal: Option<u64>,
        ) -> Result<Self, &'static str> {
            if activation.kind() != SymbolKind::Machine || author.kind() != SymbolKind::Machine {
                return Err("Build selection activation and author must be machine declarations");
            }
            if occurrence.start >= occurrence.end
                || author
                    .source()
                    .source
                    .get(occurrence.start..occurrence.end)
                    .is_none()
            {
                return Err("Build selection has an invalid authored occurrence");
            }
            let portable = |declaration: RetainedDeclaration| {
                RetainedDeclaration::from_retained_parts(
                    declaration.source().clone(),
                    declaration.span(),
                    declaration.kind(),
                )
            };
            Ok(Self {
                activation: portable(activation)?,
                author: portable(author)?,
                occurrence,
                event_ordinal,
            })
        }

        pub fn activation(&self) -> &RetainedDeclaration {
            &self.activation
        }
        pub fn author(&self) -> &RetainedDeclaration {
            &self.author
        }
        pub const fn occurrence(&self) -> Span {
            self.occurrence
        }
        pub const fn event_ordinal(&self) -> Option<u64> {
            self.event_ordinal
        }
    }
}
mod product_policy {
    use semantic_vocabulary::{CrashCause, TerminalAuthorityClass};
    use symbols::SymbolHandle;

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct AuthoredRootGrant {
        pub selector: String,
        pub selecting_machine: SymbolHandle,
        pub source_span: source::SourceSpan,
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub enum AuthoredBehaviorExclusionKind {
        CrashCause {
            cause: CrashCause,
            case_symbol: SymbolHandle,
        },
        PhysicalAuthorityClass {
            class: TerminalAuthorityClass,
            case_symbol: SymbolHandle,
        },
        Service {
            identity: String,
            name: String,
        },
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct AuthoredBehaviorExclusion {
        pub kind: AuthoredBehaviorExclusionKind,
        pub selecting_machine: SymbolHandle,
        pub source_span: source::SourceSpan,
    }

    impl AuthoredBehaviorExclusion {
        pub fn authored_label(&self) -> Option<String> {
            match &self.kind {
                AuthoredBehaviorExclusionKind::Service { name, .. } => {
                    Some(format!("service `{name}`"))
                }
                AuthoredBehaviorExclusionKind::CrashCause { .. }
                | AuthoredBehaviorExclusionKind::PhysicalAuthorityClass { .. } => None,
            }
        }
    }
}
pub mod provider_choices {
    mod identities {
        pub type ApplicationKey = (
            Option<semantic_vocabulary::PackageKeyIdentity>,
            String,
            Option<String>,
        );

        pub fn selected_application_slots(
            subject: &ApplicationKey,
            available: &[ApplicationKey],
        ) -> Vec<ApplicationKey> {
            if subject.2.is_some() {
                return vec![subject.clone()];
            }
            let matched = available
                .iter()
                .filter(|slot| slot.0 == subject.0 && slot.1 == subject.1)
                .cloned()
                .collect::<Vec<_>>();
            if matched.is_empty() {
                vec![subject.clone()]
            } else {
                matched
            }
        }
    }
    pub use identities::{ApplicationKey, selected_application_slots};
    mod selection {
        use super::{Candidate, Choice, Declaration, Error, Outcome, Reason, Tier};

        pub fn choose<CandidateView, DeclarationView>(
            candidates: &[CandidateView],
            target: &str,
            defaults: &[DeclarationView],
            overrides: &[DeclarationView],
        ) -> Outcome<CandidateView::Slot, CandidateView::Provider>
        where
            CandidateView: Candidate,
            DeclarationView: Declaration<CandidateView::Slot, CandidateView::Provider>,
        {
            let mut errors = Vec::new();
            let mut choices: Vec<Choice> = Vec::new();
            let mut slots = candidates
                .iter()
                .filter(|row| row.has_requirements())
                .map(|row| row.slot().clone())
                .collect::<Vec<_>>();
            slots.sort_unstable();
            slots.dedup();
            let mut resolved = |declarations: &[DeclarationView], tier| {
                let mut rows = Vec::new();
                for (index, declaration) in declarations
                    .iter()
                    .enumerate()
                    .filter(|(_, row)| row.target().is_none_or(|scope| scope == target))
                {
                    for slot in declaration.slots(&slots) {
                        if slots.contains(&slot) {
                            rows.push((slot, index));
                        } else {
                            errors.push(Error::UnknownSlot {
                                tier,
                                declaration: index,
                                slot,
                            });
                        }
                    }
                }
                rows
            };
            let requests = resolved(overrides, Tier::Override);
            let fallbacks = resolved(defaults, Tier::Default);
            for slot in &slots {
                let declarations = requests
                    .iter()
                    .filter(|(selected, _)| selected == slot)
                    .map(|(_, index)| *index)
                    .collect::<Vec<_>>();
                if declarations.len() > 1 {
                    errors.push(Error::RepeatedOverride {
                        slot: slot.clone(),
                        declarations,
                    });
                }
            }
            let applies = |candidate: &CandidateView| {
                candidate.target().is_empty() || candidate.target() == target
            };
            let mut pending = slots
                .into_iter()
                .map(|slot| {
                    let authored = requests
                        .iter()
                        .chain(&fallbacks)
                        .any(|(selected, _)| *selected == slot);
                    let receivers = candidates
                        .iter()
                        .filter(|row| row.slot() == &slot && applies(row))
                        .map(Candidate::receiver_count)
                        .max()
                        .unwrap_or(0);
                    (slot, authored, receivers)
                })
                .collect::<Vec<_>>();
            pending.sort_by(|left, right| {
                right
                    .1
                    .cmp(&left.1)
                    .then_with(|| right.2.cmp(&left.2))
                    .then_with(|| left.0.cmp(&right.0))
            });
            for (slot, _, _) in pending {
                let explicit = requests
                    .iter()
                    .find(|(selected, _)| *selected == slot)
                    .map(|(_, index)| *index);
                let slot_defaults = fallbacks
                    .iter()
                    .filter(|(selected, _)| *selected == slot)
                    .map(|(_, index)| *index)
                    .collect::<Vec<_>>();
                let applicable = candidates
                    .iter()
                    .enumerate()
                    .filter(|(_, row)| row.slot() == &slot && applies(row))
                    .collect::<Vec<_>>();
                let covering = applicable
                    .iter()
                    .copied()
                    .filter(|(_, row)| row.covers_requirements())
                    .collect::<Vec<_>>();
                let selected = if let Some(index) = explicit {
                    Some((
                        Tier::Override,
                        index,
                        &overrides[index],
                        Reason::Override(vec![index]),
                    ))
                } else if let Some(&index) = slot_defaults.first() {
                    let mut providers = slot_defaults
                        .iter()
                        .map(|index| defaults[*index].provider().clone())
                        .collect::<Vec<_>>();
                    providers.sort_unstable();
                    providers.dedup();
                    if providers.len() > 1 {
                        errors.push(Error::ConflictingDefaults { slot, providers });
                        continue;
                    }
                    Some((
                        Tier::Default,
                        index,
                        &defaults[index],
                        Reason::Default(slot_defaults),
                    ))
                } else {
                    None
                };
                if let Some((tier, declaration, selected, reason)) = selected {
                    let provider = selected.provider();
                    let matching = applicable
                        .iter()
                        .copied()
                        .filter(|(_, row)| row.provider() == provider)
                        .collect::<Vec<_>>();
                    match matching.as_slice() {
                        [(candidate, row)] if row.covers_requirements() => choices.push(Choice {
                            candidate: *candidate,
                            reason,
                        }),
                        [(candidate, _)] => errors.push(Error::PartialChoice {
                            tier,
                            declaration,
                            candidate: *candidate,
                        }),
                        [] => errors.push(Error::MissingChoice {
                            tier,
                            declaration,
                            wrong_target: candidates
                                .iter()
                                .any(|row| row.slot() == &slot && row.provider() == provider),
                            slot,
                        }),
                        _ => errors.push(Error::RepeatedCandidate {
                            tier,
                            declaration,
                            slot,
                        }),
                    }
                    continue;
                }
                if !applicable.is_empty() {
                    let mut inherited = 0;
                    let mut unresolved = false;
                    for choice in &choices {
                        let whole = &candidates[choice.candidate];
                        if !whole.covers_requirements() {
                            continue;
                        }
                        let mut possible = true;
                        let mut known = true;
                        for (_, candidate) in &applicable {
                            match whole.supplies_receiver(candidate) {
                                Some(true) => {}
                                Some(false) => {
                                    possible = false;
                                    break;
                                }
                                None => known = false,
                            }
                        }
                        if possible {
                            if known {
                                inherited += 1;
                            } else {
                                unresolved = true;
                            }
                        }
                    }
                    if inherited > 1 {
                        errors.push(Error::RepeatedInheritedReceiver { slot });
                        continue;
                    }
                    if unresolved {
                        errors.push(Error::UnresolvedInheritedReceiver { slot });
                        continue;
                    }
                    if inherited == 1 {
                        continue;
                    }
                }
                let keyed = covering
                    .iter()
                    .copied()
                    .filter(|(_, row)| row.build_keyed())
                    .collect::<Vec<_>>();
                match keyed.as_slice() {
                    [] => {}
                    [(candidate, _)] => choices.push(Choice {
                        candidate: *candidate,
                        reason: Reason::UniqueCoveringCandidate,
                    }),
                    rows => errors.push(Error::AmbiguousCandidates {
                        slot,
                        candidates: rows.iter().map(|(index, _)| *index).collect(),
                    }),
                }
            }
            for (position, choice) in choices.iter().enumerate() {
                for other in &choices[..position] {
                    match candidates[choice.candidate].shares_receiver(&candidates[other.candidate])
                    {
                        Some(true) => errors.push(Error::OverlappingChoices {
                            first: choice.candidate,
                            second: other.candidate,
                        }),
                        Some(false) => {}
                        None => errors.push(Error::UnresolvedOverlap {
                            first: choice.candidate,
                            second: other.candidate,
                        }),
                    }
                }
            }
            Outcome { choices, errors }
        }
    }
    pub use selection::choose;

    pub trait Candidate {
        type Slot: Clone + Eq + Ord;
        type Provider: Clone + Eq + Ord;

        fn slot(&self) -> &Self::Slot;
        fn provider(&self) -> &Self::Provider;
        fn target(&self) -> &str;
        fn build_keyed(&self) -> bool;
        fn has_requirements(&self) -> bool;
        fn covers_requirements(&self) -> bool;
        fn receiver_count(&self) -> usize;
        fn supplies_receiver(&self, demanded: &Self) -> Option<bool>;
        fn shares_receiver(&self, other: &Self) -> Option<bool>;
    }

    pub trait Declaration<Slot, Provider> {
        fn target(&self) -> Option<&str>;
        fn slots(&self, available: &[Slot]) -> Vec<Slot>;
        fn provider(&self) -> &Provider;
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub enum Tier {
        Override,
        Default,
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub enum Reason {
        Override(Vec<usize>),
        Default(Vec<usize>),
        UniqueCoveringCandidate,
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct Choice {
        pub candidate: usize,
        pub reason: Reason,
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub enum Error<Slot, Provider> {
        UnknownSlot {
            tier: Tier,
            declaration: usize,
            slot: Slot,
        },
        RepeatedOverride {
            slot: Slot,
            declarations: Vec<usize>,
        },
        ConflictingDefaults {
            slot: Slot,
            providers: Vec<Provider>,
        },
        PartialChoice {
            tier: Tier,
            declaration: usize,
            candidate: usize,
        },
        MissingChoice {
            tier: Tier,
            declaration: usize,
            slot: Slot,
            wrong_target: bool,
        },
        RepeatedCandidate {
            tier: Tier,
            declaration: usize,
            slot: Slot,
        },
        UnresolvedInheritedReceiver {
            slot: Slot,
        },
        UnresolvedOverlap {
            first: usize,
            second: usize,
        },
        RepeatedInheritedReceiver {
            slot: Slot,
        },
        AmbiguousCandidates {
            slot: Slot,
            candidates: Vec<usize>,
        },
        OverlappingChoices {
            first: usize,
            second: usize,
        },
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct Outcome<Slot, Provider> {
        pub choices: Vec<Choice>,
        pub errors: Vec<Error<Slot, Provider>>,
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        struct Map {
            slot: u32,
            provider: u32,
            required: Vec<u32>,
            rows: Vec<u32>,
            receivers: Vec<(u32, Vec<u32>)>,
            target: &'static str,
            keyed: bool,
            contracts_known: bool,
        }
        impl Candidate for Map {
            type Slot = u32;
            type Provider = u32;
            fn slot(&self) -> &u32 {
                &self.slot
            }
            fn provider(&self) -> &u32 {
                &self.provider
            }
            fn target(&self) -> &str {
                self.target
            }
            fn build_keyed(&self) -> bool {
                self.keyed
            }
            fn has_requirements(&self) -> bool {
                !self.required.is_empty()
            }
            fn covers_requirements(&self) -> bool {
                self.required
                    .iter()
                    .all(|key| self.rows.iter().filter(|row| *row == key).count() == 1)
                    && self.rows.iter().all(|row| self.required.contains(row))
            }
            fn receiver_count(&self) -> usize {
                self.receivers.len()
            }
            fn supplies_receiver(&self, other: &Self) -> Option<bool> {
                if !self.contracts_known || !other.contracts_known {
                    return None;
                }
                Some(
                    self.receivers
                        .iter()
                        .any(|receiver| other.receivers.first() == Some(receiver)),
                )
            }
            fn shares_receiver(&self, other: &Self) -> Option<bool> {
                if !self.contracts_known || !other.contracts_known {
                    return None;
                }
                Some(
                    self.receivers
                        .iter()
                        .any(|receiver| other.receivers.contains(receiver)),
                )
            }
        }
        struct Selected {
            slot: u32,
            provider: u32,
        }
        impl Declaration<u32, u32> for Selected {
            fn target(&self) -> Option<&str> {
                None
            }
            fn slots(&self, _: &[u32]) -> Vec<u32> {
                vec![self.slot]
            }
            fn provider(&self) -> &u32 {
                &self.provider
            }
        }
        fn map(slot: u32, provider: u32, required: &[u32], rows: &[u32]) -> Map {
            Map {
                slot,
                provider,
                required: required.to_vec(),
                rows: rows.to_vec(),
                receivers: vec![(slot, required.to_vec())],
                target: "",
                keyed: false,
                contracts_known: true,
            }
        }
        fn keyed(slot: u32, provider: u32, required: &[u32], rows: &[u32]) -> Map {
            Map {
                keyed: true,
                target: "host",
                ..map(slot, provider, required, rows)
            }
        }

        #[test]
        fn partial_maps_never_union_into_a_whole_choice() {
            let maps = [keyed(1, 10, &[1, 2], &[1]), keyed(1, 20, &[1, 2], &[2])];
            let outcome = choose::<_, Selected>(&maps, "host", &[], &[]);
            assert!(outcome.choices.is_empty());
            assert!(outcome.errors.is_empty());
        }

        #[test]
        fn override_replaces_the_whole_default_including_partial_default() {
            let maps = [map(1, 10, &[1, 2], &[1]), map(1, 20, &[1, 2], &[1, 2])];
            let outcome = choose(
                &maps,
                "host",
                &[Selected {
                    slot: 1,
                    provider: 10,
                }],
                &[Selected {
                    slot: 1,
                    provider: 20,
                }],
            );
            assert_eq!(
                outcome.choices,
                [Choice {
                    candidate: 1,
                    reason: Reason::Override(vec![0])
                }]
            );
            assert!(outcome.errors.is_empty());
        }

        #[test]
        fn one_descendant_map_owns_its_entire_inherited_receiver() {
            let mut child = keyed(2, 20, &[1, 2], &[1, 2]);
            child.receivers.push((1, vec![1]));
            let maps = [map(1, 10, &[1], &[1]), child];
            let outcome = choose::<_, Selected>(&maps, "host", &[], &[]);
            assert_eq!(
                outcome.choices,
                [Choice {
                    candidate: 1,
                    reason: Reason::UniqueCoveringCandidate
                }]
            );
            assert!(outcome.errors.is_empty());
        }

        #[test]
        fn a_build_row_keyed_to_the_target_is_not_rivaled_by_implementations_in_scope() {
            let maps = [
                map(1, 10, &[1], &[1]),
                keyed(1, 20, &[1], &[1]),
                map(1, 30, &[1], &[1]),
            ];
            let outcome = choose::<_, Selected>(&maps, "host", &[], &[]);
            assert_eq!(
                outcome.choices,
                [Choice {
                    candidate: 1,
                    reason: Reason::UniqueCoveringCandidate
                }]
            );
            assert!(outcome.errors.is_empty());
        }

        #[test]
        fn build_rows_keyed_to_the_same_target_still_conflict() {
            let maps = [
                map(1, 10, &[1], &[1]),
                keyed(1, 20, &[1], &[1]),
                keyed(1, 30, &[1], &[1]),
            ];
            let outcome = choose::<_, Selected>(&maps, "host", &[], &[]);
            assert!(outcome.choices.is_empty());
            assert_eq!(
                outcome.errors,
                [Error::AmbiguousCandidates {
                    slot: 1,
                    candidates: vec![1, 2]
                }]
            );
        }

        #[test]
        fn an_explicit_override_still_outranks_the_keyed_build_row() {
            let maps = [map(1, 10, &[1], &[1]), keyed(1, 20, &[1], &[1])];
            let outcome = choose(
                &maps,
                "host",
                &[],
                &[Selected {
                    slot: 1,
                    provider: 10,
                }],
            );
            assert_eq!(
                outcome.choices,
                [Choice {
                    candidate: 0,
                    reason: Reason::Override(vec![0])
                }]
            );
            assert!(outcome.errors.is_empty());
        }

        #[test]
        fn an_unavailable_target_is_not_an_empty_successful_choice() {
            let mut foreign = map(1, 10, &[1], &[1]);
            foreign.target = "other";
            let outcome = choose(
                &[foreign],
                "host",
                &[],
                &[Selected {
                    slot: 1,
                    provider: 10,
                }],
            );
            assert!(outcome.choices.is_empty());
            assert_eq!(
                outcome.errors,
                [Error::MissingChoice {
                    tier: Tier::Override,
                    declaration: 0,
                    slot: 1,
                    wrong_target: true
                }]
            );
        }

        #[test]
        fn unknown_contracts_preserve_independent_choices_without_claiming_inheritance() {
            let mut first = map(2, 20, &[1, 2], &[1, 2]);
            first.receivers.push((1, vec![1]));
            first.contracts_known = false;
            let mut second = map(3, 30, &[1, 3], &[1, 3]);
            second.receivers.push((1, vec![1]));
            second.contracts_known = false;
            let maps = [map(1, 10, &[1], &[1]), first, second];
            let outcome = choose(
                &maps,
                "host",
                &[],
                &[
                    Selected {
                        slot: 2,
                        provider: 20,
                    },
                    Selected {
                        slot: 3,
                        provider: 30,
                    },
                ],
            );
            assert_eq!(
                outcome
                    .choices
                    .iter()
                    .map(|row| row.candidate)
                    .collect::<Vec<_>>(),
                [1, 2]
            );
            assert_eq!(
                outcome.errors,
                [
                    Error::UnresolvedInheritedReceiver { slot: 1 },
                    Error::UnresolvedOverlap {
                        first: 2,
                        second: 1
                    },
                ]
            );
        }
    }
}
pub use product_policy::{
    AuthoredBehaviorExclusion, AuthoredBehaviorExclusionKind, AuthoredRootGrant,
};
mod product {
    use super::{BuildOperation, ProductTarget};

    #[derive(Debug, Clone, PartialEq, Eq, compiler_storage::Record)]
    pub enum BuildSelectionRequest<Requirement> {
        ProviderReadiness(ProviderReadinessRequest<Requirement>),
        Entry(RootBindingRequest),
        Declaration(std::sync::Arc<ProductDeclarationRequest>),
        Exclusion(EnumExclusionRequest),
    }

    impl<Requirement> Default for BuildSelectionRequest<Requirement> {
        fn default() -> Self {
            Self::Entry(RootBindingRequest::default())
        }
    }

    impl<Requirement> BuildSelectionRequest<Requirement> {
        pub fn requires_lexical_subject(&self) -> bool {
            match self {
                Self::Exclusion(_) => true,
                Self::Declaration(request) => matches!(
                    request.operation,
                    super::BuildOperation::ProviderSelection
                        | super::BuildOperation::NativeBinding
                        | super::BuildOperation::ServiceExclusion
                        | super::BuildOperation::BoundaryAcceptance
                        | super::BuildOperation::ServiceAudit
                        | super::BuildOperation::AssumedBindingContract
                        | super::BuildOperation::WireCompatibilityRequest
                ),
                Self::Entry(_) | Self::ProviderReadiness(_) => false,
            }
        }

        pub fn source_span(&self) -> source::SourceSpan {
            match self {
                Self::ProviderReadiness(request) => request.source_span,
                Self::Entry(request) => request.source_span,
                Self::Declaration(request) => request.source_span,
                Self::Exclusion(request) => request.source_span,
            }
        }

        pub fn entry(&self) -> Option<&RootBindingRequest> {
            match self {
                Self::Entry(request) => Some(request),
                Self::ProviderReadiness(_) | Self::Declaration(_) | Self::Exclusion(_) => None,
            }
        }

        pub fn designated_operands(&self) -> &[ProductDeclarationOperand] {
            match self {
                Self::ProviderReadiness(request) => std::slice::from_ref(&request.backing),
                Self::Declaration(request) => &request.operands,
                Self::Entry(_) | Self::Exclusion(_) => &[],
            }
        }

        pub fn map_requirement<Next, Error>(
            self,
            map: impl FnOnce(Requirement) -> Result<Next, Error>,
        ) -> Result<BuildSelectionRequest<Next>, Error> {
            Ok(match self {
                Self::ProviderReadiness(request) => {
                    BuildSelectionRequest::ProviderReadiness(ProviderReadinessRequest {
                        source_span: request.source_span,
                        requirement: map(request.requirement)?,
                        backing: request.backing,
                    })
                }
                Self::Entry(request) => BuildSelectionRequest::Entry(request),
                Self::Declaration(request) => BuildSelectionRequest::Declaration(request),
                Self::Exclusion(request) => BuildSelectionRequest::Exclusion(request),
            })
        }
    }

    #[derive(Debug, Clone, PartialEq, Eq, compiler_storage::Record)]
    pub struct ProviderReadinessRequest<Requirement> {
        pub source_span: source::SourceSpan,
        pub requirement: Requirement,
        pub backing: ProductDeclarationOperand,
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq, compiler_storage::Record)]
    pub enum ExclusionVocabulary {
        CrashCause,
        PhysicalAuthorityClass,
    }

    impl ExclusionVocabulary {
        pub const fn type_name(self) -> &'static str {
            match self {
                Self::CrashCause => "CrashCause",
                Self::PhysicalAuthorityClass => "PhysicalAuthorityClass",
            }
        }

        pub const fn helper(self) -> &'static str {
            match self {
                Self::CrashCause => "build_exclusion_crash_code",
                Self::PhysicalAuthorityClass => "build_exclusion_physical_code",
            }
        }

        pub const fn cases(self) -> &'static [&'static str] {
            match self {
                Self::CrashCause => &["Trap", "Abort"],
                Self::PhysicalAuthorityClass => &[
                    "FilesystemContentRead",
                    "FilesystemContentWrite",
                    "FilesystemMetadataQuery",
                    "DirectoryEnumeration",
                    "FilesystemNamespaceMutation",
                    "FilesystemMetadataMutation",
                    "ProcessOutput",
                    "ProcessTermination",
                    "MachineControl",
                    "PortIo",
                    "InterruptControl",
                    "InterruptEntry",
                    "RootMemoryAccess",
                    "ProcessInput",
                ],
            }
        }
    }

    #[derive(Debug, Clone, PartialEq, Eq, compiler_storage::Record)]
    pub struct EnumExclusionRequest {
        pub source_span: source::SourceSpan,
        pub vocabulary: ExclusionVocabulary,
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct ExecutedEnumExclusion {
        pub request: EnumExclusionRequest,
        pub code: u64,
        pub selecting_machine: source::SourceSpan,
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq, compiler_storage::Record)]
    pub enum ProductOperandRole {
        BoundarySubject,
        Provider,
        Requirement,
        Data,
        Representation,
        WireCompatibilityFact,
    }

    impl BuildOperation {
        pub const fn product_operand_role(self, position: usize) -> Option<ProductOperandRole> {
            use ProductOperandRole as Role;
            match (self, position) {
                (Self::ProviderSelection, 0)
                | (Self::ServiceExclusion | Self::ServiceAudit | Self::BoundaryAcceptance, 0) => {
                    Some(Role::BoundarySubject)
                }
                (Self::ProviderSelection, 1) => Some(Role::Provider),
                (Self::AssumedBindingContract, 0 | 1) | (Self::NativeBinding, 0) => {
                    Some(Role::Requirement)
                }
                (Self::RepresentationSelection, 0) | (Self::WireCompatibilityRequest, 0..=3) => {
                    Some(Role::Data)
                }
                (Self::RepresentationSelection, 1) => Some(Role::Representation),
                (Self::WireCompatibilityRequest, 4..) => Some(Role::WireCompatibilityFact),
                _ => None,
            }
        }

        pub const fn product_operand_count_valid(self, count: usize) -> bool {
            match self {
                Self::NativeBinding => count == 1,
                Self::ProviderSelection
                | Self::RepresentationSelection
                | Self::AssumedBindingContract => count == 2,
                Self::ServiceExclusion | Self::ServiceAudit | Self::BoundaryAcceptance => {
                    count == 1
                }
                Self::WireCompatibilityRequest => count >= 5,
                _ => false,
            }
        }
    }

    #[derive(Debug, Clone, PartialEq, Eq, compiler_storage::Record)]
    pub struct ProductDeclarationRequest {
        pub source_span: source::SourceSpan,
        pub operation: super::BuildOperation,
        pub operands: Box<[ProductDeclarationOperand]>,
    }

    #[derive(Debug, Clone, Default, PartialEq, Eq, compiler_storage::Record)]
    pub struct ProductDeclarationOperand {
        pub path: String,
        pub source_span: source::SourceSpan,
        pub application: Option<std::sync::Arc<super::product_operands::ProductStaticOperandGraph>>,
    }

    impl ProductDeclarationRequest {
        pub fn validate_shape(&self) -> Result<(), String> {
            if !self
                .operation
                .product_operand_count_valid(self.operands.len())
            {
                return Err("Build product request has the wrong declaration operand count".into());
            }
            if self.source_span.span.start >= self.source_span.span.end {
                return Err("Build product request has no authored occurrence".into());
            }
            for (position, operand) in self.operands.iter().enumerate() {
                if operand.path.is_empty()
                    || operand.source_span.span.start >= operand.source_span.span.end
                {
                    return Err("Build product operand has no authored declaration locator".into());
                }
                if self.operation.product_operand_role(position)
                    == Some(ProductOperandRole::WireCompatibilityFact)
                {
                    let fact = crate::wire::WireCompatibilityFact::from_name(&operand.path)
                        .ok_or_else(|| {
                            format!("unknown wire compatibility fact `{}`", operand.path)
                        })?;
                    if self.operands[..position]
                        .iter()
                        .enumerate()
                        .any(|(index, previous)| {
                            self.operation.product_operand_role(index)
                                == Some(ProductOperandRole::WireCompatibilityFact)
                                && crate::wire::WireCompatibilityFact::from_name(&previous.path)
                                    == Some(fact)
                        })
                    {
                        return Err(format!(
                            "wire compatibility fact `{}` is requested twice",
                            operand.path
                        ));
                    }
                }
            }
            Ok(())
        }
    }

    #[derive(Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
    pub enum CompositionMode {
        #[default]
        Fused,
        Independent,
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub enum ExecutedBuildSelection {
        Entry(ExecutedRootBinding),
        Declaration(ExecutedProductSelection),
        Exclusion(ExecutedEnumExclusion),
        Native(ExecutedNativeBinding),
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct ExecutedProductSelection {
        pub static_environment:
            Option<std::sync::Arc<super::product_operands::ProductStaticEnvironment>>,
        pub target: Option<super::ProductTarget>,
        pub request: std::sync::Arc<ProductDeclarationRequest>,
        pub selecting_machine: Option<source::SourceSpan>,
        pub composition: Option<CompositionMode>,
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct ExecutedNativeBinding {
        pub request: std::sync::Arc<ProductDeclarationRequest>,
        pub target: super::ProductTarget,
        pub locator: semantic_vocabulary::NativeBindingLocator,
        pub selecting_machine: source::SourceSpan,
    }

    #[derive(Debug, Clone, Default, PartialEq, Eq, compiler_storage::Record)]
    pub struct RootBindingRequest {
        pub source_span: source::SourceSpan,
        pub slot: String,
        pub implementation: RootBindingOperand,
    }

    #[derive(Debug, Clone, Default, PartialEq, Eq, compiler_storage::Record)]
    pub enum RootBindingOperand {
        AuthoredPath(String),
        #[default]
        Description,
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct DescribedProductEntry {
        pub declaration: symbols::RetainedDeclaration,
        pub machine_name: String,
        pub slot: String,
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct DescribedProductSchema {
        pub declaration: symbols::RetainedDeclaration,
        pub canonical_path: String,
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct DescribedProductProvider {
        pub declaration: symbols::RetainedDeclaration,
        pub canonical_path: String,
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub enum RootBindingSelection {
        AuthoredPath(String),
        Described(DescribedProductEntry),
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct ExecutedRootBinding {
        pub source_span: source::SourceSpan,
        pub slot: String,
        pub implementation: RootBindingSelection,
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct NativeBindingProducer {
        pub package: semantic_vocabulary::PackageKeyIdentity,
        pub declaration_path: String,
        pub callable_identity: String,
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct NativeBindingSource {
        pub producer: NativeBindingProducer,
        pub requirement_path: String,
        pub source_occurrence_digest: [u8; 32],
        pub resolution_stratum: source::SourceResolutionStratum,
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct CapturedNativeDeclaration {
        pub source: NativeBindingSource,
        pub requirement: symbols::RetainedDeclaration,
        pub requirement_identity: String,
        /// Portable identity of the owning boundary's closed static
        /// application when the requirement belongs to a generic boundary;
        /// a receiving package matches it against its own use sites.
        pub requirement_application: Option<String>,
        pub dependency_alias: Option<String>,
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct CapturedNativeIntent {
        pub declaration: u32,
        pub target: ProductTarget,
        pub locator: semantic_vocabulary::NativeBindingLocator,
    }

    #[derive(Debug, Clone, PartialEq, Eq, compiler_storage::Record)]
    pub struct NativeBindingFormation {
        pub request: std::sync::Arc<ProductDeclarationRequest>,
        pub author: source::SourceSpan,
        pub author_package: Option<semantic_vocabulary::PackageKeyIdentity>,
        pub root_scope: bool,
        pub declaration_path: String,
        pub callable_identity: String,
        pub source_occurrence_digest: [u8; 32],
        pub resolution_stratum: source::SourceResolutionStratum,
    }
    impl NativeBindingFormation {
        pub fn seal(
            &self,
            package: semantic_vocabulary::PackageKeyIdentity,
        ) -> Result<NativeBindingSource, String> {
            if self.author_package.is_some_and(|owner| owner != package) || !self.root_scope {
                return Err(
                    "native output cannot substitute another checked Build package's scope".into(),
                );
            }
            let [operand] = self.request.operands.as_ref() else {
                return Err("native formation lost its requirement".into());
            };
            Ok(NativeBindingSource {
                producer: NativeBindingProducer {
                    package,
                    declaration_path: self.declaration_path.clone(),
                    callable_identity: self.callable_identity.clone(),
                },
                requirement_path: operand.path.clone(),
                source_occurrence_digest: self.source_occurrence_digest,
                resolution_stratum: self.resolution_stratum,
            })
        }
    }

    #[cfg(test)]
    mod native_formation_tests {
        use super::*;
        #[test]
        fn native_output_never_crosses_checked_package_scope() {
            let package = semantic_vocabulary::PackageKeyIdentity::from_digest([7; 32]).unwrap();
            let other = semantic_vocabulary::PackageKeyIdentity::from_digest([8; 32]).unwrap();
            let mut formation = NativeBindingFormation {
                request: std::sync::Arc::new(ProductDeclarationRequest {
                    source_span: Default::default(),
                    operation: BuildOperation::NativeBinding,
                    operands: vec![ProductDeclarationOperand {
                        application: None,
                        path: "Raw::read".into(),
                        source_span: Default::default(),
                    }]
                    .into_boxed_slice(),
                }),
                author: Default::default(),
                author_package: Some(package),
                root_scope: true,
                declaration_path: "build".into(),
                callable_identity: "build(&mut Build)".into(),
                source_occurrence_digest: [9; 32],
                resolution_stratum: source::SourceResolutionStratum::Base,
            };
            assert_eq!(formation.seal(package).unwrap().producer.package, package);
            formation.root_scope = false;
            assert!(formation.seal(package).is_err());
            formation.root_scope = true;
            assert!(formation.seal(other).is_err());
            formation.author_package = None;
            assert!(formation.seal(package).is_ok());
            formation.root_scope = false;
            assert!(formation.seal(package).is_err());
        }
    }
}
pub use build_selection_origin::BuildSelectionOrigin;
pub mod product_operands {

    use arena::{Arena, Handle, HandleSpan};
    use source::SourceSpan;

    #[derive(Debug, Clone, Default, PartialEq, Eq, compiler_storage::Record)]
    pub struct ProductOperandName {
        pub text: String,
        pub source_span: SourceSpan,
        pub declaration: Option<SourceSpan>,
        pub builtin_type: Option<symbols::BuiltinTypeAtom>,
        pub builtin_function: Option<symbols::BuiltinFunction>,
        pub authored_selection: Handle<ProductOperandSelection>,
    }

    #[derive(Debug, Clone, Default, PartialEq, Eq, compiler_storage::Record)]
    pub struct ProductStaticOperandGraph {
        pub root: Handle<ProductStaticOperand>,
        pub names: Arena<ProductOperandName>,
        pub operands: Arena<ProductStaticOperand>,
        pub applications: Arena<ProductStaticApplication>,
        pub types: Arena<ProductOperandType>,
        pub expressions: Arena<ProductOperandExpression>,
        pub selections: Arena<ProductOperandSelection>,
        pub constraints: Arena<ProductOperandConstraint>,
        pub fields: Arena<ProductOperandField>,
        pub match_subjects: Arena<ProductMatchSubject>,
        pub match_arms: Arena<ProductMatchArm>,
        pub match_fields: Arena<ProductMatchField>,
        pub proof_selectors: Arena<ProductProofSelector>,
        pub type_arguments: Arena<Handle<ProductOperandType>>,
        pub static_arguments: Arena<Handle<ProductStaticOperand>>,
        pub expression_arguments: Arena<Handle<ProductOperandExpression>>,
    }

    #[derive(Debug, Clone, Default, PartialEq, Eq, compiler_storage::Record)]
    pub struct ProductStaticOperand {
        pub authored_selection: Handle<ProductOperandSelection>,
        pub path: HandleSpan<ProductOperandName>,
        pub application: Handle<ProductStaticApplication>,
        pub structural_type: Handle<ProductOperandType>,
        pub integer: Option<numerics::literals::IntegerLiteral>,
        pub projection: Option<(Handle<ProductOperandName>, Handle<ProductOperandName>)>,
        pub declaration: Option<SourceSpan>,
    }

    #[derive(Debug, Clone, Default, PartialEq, Eq, compiler_storage::Record)]
    pub struct ProductStaticApplication {
        pub lifetimes: HandleSpan<ProductOperandName>,
        pub arguments: HandleSpan<Handle<ProductStaticOperand>>,
        pub member: Handle<ProductStaticOperand>,
    }

    #[derive(Debug, Clone, Default, PartialEq, Eq, compiler_storage::Record)]
    pub enum ProductOperandType {
        CanonicalConst(crate::const_value::CanonicalConstValue),
        #[default]
        Invalid,
        Unit,
        Named(Handle<ProductOperandName>),
        Generic {
            name: Handle<ProductOperandName>,
            lifetimes: HandleSpan<ProductOperandName>,
            arguments: HandleSpan<Handle<ProductOperandType>>,
        },
        Reference {
            referee: Handle<ProductOperandType>,
            access: language_core::ReferenceAccess,
            lifetime: Option<Handle<ProductOperandName>>,
        },
        Array {
            element: Handle<ProductOperandType>,
            length: ProductArrayLength,
        },
        Slice(Handle<ProductOperandType>),
        Constrained {
            base: Handle<ProductOperandType>,
            constraints: HandleSpan<ProductOperandConstraint>,
        },
        Dynamic {
            name: Handle<ProductOperandName>,
            arguments: HandleSpan<Handle<ProductOperandType>>,
        },
        ConstExpression(Handle<ProductOperandExpression>),
    }

    #[derive(Debug, Clone, PartialEq, Eq, compiler_storage::Record)]
    pub enum ProductArrayLength {
        Literal(usize),
        Expression(Handle<ProductOperandType>),
        Parameter(Handle<ProductOperandName>),
        Call(Handle<ProductOperandName>),
    }

    #[derive(Debug, Clone, Default, PartialEq, Eq, compiler_storage::Record)]
    pub enum ProductOperandConstraint {
        #[default]
        Invalid,
        Named(Handle<ProductOperandName>),

        Arithmetic(numerics::arithmetic::ArithmeticDomain),
        Domain {
            name: Handle<ProductOperandName>,
            arguments: HandleSpan<Handle<ProductOperandType>>,
        },
    }

    #[derive(Debug, Clone, Default, PartialEq, Eq, compiler_storage::Record)]
    pub struct ProductOperandExpression {
        pub source_span: SourceSpan,
        pub authored_selections: HandleSpan<ProductOperandSelection>,
        pub kind: ProductExpressionKind,
    }

    #[derive(Debug, Clone, Default, PartialEq, Eq, compiler_storage::Record)]
    pub struct ProductOperandSelection {
        pub source_span: SourceSpan,
        pub exposure: super::AuthoredDeclarationSelectionExposure,
        pub kind: Option<super::AuthoredDeclarationSelectionKind>,
    }

    #[derive(Debug, Clone, Default, PartialEq, Eq, compiler_storage::Record)]
    pub struct ProductOperandField {
        pub name: Handle<ProductOperandName>,
        pub value: Handle<ProductOperandExpression>,
    }

    #[derive(Debug, Clone, Default, PartialEq, Eq, compiler_storage::Record)]
    pub enum ProductExpressionKind {
        #[default]
        Invalid,
        Match {
            subject: Handle<ProductOperandExpression>,
            capture: Handle<ProductMatchSubject>,
            arms: HandleSpan<ProductMatchArm>,
        },
        SavedSubject {
            component: language_core::PatternComponent,
            capture: Handle<ProductMatchSubject>,
            pattern_head: Handle<ProductOperandExpression>,
        },
        Borrow {
            target: Handle<ProductOperandExpression>,
            access: language_core::ReferenceAccess,
        },
        Atomic {
            value: Handle<ProductOperandExpression>,
            result: Handle<ProductOperandExpression>,
            ordering: language_core::atomic::AtomicOrderingPlan,
            result_custody: language_core::atomic::AtomicExpressionResultCustody,
        },
        Range {
            start: Handle<ProductOperandExpression>,
            end: Handle<ProductOperandExpression>,
            inclusive: bool,
        },
        Membership {
            value: Handle<ProductOperandExpression>,
            domain: HandleSpan<ProductOperandName>,
            declaration: Option<SourceSpan>,
            case_type: Option<SourceSpan>,
            case: Option<SourceSpan>,
        },
        ZeroValue(Handle<ProductOperandType>),
        UndeterminedNaN(numerics::literals::FloatFormat),
        Integer(numerics::literals::IntegerLiteral),
        Boolean(bool),
        Float(numerics::literals::FloatLiteral),
        Text(Vec<u8>),
        Name(HandleSpan<ProductOperandName>),
        Unary {
            operator: ProductUnaryOperator,
            operand: Handle<ProductOperandExpression>,
        },
        Binary {
            left: Handle<ProductOperandExpression>,
            operator: ProductBinaryOperator,
            right: Handle<ProductOperandExpression>,
        },
        Call {
            receiver: Handle<ProductOperandExpression>,
            target: Handle<ProductOperandName>,
            owner: Handle<ProductOperandType>,
            static_arguments: HandleSpan<Handle<ProductStaticOperand>>,
            arguments: HandleSpan<Handle<ProductOperandExpression>>,
            evidence: HandleSpan<ProductOperandName>,
            acknowledgement: crate::CallOperationalAcknowledgement,
        },
        Member {
            receiver: Handle<ProductOperandExpression>,
            member: Handle<ProductOperandName>,
        },
        Indexed {
            receiver: Handle<ProductOperandExpression>,
            index: Handle<ProductOperandExpression>,
        },
        Array(HandleSpan<Handle<ProductOperandExpression>>),
        Record {
            name: Handle<ProductOperandName>,
            case: Option<Handle<ProductOperandName>>,
            fields: HandleSpan<ProductOperandField>,
        },
        Cast {
            value: Handle<ProductOperandExpression>,
            target: Handle<ProductOperandType>,
            policy: numerics::arithmetic::ArithmeticDomain,
            domain: HandleSpan<ProductOperandName>,
            domain_arguments: HandleSpan<Handle<ProductOperandType>>,
            form: language_core::cast_form::CastForm,
        },
        TypeExpression(Handle<ProductOperandType>),
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq, compiler_storage::Record)]
    pub enum ProductUnaryOperator {
        BitwiseNot,
        LogicalNot,
    }
    #[derive(Debug, Clone, Copy, PartialEq, Eq, compiler_storage::Record)]
    pub enum ProductBinaryOperator {
        CaseMembership,
        Add,
        And,
        BitwiseAnd,
        BitwiseOr,
        BitwiseXor,
        Divide,
        Equal,
        Greater,
        GreaterOrEqual,
        Less,
        LessOrEqual,
        Modulo,
        Multiply,
        NotEqual,
        Or,
        ShiftLeft,
        ShiftRight,
        Subtract,
    }

    #[derive(Debug, Clone, Default, PartialEq, Eq, compiler_storage::Record)]
    pub struct ProductMatchSubject {
        pub subject: Handle<ProductOperandExpression>,
    }
    #[derive(Debug, Clone, Default, PartialEq, Eq, compiler_storage::Record)]
    pub struct ProductMatchArm {
        pub pattern: ProductMatchPattern,
        pub value: Handle<ProductOperandExpression>,
        pub source_span: SourceSpan,
    }
    #[derive(Debug, Clone, Default, PartialEq, Eq, compiler_storage::Record)]
    pub enum ProductMatchPattern {
        #[default]
        Wildcard,
        Value(Handle<ProductOperandExpression>),
        Destructure {
            head: Handle<ProductOperandExpression>,
            fields: HandleSpan<ProductMatchField>,
            has_rest: bool,
            guard: Handle<ProductOperandExpression>,
            condition: Handle<ProductOperandExpression>,
            proof_selectors: HandleSpan<ProductProofSelector>,
        },
    }
    #[derive(Debug, Clone, Default, PartialEq, Eq, compiler_storage::Record)]
    pub struct ProductMatchField {
        pub component: language_core::PatternComponent,
        pub member: Handle<ProductOperandName>,
        pub action: ProductMatchFieldAction,
    }
    #[derive(Debug, Clone, Default, PartialEq, Eq, compiler_storage::Record)]
    pub enum ProductMatchFieldAction {
        Bind(Handle<ProductOperandName>),
        Equal(Handle<ProductOperandExpression>),
        #[default]
        Waive,
    }
    #[derive(Debug, Clone, Default, PartialEq, Eq, compiler_storage::Record)]
    pub struct ProductProofSelector {
        pub output: Handle<ProductOperandName>,
        pub binding: Handle<ProductOperandName>,
    }

    #[derive(Debug, Clone, Default, PartialEq, Eq, compiler_storage::Record)]
    pub struct ProductStaticEnvironment {
        pub identity: Vec<u8>,
        pub template: Option<SourceSpan>,
        pub lexical_owner: Option<SourceSpan>,
        pub graph: ProductStaticOperandGraph,
        pub bindings: Arena<ProductStaticEnvironmentBinding>,
        pub lifetimes: Arena<ProductStaticEnvironmentLifetime>,
    }

    #[derive(Debug, Clone, Default, PartialEq, Eq, compiler_storage::Record)]
    pub struct ProductStaticEnvironmentBinding {
        pub parameter: SourceSpan,
        pub argument: Handle<ProductStaticOperand>,
        pub lexical_owner: SourceSpan,
        pub declared_type: Handle<ProductOperandType>,
    }

    #[derive(Debug, Clone, Default, PartialEq, Eq, compiler_storage::Record)]
    pub struct ProductStaticEnvironmentLifetime {
        pub parameter_owner: SourceSpan,
        pub parameter_ordinal: u32,
        pub argument: Handle<ProductOperandName>,
        pub lexical_owner: SourceSpan,
    }

    impl ProductStaticOperandGraph {
        pub fn remap_source_spans(&mut self, mut remap: impl FnMut(SourceSpan) -> SourceSpan) {
            self.names.for_each_mut(|_, name| {
                name.source_span = remap(name.source_span);
                name.declaration = name.declaration.map(&mut remap);
            });
            self.operands.for_each_mut(|_, operand| {
                operand.declaration = operand.declaration.map(&mut remap);
            });
            self.expressions.for_each_mut(|_, expression| {
                expression.source_span = remap(expression.source_span);
                if let ProductExpressionKind::Membership {
                    declaration,
                    case_type,
                    case,
                    ..
                } = &mut expression.kind
                {
                    *declaration = declaration.map(&mut remap);
                    *case_type = case_type.map(&mut remap);
                    *case = case.map(&mut remap);
                }
            });
            self.selections
                .for_each_mut(|_, selection| selection.source_span = remap(selection.source_span));
            self.match_arms
                .for_each_mut(|_, arm| arm.source_span = remap(arm.source_span));
        }

        pub fn authored_operand_span(&self, leading: SourceSpan) -> SourceSpan {
            let mut end = leading.span.end;
            let mut include = |span: SourceSpan| {
                if span.source_id == leading.source_id && span.span.start >= leading.span.start {
                    end = end.max(span.span.end);
                }
            };
            for (_, name) in self.names.iter() {
                include(name.source_span);
            }
            for (_, expression) in self.expressions.iter() {
                include(expression.source_span);
            }
            for (_, arm) in self.match_arms.iter() {
                include(arm.source_span);
            }
            SourceSpan::new(
                leading.source_id,
                source::Span::new(leading.span.start, end),
            )
        }
    }
}
mod product_target {
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
    pub enum ProductTarget {
        LinuxArm64,
        LinuxX86_64,
        MacosArm64,
        MacosX86_64,
        WindowsX86_64,
        UefiX86_64,
        CrossPlatformCli,
        LocalUnchecked,
        AlphaBootstrap,
    }

    impl ProductTarget {
        pub const ALL: [Self; 9] = [
            Self::LinuxArm64,
            Self::LinuxX86_64,
            Self::MacosArm64,
            Self::MacosX86_64,
            Self::WindowsX86_64,
            Self::UefiX86_64,
            Self::CrossPlatformCli,
            Self::LocalUnchecked,
            Self::AlphaBootstrap,
        ];

        pub fn from_code(code: u64) -> Option<Self> {
            usize::try_from(code)
                .ok()
                .and_then(|index| Self::ALL.get(index))
                .copied()
        }

        pub fn from_canonical_name(name: &str) -> Option<Self> {
            Self::ALL
                .into_iter()
                .find(|target| target.canonical_name() == name)
        }

        pub fn code(self) -> u64 {
            Self::ALL
                .into_iter()
                .position(|target| target == self)
                .expect("complete target catalog") as u64
        }

        pub const fn canonical_name(self) -> &'static str {
            match self {
                Self::LinuxArm64 => "linux_arm64",
                Self::LinuxX86_64 => "linux_x86_64",
                Self::MacosArm64 => "macos_arm64",
                Self::MacosX86_64 => "macos_x86_64",
                Self::WindowsX86_64 => "windows_x86_64",
                Self::UefiX86_64 => "uefi_x86_64",
                Self::CrossPlatformCli => "cross_platform_cli",
                Self::LocalUnchecked => "local_unchecked",
                Self::AlphaBootstrap => "alpha_bootstrap",
            }
        }

        pub const fn source_case(self) -> &'static str {
            match self {
                Self::LinuxArm64 => "LinuxArm64",
                Self::LinuxX86_64 => "LinuxX86_64",
                Self::MacosArm64 => "MacosArm64",
                Self::MacosX86_64 => "MacosX86_64",
                Self::WindowsX86_64 => "WindowsX86_64",
                Self::UefiX86_64 => "UefiX86_64",
                Self::CrossPlatformCli => "CrossPlatformCli",
                Self::LocalUnchecked => "LocalUnchecked",
                Self::AlphaBootstrap => "AlphaBootstrap",
            }
        }
    }
}
pub use product::{
    BuildSelectionRequest, CompositionMode, DescribedProductEntry, DescribedProductProvider,
    DescribedProductSchema, EnumExclusionRequest, ExclusionVocabulary, ExecutedBuildSelection,
    ExecutedEnumExclusion, ExecutedNativeBinding, ExecutedProductSelection, ExecutedRootBinding,
    ProductDeclarationOperand, ProductDeclarationRequest, ProductOperandRole,
    ProviderReadinessRequest, RootBindingOperand, RootBindingRequest, RootBindingSelection,
};
pub use product_target::ProductTarget;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, compiler_storage::Record)]
pub enum AuthoredDeclarationSelectionExposure {
    #[default]
    PrivateImplementation,
    PublicInterface,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, compiler_storage::Record)]
pub enum AuthoredDeclarationSelectionKind {
    TypeReference,
    StaticPathSegment,
    MemberAccess,
    StructLiteralType,
    StructLiteralCase,
    StructLiteralField,
    CaseReference,
    CaseMembership,
    DomainMembership,
    DomainIssuerAuthorization,
    DomainCleanupSelection,
    Call,
    StaticArgument,
    Operator,
    Conformance,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, compiler_storage::Record)]
pub enum AuthoredDeclarationSelectionLateBinding {
    CheckedStaticPathSegment,
    CheckedMember,
    CheckedStructLiteralType,
    CheckedStructLiteralCase,
    CheckedStructLiteralField,
    CheckedCaseMembership,
    CheckedDomainMembership,
    CheckedCall,
    CheckedStaticArgument,
    CheckedOperator,
    CheckedConformance,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, compiler_storage::Record)]
pub struct ResolvedAuthoredDeclarationSelection {
    selected_symbol: SymbolHandle,
}

impl ResolvedAuthoredDeclarationSelection {
    pub fn new(selected_symbol: SymbolHandle) -> Option<Self> {
        selected_symbol
            .is_valid()
            .then_some(Self { selected_symbol })
    }

    pub fn selected_symbol(self) -> SymbolHandle {
        self.selected_symbol
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, compiler_storage::Record)]
pub enum AuthoredDeclarationSelectionTarget {
    Resolved(ResolvedAuthoredDeclarationSelection),
    Intrinsic(AuthoredDeclarationSelectionIntrinsic),
    LateBound(AuthoredDeclarationSelectionLateBinding),
}

impl AuthoredDeclarationSelectionTarget {
    pub fn as_resolved(&self) -> Option<&ResolvedAuthoredDeclarationSelection> {
        let AuthoredDeclarationSelectionTarget::Resolved(resolved) = self else {
            return None;
        };
        Some(resolved)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CollectionMeasure {
    Length,
    Capacity,
}

impl CollectionMeasure {
    pub const ALL: [Self; 2] = [Self::Length, Self::Capacity];

    pub const fn authored_spelling(self) -> &'static str {
        match self {
            Self::Length => "len",
            Self::Capacity => "capacity",
        }
    }

    pub fn from_authored_spelling(spelling: &str) -> Option<Self> {
        Self::ALL
            .into_iter()
            .find(|measure| measure.authored_spelling() == spelling)
    }

    pub const fn intrinsic(self) -> AuthoredDeclarationSelectionIntrinsic {
        match self {
            Self::Length => AuthoredDeclarationSelectionIntrinsic::CollectionLength,
            Self::Capacity => AuthoredDeclarationSelectionIntrinsic::CollectionCapacity,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, compiler_storage::Record)]
pub enum BuildOperation {
    ProviderReadiness,
    ProviderSelection,
    AssumedBindingContract,
    RepresentationSelection,
    ServiceExclusion,
    ServiceAudit,
    OptimizationSelection,
    OptimizationReportRequest,
    BoundaryAcceptance,
    WireCompatibilityRequest,
    IncludedSourceHandoff,
    LogWriteLine,
    NativeBinding,
}
pub fn is_unresolved_provider_selection_call(
    target: &str,
    target_symbol: symbols::SymbolHandle,
) -> bool {
    BuildOperation::from_call_target(target) == Some(BuildOperation::ProviderSelection)
        && !target_symbol.is_valid()
}

impl BuildOperation {
    pub const fn has_designated_operands(self) -> bool {
        self.selects_product_declarations() || matches!(self, Self::ProviderReadiness)
    }
    pub const fn selects_product_declarations(self) -> bool {
        matches!(
            self,
            Self::ProviderSelection
                | Self::NativeBinding
                | Self::ServiceExclusion
                | Self::ServiceAudit
                | Self::BoundaryAcceptance
                | Self::WireCompatibilityRequest
                | Self::RepresentationSelection
                | Self::AssumedBindingContract
        )
    }

    pub const ALL: [Self; 13] = [
        Self::ProviderReadiness,
        Self::ProviderSelection,
        Self::AssumedBindingContract,
        Self::RepresentationSelection,
        Self::ServiceExclusion,
        Self::ServiceAudit,
        Self::OptimizationSelection,
        Self::OptimizationReportRequest,
        Self::BoundaryAcceptance,
        Self::WireCompatibilityRequest,
        Self::IncludedSourceHandoff,
        Self::LogWriteLine,
        Self::NativeBinding,
    ];

    pub const fn authored_spelling(self) -> &'static str {
        match self {
            Self::NativeBinding => "bind",
            Self::ProviderReadiness => "await_provider",
            Self::ProviderSelection => "select_provider",
            Self::AssumedBindingContract => "assume_binding_contract",
            Self::RepresentationSelection => "select_representation",
            Self::ServiceExclusion => "exclude_service",
            Self::ServiceAudit => "audit_service",
            Self::OptimizationSelection => "enable",
            Self::OptimizationReportRequest => "emit_report",
            Self::BoundaryAcceptance => "accept_boundary",
            Self::WireCompatibilityRequest => "require_wire_compatibility",
            Self::IncludedSourceHandoff => "include_source",
            Self::LogWriteLine => "write_line",
        }
    }

    pub fn from_call_target(target: &str) -> Option<Self> {
        Self::ALL
            .into_iter()
            .find(|operation| operation.authored_spelling() == target)
    }

    pub const fn intrinsic(self) -> AuthoredDeclarationSelectionIntrinsic {
        match self {
            Self::NativeBinding => AuthoredDeclarationSelectionIntrinsic::BuildNativeBinding,
            Self::ProviderReadiness => {
                AuthoredDeclarationSelectionIntrinsic::BuildProviderReadiness
            }
            Self::ProviderSelection => {
                AuthoredDeclarationSelectionIntrinsic::BuildProviderSelection
            }
            Self::AssumedBindingContract => {
                AuthoredDeclarationSelectionIntrinsic::BuildAssumedBindingContract
            }
            Self::RepresentationSelection => {
                AuthoredDeclarationSelectionIntrinsic::BuildRepresentationSelection
            }
            Self::ServiceExclusion => AuthoredDeclarationSelectionIntrinsic::BuildServiceExclusion,
            Self::ServiceAudit => AuthoredDeclarationSelectionIntrinsic::BuildServiceAudit,
            Self::OptimizationSelection => {
                AuthoredDeclarationSelectionIntrinsic::BuildOptimizationSelection
            }
            Self::OptimizationReportRequest => {
                AuthoredDeclarationSelectionIntrinsic::BuildOptimizationReportRequest
            }
            Self::BoundaryAcceptance => {
                AuthoredDeclarationSelectionIntrinsic::BuildBoundaryAcceptance
            }
            Self::WireCompatibilityRequest => {
                AuthoredDeclarationSelectionIntrinsic::BuildWireCompatibilityRequest
            }
            Self::IncludedSourceHandoff => {
                AuthoredDeclarationSelectionIntrinsic::BuildIncludedSourceHandoff
            }
            Self::LogWriteLine => AuthoredDeclarationSelectionIntrinsic::BuildLogWriteLine,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, compiler_storage::Record)]
pub enum AuthoredDeclarationSelectionIntrinsic {
    BuildNativeBinding,
    BuildProviderReadiness,
    BuiltinOperator,
    CarryPermission(crate::CarryPermission),
    CollectionLength,
    CollectionCapacity,
    ByteSequencePredicate(crate::byte_predicates::ByteSequencePredicate),
    StorageAddress,
    BuildProductOperand {
        operation: BuildOperation,
        position: usize,
        role: ProductOperandRole,
    },
    BuildProviderSelection,
    BuildAssumedBindingContract,
    BuildRepresentationSelection,
    BuildServiceExclusion,
    BuildServiceAudit,
    BuildOptimizationSelection,
    BuildOptimizationReportRequest,
    BuildBoundaryAcceptance,
    BuildWireCompatibilityRequest,
    WireCompatibilityFact(crate::wire::WireCompatibilityFact),
    BuildIncludedSourceHandoff,
    BuildLogWriteLine,
    WireEncode,
    WireDecode,
    InlineAssemblyOperation,
    ProofView,
    QuotientDefine,
    QuotientLift,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, compiler_storage::Record)]
pub struct AuthoredDeclarationSelectionOccurrenceId(u64);

impl AuthoredDeclarationSelectionOccurrenceId {
    pub fn ordinal(self) -> u64 {
        self.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, compiler_storage::Record)]
pub struct CompilerDerivedSelectionPartition(u64);

impl CompilerDerivedSelectionPartition {
    pub fn from_compiler_ordinal(ordinal: u64) -> Self {
        Self(ordinal)
    }

    pub fn ordinal(self) -> u64 {
        self.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, compiler_storage::Record)]
pub enum AuthoredDeclarationSelectionCoordinateError {
    Missing,
    Ambiguous,
    CompilerCopyRequiresOwner,
}

impl std::fmt::Display for AuthoredDeclarationSelectionCoordinateError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(match self {
            Self::Missing => "authored selection source coordinate has no current ledger row",
            Self::Ambiguous => "authored selection source coordinate has multiple current ledger rows",
            Self::CompilerCopyRequiresOwner =>
                "authored selection belongs to a compiler-derived copy whose exact semantic owner is required",
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AuthoredDeclarationSelectionRecordError {
    InvalidSelectedSymbol,
    OccurrenceCapacityExceeded,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AuthoredDeclarationSelectionFinalizationError {
    UnknownOccurrence,
    AlreadyResolved,
    LateBindingMismatch,
    InvalidSelectedSymbol,
    StaticArgumentPositionMismatch,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AuthoredDeclarationSelectionSuffixRebaseError {
    SourceFrontierOutOfRange,
    DestinationPrefixTooShort,
    PrefixIdentityMismatch,
    PrefixTargetMismatch,
    OccurrenceCapacityExceeded,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AuthoredDeclarationSelectionSuffixRebase {
    source_frontier: u64,
    source_end: u64,
    destination_frontier: u64,
}

impl AuthoredDeclarationSelectionSuffixRebase {
    pub fn retain_base(
        self,
        occurrence: AuthoredDeclarationSelectionOccurrenceId,
    ) -> Option<AuthoredDeclarationSelectionOccurrenceId> {
        (occurrence.0 < self.source_frontier).then_some(occurrence)
    }

    pub fn rebase_extension(
        self,
        occurrence: AuthoredDeclarationSelectionOccurrenceId,
    ) -> Option<AuthoredDeclarationSelectionOccurrenceId> {
        if occurrence.0 < self.source_frontier || occurrence.0 >= self.source_end {
            return None;
        }
        let suffix_offset = occurrence.0.checked_sub(self.source_frontier)?;
        Some(AuthoredDeclarationSelectionOccurrenceId(
            self.destination_frontier.checked_add(suffix_offset)?,
        ))
    }

    pub fn rebase_appended(
        self,
        occurrence: AuthoredDeclarationSelectionOccurrenceId,
    ) -> Option<AuthoredDeclarationSelectionOccurrenceId> {
        self.retain_base(occurrence)
            .or_else(|| self.rebase_extension(occurrence))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, compiler_storage::Record)]
pub struct AuthoredDeclarationSelection {
    occurrence_id: AuthoredDeclarationSelectionOccurrenceId,
    source_span: SourceSpan,
    exposure: AuthoredDeclarationSelectionExposure,
    kind: AuthoredDeclarationSelectionKind,
    compiler_partition: Option<CompilerDerivedSelectionPartition>,
    target: AuthoredDeclarationSelectionTarget,
    top_level_static_argument_position: Option<usize>,
}

impl AuthoredDeclarationSelection {
    fn resolved(
        occurrence_id: AuthoredDeclarationSelectionOccurrenceId,
        source_span: SourceSpan,
        exposure: AuthoredDeclarationSelectionExposure,
        kind: AuthoredDeclarationSelectionKind,
        compiler_partition: Option<CompilerDerivedSelectionPartition>,
        selected_symbol: SymbolHandle,
    ) -> Result<Self, AuthoredDeclarationSelectionRecordError> {
        let selected = ResolvedAuthoredDeclarationSelection::new(selected_symbol)
            .ok_or(AuthoredDeclarationSelectionRecordError::InvalidSelectedSymbol)?;
        Ok(Self {
            occurrence_id,
            source_span,
            exposure,
            kind,
            compiler_partition,
            target: AuthoredDeclarationSelectionTarget::Resolved(selected),
            top_level_static_argument_position: None,
        })
    }

    fn late_bound(
        occurrence_id: AuthoredDeclarationSelectionOccurrenceId,
        source_span: SourceSpan,
        exposure: AuthoredDeclarationSelectionExposure,
        kind: AuthoredDeclarationSelectionKind,
        compiler_partition: Option<CompilerDerivedSelectionPartition>,
        late_binding: AuthoredDeclarationSelectionLateBinding,
    ) -> Self {
        Self {
            occurrence_id,
            source_span,
            exposure,
            kind,
            compiler_partition,
            target: AuthoredDeclarationSelectionTarget::LateBound(late_binding),
            top_level_static_argument_position: None,
        }
    }

    pub fn occurrence_id(self) -> AuthoredDeclarationSelectionOccurrenceId {
        self.occurrence_id
    }

    pub fn source_span(self) -> SourceSpan {
        self.source_span
    }

    pub fn exposure(self) -> AuthoredDeclarationSelectionExposure {
        self.exposure
    }

    pub fn kind(self) -> AuthoredDeclarationSelectionKind {
        self.kind
    }

    pub fn compiler_partition(self) -> Option<CompilerDerivedSelectionPartition> {
        self.compiler_partition
    }

    pub fn target(self) -> AuthoredDeclarationSelectionTarget {
        self.target
    }

    pub fn top_level_static_argument_position(self) -> Option<usize> {
        self.top_level_static_argument_position
    }
}

#[derive(Clone, Default, PartialEq, Eq, compiler_storage::Record)]
pub struct AuthoredDeclarationSelections {
    rows: Vec<AuthoredDeclarationSelection>,
    by_span: SpanIndex,
}

impl std::fmt::Debug for AuthoredDeclarationSelections {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("AuthoredDeclarationSelections")
            .field("rows", &self.rows)
            .finish()
    }
}

#[derive(Clone, Default, compiler_storage::Record)]
struct SpanIndex(std::collections::HashMap<SourceSpan, Vec<u32>>);

impl PartialEq for SpanIndex {
    fn eq(&self, _: &Self) -> bool {
        true
    }
}

impl Eq for SpanIndex {}

impl SpanIndex {
    fn of(rows: &[AuthoredDeclarationSelection]) -> Self {
        let mut index = Self::default();
        for (position, row) in rows.iter().enumerate() {
            index.add(row.source_span, position);
        }
        index
    }

    fn add(&mut self, source_span: SourceSpan, position: usize) {
        let position = u32::try_from(position).expect("selection ledger position overflow");
        self.0.entry(source_span).or_default().push(position);
    }
}

impl AuthoredDeclarationSelections {
    pub fn replace_prefix_and_rebase_suffix(
        &self,
        source_frontier: usize,
        destination_base: &Self,
    ) -> Result<
        (Self, AuthoredDeclarationSelectionSuffixRebase),
        AuthoredDeclarationSelectionSuffixRebaseError,
    > {
        if source_frontier > self.rows.len() {
            return Err(AuthoredDeclarationSelectionSuffixRebaseError::SourceFrontierOutOfRange);
        }
        if destination_base.rows.len() < source_frontier {
            return Err(AuthoredDeclarationSelectionSuffixRebaseError::DestinationPrefixTooShort);
        }

        for (source, destination) in self.rows[..source_frontier]
            .iter()
            .zip(&destination_base.rows[..source_frontier])
        {
            if source.occurrence_id != destination.occurrence_id
                || source.source_span != destination.source_span
                || source.exposure != destination.exposure
                || source.kind != destination.kind
                || source.compiler_partition != destination.compiler_partition
                || source.top_level_static_argument_position
                    != destination.top_level_static_argument_position
            {
                return Err(AuthoredDeclarationSelectionSuffixRebaseError::PrefixIdentityMismatch);
            }
            let target_is_compatible = source.target == destination.target
                || matches!(
                    (source.target, destination.target),
                    (
                        AuthoredDeclarationSelectionTarget::LateBound(_),
                        AuthoredDeclarationSelectionTarget::Resolved(_)
                            | AuthoredDeclarationSelectionTarget::Intrinsic(_)
                    )
                );
            if !target_is_compatible {
                return Err(AuthoredDeclarationSelectionSuffixRebaseError::PrefixTargetMismatch);
            }
        }

        let source_frontier_index = source_frontier;
        let source_frontier = u64::try_from(source_frontier_index).map_err(|_| {
            AuthoredDeclarationSelectionSuffixRebaseError::OccurrenceCapacityExceeded
        })?;
        let source_end = u64::try_from(self.rows.len()).map_err(|_| {
            AuthoredDeclarationSelectionSuffixRebaseError::OccurrenceCapacityExceeded
        })?;
        let destination_frontier = u64::try_from(destination_base.rows.len()).map_err(|_| {
            AuthoredDeclarationSelectionSuffixRebaseError::OccurrenceCapacityExceeded
        })?;
        let rebase = AuthoredDeclarationSelectionSuffixRebase {
            source_frontier,
            source_end,
            destination_frontier,
        };
        let mut rows = destination_base.rows.clone();
        rows.reserve(self.rows.len() - source_frontier_index);
        for source in &self.rows[source_frontier_index..] {
            let occurrence_id = rebase
                .rebase_extension(source.occurrence_id)
                .ok_or(AuthoredDeclarationSelectionSuffixRebaseError::OccurrenceCapacityExceeded)?;
            let mut rebased = *source;
            rebased.occurrence_id = occurrence_id;
            rows.push(rebased);
        }
        let by_span = SpanIndex::of(&rows);
        Ok((Self { rows, by_span }, rebase))
    }

    pub fn record_resolved(
        &mut self,
        source_span: SourceSpan,
        exposure: AuthoredDeclarationSelectionExposure,
        kind: AuthoredDeclarationSelectionKind,
        selected_symbol: SymbolHandle,
    ) -> Result<AuthoredDeclarationSelectionOccurrenceId, AuthoredDeclarationSelectionRecordError>
    {
        self.record_resolved_in_partition(source_span, exposure, kind, None, selected_symbol)
    }

    pub fn record_resolved_in_partition(
        &mut self,
        source_span: SourceSpan,
        exposure: AuthoredDeclarationSelectionExposure,
        kind: AuthoredDeclarationSelectionKind,
        compiler_partition: Option<CompilerDerivedSelectionPartition>,
        selected_symbol: SymbolHandle,
    ) -> Result<AuthoredDeclarationSelectionOccurrenceId, AuthoredDeclarationSelectionRecordError>
    {
        let occurrence_id = self.next_occurrence_id()?;
        let selection = AuthoredDeclarationSelection::resolved(
            occurrence_id,
            source_span,
            exposure,
            kind,
            compiler_partition,
            selected_symbol,
        )?;
        self.by_span.add(source_span, self.rows.len());
        self.rows.push(selection);
        Ok(occurrence_id)
    }

    pub fn record_late_bound(
        &mut self,
        source_span: SourceSpan,
        exposure: AuthoredDeclarationSelectionExposure,
        kind: AuthoredDeclarationSelectionKind,
        late_binding: AuthoredDeclarationSelectionLateBinding,
    ) -> Result<AuthoredDeclarationSelectionOccurrenceId, AuthoredDeclarationSelectionRecordError>
    {
        self.record_late_bound_in_partition(source_span, exposure, kind, None, late_binding)
    }

    pub fn record_late_bound_in_partition(
        &mut self,
        source_span: SourceSpan,
        exposure: AuthoredDeclarationSelectionExposure,
        kind: AuthoredDeclarationSelectionKind,
        compiler_partition: Option<CompilerDerivedSelectionPartition>,
        late_binding: AuthoredDeclarationSelectionLateBinding,
    ) -> Result<AuthoredDeclarationSelectionOccurrenceId, AuthoredDeclarationSelectionRecordError>
    {
        let occurrence_id = self.next_occurrence_id()?;
        self.by_span.add(source_span, self.rows.len());
        self.rows.push(AuthoredDeclarationSelection::late_bound(
            occurrence_id,
            source_span,
            exposure,
            kind,
            compiler_partition,
            late_binding,
        ));
        Ok(occurrence_id)
    }

    pub fn source_coordinate_occurrence(
        &self,
        source_span: SourceSpan,
        exposure: AuthoredDeclarationSelectionExposure,
        kind: AuthoredDeclarationSelectionKind,
    ) -> Result<AuthoredDeclarationSelectionOccurrenceId, AuthoredDeclarationSelectionCoordinateError>
    {
        use AuthoredDeclarationSelectionCoordinateError as Error;
        let positions = self.by_span.0.get(&source_span).ok_or(Error::Missing)?;
        let mut selected = None;
        let mut ambiguous = false;
        let mut partitioned = false;
        for position in positions {
            let row = &self.rows[*position as usize];
            if row.exposure != exposure || row.kind != kind {
                continue;
            }
            partitioned |= row.compiler_partition.is_some();
            if selected.replace(row.occurrence_id).is_some() {
                ambiguous = true;
            }
        }
        if partitioned {
            Err(Error::CompilerCopyRequiresOwner)
        } else if ambiguous {
            Err(Error::Ambiguous)
        } else {
            selected.ok_or(Error::Missing)
        }
    }

    pub fn resolved_occurrence(
        &self,
        source_span: SourceSpan,
        exposure: AuthoredDeclarationSelectionExposure,
        kind: AuthoredDeclarationSelectionKind,
        selected_symbol: SymbolHandle,
    ) -> Option<AuthoredDeclarationSelectionOccurrenceId> {
        self.by_span
            .0
            .get(&source_span)?
            .iter()
            .map(|position| &self.rows[*position as usize])
            .find(|selection| {
                selection.exposure == exposure
                    && selection.kind == kind
                    && selection.compiler_partition.is_none()
                    && matches!(
                        selection.target,
                        AuthoredDeclarationSelectionTarget::Resolved(target)
                            if target.selected_symbol() == selected_symbol
                    )
            })
            .map(|selection| selection.occurrence_id)
    }

    pub fn get(
        &self,
        occurrence_id: AuthoredDeclarationSelectionOccurrenceId,
    ) -> Option<&AuthoredDeclarationSelection> {
        let index = usize::try_from(occurrence_id.0).ok()?;
        let selection = self.rows.get(index)?;
        (selection.occurrence_id == occurrence_id).then_some(selection)
    }

    pub fn iter(&self) -> impl ExactSizeIterator<Item = &AuthoredDeclarationSelection> {
        self.rows.iter()
    }

    pub fn as_slice(&self) -> &[AuthoredDeclarationSelection] {
        &self.rows
    }

    pub fn len(&self) -> usize {
        self.rows.len()
    }

    pub fn retain_top_level_static_argument_position(
        &mut self,
        occurrence: AuthoredDeclarationSelectionOccurrenceId,
        position: usize,
    ) -> Result<(), AuthoredDeclarationSelectionFinalizationError> {
        let row = usize::try_from(occurrence.0)
            .ok()
            .and_then(|index| self.rows.get_mut(index))
            .filter(|row| row.occurrence_id == occurrence)
            .ok_or(AuthoredDeclarationSelectionFinalizationError::UnknownOccurrence)?;
        if !matches!(
            row.kind,
            AuthoredDeclarationSelectionKind::StaticArgument
                | AuthoredDeclarationSelectionKind::Conformance
        ) || row
            .top_level_static_argument_position
            .is_some_and(|retained| retained != position)
        {
            return Err(
                AuthoredDeclarationSelectionFinalizationError::StaticArgumentPositionMismatch,
            );
        }
        row.top_level_static_argument_position = Some(position);
        Ok(())
    }

    pub fn is_empty(&self) -> bool {
        self.rows.is_empty()
    }

    pub fn finalize_late_bound(
        &mut self,
        occurrence_id: AuthoredDeclarationSelectionOccurrenceId,
        expected_binding: AuthoredDeclarationSelectionLateBinding,
        selected_symbol: SymbolHandle,
    ) -> Result<(), AuthoredDeclarationSelectionFinalizationError> {
        let selected = ResolvedAuthoredDeclarationSelection::new(selected_symbol)
            .ok_or(AuthoredDeclarationSelectionFinalizationError::InvalidSelectedSymbol)?;
        let index = usize::try_from(occurrence_id.0)
            .map_err(|_| AuthoredDeclarationSelectionFinalizationError::UnknownOccurrence)?;
        let row = self
            .rows
            .get_mut(index)
            .filter(|row| row.occurrence_id == occurrence_id)
            .ok_or(AuthoredDeclarationSelectionFinalizationError::UnknownOccurrence)?;
        match row.target {
            AuthoredDeclarationSelectionTarget::Resolved(_)
            | AuthoredDeclarationSelectionTarget::Intrinsic(_) => {
                Err(AuthoredDeclarationSelectionFinalizationError::AlreadyResolved)
            }
            AuthoredDeclarationSelectionTarget::LateBound(actual) if actual != expected_binding => {
                Err(AuthoredDeclarationSelectionFinalizationError::LateBindingMismatch)
            }
            AuthoredDeclarationSelectionTarget::LateBound(_) => {
                row.target = AuthoredDeclarationSelectionTarget::Resolved(selected);
                Ok(())
            }
        }
    }

    pub fn finalize_intrinsic(
        &mut self,
        occurrence_id: AuthoredDeclarationSelectionOccurrenceId,
        expected_binding: AuthoredDeclarationSelectionLateBinding,
        intrinsic: AuthoredDeclarationSelectionIntrinsic,
    ) -> Result<(), AuthoredDeclarationSelectionFinalizationError> {
        let index = usize::try_from(occurrence_id.0)
            .map_err(|_| AuthoredDeclarationSelectionFinalizationError::UnknownOccurrence)?;
        let row = self
            .rows
            .get_mut(index)
            .filter(|row| row.occurrence_id == occurrence_id)
            .ok_or(AuthoredDeclarationSelectionFinalizationError::UnknownOccurrence)?;
        match row.target {
            AuthoredDeclarationSelectionTarget::Resolved(_)
            | AuthoredDeclarationSelectionTarget::Intrinsic(_) => {
                Err(AuthoredDeclarationSelectionFinalizationError::AlreadyResolved)
            }
            AuthoredDeclarationSelectionTarget::LateBound(actual) if actual != expected_binding => {
                Err(AuthoredDeclarationSelectionFinalizationError::LateBindingMismatch)
            }
            AuthoredDeclarationSelectionTarget::LateBound(_) => {
                row.target = AuthoredDeclarationSelectionTarget::Intrinsic(intrinsic);
                Ok(())
            }
        }
    }

    pub fn all_finalized(&self) -> bool {
        self.rows.iter().all(|row| {
            matches!(
                row.target,
                AuthoredDeclarationSelectionTarget::Resolved(_)
                    | AuthoredDeclarationSelectionTarget::Intrinsic(_)
            )
        })
    }

    fn next_occurrence_id(
        &self,
    ) -> Result<AuthoredDeclarationSelectionOccurrenceId, AuthoredDeclarationSelectionRecordError>
    {
        let ordinal = u64::try_from(self.rows.len())
            .map_err(|_| AuthoredDeclarationSelectionRecordError::OccurrenceCapacityExceeded)?;
        Ok(AuthoredDeclarationSelectionOccurrenceId(ordinal))
    }
}

impl<'ledger> IntoIterator for &'ledger AuthoredDeclarationSelections {
    type Item = &'ledger AuthoredDeclarationSelection;
    type IntoIter = std::slice::Iter<'ledger, AuthoredDeclarationSelection>;

    fn into_iter(self) -> Self::IntoIter {
        self.rows.iter()
    }
}

pub use product::{
    CapturedNativeDeclaration, CapturedNativeIntent, NativeBindingFormation, NativeBindingProducer,
    NativeBindingSource,
};

#[cfg(test)]
mod tests {
    use super::AuthoredDeclarationSelectionIntrinsic;
    use super::{
        AuthoredDeclarationSelectionExposure, AuthoredDeclarationSelectionFinalizationError,
        AuthoredDeclarationSelectionKind, AuthoredDeclarationSelectionLateBinding,
        AuthoredDeclarationSelectionOccurrenceId, AuthoredDeclarationSelectionRecordError,
        AuthoredDeclarationSelectionSuffixRebaseError, AuthoredDeclarationSelections,
        BuildOperation, CollectionMeasure, CompilerDerivedSelectionPartition, SourceSpan,
        SymbolHandle,
    };
    use source::{SourceId, Span};

    fn source_span(start: usize, end: usize) -> SourceSpan {
        SourceSpan::new(SourceId(7), Span::new(start, end))
    }

    #[test]
    fn wire_request_formation_names_unknown_and_repeated_facts() {
        let request = |facts: &[&str]| super::ProductDeclarationRequest {
            source_span: source_span(1, 30),
            operation: BuildOperation::WireCompatibilityRequest,
            operands: ["Edge", "Lineage", "Local", "Peer"]
                .into_iter()
                .chain(facts.iter().copied())
                .map(|name| super::ProductDeclarationOperand {
                    path: name.to_owned(),
                    source_span: source_span(2, 5),
                    application: None,
                })
                .collect(),
        };
        assert_eq!(request(&["Readable", "Writable"]).validate_shape(), Ok(()));
        assert_eq!(
            request(&["Imaginary"]).validate_shape(),
            Err("unknown wire compatibility fact `Imaginary`".to_owned())
        );
        assert_eq!(
            request(&["Readable", "Readable"]).validate_shape(),
            Err("wire compatibility fact `Readable` is requested twice".to_owned())
        );
    }

    #[test]
    fn static_operand_positions_survive_resolution_and_reject_drift() {
        let mut selections = AuthoredDeclarationSelections::default();
        let conformance = selections
            .record_resolved(
                source_span(10, 17),
                AuthoredDeclarationSelectionExposure::PrivateImplementation,
                AuthoredDeclarationSelectionKind::Conformance,
                SymbolHandle::from_arena_index(1),
            )
            .expect("conformance occurrence");
        let argument = selections
            .record_late_bound(
                source_span(30, 37),
                AuthoredDeclarationSelectionExposure::PrivateImplementation,
                AuthoredDeclarationSelectionKind::StaticArgument,
                AuthoredDeclarationSelectionLateBinding::CheckedStaticArgument,
            )
            .expect("static occurrence");
        selections
            .retain_top_level_static_argument_position(conformance, 1)
            .expect("conformance slot");
        selections
            .retain_top_level_static_argument_position(argument, 4)
            .expect("argument slot");
        let original = selections.clone();
        selections
            .finalize_late_bound(
                argument,
                AuthoredDeclarationSelectionLateBinding::CheckedStaticArgument,
                SymbolHandle::from_arena_index(2),
            )
            .expect("resolved argument");
        assert_eq!(
            selections
                .get(argument)
                .expect("retained occurrence")
                .top_level_static_argument_position(),
            Some(4)
        );
        assert_eq!(
            selections.retain_top_level_static_argument_position(argument, 3),
            Err(AuthoredDeclarationSelectionFinalizationError::StaticArgumentPositionMismatch)
        );
        let (rebased, _) = original
            .replace_prefix_and_rebase_suffix(original.len(), &selections)
            .expect("resolution preserves operand custody");
        assert_eq!(
            rebased
                .get(conformance)
                .expect("retained conformance")
                .top_level_static_argument_position(),
            Some(1)
        );
    }

    fn record_fixture(selections: &mut AuthoredDeclarationSelections) {
        selections
            .record_resolved(
                source_span(2, 6),
                AuthoredDeclarationSelectionExposure::PrivateImplementation,
                AuthoredDeclarationSelectionKind::MemberAccess,
                SymbolHandle::from_arena_index(3),
            )
            .expect("valid resolved selection");
        selections
            .record_late_bound(
                source_span(2, 6),
                AuthoredDeclarationSelectionExposure::PrivateImplementation,
                AuthoredDeclarationSelectionKind::MemberAccess,
                AuthoredDeclarationSelectionLateBinding::CheckedCall,
            )
            .expect("ledger capacity");
    }

    #[test]
    fn occurrence_identities_are_unique_and_deterministic() {
        let mut first_run = AuthoredDeclarationSelections::default();
        let mut second_run = AuthoredDeclarationSelections::default();
        record_fixture(&mut first_run);
        record_fixture(&mut second_run);

        let first_ids = first_run
            .iter()
            .map(|selection| selection.occurrence_id())
            .collect::<Vec<_>>();
        let second_ids = second_run
            .iter()
            .map(|selection| selection.occurrence_id())
            .collect::<Vec<_>>();

        assert_eq!(first_ids, second_ids);
        assert_eq!(first_ids.len(), 2);
        assert_ne!(first_ids[0], first_ids[1]);
        assert_eq!(first_ids[0].ordinal(), 0);
        assert_eq!(first_ids[1].ordinal(), 1);
        assert_eq!(first_run.get(first_ids[0]), first_run.as_slice().first());
        assert_eq!(first_run.get(first_ids[1]), first_run.as_slice().get(1));
    }

    #[test]
    fn suffix_rebase_preserves_destination_prefix_and_shifts_only_extension_rows() {
        let mut combined = AuthoredDeclarationSelections::default();
        combined
            .record_late_bound(
                source_span(1, 2),
                AuthoredDeclarationSelectionExposure::PrivateImplementation,
                AuthoredDeclarationSelectionKind::Call,
                AuthoredDeclarationSelectionLateBinding::CheckedCall,
            )
            .expect("base row");
        let mut destination = combined.clone();
        destination
            .finalize_late_bound(
                AuthoredDeclarationSelectionOccurrenceId(0),
                AuthoredDeclarationSelectionLateBinding::CheckedCall,
                SymbolHandle::from_arena_index(4),
            )
            .expect("typed base finalization");
        destination
            .record_resolved(
                source_span(3, 4),
                AuthoredDeclarationSelectionExposure::PrivateImplementation,
                AuthoredDeclarationSelectionKind::MemberAccess,
                SymbolHandle::from_arena_index(5),
            )
            .expect("typed-only base row");
        let extension = combined
            .record_resolved(
                source_span(5, 6),
                AuthoredDeclarationSelectionExposure::PublicInterface,
                AuthoredDeclarationSelectionKind::TypeReference,
                SymbolHandle::from_arena_index(6),
            )
            .expect("extension row");

        let (joined, rebase) = combined
            .replace_prefix_and_rebase_suffix(1, &destination)
            .expect("compatible retained prefix");

        assert_eq!(
            &joined.as_slice()[..destination.len()],
            destination.as_slice()
        );
        assert_eq!(joined.len(), 3);
        assert_eq!(joined.as_slice()[2].occurrence_id().ordinal(), 2);
        assert_eq!(
            rebase.rebase_extension(extension).map(|id| id.ordinal()),
            Some(2)
        );
        assert_eq!(
            rebase.retain_base(AuthoredDeclarationSelectionOccurrenceId(0)),
            Some(AuthoredDeclarationSelectionOccurrenceId(0))
        );
        assert_eq!(
            rebase.retain_base(extension),
            None,
            "extension identity cannot be laundered through a base site"
        );
    }

    #[test]
    fn suffix_rebase_rejects_prefix_identity_and_target_tampering() {
        let mut combined = AuthoredDeclarationSelections::default();
        combined
            .record_resolved(
                source_span(1, 2),
                AuthoredDeclarationSelectionExposure::PrivateImplementation,
                AuthoredDeclarationSelectionKind::Call,
                SymbolHandle::from_arena_index(4),
            )
            .expect("base row");

        let mut identity_tamper = AuthoredDeclarationSelections::default();
        identity_tamper
            .record_resolved(
                source_span(9, 10),
                AuthoredDeclarationSelectionExposure::PrivateImplementation,
                AuthoredDeclarationSelectionKind::Call,
                SymbolHandle::from_arena_index(4),
            )
            .expect("tampered row");
        assert_eq!(
            combined.replace_prefix_and_rebase_suffix(1, &identity_tamper),
            Err(AuthoredDeclarationSelectionSuffixRebaseError::PrefixIdentityMismatch)
        );

        let mut target_tamper = AuthoredDeclarationSelections::default();
        target_tamper
            .record_resolved(
                source_span(1, 2),
                AuthoredDeclarationSelectionExposure::PrivateImplementation,
                AuthoredDeclarationSelectionKind::Call,
                SymbolHandle::from_arena_index(9),
            )
            .expect("retargeted row");
        assert_eq!(
            combined.replace_prefix_and_rebase_suffix(1, &target_tamper),
            Err(AuthoredDeclarationSelectionSuffixRebaseError::PrefixTargetMismatch)
        );

        let mut partitioned = AuthoredDeclarationSelections::default();
        partitioned
            .record_resolved_in_partition(
                source_span(1, 2),
                AuthoredDeclarationSelectionExposure::PrivateImplementation,
                AuthoredDeclarationSelectionKind::Call,
                Some(CompilerDerivedSelectionPartition::from_compiler_ordinal(3)),
                SymbolHandle::from_arena_index(4),
            )
            .expect("partitioned row");
        assert_eq!(
            combined.replace_prefix_and_rebase_suffix(1, &partitioned),
            Err(AuthoredDeclarationSelectionSuffixRebaseError::PrefixIdentityMismatch),
            "compiler application custody cannot be changed while rebasing a retained row"
        );
    }

    #[test]
    fn invalid_resolved_target_fails_closed_without_consuming_an_identity() {
        let mut selections = AuthoredDeclarationSelections::default();

        let error = selections
            .record_resolved(
                source_span(4, 9),
                AuthoredDeclarationSelectionExposure::PrivateImplementation,
                AuthoredDeclarationSelectionKind::TypeReference,
                SymbolHandle::invalid(),
            )
            .expect_err("invalid target must reject");

        assert_eq!(
            error,
            AuthoredDeclarationSelectionRecordError::InvalidSelectedSymbol
        );
        assert!(selections.is_empty());

        let first_valid = selections
            .record_late_bound(
                source_span(4, 9),
                AuthoredDeclarationSelectionExposure::PrivateImplementation,
                AuthoredDeclarationSelectionKind::Call,
                AuthoredDeclarationSelectionLateBinding::CheckedCall,
            )
            .expect("ledger capacity");
        assert_eq!(first_valid.ordinal(), 0);
    }

    #[test]
    fn late_finalization_is_exact_and_transactional() {
        let mut selections = AuthoredDeclarationSelections::default();
        let occurrence = selections
            .record_late_bound(
                source_span(9, 12),
                AuthoredDeclarationSelectionExposure::PrivateImplementation,
                AuthoredDeclarationSelectionKind::Operator,
                AuthoredDeclarationSelectionLateBinding::CheckedOperator,
            )
            .expect("record late operator");
        let before = selections.clone();

        assert_eq!(
            selections.finalize_late_bound(
                occurrence,
                AuthoredDeclarationSelectionLateBinding::CheckedCall,
                SymbolHandle::from_arena_index(7),
            ),
            Err(AuthoredDeclarationSelectionFinalizationError::LateBindingMismatch)
        );
        assert_eq!(selections, before);
        assert!(!selections.all_finalized());

        selections
            .finalize_late_bound(
                occurrence,
                AuthoredDeclarationSelectionLateBinding::CheckedOperator,
                SymbolHandle::from_arena_index(7),
            )
            .expect("finalize exact operator occurrence");
        assert!(selections.all_finalized());
        assert_eq!(
            selections.finalize_late_bound(
                occurrence,
                AuthoredDeclarationSelectionLateBinding::CheckedOperator,
                SymbolHandle::from_arena_index(8),
            ),
            Err(AuthoredDeclarationSelectionFinalizationError::AlreadyResolved)
        );
    }

    #[test]
    fn collection_measure_spellings_round_trip() {
        for measure in CollectionMeasure::ALL {
            assert_eq!(
                CollectionMeasure::from_authored_spelling(measure.authored_spelling()),
                Some(measure),
                "{} must select the measure it spells",
                measure.authored_spelling()
            );
        }
    }

    #[test]
    fn collection_measure_spellings_are_the_exact_authored_vocabulary() {
        assert_eq!(
            CollectionMeasure::ALL.map(CollectionMeasure::authored_spelling),
            ["len", "capacity"]
        );
        for spelling in ["length", "Len", "LEN", "as_slice", "bytes", "cap", ""] {
            assert_eq!(
                CollectionMeasure::from_authored_spelling(spelling),
                None,
                "`{spelling}` names no compiler-owned measure"
            );
        }
    }

    #[test]
    fn collection_measures_map_onto_their_intrinsic_selection_targets() {
        assert_eq!(
            CollectionMeasure::Length.intrinsic(),
            AuthoredDeclarationSelectionIntrinsic::CollectionLength
        );
        assert_eq!(
            CollectionMeasure::Capacity.intrinsic(),
            AuthoredDeclarationSelectionIntrinsic::CollectionCapacity
        );
    }

    #[test]
    fn build_operation_spellings_round_trip() {
        for operation in BuildOperation::ALL {
            let target = operation.authored_spelling();
            assert_eq!(
                BuildOperation::from_call_target(target),
                Some(operation),
                "`{target}` must select the operation it spells"
            );
        }
    }

    #[test]
    fn build_operation_spellings_reject_non_operations() {
        assert_eq!(
            BuildOperation::from_call_target("accept_boundary"),
            Some(BuildOperation::BoundaryAcceptance)
        );
        assert_eq!(
            BuildOperation::from_call_target("require_wire_compatibility"),
            Some(BuildOperation::WireCompatibilityRequest)
        );
        for spelling in [
            "accept_boundary#pkg::Console",
            "wire_compatibility",
            "wire_compatibility#Edge#Lineage#Local#Peer#Readable",
            "accept_boundary::Console",
            "select_provider#Console",
            "assume_binding_contract#WritableMemory::claim",
            "enable#Fast",
            "exclude_crash",
            "depend",
            "depend_as",
            "as_slice",
            "len",
            "select_providers",
            "Select_Provider",
            "SELECT_PROVIDER",
            "select_provider ",
            "write",
            "write_error_line",
            "",
        ] {
            assert_eq!(
                BuildOperation::from_call_target(spelling),
                None,
                "`{spelling}` names no toolchain-owned build operation"
            );
        }
    }
}
