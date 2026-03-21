#!/usr/bin/env python3
"""
validate_site.py — Assert that docs/index.html contains required HTML elements.

Usage:
    python3 scripts/validate_site.py           # validate docs/index.html
    python3 scripts/validate_site.py --quiet   # suppress output on success
    python3 scripts/validate_site.py --test    # run built-in unit tests

Exits with code 0 on success, 1 if any required elements are missing.
"""

import sys
import os
import argparse
import unittest

REQUIRED_ELEMENTS = [
    'id="stream-console"',
    'id="stream-log"',
    'id="countdown"',
    'EventSource',
]


def validate_html(html: str) -> list[str]:
    """
    Return a list of missing required element markers.
    An empty list means all elements are present.
    """
    missing = []
    for element in REQUIRED_ELEMENTS:
        if element not in html:
            missing.append(element)
    return missing


def validate_file(path: str, quiet: bool = False) -> bool:
    """
    Validate the HTML file at `path`.
    Returns True if all required elements are present, False otherwise.
    Prints results unless quiet=True and all pass.
    """
    if not os.path.exists(path):
        print(f"error: file not found: {path}", file=sys.stderr)
        return False

    with open(path, "r", encoding="utf-8") as fh:
        html = fh.read()

    missing = validate_html(html)

    if missing:
        print(f"FAIL: {path} is missing required elements:")
        for m in missing:
            print(f"  ✗ {m}")
        return False
    else:
        if not quiet:
            print(f"OK: {path} contains all required elements:")
            for e in REQUIRED_ELEMENTS:
                print(f"  ✓ {e}")
        return True


# ---------------------------------------------------------------------------
# Unit tests
# ---------------------------------------------------------------------------

class TestValidateHtml(unittest.TestCase):

    def _full_html(self):
        return (
            '<html><body>'
            '<section id="stream-console">'
            '<div id="stream-log"></div>'
            '<span id="countdown"></span>'
            'new EventSource("/stream")'
            '</body></html>'
        )

    def test_all_elements_present_passes(self):
        """All four required elements present → no missing items."""
        html = self._full_html()
        missing = validate_html(html)
        self.assertEqual(missing, [], f"Expected no missing elements, got: {missing}")

    def test_missing_stream_console_fails(self):
        """Removing stream-console → reported as missing."""
        html = self._full_html().replace('id="stream-console"', 'id="other"')
        missing = validate_html(html)
        self.assertIn('id="stream-console"', missing)
        self.assertNotIn('id="stream-log"', missing)

    def test_missing_eventsource_fails(self):
        """Removing EventSource → reported as missing."""
        html = self._full_html().replace("EventSource", "WebSocket")
        missing = validate_html(html)
        self.assertIn("EventSource", missing)

    def test_empty_file_fails(self):
        """Empty string → all four elements missing."""
        missing = validate_html("")
        self.assertEqual(len(missing), len(REQUIRED_ELEMENTS))
        for element in REQUIRED_ELEMENTS:
            self.assertIn(element, missing)

    def test_partial_html_reports_correct_missing(self):
        """HTML with only stream-console and countdown → stream-log and EventSource missing."""
        html = '<section id="stream-console"><span id="countdown"></span>'
        missing = validate_html(html)
        self.assertNotIn('id="stream-console"', missing)
        self.assertNotIn('id="countdown"', missing)
        self.assertIn('id="stream-log"', missing)
        self.assertIn("EventSource", missing)
        self.assertEqual(len(missing), 2)


# ---------------------------------------------------------------------------
# Entry point
# ---------------------------------------------------------------------------

def main():
    parser = argparse.ArgumentParser(
        description="Validate that docs/index.html contains required HTML elements."
    )
    parser.add_argument(
        "--quiet",
        action="store_true",
        help="Suppress output on success.",
    )
    parser.add_argument(
        "--test",
        action="store_true",
        help="Run built-in unit tests instead of validating the site.",
    )
    args = parser.parse_args()

    if args.test:
        # Re-invoke unittest on this module
        loader = unittest.TestLoader()
        suite = loader.loadTestsFromTestCase(TestValidateHtml)
        runner = unittest.TextTestRunner(verbosity=2)
        result = runner.run(suite)
        sys.exit(0 if result.wasSuccessful() else 1)

    # Default: validate docs/index.html relative to repo root
    repo_root = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
    index_path = os.path.join(repo_root, "docs", "index.html")

    ok = validate_file(index_path, quiet=args.quiet)
    sys.exit(0 if ok else 1)


if __name__ == "__main__":
    # Allow `python3 -m unittest scripts/validate_site.py` to discover tests
    # by checking if we're being run directly vs as a unittest discovery target.
    # When invoked via `python3 -m unittest`, __name__ is still "__main__" but
    # sys.argv[0] will reference the unittest runner.
    if len(sys.argv) > 0 and "unittest" in sys.argv[0]:
        unittest.main()
    else:
        main()
