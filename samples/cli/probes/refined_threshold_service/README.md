# Refined threshold service

An explicitly selected implementation accepts inputs up to 200 where the
requirement only admits 100, and promises an output bounded by 90 where the
requirement promises 100. Parameter renaming must not change the proof.
The caller checks zero, an interior value and the upper endpoint; a direct
implementation call also exercises its wider input contract.
The standalone implementation is selected by a named map. An exact `satisfies`
edge instead inherits the requirement's contract, including its input bound.

Run `mbx xtask sample-gate --filter samples/cli/probes/refined_threshold_service`.
Expected exit: 0; wrong results trap.

The trait fixtures also cover const-applied and inherited contracts, generic
named maps, and rejection of stronger prerequisites, weaker guarantees, and
facts belonging to a different application or binder.
