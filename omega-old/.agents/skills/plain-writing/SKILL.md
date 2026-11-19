---
name: plain-writing
description: Rewrite or review Omega documentation, task boards, instructions and code comments in plain language without changing what they say, and check the result automatically. Use for plain-language rewrites, documentation cleanup, "make this readable", or reviewing whether a draft, PR description or report is clear. Not for changing a rule, the specification's meaning, or code behavior.
---

# Plain Writing

Agents copy the wording of what they read. A file full of shorthand,
metaphors and noun stacks produces reports in the same style that people
cannot follow. This skill turns such text into plain language while keeping
every fact, rule and identifier, and it ends with a script that checks
nothing was lost.

## The three faults

Check every sentence for these. They are the house style from
[Explain the change](../../../AGENTS.md#explain-the-change).

1. **Hidden actor or action.** Make the thing that acts the subject and what
   it does the verb. Not "a refusal is produced upon checker invocation" but
   "the checker refuses the program".
2. **Vague or invented wording.** Use the words a person on this project would
   say aloud. Replace noun stacks ("loan restoration window obligation") and
   coined labels ("the entry boundary") with the function, file or behavior
   they mean. Define a project term the first time it appears, in a few words,
   or link the glossary at the top of [AGENTS.md](../../../AGENTS.md).
3. **Empty statements and buried points.** Put the point first. Cut sentences
   that say nothing. Not "nothing else depends on anything but the result, so
   I'll report when it returns" but "I'll report when the test finishes."

Replace every metaphor with what it means.

## What must not change

- **Meaning.** No rule, requirement, step, verdict or status changes. If you
  cannot tell what a passage means after reading the code or document it
  names, leave it as it is and list it in your report.
- **Identifiers.** Keep every command, flag, file path, link target, task ID
  (`**LOWERED-FORM-CHECKER**`), commit hash, count, lane name and skill
  `name:` exactly. A skill's `description:` may be reworded if every trigger
  phrase stays.
- **Quoted text.** Keep compiler diagnostics, error messages and other quoted
  output word for word. They are what a reader searches for.
- **Structure.** Keep headings, numbered steps, tables and code blocks. Keep
  task-board bullets as bullets; you may split one long bullet into
  sub-bullets.
- **Length.** The result must be no longer than the original. Plain writing
  is shorter: link to a definition instead of repeating it, and cut restated
  examples and hedges.
- **Code.** In a code comment rewrite, change only comments. The script below
  refuses any change to a non-comment line.

## Stale references

While rewriting, check that what the text names still exists:

- `grep` each function, type and path the text tells a reader to use. A name
  that no longer exists is stale.
- Compare claims with current rulings in `AGENTS.md`, `TASKS.md` and the
  dated decisions they link.

Fix a stale reference when the current equivalent is certain: name the
current code, or describe the behavior without a name. Otherwise leave it and
report it. Dated records (measurements, experiment logs, the
`.agents/atlas/` snapshot) describe the past; leave them as written.

## Check the result

Run the check against the commit you started from:

```bash
python3 .agents/skills/plain-writing/scripts/check_rewrite.py --base <commit> <files...>
```

It reports, for each file:
- single-line code spans, link targets, bold task IDs and quoted strings that
  are in the original but not in the rewrite;
- the word count before and after;
- for `.rs` and `.omg` files, any changed line that is not a comment.

A missing item is not always wrong: a stale name you removed on purpose will
show up. Account for every item in your report. Fix the rest.

## Report

Keep it short. For each file: the main changes in a few bullets, its word
count before and after, the stale references you fixed, and the passages you
left alone because their meaning was unclear.
