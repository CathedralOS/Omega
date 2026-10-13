"""Run with python3 samples/cli/audio/test_verify_wav.py."""

import contextlib
import io
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch

import verify_wav


class WaveformOracleTests(unittest.TestCase):
    def verify(self, image, sample, *options):
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "test.wav"
            path.write_bytes(image)
            command = ["verify_wav.py", str(path), "--expect", sample, *options]
            with patch("sys.argv", command), contextlib.redirect_stdout(io.StringIO()), \
                    contextlib.redirect_stderr(io.StringIO()):
                return verify_wav.main()

    def test_exact_images(self):
        for sample in ("square_tone", "sine_tone", "scale_melody"):
            with self.subTest(sample=sample):
                self.assertEqual(self.verify(verify_wav.expected_image(sample), sample), 0)

    def test_header_pcm_length_and_trailing_bytes(self):
        image = verify_wav.expected_image("square_tone")
        for offset in (0, 24, 44, len(image) - 1):
            with self.subTest(offset=offset):
                changed = bytearray(image)
                changed[offset] ^= 1
                self.assertEqual(self.verify(changed, "square_tone"), 1)
        self.assertEqual(self.verify(image[:-1], "square_tone"), 1)
        self.assertEqual(self.verify(image + b"\0", "square_tone"), 1)
        self.assertEqual(self.verify(image, "sine_tone"), 1)

    def test_both_block_spellings(self):
        image = verify_wav.expected_image("scale_melody")
        self.assertEqual(self.verify(image, "scale_melody", "--blocks=2000"), 0)
        self.assertEqual(self.verify(image, "scale_melody", "--blocks", "2000"), 0)


if __name__ == "__main__":
    unittest.main()
