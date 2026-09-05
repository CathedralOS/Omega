# Exact-name construction cursors

[`../names.gamma`](../names.gamma) owns ordinary exact-name lookup and insertion.
The helpers here share that sparse-trie representation while retaining a
construction focus across consecutive admitted names. They serve two builders:

- [`../collection.gamma`](../collection.gamma) owns authored-order collection
  and global row provision. It keeps separate type, constructor, and function
  cursors, then finishes their roots into the ordinary global catalog.
- [`../declarations/functions.gamma`](../declarations/functions.gamma) owns
  left-to-right parameter cataloging. It carries a local-name cursor while
  resolving parameter annotations and provisioning active rows, then finishes
  the ordinary trie in the counted environment used by body checking.

Neither builder changes the name representation delivered to later consumers.

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

Seeking does not insert rows. Callers retain their own admission order: global
duplicates precede provision, type provision precedes constructor collection,
and parameter conflicts precede their own annotations and provision. No later
declaration or parameter is examined after failure. Counters, per-type tags,
and exact metadata payloads remain owned by the callers.
Names are compared byte for byte, including prefix endpoints; pair references
never act as truth values or identities. Navigation and long fresh suffixes
use tail loops, not call depth proportional to identifier length.

This is shared construction working state, not a new name representation or a
source sorting pass. General lookup and insertion in `../names.gamma` remain intact.
The change reduces repeated rebuilding when consecutive authored names share
prefixes; arbitrary name order retains the same semantics without promising
the same allocation savings. It does not raise a Gamma bound or convert a raw
evaluator resource failure into a compiler-owned outcome.
