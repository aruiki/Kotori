"""release_tag.py の単体テスト(作業カード 40)。python -m unittest discover -s mozc/tools -p "test_*.py" """
import unittest

from release_tag import check_notes, next_release

TAGS = ["v0.1.0-beta.3", "v0.3.0-beta.7", "v0.3.0-beta.8", "v0.3.0-beta.10x"]


class NextReleaseTest(unittest.TestCase):
    def test_beta_follows_last_beta(self):
        self.assertEqual(next_release("0.3.0", "beta", TAGS),
                         {"TAG": "v0.3.0-beta.9", "PRERELEASE": "false", "MAKE_LATEST": "true"})

    def test_first_rc_is_prerelease_not_latest(self):
        self.assertEqual(next_release("1.0.0", "rc", TAGS),
                         {"TAG": "v1.0.0-rc.1", "PRERELEASE": "true", "MAKE_LATEST": "false"})

    def test_rc_counts_only_its_own_version(self):
        tags = TAGS + ["v1.0.0-rc.1", "v1.0.0-rc.2", "v1.1.0-rc.5", "v1.0.0-beta.4"]
        self.assertEqual(next_release("1.0.0", "rc", tags)["TAG"], "v1.0.0-rc.3")

    def test_stable(self):
        self.assertEqual(next_release("1.0.0", "stable", TAGS + ["v1.0.0-rc.2"]),
                         {"TAG": "v1.0.0", "PRERELEASE": "false", "MAKE_LATEST": "true"})

    def test_nothing_after_stable_with_same_version(self):
        for channel in ("beta", "rc", "stable"):
            with self.assertRaises(ValueError):
                next_release("1.0.0", channel, TAGS + ["v1.0.0"])

    def test_bad_input(self):
        with self.assertRaises(ValueError):
            next_release("1.0", "rc", TAGS)
        with self.assertRaises(ValueError):
            next_release("1.0.0", "alpha", TAGS)


class NotesTest(unittest.TestCase):
    def test_stable_rejects_beta_notes(self):
        with self.assertRaises(ValueError):
            check_notes("stable", "Windows 版です(ベータ版、未署名)。")
        check_notes("rc", "Windows 版です(ベータ版、未署名)。")
        check_notes("stable", "Windows 版です。")


if __name__ == "__main__":
    unittest.main()
