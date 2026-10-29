# Batch actual unplaced allocation components

Baseline compiler source `bf39a3c901bc8ea3ee60b8b57fdc408a591dbacb`; workbench
`a72dba2e1dc` has identical compiler source. The
[preceding experiment](../spill_pressure_2026_10_28/README.md) provides the
profile, primary-source research, producer identity and rejected first rule.

## Hypothesis

Choose an eligible register of the failed, unplaced component before an
assigned interfering neighbour. Continue trying fixed-view splits first, and
retain the old neighbour ranking if the failed component's candidates are
retired or inadmissible. As in the prior screen, an unplaced victim cannot free
a proposed physical home, so keep its other failed neighbours in the batch.
This should remove a substantial fraction of 103 repeated analyses, potentially
about 0.5 s of a 1.8 s warm compile. This changes allocation heuristics, not just
analysis reuse: native bytes may change, and spill count, image size and runtime
quality need separate qualification. No checks or admissions are removed.

Reject if the real workload does not reduce analysis rounds substantially,
end-to-end benefit fails to repeat, spills/output size grow materially, or
focused allocator/runtime/reject controls regress. Stop after a repeatable
roughly 20% end-to-end win and its quality checks; no global optimum is claimed.

Run the existing screen under one build-slot hold:

```powershell
python tools/build_slot.py --timeout 600 -- python wiki/drafts/measurements/spill_pressure_2026_10_28/experiment.py build/unplaced-batch-experiment
```

Candidate not yet measured. The extra test exercises own-component preference,
tied-member retirement, neighbour fallback and exhaustion.
