#!/usr/bin/env python3
"""Check that files don't exceed max line count."""
import sys

MAX_LINES = 500

def main():
    failed = False
    for filepath in sys.argv[1:]:
        try:
            with open(filepath) as f:
                lines = len(f.readlines())
            if lines > MAX_LINES:
                print(f"{filepath}: {lines} lines (max {MAX_LINES})")
                failed = True
        except Exception:
            pass
    return 1 if failed else 0

if __name__ == "__main__":
    sys.exit(main())
