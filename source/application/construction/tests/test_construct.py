"""Exercise the two constructor output contracts through the publication path."""
import contextlib
import io
import json
from pathlib import Path
import struct
import sys
import tempfile
import unittest
from unittest.mock import patch

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
import construct


class ConstructionOutputTests(unittest.TestCase):
    def test_raw_and_framed_publication(self):
        wire = b'fixed construction input'
        manifest = {'units': [], 'source_bytes': 0, 'input_sha256': construct.digest(wire)}
        payload = b'\0\0\0\0\0arbitrary tape bytes'
        success = bytes(5) + payload
        refusal = struct.pack('<BiI', 0, 2, 73)
        cases = [
            ('--compiler', payload, 0, payload),
            ('--epsilon-compiler', success, 0, payload),
            ('--epsilon-compiler', success, 1, None),
            ('--epsilon-compiler', bytes(5), 0, None),
            ('--epsilon-compiler', bytes(4), 0, None),
            ('--epsilon-compiler', refusal, 0, None),
            ('--epsilon-compiler', b'\x01\x02prefix', 0, None),
            ('--epsilon-compiler', b'\x02\x12\x04\0\0\0', 0, None),
            ('--epsilon-compiler', b'\xffEEOUT\x01\0' + bytes(32), 0, None),
        ]
        with tempfile.TemporaryDirectory() as temp, patch.object(construct, 'compiler_input', return_value=(wire, manifest)):
            work = Path(temp)
            producer, output, request = work / 'producer', work / 'result.tape', work / 'request.bin'
            for option, stdout, exit_code, expected in cases:
                with self.subTest(option=option, stdout=stdout, exit_code=exit_code):
                    producer.write_text(f'#!{sys.executable}\nimport sys\nassert sys.stdin.buffer.read() == {wire!r}\n'
                                        f'sys.stdout.buffer.write({stdout!r})\nraise SystemExit({exit_code})\n')
                    producer.chmod(0o700)
                    output.write_bytes(b'previous tape')
                    args = ['construct.py', option, str(producer), '--input', str(request), '--output', str(output)]
                    with patch.object(sys, 'argv', args), contextlib.redirect_stdout(io.StringIO()):
                        self.assertEqual(construct.main(), int(expected is None))
                    receipt = json.loads(request.with_suffix('.result.json').read_text())
                    self.assertEqual(receipt['published'], expected is not None)
                    self.assertEqual(receipt['stdout_sha256'], construct.digest(stdout))
                    self.assertEqual(output.read_bytes(), expected if expected is not None else b'previous tape')
                    if expected is not None:
                        self.assertEqual(receipt['tape_sha256'], construct.digest(expected))
                    if stdout == refusal:
                        self.assertEqual((receipt['epsilon_exit'], receipt['source_offset']), (2, 73))


if __name__ == '__main__':
    unittest.main()
