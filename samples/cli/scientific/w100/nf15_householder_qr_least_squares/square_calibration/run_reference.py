"""Run this project's frozen independent mathematical qualification."""
from pathlib import Path
import runpy
import sys
sys.argv = [str(Path(__file__).resolve().parents[1] / 'reference.py'),
            Path(__file__).resolve().parent.name]
runpy.run_path(sys.argv[0], run_name='__main__')
