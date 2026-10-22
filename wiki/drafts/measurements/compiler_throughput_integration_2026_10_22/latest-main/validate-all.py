"""Run independent source and engine comparisons against immutable producers."""
from pathlib import Path
import subprocess,sys
folder=Path('build/throughput-latest').resolve()
processes=[subprocess.Popen(['python3',str(folder/'validate.py'),(sys.argv[1] if len(sys.argv)>1 else 'candidate'),leg]) for leg in ('check','both')]
exits=[process.wait() for process in processes]
assert exits == [0,0],exits
