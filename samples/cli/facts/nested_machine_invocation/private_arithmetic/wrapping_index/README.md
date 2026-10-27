# Wrapping index arithmetic

Every recorded offset is below 495, so its wrapping sum with 17 is below 512
and `read` can index the window. This is a remaining numeric-observation
requirement of `NESTED-MACHINE-UNIFICATION` in
[TASKS.md](../../../../../../TASKS.md), exposed by directory-buffer reads.

The nine places retain the same guard, offset, addition and array extent. The
weakened-bound twins permit offset 495 and therefore index 512. Optimized
producer `1e9d4d0939bd60731383da4630bc0150dd1d7b4c` refuses all eighteen at
the intended array index. A direct offset copy and the equivalent Exact
addition already check; a Wrapping local temporary also refuses. Sources,
oracles and baseline outcomes were frozen before compiler edits.

The target place uses an ordinary named implementation chosen by Build.
No helper precondition or post-hoc caller guard supplies the bound. Each
positive checks the upper valid index, zero offset, rejected boundary, maximum
input, disabled path, stored index and untouched canary. Native/Terminal
acceptance, pinned fixtures and requirement accounting remain unfinished.

The repair must reuse selected carrier/policy semantics: wrapping reduction and
saturation remain real values. Independent live premises must justify any
unbounded-arithmetic consequence. A guard on an actually wrapped result cannot
prove that the original input never wrapped. Original Exact child formation
obligations and storage epochs remain load-bearing.
