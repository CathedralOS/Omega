#!/usr/bin/env python3
from __future__ import annotations

import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
from bisect_helpers import classify_observation


def main() -> int:
    witness, rec_path = sys.argv[1], Path(sys.argv[2])
    return classify_observation(witness, rec_path)


if __name__ == "__main__":
    raise SystemExit(main())
