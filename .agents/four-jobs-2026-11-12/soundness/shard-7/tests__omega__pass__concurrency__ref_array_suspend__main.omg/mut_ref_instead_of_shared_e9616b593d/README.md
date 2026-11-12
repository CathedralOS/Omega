# ref_array_suspend

An array of references loans through a suspending boundary call:
`[&i32; 2]` local `references`, suspending `Scheduler::read(&[&i32; 2])`
returns `i64`, and the elements stay observable afterward
(`observe(references[0])`, `read_at(&references, 1)`).

Compile-checked only: calls name the static requirement directly, and no
provider selection runs at `--check`.
