# `compiler/beta-lang-py/` — compatibility facade

This compatibility facade preserves historical entry points while the retained
tools live with their actual owners:

- executable Beta meaning and semantic gates:
  `bootstrap/rungs/beta/reference/`;
- symbolic obligation reconstruction and its soundness gate:
  `bootstrap/assurance/refinement/beta/`.

Every executable or module here is a narrow forwarding wrapper. New callers
must use the canonical role paths. The former `bc2.py` backend and its
`independent-floor.sh` composition were removed: neither was a lattice gate,
and all canonical behavior they touched already has focused interpreter,
assembler-reference, and VM-reference coverage. They did not close the
lower-rooted `bc` refinement obligation.
