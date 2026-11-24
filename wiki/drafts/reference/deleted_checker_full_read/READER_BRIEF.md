# Full read of the deleted Omega checker: reader brief

You are one of many readers. Together you read every line of code that one pull request deleted from the Omega compiler, so that nothing in it is missed. Your job is to read your slice line by line and write down what is in it.

This is a read-only task. Do not edit, create or delete any file except your own report file. Do not build or run anything. Do not use the network. Do not start other agents.

## Background

Omega is a proof-checked systems language. Programs are state machines. A `machine` has `requires` and `ensures` contracts; `data` types can carry `requires` invariants; `domain`s refine types; values have ownership and loans; a machine may be declared `terminates by <measure>`.

Pull request #2823 deleted the old source checker, about 860,000 lines of Rust, after a new checker replaced it. The new design lowers a program first and checks the lowered form, where a fact belongs to a value and a storage place only says which value it holds. The old checker worked on source trees.

Earlier sampling of the deleted code found three recurring properties. Treat them as things to test against your slice, not as conclusions. Report evidence for or against each.

1. Acceptance was the code's own yes or no, not evidence checked by the proof kernel (`proof_admission`, `ProofNode`, `verify_obligation`, certificates).
2. Values and storage were often identified by text (names, rendered labels, joined path strings), and names sometimes decided meaning.
3. One rule was written again for each source placement (local, field, parameter, call result, constant, match join, another state, another machine, return, initializer, assignment, argument) or each construct.

We also want what the sampling could not give: a complete inventory of the rules this code enforced, and anything nobody has noticed yet.

## Your slice

Your slice is a text file given in your prompt. After a `#` header line, each line is `absolute path<TAB>first line<TAB>last line`. Read exactly those line ranges, all of them, in order. One special file, `other_changes.diff`, is a unified diff: in it only the lines starting with `-` are deleted code, and the lines around them are context.

## How to read

- Use the Read tool with `offset` and `limit`, at most 700 lines per call, moving through each range from its first line to its last with no gaps. Do not skip tests, long tables, or code that looks repetitive: repetition is one of the things we are measuring.
- Do not rely on grep or on summaries in place of reading. Grep is fine for looking something up after you have read it.
- After each file range, or every 2,500 lines in a long range, append what you found to your report file before you continue. Your memory of early lines will fade; the report must not depend on it.
- If a range starts or ends in the middle of a function, read it anyway and note the function name.
- If you cannot finish, stop and say exactly which ranges you did not read. A partial read stated honestly is useful. A claimed full read that was not is harmful.

## Your report file

Write your report to the path given in your prompt, in Markdown, with exactly these sections. Cite `file name:line` for every claim. Write plainly: say what the code does, who acts, and what happens. No recommendations.

```
# Slice NNN

## Coverage log
One line per Read call you made on your slice: `READ <file name> <first line>-<last line>`.
Then one line: `NOT READ: none` or the ranges you did not read.

## What this code is for
Five to ten sentences.

## Rules enforced
A table, one row per language rule or check this code enforces:
| Rule in plain words | Where | How it decides | Diagnostic text, if any |
"How it decides" is one of: own judgment (say what it compares), asks another module (name the function), proof kernel (name the call). Be complete: this table is the inventory. Aim for every distinct rule, not a sample.

## Mechanisms
Algorithms that are general rather than per case (for example a fixpoint, a unification, an interval analysis, a search with a budget). For each: what it computes, how, its limits or budgets, and which language feature it serves.

## Repetition by placement or construct
Every place where the same rule or computation is written again per source placement or per construct. Give the line ranges of each copy. Estimate what share of your slice is such repetition.

## Identity by text
Every place where a value, storage place, declaration or meaning is identified or decided by a string: names, rendered labels, joined paths, string prefixes, comparisons against string literals.

## Accepts or skips without proof
Every place where the code accepts, skips a check, or keeps a fact without having established it: defaults that return true, a failed lookup that continues, `_ =>` arms that accept, deferring instead of refusing, trusting a declared type. Say for each whether you can see another check that would catch the case, or cannot tell.

## Notes the authors left
Comments that record a known gap, a decision, a ruling, an invariant relied on from another stage, a "not yet", a workaround, or a reason. Quote the important ones briefly.

## Tests in this slice
What behaviours the tests pin, by test name.

## Surprises
Anything that does not fit the three properties above, anything that looks like a bug, dead code, a contradiction between a comment and the code, or a capability you would not have expected. This section matters most: write what a careful reader would want to know.
```

## Your reply

When the report file is complete, reply with at most 80 words: the number of lines you read, whether anything was not read, and the two or three most important things you found. The report file is the deliverable; the reply is only a receipt.
