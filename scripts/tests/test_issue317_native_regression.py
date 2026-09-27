"""Fork-only validation of the pinned Windows renderer's colored pixels."""

import subprocess
import sys
import unittest


@unittest.skipUnless(sys.platform == "win32", "DirectWrite requires Windows")
class ColorEmojiRegression(unittest.TestCase):
    def test_color_pixels(self):
        result = subprocess.run(
            ["cargo", "test", "--locked", "--config", ".github/ci-profile.toml",
             "--profile", "ci", "-p", "gpui_windows",
             "color_emoji_rasterization_preserves_color", "--", "--nocapture"],
            check=True, text=True, stdout=subprocess.PIPE,
        )
        print(result.stdout)
        self.assertIn("1 passed", result.stdout)
