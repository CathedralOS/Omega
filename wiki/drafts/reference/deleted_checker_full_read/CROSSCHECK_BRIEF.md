# Cross-check brief: does today's compiler still do what the deleted code did?

Read-only task except for your one output file. Do not build or run anything, do not use the network, do not start other agents.

## Background

Omega's old source checker was deleted and replaced. The current compiler is in the repository at `/home/user/Omega`. Its source compiler is under `source-rust/subsystems/compiler/source-to-psi/pipeline/` (numbered stages; stage 03 types, stage 05 `05_prepared-typed-trees-to-lowered-program` reads source into a lowered program and refuses what it cannot read with "not supported yet" through `Unserved::...(...)` or `unserved("...")`; stage 06 `06_lowered-program-to-checked-program` is the lowered-form checker and prover). The proof kernel is under `source-rust/subsystems/compiler/psi/semantics/proofs/`. Each stage has a README.md that says what it serves; read the stage 05 and stage 06 READMEs first. The specification is in `wiki/spec/`.

A synthesis of what one group of the deleted code enforced is at the path your prompt gives. Its sections "Rule inventory" and "What a rebuilt compiler must still do" list rules of the language.

## Your job

For every row of the rule inventory that is a rule of the language (skip the old code's internal bookkeeping), find out what the current compiler does with a program that the rule concerns. Search the current tree with Grep and read the code you find. Classify each rule as one of:

- **Enforced**: the current compiler checks it. Cite the file and line of the check, and a test if you find one.
- **Refused wholesale**: the current compiler does not check it but refuses the construct it applies to. Cite the refusal site.
- **Not found**: you searched and found neither a check nor a refusal. Say what you searched for. This is the important category: it may mean a rule the old compiler enforced is now silently unenforced, or only that you did not find it. Say which you think and why.
- **Unclear**: you found related code but cannot tell.

Do not guess. A rule you did not have time to look up is "Not looked up", listed separately.

Write your result to the path your prompt gives, with exactly these sections:

```
# Cross-check GNN

## Summary
Counts per class, and three to six sentences on what stands out.

## Rules not found in the current compiler
A table: | Rule | What the old code did (where) | What you searched | Your reading (likely unenforced / likely enforced elsewhere / cannot tell) |

## Rules refused wholesale
A table: | Rule | Refusal site in the current compiler |

## Rules enforced
A table: | Rule | Where in the current compiler | Test, if found |

## Unclear and not looked up
```

Write plainly and cite `path:line`. Length: at most 2,500 words plus the tables.

## Your reply

At most 60 words: the counts per class and the most worrying "not found" rule.
