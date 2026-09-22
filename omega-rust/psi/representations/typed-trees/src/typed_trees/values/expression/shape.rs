use crate::expression::TableCallExpression;

impl TableCallExpression {
    /// Whether the call reaches its callee through the ordinary nominal route,
    /// so `target_symbol` alone names the operation that runs.
    ///
    /// Three retained requests each redirect a call away from that route, and a
    /// consumer modelling an ordinary application must exclude every one of
    /// them:
    ///
    /// * `static_requirement_dispatch` — static conformance dispatch rewrote
    ///   `target_symbol` to a satisfier's private closed realization. The
    ///   public requirement, not the rewritten symbol, remains the contract and
    ///   proof interface, so the call is not the plain application it spells.
    /// * `quotient_operation` — an authored sealed quotient request. Its
    ///   representative operation and role-ordered theorem evidence are
    ///   retained inside the request instead of being spelled as operands, and
    ///   admission is a separate judgment.
    /// * `private_layout_operation` — an exact compiler-known
    ///   `Plan::place_private<Slot>` request. Its selected conformance is
    ///   proof-static identity that ordinary generic dispatch never sees.
    ///
    /// The predicate deliberately says nothing about the receiver, the target
    /// symbol or the positional arguments: each of those is a caller's own
    /// narrowing and stays visible at its site.
    pub fn selects_only_nominal_route(&self) -> bool {
        self.static_requirement_dispatch.is_none()
            && self.quotient_operation.is_none()
            && self.private_layout_operation.is_none()
    }

    /// Whether the call supplies its callee nothing but positional
    /// `arguments` — no static application and no evidence terms.
    ///
    /// * `machine_arguments` are the call's static application: type, const and
    ///   machine-proposition arguments that select which specialization of the
    ///   callee runs. A consumer that resolves a callee by symbol alone, or
    ///   that reuses a call's operands as ordinary values, cannot account for
    ///   them.
    /// * `evidence_arguments` name evidence terms handed to the callee's proof
    ///   obligations. They carry no runtime operand, so a consumer reading
    ///   only the positional arguments would silently drop the obligation.
    ///
    /// Callers that accept one of the two kinds (a selection intrinsic reading
    /// its own static arguments, for example) check the other kind themselves
    /// rather than using this predicate.
    pub fn carries_only_positional_arguments(&self) -> bool {
        self.machine_arguments.is_empty() && self.evidence_arguments.is_empty()
    }
}
