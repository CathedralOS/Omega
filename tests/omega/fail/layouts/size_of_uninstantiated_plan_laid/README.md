# A size query rejects an invalid demanded plan

The size query is the only demand for this layout; no stored instance is needed.
Its selected policy returns a one-byte Boolean plan with alignment zero, so the
query must reject during plan validation.

The former pin, `schema instance has no runtime layout`, described a retired
rule. The current rule makes the query obtain and validate its plan. This
existing negative now checks that validation, while `size_of_demanded_plan_exit`
uses the same program with alignment one and observes size one.
