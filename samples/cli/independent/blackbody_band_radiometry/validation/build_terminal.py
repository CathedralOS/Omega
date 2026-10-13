"""Link this private observer using the verified CLI's exact dependency closure.

The shared linker checks CLI byte identity and Cargo dependency fingerprints.
This sample neither rebuilds a nested workspace nor chooses duplicate libraries
by recency. Use --omega with the compiler qualified for this sample.
"""
import argparse
from pathlib import Path
import subprocess
import sys

parser = argparse.ArgumentParser()
parser.add_argument('--omega', required=True, type=Path)
arguments = parser.parse_args()
root = Path(__file__).resolve().parents[1]
repository = root.parents[3]
command = [sys.executable, str(repository/'tools/link_terminal_sponsor.py'),
           '--omega', str(arguments.omega.resolve()),
           '--source', str(root/'validation/terminal.rs'),
           '--output', str(root/'build/validation/terminal-validator')]
raise SystemExit(subprocess.run(command).returncode)
