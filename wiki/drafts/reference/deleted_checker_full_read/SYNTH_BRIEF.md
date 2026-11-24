# Synthesis brief: one group of reader reports

Read-only task except for your one output file. Do not build or run anything, do not use the network, do not start other agents.

## Background

Omega is a proof-checked systems language (state machines with `requires`/`ensures` contracts, `data` invariants, domains, ownership and loans, termination measures). Pull request #2823 deleted its old source checker, about 860,000 lines of Rust. A new checker replaced it: it lowers a program first and checks the lowered form, where a fact belongs to a value and a place only says which value it holds.

Every deleted line has now been read. The deletion was cut into 137 slices; one reader per slice wrote a report with fixed sections (coverage log, what the code is for, rules enforced, mechanisms, repetition by placement or construct, identity by text, accepts or skips without proof, notes the authors left, tests, surprises). Reports are in `/tmp/claude-0/-home-user-Omega/1601736f-52e4-5ba1-a47f-e8466f18ac78/scratchpad/full/reports/NNN.md`. The slice definitions are in `.../full/slices/NNN.txt`, and the old source itself is under `/tmp/claude-0/-home-user-Omega/1601736f-52e4-5ba1-a47f-e8466f18ac78/scratchpad/oldtree/omega-rust/` if you need to check a claim.

Three properties were proposed from early sampling, to be tested and not assumed:
1. Acceptance was the code's own yes or no, not evidence checked by the proof kernel.
2. Values and storage were often identified by text, and names sometimes decided meaning.
3. One rule was written again per source placement or construct.

## Your job

Your prompt names a group and its slice numbers. Read every report in the group in full, in order. Then write one synthesis file at the path your prompt gives. The purpose is to find what a reader of single reports cannot see: patterns across slices, the complete list of rules, and anything missed so far. Write plainly: say who does what. Cite `file name:line` (from the reports) for every claim. If two reports disagree, say so. If a report's claim looks doubtful and matters, open the old source and check it; say which claims you checked.

Use exactly these sections:

```
# Group GNN: <one-line name>

## What this code was
Five to eight sentences.

## Rule inventory
One table, one row per distinct language rule, merged across slices (the same rule found in several slices is one row):
| Rule in plain words | Where it was enforced | How it decided | Copies | Slices |
"Copies" is how many separate implementations of the rule the reports found. Be complete.

## Mechanisms
General algorithms: what each computes, how, budgets, which language feature it serves.

## The three properties, tested
For each property: evidence for, evidence against, and your verdict for this group, with counts where the reports give them. Name every place that did use the proof kernel and every part that identified things by handles.

## Suspected defects in the old code
A table: | Defect | Where | Effect (wrong accept, wrong refusal, dead code, other) | Confidence (checked in source by you / reader was sure / reader was unsure) |
Include every one the reports raise.

## Patterns not seen before
Things that recur across slices of this group and are not one of the three properties. For example: the same fact computed by several stages and then compared; refusals that are silent; rules decided in a lowering planner; fixed points with no bound. Give counts and examples.

## What a rebuilt compiler must still do
A list of language-level capabilities, each one sentence, each naming the rule rows it comes from. Only rules of the language, not this code's bookkeeping.

## Notes the authors left
The important known gaps, decisions and reasons, quoted briefly.
```

Length: at most 2,800 words plus the tables.

## Your reply

At most 60 words: how many reports you read, and the two most important things.
