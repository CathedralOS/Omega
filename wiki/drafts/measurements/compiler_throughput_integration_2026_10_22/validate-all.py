"""Run independent source and engine comparisons against immutable producers."""
from pathlib import Path
import subprocess
folder=Path('build/throughput-final').resolve()
processes=[subprocess.Popen(['python3',str(folder/'validate.py'),'candidate',leg]) for leg in ('check','both')]
exits=[process.wait() for process in processes]
assert exits == [0,0],exits
