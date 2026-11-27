---
name: owner-question-answering
description: When explicitly requested, turn a proposed Omega owner question into a four-part decision brief with a checked premise, a recommendation, and examples that distinguish the choices.
---

Use exactly the four headings below, in order. Write for the person making the decision in everyday, concrete language—not compiler or proof-theory jargon (say "enum", "struct", "plain numbers", "reusable proof" first; introduce a spec term only when needed and define it immediately). Define names and variables before using them. Prefer short bullets with bold lead-ins (**Good:**, **Bad:**, **Trade-off:**) or a compact comparison table over dense paragraphs, and end the choice summary with a one-line **Bottom line** so the owner can answer in a sentence.

## Context

Open with the concrete tension in two or three lines: what works today, what is refused or missing, and the one plain-English question the owner is actually deciding. Link the governing spec clause. Use implementation and test results to show current behavior, labeling each as such. Check the current owner queue for an existing ruling before presenting anything as open.

## Problem statement

Open with the premise on its own line, in bold, as one plain sentence the owner can check: **Premise: …** Follow it with **Holds** or **Fails** and the one fact that decides it, labeled as spec, observed or expected. If the question rests on more than one premise, list each the same way before discussing any of them.

Put the premise to a concrete test. Ask whether one fact already implies another, whether two spellings mean the same thing, and whether the apparent restriction comes from the language or from today's compiler. If the premise fails, say which part fails, show the smallest counterexample, and restate the real decision. If the spec already settles it, give that answer here and identify the implementation or documentation work.

## Proposed solution

Give one recommended ruling in language the owner can approve verbatim. Separate **language meaning** (what the user can write and what it guarantees) from **compiler representation** (how Terminal Psi or the checker encodes it). If a proposed ruling mixes the two, call that out in a short **Watch out** note so the owner does not accidentally bake an internal representation into the spec.

Summarize the recommendation in 2–3 tight bullets ending with an explicit **Trade-off:** bullet (what it gives up or leaves out, and what it costs), matching the format of Alternates below so the owner can compare choices side by side.

Show the smallest pair of examples whose outcomes distinguish the choices. Hold the surrounding program constant. Highlight the **exact token, clause, or fact** that matters and say what each example accepts, rejects, or requires. For a compiler handoff, distinguish accepting the source, recording the obligation, and proving it. State what evidence the ruling still requires.

For example, suppose `p` already carries `Positive` and `Small` is its child. The question “must I prove the parent again?” assumes the input lost its membership. It still carries it. The remaining question is whether the cast needs to spell that out.

- `p as` **`=Positive`** `+Positive::Small` and `p as +Positive::Small` both retain the parent. Both still owe the child's predicate.
- `p as` **`-Positive`** `+Positive::Small` explicitly removes the parent and must fail. The minus sign changes the requested result.

Use examples from the actual question in each brief. State the expected outcome of a proposed rule separately from any outcome observed in a test.

Then show the code strictly as **Before** and **After**, each in its own fenced `omega` block:

- **Before** is the smallest complete program that shows today's behavior. Under it, state the observed outcome, quoting the compiler's message verbatim, and label it as observed.
- **After** is the same program under the recommended ruling. Change only the lines the ruling affects, and keep every other line identical so the diff is the decision. Show only the declarations that differ, and say in a plain sentence which ones, such as "Only `entry` changes. It passes the other case:". If no code changes, say so plainly, such as "No code changes. The same program compiles under the ruling.", and give no block. Never use shorthand labels the owner has to decode. Under it, state the expected outcome and label it as expected.
- If the ruling both accepts and rejects something, give an After for each, each differing from Before by as few lines as possible.
- An alternate whose outcome differs from the recommendation gets its own After block under Alternates, built from the same Before.
- No prose inside the code blocks beyond one short comment on the changed line. Do not paraphrase code in sentences when the block can show it.

## Alternates

Present each credible alternative in the same parallel structure as the recommendation: a bold name, 1–2 short bullets saying what changes for the user, and an explicit **Trade-off:** bullet naming the guarantee, limitation, or implementation cost. Include the existing rule when keeping it is a real option. If the premise check resolves the question, say why no owner choice remains. A settled compiler-internal choice follows the project's engineering process. Only a genuinely open language or architecture decision belongs in the owner queue.
