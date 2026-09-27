# Reference-view locals: binding is not the same capability as writing through

`&mut` view locals are discussed on the board as one producer problem
(**REFERENCE-LOCAL-BINDING-ROUTES** lists "shared borrows, runtime-indexed
places, reference recasts, plain-local borrows, and reference-returning
calls"). That framing is producer-only. This file records a control matrix
showing the axis the board does not name: for one producer the **binding
checks and the write through it does not**, which makes it a consumer-side
store gap rather than a producer gap.

## 2026-09-27 at `f7cbf4e93375`, macOS arm64, `omega --check`

Every row is the same program with one token changed; the `Main` data carries
`size: u32`, `buf: [u32; 4]`, `k: u32 [0..=3]`. Reported phase is from the
`omitted at local construction at ...` line.

| place | bind only | written through |
| --- | --- | --- |
| field `&mut self.size` | OK | **OK** |
| element, literal index `&mut self.buf[0]` | OK | **OMITTED — `structural field store: carrier path`** |
| element, runtime index `&mut self.buf[self.k]` | **OMITTED — `statement sequence: unsupported statement kind`** | OMITTED, same phase |

Controls, no binding at all: `self.buf[0] = 7;` and `self.buf[self.k] = 7;`
both **OK**. So neither element write is itself unsupported — it is writing
one *through a view* that fails.

## Three distinct gaps, not one

1. **Runtime-indexed producer.** `&mut self.buf[self.k]` omits at
   `unsupported statement kind` for the binding **alone**; whether it is
   written through is irrelevant. A statement-level producer gap.
2. **Recast producer.** `as &mut T` omits at the same phase **independent of
   place**: `&mut self.size as &mut u32` (plain field, identity recast) and
   `&mut self.buf[self.k] as &mut u32` report identically, while those two
   places *without* the recast behave differently from each other. So the
   recast arm is one gap in statement sequencing, not a per-place one.
   Coordinates with **RECAST-SOURCE-POSITIONS**.
3. **Literal-indexed element consumer.** The only row where bind and write
   differ. The producer admits the view; the store route for it is missing.

## Negative result: field projection through a view is fully supported

This rules out the tempting hypothesis that writing *through* a view is
unimplemented in general. Both of these check:

```
let v: &mut Leaf = &mut self.node.value;   v.value = self.source;
let v: &mut Node = &mut self.node;         v.value.value = self.source;
```

One level and two levels of field projection through a `&mut` view both work.
The store route therefore exists for field projections and is missing only for
element projections. Anyone repairing row 3 should look for why the element
path does not reach the store route the field path already uses, rather than
building a new one.

## Caution on phase attribution

`trace.phase(...)` records the last phase **entered**, not what declined.
Eight corpus fixtures report `structural field store: target place`
(4 `layouts/runtime_plan_laid_*`, 3 `recast/*_mutable_write_*`,
1 `traits/runtime_local_named_dyn_pass_through_exit`). **No reduction of
their shapes reproduces that phase** — every one built here reports
`unsupported statement kind` or `carrier path` instead, including the
nested-projection and runtime-index recast forms those fixtures use. Do not
treat `target place` as the site without instrumenting. This matches the
board's own warning that "phase labels do not define separate capabilities".

## Live claims at time of writing

`ELEMENT-VIEW-FIELD-READ` (Codex) holds the omega-side element-view lowering
and `LOCAL-DATA-CLUSTER-REPAIR` (Devin, widened to "&mut view/call-result
locals") holds the psi-side statement sequencing. Both received this matrix
as a claim note; no paths of theirs were touched in producing it.
