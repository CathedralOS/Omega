# A shared Binding cannot become mutable at a state edge

Checking rejects forwarding a shared Binding parameter to a mutable state
parameter. Supporting Binding custody for scalar and structural completions
must preserve the existing `immutable access` rejection.
