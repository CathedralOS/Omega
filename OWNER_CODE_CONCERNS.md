# Owner Code Concerns

Agents write down structural rot here when they see it, and keep working. The
owner reads this file in a session of its own and decides what to clean up.
Nothing in it is a task, a ruling or a reason to stop: an entry never blocks the
change that reported it.

This is not [OWNER_QUESTIONS.md](OWNER_QUESTIONS.md). That file holds language
and architecture decisions only the owner can make. This one holds things that
look wrong in the code and tooling as they stand.

## How to add one

Add an entry when something would mislead the next agent, or when fixing it
properly reaches beyond the crate you are in. An ordinary bug, a style
preference, or anything you can repair inside your own task does not belong
here: just fix it.

The best moment is your first hour in an area, before you have a change of your
own to defend. Say what you saw, not what you would build.

- Put the entry under the category it fits, newest first within the category.
- Keep it to the five lines of the template. Link the file and line.
- Add it in the pull request you are already landing; never open one for it.
- If two entries collide in a merge, keep both.
- Only the owner removes an entry or marks it settled.

```markdown
### <one line naming the concern>
- Where: <path:line, crate, or tool>
- Saw: <what is there, concretely>
- Cost: <who it misleads or what it makes expensive>
- Reported: <date>, <agent>, while <what you were doing>
```

## Categories

**Structure**

1. **One idea, several implementations.** Your fix had to be applied in more
   than one place to work.
2. **Knowledge crossing a boundary.** A stage imports something that is not its
   input representation, its output representation or a foundation crate.
3. **Branch per shape.** Logic keyed on where a value sits (local, field,
   parameter, match arm) instead of what it is.
4. **Re-deriving.** A later stage recomputes what an earlier one knew, or a
   stage checks its own output again.
5. **Too big to read.** You had to search the file because you could not read
   it.

**Honesty**

6. **A verdict with nothing behind it.** Something is accepted by default, by
   fallback, or because a lookup came back empty.
7. **Tests that pin the bug.** A fixture that would still pass if the feature
   were broken, or a check that quietly stopped running.
8. **Zombie machinery.** Code only tests call, a retired path still being fed,
   a fallback nobody can remove.

**Direction**

9. **The documents disagree.** You can cite a rule for either answer, or a
   document blesses the thing that is rotting.
10. **Verifying costs more than writing.** You waited longer than you worked.

## Entries

### 1. One idea, several implementations

### An evaluated const operand is recognised separately by at least three readers
- Where: `omega-rust/psi/compilation/representations/typed-trees/src/type_identity.rs` (`index_argument`, `StaticArgument` arm), and two readers in the validation crate (`close_const_argument`, `validate_types`)
- Saw: build-time evaluation folds a closed record operand into a type reference named `#omega-const:...`; each reader decides on its own whether a `StaticMachineArgument` is a const, and each missed this form, so the program failed at one reader after another
- Cost: a fix at one reader moves the failure to the next; about 96 `ConstExpression` matches suggest more readers exist
- Reported: 2026-11-11, Claude Code, while repairing a record-literal operand `category_error`

### 2. Knowledge crossing a boundary

### Earlier stages and later representations import the stage 05 output type
- Where: the `Cargo.toml` dependencies of stage 03, stage 04, `lowered-psi`, stage 08 and `build-time-evaluation` (read with `cargo metadata --no-deps`)
- Saw: stage 04 depends on `checked-trees`, which stage 05 produces after it; the `lowered-psi` representation and stage 08 depend on it too; `build-time-evaluation` depends on the stage 05 crate itself
- Cost: "each stage knows only its input and output" holds on paper while one stage's output type is visible up and down the pipeline
- Reported: 2026-11-12, Claude Code, while mapping what depends on the old checker before its removal

### 5. Too big to read

### `terminal-installation` lives under the validation directory but serves the whole back half
- Where: `omega-rust/psi/compilation/validation/terminal-installation/`
- Saw: about twenty backend, interpreter and CLI crates depend on it; it sits inside the directory of a crate being deleted
- Cost: its location says "old checker" and its users say "shared runtime contract"; it will be swept up or left stranded by the removal
- Reported: 2026-11-12, Claude Code, while mapping what depends on the old checker

### 7. Tests that pin the bug

### A corpus check can stop running without anything noticing
- Where: `omega-rust/omega/compilation/compiler/tests/corpus_runner.rs`, fixtures carrying `expected_native_rejection.txt`
- Saw: the reader of that file lived in Python tooling deleted by #2086; for weeks nothing read it, nine ledger rows still claimed a match, and three regressions hid behind it until #2564 restored the reader
- Cost: nothing asserts that every `expected_*` file a fixture carries has a reader, so the next deleted check fails the same silent way
- Reported: 2026-11-11, Claude Code, while repairing three `scalar_reference_*_backing_foreign_export` rows

### 9. The documents disagree

### `pipeline.md` blesses a shared pool of judgments beside the pipeline
- Where: `omega-rust/pipeline.md` ("`psi/compilation/validation/` owns reusable judgments"; "none of these is a pipeline program IR")
- Saw: a third category next to stages and representations, with no limit on size or on who imports it; it grew to 344,661 lines imported by stages 03, 04, 05 and 08
- Cost: the next agent can rebuild the pool after the removal and cite the document for it
- Reported: 2026-11-12, Claude Code, while answering how the old checker got that large

### 10. Verifying costs more than writing

### One fixture takes minutes in the front end on a shared workstation
- Where: `omega --check` on a single `tests/omega` fixture; `tests/psi/checking` harness
- Saw: 220 to 420 seconds per fixture at Idle priority with other builds running; one `tests/psi/checking` pass took 2,067 seconds and about 11 GB
- Cost: agents validate with a few named probes instead of the corpus, and skip coverage tools; a wider check is not practical locally
- Reported: 2026-11-12, Claude Code (two workers), while building lowered-form checker lanes
