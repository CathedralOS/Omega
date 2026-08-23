"""Compatibility import for Beta's canonical reference parser."""

from pathlib import Path
import sys

_ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(_ROOT))
from bootstrap.rungs.beta.reference.beta_parser import *  # noqa: F401,F403,E402
