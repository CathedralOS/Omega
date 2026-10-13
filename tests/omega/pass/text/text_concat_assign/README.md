# text_concat_assign

`main.text = main.text + "c"` bounded-text concat into `[u8;16] in Utf8` across iterations; immutable literal storage creates no disposal debt at joins.

Compile-checked only: run a check target, not native.

## Learnings

- Bounded-text writes prove fit against CAPACITY, not live length: a
  re-assigned literal (`text = "ab"` before each `+ "c"`) bounds the
  analysis; accumulating `text + "c"` unbounded fails "can be up to
  17 byte(s) exceeding 16".
