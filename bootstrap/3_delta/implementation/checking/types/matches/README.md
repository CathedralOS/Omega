# Match checking

Start at [`../matches.gamma`](../matches.gamma). It checks the subject, selects
each authored arm, and validates constructor identity, owner, arity, and exact
duplicate absence before delegating to [`arms.gamma`](arms.gamma).

The arm flow binds from the saved outer environment, then commits coverage and
checks the body. Binder-free patterns enter the body directly; nonempty ones
retain the shared active-conflict, repetition, and environment-row checks.
After the body returns, result-type agreement precedes advancing the arm list.
The retained current arm supplies the exact body coordinate for disagreement.

[`context.gamma`](context.gamma) owns fixed match facts and continuation
payloads. Source node, subject owner, outer locals, and total arm count are
shared throughout the match. The first admitted pattern supplies constructor
count, and its successful body supplies result type. Later arms reuse that
completed context. A continuation needs only context, current arms, coverage
cursor, and distinct count: four fields in three immutable pairs.

Each match has its own exact-name construction cursor. Seeking does not insert;
commit follows successful binders. Nested matches retain independent cursor,
count, and context snapshots, and sibling arms restart from the saved outer
locals. Finishing compares distinct count with the owner's constructor count;
no downstream consumer needs a materialized coverage trie.

These are compiler working-state savings, not a changed matching policy or a
new aggregate coverage-row limit. D110's match-local set, authored arm order,
exact failure selection, and immutable scopes remain intact. Physical pair
exhaustion is still a separate evaluator observation, not fabricated DCOUT.
