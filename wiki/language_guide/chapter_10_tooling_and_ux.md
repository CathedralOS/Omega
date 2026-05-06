# Chapter 10: Tooling And UX

This design should remain visible to programmers.

A debugger should be able to show:

- The active state.
- The current proof assumptions.
- Which invariants are currently weakened.
- Which outgoing transitions are enabled.
- What proof debt each transition carries.
- Why a transition failed argument or return value compatibility.

Diagnostics should avoid magical proof language. If a guard is not strong enough to prove a target invariant, the compiler should point at the transition and explain which fact is missing.

The language is only worth being strange if the tools make the strangeness feel obvious.
