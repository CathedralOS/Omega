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
The ancestors retain complete tagged tries, including terminal options and
every live or superseded sibling row. A seek rebuilds
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

The [customer cost investigation](../../../../../wiki/drafts/bootstrap_cost_review.md#delta-simplification)
compares prefix reuse with rebuilding from the root on the exact Epsilon source.
It supports retaining the current implementation without expansion, not claiming
that the customer requires it to fit or that boundary conformance is complete.

Trie tag `0` is absence. Tag `1` carries a terminal option and sparse child
rows. A nonterminal with one edge instead uses `(pair (byte + 2) child)`: one
pair per byte, with no separate absent terminal or singleton child list. Source
identifier bytes cannot collide with the absence or branch tags. Lookup and
ancestor reconstruction read this form directly. Adding a prefix terminal or
sibling expands just that node into the ordinary tag-1 form, preserving its
existing edge. Terminal nodes keep tag 1 even when they
have no children. No node deletion or recompression pass is needed.

For a fresh length-`L` name subsequently visited and rebuilt during declaration
resolution, the unary path, ancestor spine, and rebuilt unary path cost one
pair per byte each, rather than the former five, one, and five. Branch and
terminal overhead is separate. This is a path-specific allocation argument,
not a bound on the complete compiler or all admitted source shapes.

## Pair accounting

This section charges every pair the shared tries and cursors allocate, in the
same style as the
[normalization audit](../../normalization/README.md#traversal-and-rebuild-pairs).
It covers `../names.gamma` and the cursor helpers here; the builders that call
them are listed above. Parser and grammar pairs are not covered — they are
preflighted under the
[syntax provision](../syntax/README.md#syntax-storage), which bounds them below
2,857,368 pairs at 40 bytes each.

| Site | Pairs | Charged per |
| --- | ---: | --- |
| `name_trie_find_option` / `name_trie_find_packed_option` | <=2 | lookup (unary miss marker and absent-terminal option) |
| Ancestor spine cell `(pair focus ancestors)` / `(pair trie ancestors)` | 1 | descended existing edge |
| `name_trie_build_path` unary node | 1 | fresh suffix byte |
| `name_trie_new_path` terminal node | 3 | fresh name |
| `name_trie_with_terminal` | <=7 | terminal committed on an existing node |
| `name_trie_with_child` | <=10 | divergent-edge commit point |
| `name_trie_replace_child` on a unary parent | 1 | rebuilt ancestor level |
| `name_trie_replace_child` on a branch parent | <=5 | rebuilt ancestor level |
| `identity_cursor` record | 3 | seek |
| `identity_cursor_commit` wrapper | 1 | commit |
| `identity_cursor_replace` | <=8 | resolved-row replacement |
| `identity_cursor_present` | <=1 | presence check |
| `empty_identity_cursor` | 4 | builder |

The 5-pair level bound is a rebuilt tag-1 node: two pairs retain its tag and
terminal option, and the replacement prepends one fresh
`(pair stored-byte (pair child rest))` row at three pairs. Replacement no
longer copies each younger sibling over the retained row: exact-byte lookup
stops at the first matching row, so a superseded row stays an unreachable
predecessor inside this version and inside every older immutable snapshot.
The presence precondition still scans the existing rows without allocating,
and absence remains a contradiction. Byte-keyed rows admit at most 63
children because source identifier bytes are 26 uppercase, 26 lowercase, ten
digits, and underscore ([`../source.gamma`](../source.gamma)); retained
predecessors may lengthen a children list past that count, but a lookup scan
still stops at the newest matching row, so repeated replacement costs its
prepending events, not a rescanned copy per level.

### Builder aggregates

Let a builder's name events be its seeks, commits, replaces, or ordinary
inserts, and let `W_b` total the bytes of the names it processes. A cursor
seek departs `depth_before - common` levels and descends
`depth_after - common` existing edges, so over the builder's life
`departed + final depth = descended` exactly. Departed rebuild levels,
descended spine cells, and fresh suffix bytes are therefore each bounded by
`W_b`. An ordinary `name_trie_insert` rebuilds exactly its descended levels,
so the same `W_b` bound applies to a plain insertion sequence. One finish per
builder rebuilds its remaining depth, itself bounded by its last name's bytes.
Each cursor event allocates at most `19 + descended + fresh + 5*departed`
pairs — the cursor record, absent-edge marker, presence check, and commit node
work — and each ordinary insert at most
`15 + descended + fresh + 5*rebuilt`.

### Whole-compiler rollup

Across every builder, define `V <= S` name-token occurrences that are bound,
declared, or coverage-committed — declaration, parameter, `let`, and pattern
binder names plus match-arm constructor names — where `S` is the retained-node
count and `N <= 4,194,304` the admitted source bytes. Each token participates in at most two events —
census commit then resolution seek for declaration names, catalog commit then
lowering bind for parameters, checking then lowering binds for `let` and
pattern binders, and one coverage seek/commit for arm names — so
`E_n <= 2*V` events total `W_n <= 2*N` name bytes. Resolved-row replacements
`K_r <= V`, builders number `F + M + 5` for `F` functions and `M` matches,
and `Q <= 3*S + 1` lookup evaluations serve every caller.

```text
name-trie and cursor pairs
  <= 2*Q + 19*E_n + 8*K_r + 4*(F + M + 5) + 2*W_n + 15*W_n
  <= 2*Q + 19*E_n + 8*K_r + 4*(F + M + 5) + 17*W_n
  <= 6*S + 2 + 38*V + 8*V + 4*S + 20 + 34*N
  <= 10*S + 46*V + 34*N + 22
```

The former `575*W_n` coefficient was dominated by at-most-191 sibling-row
copies charged per rebuilt branch level. Replacement now prepends the fresh
row at a fixed five pairs per level, so the product no longer multiplies
departed levels by sibling counts. The remaining `34*N` term is still a loose
envelope: departed and rebuilt levels remain amortized by earlier descents
through the zipper identity above, but a builder processing names with short
shared prefixes departs a level per name byte, so this accounting does not by
itself prove the product stays below the 40,265,318-pair arena for every
admitted name order. The capture merge aggregate is closed separately in the
[normalization audit](../../normalization/README.md#capture-allocation-ownership);
this residual name-event envelope is a closed bound of the same kind, not a
demonstrated overflow or a demonstration that the whole-producer total stays
below the arena.
