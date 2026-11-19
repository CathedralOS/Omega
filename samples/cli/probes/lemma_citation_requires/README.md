# lemma_citation_requires

Requires-bearing lemma citation: `cancel` requires `(a)==(b)`; `cite_with_requires` carries the same requires verbatim, and its body `_ = cancel(x, y)` discharges the instantiated premise against its own hypothesis to settle `(minus(x,x))==(minus(x,y))`.

## Gaps found while authoring (filed)

- The shipped kernel lemma `add_zero_right` is package-private (`selects private machine add_zero_right::entry`) — corpus fixtures cite it, user packages cannot. Same admission class as the Nat operator surface and `TaskRuntime::start`.

Compile-checked only: no standard-library dependency and
no `Main::exit` — run a check target, not native.
