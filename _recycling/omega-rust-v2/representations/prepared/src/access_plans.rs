//! The issuer decision produced by access-plan validation.

/// Who may introduce the domain's qualification, as the returned plan states it
/// (`wiki/spec/resources/access.md`, named decision `required-issuance-choice`).
///
/// The third row of that table, `Issuance::Only` with routes, has no value
/// here. Validation refuses it until a route's `CallableInfo` has a frozen
/// form that can be compared with the callable L3 selected.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PlanIssuance {
    /// `Issuance::Unrestricted`: the policy adds no issuer restriction.
    Unrestricted,
    /// `Issuance::Only { routes: [] }`: no machine may introduce the
    /// qualification. A value that already carries it still moves and forwards.
    Nobody,
}
