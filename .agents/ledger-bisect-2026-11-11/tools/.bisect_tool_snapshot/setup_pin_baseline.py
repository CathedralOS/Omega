#!/usr/bin/env python3
from __future__ import annotations

import sys
from pathlib import Path

from bisect_helpers import setup_expectations_from_observed


def main() -> int:
    observed = Path(sys.argv[1])
    pin = sys.argv[2]
    witnesses = sys.argv[3:]
    setup_expectations_from_observed(witnesses, observed, pin)
    print("wrote /tmp/ledger_bisect_corpus_outcomes.txt and /tmp/ledger_bisect_expectations.json")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
