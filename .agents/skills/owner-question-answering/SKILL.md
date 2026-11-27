---
name: owner-question-answering
description: When explicitly requested, turn a proposed Omega owner question into a four-part decision brief with a checked premise, a recommendation, and examples that distinguish the choices.
---

Use exactly the four headings below, in order. Write for the person making the decision. Define names and variables before using them. Keep the brief short enough that the owner can answer with a sentence.

## Context

State what the user is trying to express and what the canonical spec already guarantees. Link the governing clause. Use implementation and test results to show current behavior, labeling each as such. Check the current owner queue for an existing ruling before presenting anything as open.

## Problem statement

Open with the premise on its own line, in bold, as one plain sentence the owner can check: **Premise: …** Follow it with **Holds** or **Fails** and the one fact that decides it, labeled as spec, observed or expected. If the question rests on more than one premise, list each the same way before discussing any of them.

Put the premise to a concrete test. Ask whether one fact already implies another, whether two spellings mean the same thing, and whether the apparent restriction comes from the language or from today's compiler. If the premise fails, say which part fails, show the smallest counterexample, and restate the real decision. If the spec already settles it, give that answer here and identify the implementation or documentation work.

## Proposed solution

Give one recommended ruling in language the owner can approve verbatim. Show the smallest pair of examples whose outcomes distinguish the choices. Hold the surrounding program constant. Highlight the **exact token, clause, or fact** that matters and say what each example accepts, rejects, or requires. For a compiler handoff, distinguish accepting the source, recording the obligation, and proving it. State what evidence the ruling still requires.

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

Present credible alternatives and the specific example, guarantee, or implementation cost each changes. Include the existing rule when keeping it is a real option. If the premise check resolves the question, say why no owner choice remains. A settled compiler-internal choice follows the project's engineering process. Only a genuinely open language or architecture decision belongs in the owner queue.
