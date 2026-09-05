# Exact-name construction cursors

[`../names.gamma`](../names.gamma) owns ordinary exact-name lookup and insertion.
The helpers here share that sparse-trie representation while retaining a
construction focus across consecutive admitted names. They serve these builders:

- [`../collection.gamma`](../collection.gamma) owns authored-order collection
  and global row provision. It keeps separate type, constructor, and function
  cursors, then finishes their roots into the ordinary global catalog.
- [`../declarations/functions.gamma`](../declarations/functions.gamma) owns
  left-to-right parameter cataloging. It carries a local-name cursor while
  resolving parameter annotations and provisioning active rows, then finishes
  the ordinary trie in the counted environment used by body checking.
- [`../declarations.gamma`](../declarations.gamma) starts from the completed
  census tries. It retains already-complete nullary constructor rows, replaces
  payload constructor and function rows after their declarations resolve, then
  finishes both typed catalogs. Original census roots remain available for
  declaration custody checks throughout that pass.
- [`../types/matches.gamma`](../types/matches.gamma) retains a private cursor
  for each match's exact coverage set. It commits only after binder checks,
  and nested matches retain independent immutable state. Final exhaustiveness
  uses the distinct count; no later phase consumes this working trie.

These builders do not change the name representation delivered to later consumers.

[`cursor.gamma`](cursor.gamma) defines the private cursor and its endpoint and
finish operations. [`cursor_navigation.gamma`](cursor_navigation.gamma) seeks
an exact source name by retaining its shared prefix, climbing departed parents,
and descending existing byte edges. [`cursor_commit.gamma`](cursor_commit.gamma)
inserts an absent terminal or child only after the caller admits that row, or
replaces an existing terminal after resolution without changing child edges.

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

Fresh suffix construction also reuses one known-empty carrier for absent
terminals and empty child lists throughout the path. An absent focus or child
already supplies that carrier; an internal prefix's absent terminal can supply
it as well. A present terminal instead needs a separate empty value. These
fields have the same immutable `(0, 0)` representation, so sharing does not
change either presence tests or child lookup.
