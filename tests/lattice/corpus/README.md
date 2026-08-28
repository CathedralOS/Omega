# `tests/lattice/corpus/` — frozen lattice inputs

This directory contains stable Omega programs shared by bootstrap and proof
checks. Rungs and assurance gates consume these files directly so ordinary
sample churn cannot silently change a lattice claim.

The corpus is input, not authority. A runner does not make a compiler edge
trustworthy; the edge must still reconstruct and discharge its own obligations.
Update this snapshot deliberately and keep negative controls beside the claim
they test.
