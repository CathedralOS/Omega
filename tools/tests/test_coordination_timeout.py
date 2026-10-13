"""A bounded coordination call must settle the transport descendants it starts."""
import os
from pathlib import Path
import sys
import subprocess
from types import SimpleNamespace
import tempfile
import time
import unittest
from unittest.mock import patch, Mock
sys.path.insert(0,str(Path(__file__).resolve().parents[1]))
import coordination

class CoordinationTimeoutTests(unittest.TestCase):
    def test_windows_tree_cleanup_failure_still_reaps_leader(self):
        for failure in (OSError("unavailable"), subprocess.TimeoutExpired("taskkill",5), None):
            with self.subTest(failure=type(failure).__name__):
                process=Mock(pid=12345,args=["git","fetch"])
                process.wait.side_effect=[subprocess.TimeoutExpired("git",0.1),0]
                platform=SimpleNamespace(name="nt",environ=os.environ)
                with patch.object(coordination,"os",platform), patch.object(coordination.subprocess,"Popen",return_value=process), patch.object(coordination.subprocess,"run",side_effect=failure,return_value=subprocess.CompletedProcess([],1)):
                    with self.assertRaisesRegex(coordination.CoordinationTimeout,"descendant cleanup is unconfirmed"):
                        coordination.git(".","fetch",deadline=time.monotonic()+0.1)
                process.kill.assert_called_once()
                self.assertEqual(process.wait.call_count,2)

    @unittest.skipIf(os.name=='nt','POSIX transport-tree control')
    def test_timeout_stops_descendant_work(self):
        with tempfile.TemporaryDirectory() as directory:
            root=Path(directory);marker=root/'orphan-work';started=root/'started'
            child=f'import time; from pathlib import Path; time.sleep(1); Path({str(marker)!r}).write_text("orphan")'
            fake=root/'git'
            fake.write_text(f'#!{sys.executable}\nimport subprocess,time\nfrom pathlib import Path\nsubprocess.Popen([{sys.executable!r},"-c",{child!r}])\nPath({str(started)!r}).write_text("ready")\ntime.sleep(30)\n')
            fake.chmod(0o700)
            with patch.dict(os.environ,{'PATH':str(root)+os.pathsep+os.environ.get('PATH','')}):
                with self.assertRaises(coordination.CoordinationTimeout):
                    coordination.git(root,'fetch',deadline=time.monotonic()+0.4)
            self.assertTrue(started.exists(),'the descendant must have actually launched')
            time.sleep(1.1)
            self.assertFalse(marker.exists(),'timed-out transport continued doing work')
if __name__=='__main__':unittest.main()
