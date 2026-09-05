# Census identity cursors

[`../collection.gamma`](../collection.gamma) owns authored-order collection and
the three row provisions. It keeps separate type, constructor, and function
cursors while collecting, then finishes their roots into the unchanged global
catalog. Declaration resolution and all later consumers use ordinary tries.

[`cursor.gamma`](cursor.gamma) defines the private cursor and its endpoint and
finish operations. [`cursor_navigation.gamma`](cursor_navigation.gamma) seeks
an exact source name by retaining its shared prefix, climbing departed parents,
and descending existing byte edges. [`cursor_commit.gamma`](cursor_commit.gamma)
inserts an absent terminal or child only after the caller admits that row.

A cursor contains its focus depth, a source coordinate identifying that prefix,
the ordinary sparse trie at the focus, and a counted immutable ancestor spine.
The ancestor nodes retain terminal options and sibling order. A seek rebuilds
only departed prefixes; a commit leaves common ancestors deferred. Earlier
cursors and finished roots remain immutable and independently usable.

Seeking does not insert declarations. Duplicate identity is decided before row
provision, type provision precedes its constructor collection, and later
declarations are not examined after a failure. Type and constructor counters,
per-type tags, exact metadata payloads, and returned globals are unchanged.
Names are compared byte for byte, including prefix endpoints; pair references
never act as truth values or identities. Navigation and long fresh suffixes
use tail loops, not call depth proportional to identifier length.

This is census working-state reuse, not a new name representation or a source
sorting pass. General lookup and insertion in `../names.gamma` remain intact.
The change reduces repeated rebuilding when consecutive authored names share
prefixes; arbitrary name order retains the same semantics without promising
the same allocation savings. It does not raise a Gamma bound or convert a raw
evaluator resource failure into a compiler-owned outcome.
