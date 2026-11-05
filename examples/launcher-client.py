#!/usr/bin/env python3
"""Read current apps; optionally pass JSON Lines to an explicit launcher adapter.

The adapter consumes one application object per line and returns {"id": "..."}.
This client prints the selected application as JSON. It never executes its Exec.
"""
import argparse
import json
import subprocess
import sys


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--session-manager", default="session-manager")
    parser.add_argument("launcher", nargs=argparse.REMAINDER)
    args = parser.parse_args()
    launcher = args.launcher
    if launcher[:1] == ["--"]:
        launcher = launcher[1:]
    try:
        response = subprocess.run(
            [args.session_manager, "--json", "application", "list"],
            check=True, capture_output=True, text=True, encoding="utf-8", timeout=35,
        )
        applications = json.loads(response.stdout)["applications"]
        by_id = {app["id"]: app for app in applications}
        if len(by_id) != len(applications):
            raise ValueError("duplicate IDs")
        lines = "".join(json.dumps(app, ensure_ascii=False) + "\n" for app in applications)
        if not launcher:
            sys.stdout.write(lines)
            return 0
        selection = subprocess.run(
            launcher, input=lines, check=True, capture_output=True,
            text=True, encoding="utf-8",
        )
        selected = json.loads(selection.stdout)
        if set(selected) != {"id"}:
            raise ValueError("invalid selection")
        app = by_id[selected["id"]]
        print(json.dumps(app, ensure_ascii=False))
        return 0
    except (OSError, subprocess.SubprocessError, ValueError, KeyError, TypeError):
        print("launcher-client: query or selection failed", file=sys.stderr)
        return 1


if __name__ == "__main__":
    sys.exit(main())
