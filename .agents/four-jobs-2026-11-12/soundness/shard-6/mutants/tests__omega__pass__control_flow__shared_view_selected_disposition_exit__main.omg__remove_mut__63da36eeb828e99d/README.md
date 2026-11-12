# Shared view selected disposition

A shared local view is observed by a call before a closed-sum transition.
One case forwards it to a named state, another ends it before a named state,
and a third returns a scalar after ending it. All three cases execute and
produce the same authored length. Each selected exit accounts for its own
disposition; a transfer on one case cannot cover an uncovered sibling.

An ordinary helper returns a narrowed view and increments a counter. Running
the same three exits retains length one and exactly three invocations, excluding
erasure, replay, or substitution of the helper's whole backing.
