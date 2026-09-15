//! The interned semantic identities: ranking views, service reach rows and
//! their tables.

macro_rules! semantic_id {
    ($(#[$doc:meta])* $name:ident) => {
        $(#[$doc])*
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
        pub struct $name(pub u32);

        impl $name {
            /// The ZII-inert null identity (index 0 is reserved).
            pub const NULL: Self = Self(0);

            pub fn is_valid(self) -> bool {
                self.0 != 0
            }
        }
    };
}

semantic_id!(
    /// Normalized semantic-domain identity (record §Domain theory): the
    /// deterministic normalizer owns it; checked types/bindings carry it;
    /// layout keeps using the carrier ABI (semantic interface identity and
    /// physical ABI identity are DISTINCT and both queryable).
    SemanticDomainId
);
semantic_id!(
    /// Normalized identity of one boundary-service trait. Ordinary traits and
    /// operational may-clauses never receive this identity.
    ServiceReachId
);
semantic_id!(
    /// Normalized service-reach row identity (service set + parent closure).
    /// Suspension and blocking are deliberately absent from this identity.
    ServiceReachRowId
);
semantic_id!(
    /// A normalized EXTERNAL-BINDING identity (PRV4 step 1): the structural
    /// `via <Binding>` value of an ExternalRealization leaf, interned so supply
    /// modes stay Copy and equal bindings share one identity without rendering.
    ExternalBindingId
);
semantic_id!(
    /// A canonical ranking view (e.g. `Nat::Descending`); the witness names
    /// it explicitly, defaults elaborate at once.
    RankingViewId
);
