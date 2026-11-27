/*
We reconstruct the observable behavior of an optimization unit (`PsiObservationModel`)
and of any closed multi-block region (`PsiClosedRegionObservation`). Every abstract
operation is classified into its exact `ObservationEventClass`, crash knowledge,
and suspension knowledge so candidate validators can verify that a rewrite
preserves live-in/live-out values, boundary edges, and observable side effects.
*/
