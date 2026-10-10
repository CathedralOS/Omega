# Declared open range captures its collection once

Acceptance: `omega run --both main.omg`. Both engines must exit 0.

Current preparation refuses the omitted end of the selected range declaration.
Expanding it to a second collection expression is not a valid repair: guarded
collection producers must still execute exactly once. With that tentative
expansion, source admission succeeds but native lowering also reports that
`choose` has no executable body (outer calls: statement window). Keep both
dependencies with the collection-view migration outcome.

The selected range body is an ordinary machine over a copied nominal collection.
Its omitted end reads that collection's length. An effectful collection producer
runs once in the selected Match arm and never in the unselected arm; deriving the
end must not invoke it again. The result 11 also witnesses the selected body,
rather than builtin window construction.
