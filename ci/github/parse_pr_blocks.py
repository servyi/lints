#!/usr/bin/env python3
"""Parse the Negative Control / cosmetic blocks out of the PR body.

Reads the body from the raw event file (byte-faithful, no API call,
no gh dependency — the runner image may ship none).  Interpolating
the body through an env mapping YAML-folds multi-line values: blank
lines are eaten and lines run together, which hides the
"Negative Control:" marker.

Writes `branches` and `cosmetic` (JSON arrays) to GITHUB_OUTPUT.
"""
import json
import os

with open(os.environ["GITHUB_EVENT_PATH"]) as f:
    event = json.load(f)
body = (event.get("pull_request") or {}).get("body") or ""

branches = []
cosmetic = []
in_block = False
seen_marker = False
which = None
for line in body.splitlines():
    stripped = line.strip()
    low = stripped.lower()
    # A NEW marker resets any earlier block: PR bodies quote the
    # contract (example blocks) — the LAST declaration is the real one.
    if low.startswith("negative control") and stripped.endswith(":"):
        seen_marker = True; which = "nc"; branches = []; in_block = False
        continue
    if low.startswith("the following tests have been touched only cosmetically"):
        seen_marker = True; which = "cosmetic"; cosmetic = []; in_block = False
        continue
    if not seen_marker:
        continue
    if stripped.startswith("```"):
        in_block = not in_block
        continue
    if in_block and which == "nc" and stripped.startswith("/"):
        branches.append(stripped)
    if in_block and which == "cosmetic" and stripped:
        cosmetic.append(stripped)

with open(os.environ["GITHUB_OUTPUT"], "a") as out:
    out.write(f"branches={json.dumps(branches)}\n")
    out.write(f"cosmetic={json.dumps(cosmetic)}\n")
print("negative-control branches:", branches or "(none)")
print("cosmetic-only tests:", cosmetic or "(none)")
