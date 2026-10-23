"""Use the existing two-slot protocol with a shorter local acquisition poll.

The original two-second poll repeatedly lost released slots during this capture.
This changes neither the slot count nor ownership/reclamation rules, and leaves
the shared tool and other processes untouched. The remaining wait is bounded.
"""
from pathlib import Path
import sys

sys.path.insert(0, str(Path('tools').resolve()))
import build_slot

build_slot.POLL_SECONDS = 0.05
raise SystemExit(build_slot.main([
    '--slots','2','--timeout','1200','--','python3',
    str(Path(__file__).with_name(sys.argv[1] if len(sys.argv)>1 else 'build.py')),
]))
