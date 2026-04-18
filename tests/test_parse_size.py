import unittest

from bigdiff.io_utils import parse_size


class ParseSizeTests(unittest.TestCase):
    def test_accepts_documented_decimal_units(self):
        self.assertEqual(parse_size("5MB"), 5_000_000)
        self.assertEqual(parse_size("5mb"), 5_000_000)
        self.assertEqual(parse_size("5B"), 5)
        self.assertEqual(parse_size("1.5GB"), 1_500_000_000)
        self.assertEqual(parse_size("1.5gb"), 1_500_000_000)
        self.assertEqual(parse_size("200k"), 200_000)
        self.assertEqual(parse_size("200K"), 200_000)

    def test_accepts_binary_units_and_raw_bytes(self):
        self.assertEqual(parse_size("5MiB"), 5 * 1024 * 1024)
        self.assertEqual(parse_size("5mib"), 5 * 1024 * 1024)
        self.assertEqual(parse_size("2GiB"), 2 * 1024 * 1024 * 1024)
        self.assertEqual(parse_size("2gib"), 2 * 1024 * 1024 * 1024)
        self.assertEqual(parse_size("10"), 10)
        self.assertEqual(parse_size("10B"), 10)


if __name__ == "__main__":
    unittest.main()
