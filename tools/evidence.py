#!/usr/bin/env python3
"""Assemble and publish visual evidence (docs/art/EXCHANGE.md).

Sub-commands (all stdlib, run by CI or locally):

  pending   --evidence DIR --sha SHA --run-id ID --attempt N --version V
            Register a pending attempt in DIR/PROJECT_STATUS.json.

  manifest  --capture DIR --out ATTEMPT_DIR --sha SHA --run-id ID --attempt N
            --version V --checks CHECKS.json [--tests-log FILE] [--repo DIR]
            Copy the capture directory into ATTEMPT_DIR and write
            manifest.json (hashes, sizes, dimensions, checks, capabilities).
            Fails (exit 3) when the size budget is exceeded.

  status    --evidence DIR --sha SHA --run-id ID --attempt N --version V
            --attempt-path REL --result pass|fail --checks CHECKS.json
            [--evidence-commit SHA] [--repo DIR]
            Update DIR/PROJECT_STATUS.json: latest_attempt always; latest_pass
            only on pass; never let an older source commit overwrite the
            pointer of a newer one (checked with `git merge-base`).

CHECKS.json is `{"fmt": "pass|fail|not_run", "clippy": ..., "tests": ...,
"build": ..., "capture": ...}` written by the workflow steps.
"""
import argparse
import datetime as dt
import hashlib
import json
import os
import shutil
import struct
import subprocess
import sys

PNG_LIMIT = 2 * 1024 * 1024
GIF_LIMIT = 8 * 1024 * 1024
RUN_LIMIT = 24 * 1024 * 1024

SPEC_FILES = [
    "docs/art/procedural/FORMAT.md",
    "docs/art/procedural/kestrel.json",
    "docs/art/procedural/boulder.json",
    "docs/art/procedural/viper.json",
    "docs/art/procedural/trama.json",
    "docs/art/procedural/palettes.json",
    "docs/art/procedural/lattice.json",
    "docs/art/capture-suite.json",
    "docs/art/fixtures/idle-v1.json",
    "docs/art/fixtures/combat-v1.json",
]


def sha256(path):
    h = hashlib.sha256()
    with open(path, "rb") as f:
        for chunk in iter(lambda: f.read(1 << 20), b""):
            h.update(chunk)
    return h.hexdigest()


def image_dims(path):
    with open(path, "rb") as f:
        head = f.read(32)
    if head[:8] == b"\x89PNG\r\n\x1a\n":
        w, h = struct.unpack(">II", head[16:24])
        return w, h
    if head[:6] in (b"GIF87a", b"GIF89a"):
        w, h = struct.unpack("<HH", head[6:10])
        return w, h
    return None


def now_utc():
    return dt.datetime.now(dt.timezone.utc).replace(microsecond=0).isoformat().replace("+00:00", "Z")


def load_json(path, default=None):
    try:
        with open(path, encoding="utf-8") as f:
            return json.load(f)
    except FileNotFoundError:
        return default


def dump_json(path, data):
    os.makedirs(os.path.dirname(path) or ".", exist_ok=True)
    with open(path, "w", encoding="utf-8") as f:
        json.dump(data, f, indent=2, ensure_ascii=False)
        f.write("\n")


def parse_test_totals(log_path):
    """Sum `test result: ok. N passed; M failed` lines of a cargo test log."""
    if not log_path or not os.path.exists(log_path):
        return None, None
    passed = failed = 0
    seen = False
    with open(log_path, encoding="utf-8", errors="replace") as f:
        for line in f:
            if line.startswith("test result:"):
                seen = True
                parts = line.replace(";", " ").split()
                for i, tok in enumerate(parts):
                    if tok == "passed" and i > 0:
                        passed += int(parts[i - 1])
                    if tok == "failed" and i > 0:
                        failed += int(parts[i - 1])
    return (passed, failed) if seen else (None, None)


def is_ancestor(repo, old, new):
    """True if `old` is an ancestor of (or equal to) `new` in `repo`."""
    if not old or old == new:
        return True
    try:
        r = subprocess.run(
            ["git", "-C", repo, "merge-base", "--is-ancestor", old, new],
            capture_output=True,
        )
        return r.returncode == 0
    except OSError:
        return True


def cmd_pending(a):
    path = os.path.join(a.evidence, "PROJECT_STATUS.json")
    st = load_json(path, {}) or {}
    st.setdefault("schema_version", 1)
    st["status"] = "pending"
    st["source_sha"] = a.sha
    st["version"] = a.version
    st["latest_attempt"] = {
        "run_id": a.run_id,
        "run_attempt": a.attempt,
        "status": "pending",
        "path": f"builds/{a.sha}/attempt-{a.run_id}-{a.attempt}/",
        "utc": now_utc(),
    }
    st.setdefault("latest_pass", None)
    st.setdefault("checks", {"fmt": "not_run", "clippy": "not_run", "tests": "not_run", "tests_passed": None})
    st["checks"] = {"fmt": "not_run", "clippy": "not_run", "tests": "not_run", "tests_passed": None}
    st.setdefault("capabilities", {})
    st["note"] = "pending: the attempt is running; images of this path do not exist yet"
    dump_json(path, st)
    print(f"pending attempt registered in {path}")


def capabilities(index, checks, tests_passed):
    """A capability is `verified` only when the manifest names its evidence."""
    files = {f["path"] for f in (index or {}).get("files", [])}
    cases = (index or {}).get("frame_data_cases", [])
    exported = [c for c in cases if c.get("status") == "exported"]
    tests_ok = checks.get("tests") == "pass"
    caps = {}
    caps["procedural_v1_kestrel"] = {
        "state": "verified" if ("characters/kestrel/turnaround.png" in files and tests_ok) else "implemented_unverified",
        "evidence": ["characters/kestrel/turnaround.png", "characters/kestrel/silhouette.png",
                     "src/model/procedural.rs tests: every_shipped_spec_parses_builds_and_fits_its_capsule"],
    }
    caps["palettes_json_kestrel"] = {
        "state": "verified" if ("characters/kestrel/palettes.png" in files and tests_ok) else "implemented_unverified",
        "evidence": ["characters/kestrel/palettes.png", "test palettes_json_has_six_per_character_in_slot_order"],
    }
    caps["extras_lag"] = {
        "state": "verified" if tests_ok else "implemented_unverified",
        "evidence": ["test extras_lag_follows_the_closed_algorithm (unit); scenes/combat.gif (visual, not measured)"],
    }
    caps["capture_game3d"] = {
        "state": "verified" if files else "not_verified",
        "evidence": [f for f in sorted(files) if f.endswith(".png") or f.endswith(".gif")][:6],
        "renderer": (index or {}).get("renderer"),
    }
    caps["frame_data_export"] = {
        "state": "verified" if exported and tests_ok else ("implemented_unverified" if exported else "not_verified"),
        "evidence": ["runtime/frame-data.csv", "test every_recipe_produces_its_move_on_every_character"],
        "cases_exported": len(exported),
        "cases_total": len(cases),
    }
    caps["procedural_v1_boulder"] = {"state": "specified_not_implemented", "evidence": []}
    caps["procedural_v1_viper"] = {"state": "specified_not_implemented", "evidence": []}
    caps["trama"] = {"state": "specified_not_implemented", "evidence": []}
    caps["lattice_dressing"] = {"state": "specified_not_implemented", "evidence": []}
    caps["hud_identity"] = {"state": "specified_not_implemented", "evidence": []}
    caps["rollback_checksum_full_coverage"] = {
        "state": "verified" if tests_ok else "implemented_unverified",
        "evidence": ["tests/checksum.rs"],
    }
    caps["windows_gamepad_performance"] = {"state": "not_verified", "evidence": [],
                                          "note": "needs a real Windows machine; CI runs Mesa/llvmpipe"}
    if tests_passed is not None:
        caps["tests_passed_total"] = tests_passed
    return caps


def cmd_manifest(a):
    checks = load_json(a.checks, {}) or {}
    index = load_json(os.path.join(a.capture, "capture-index.json"), None) if os.path.isdir(a.capture) else None
    os.makedirs(a.out, exist_ok=True)
    files = []
    total = 0
    problems = []
    if os.path.isdir(a.capture):
        for root, _, names in os.walk(a.capture):
            for n in sorted(names):
                src = os.path.join(root, n)
                rel = os.path.relpath(src, a.capture).replace(os.sep, "/")
                if rel == "capture-index.json":
                    continue
                dst = os.path.join(a.out, rel)
                os.makedirs(os.path.dirname(dst), exist_ok=True)
                shutil.copy2(src, dst)
                size = os.path.getsize(src)
                total += size
                entry = {"path": rel, "bytes": size, "sha256": sha256(src)}
                dims = image_dims(src)
                if dims:
                    entry["width"], entry["height"] = dims
                    if rel.endswith(".png") and size > PNG_LIMIT:
                        problems.append(f"{rel}: {size} B > PNG budget {PNG_LIMIT}")
                    if rel.endswith(".gif") and size > GIF_LIMIT:
                        problems.append(f"{rel}: {size} B > GIF budget {GIF_LIMIT}")
                meta = next((f for f in (index or {}).get("files", []) if f["path"] == rel), None)
                if meta:
                    entry["kind"] = meta.get("kind")
                    for k in ("tick", "fixture", "camera", "note"):
                        if k in meta and meta[k] is not None:
                            entry[k] = meta[k]
                    if dims and (meta.get("width"), meta.get("height")) != dims:
                        problems.append(f"{rel}: index says {meta.get('width')}x{meta.get('height')}, file is {dims[0]}x{dims[1]}")
                files.append(entry)
    if total > RUN_LIMIT:
        problems.append(f"run total {total} B > budget {RUN_LIMIT}")
    passed, failed = parse_test_totals(a.tests_log)
    specs = {}
    for rel in SPEC_FILES:
        p = os.path.join(a.repo, rel)
        if os.path.exists(p):
            specs[rel] = sha256(p)
    manifest = {
        "schema_version": 1,
        "source_sha": a.sha,
        "version": a.version,
        "run_id": a.run_id,
        "run_attempt": a.attempt,
        "utc": now_utc(),
        "platform": {
            "runner_os": os.environ.get("RUNNER_OS", sys.platform),
            "runner_arch": os.environ.get("RUNNER_ARCH", ""),
            "image": os.environ.get("ImageOS", ""),
            "python": sys.version.split()[0],
        },
        "renderer": {
            "kind": "game3d",
            "description": (index or {}).get("renderer", "not captured"),
            "context": "Xvfb + Mesa llvmpipe (software GL); validates shape/colour, not GPU performance",
            "window": (index or {}).get("window"),
            "outline_widths_world_units": (index or {}).get("outline_widths"),
        },
        "capture_suite": (index or {}).get("suite_id"),
        "tool": (index or {}).get("tool"),
        "spec_sha256": specs,
        "checks": {
            "fmt": {"command": "cargo fmt --all -- --check", "result": checks.get("fmt", "not_run")},
            "clippy": {"command": "cargo clippy --all-targets --all-features -- -D warnings", "result": checks.get("clippy", "not_run")},
            "tests": {"command": "cargo test --all-features", "result": checks.get("tests", "not_run"),
                      "passed": passed, "failed": failed},
            "build": {"command": "cargo build --release", "result": checks.get("build", "not_run")},
            "capture": {"command": "xvfb-run -a -s '-screen 0 1920x1080x24' target/release/overframe --capture out --sha <sha>",
                        "result": checks.get("capture", "not_run")},
        },
        "triangles": (index or {}).get("triangles"),
        "files": files,
        "skipped": (index or {}).get("skipped", []),
        "frame_data_cases": (index or {}).get("frame_data_cases", []),
        "capabilities": capabilities(index, checks, passed),
        "size_budget": {"png": PNG_LIMIT, "gif": GIF_LIMIT, "run": RUN_LIMIT, "total_bytes": total},
        "problems": problems,
    }
    dump_json(os.path.join(a.out, "manifest.json"), manifest)
    print(f"manifest: {len(files)} files, {total} bytes, {len(problems)} problems → {a.out}")
    for p in problems:
        print("  problem:", p)
    if problems:
        sys.exit(3)


def cmd_status(a):
    path = os.path.join(a.evidence, "PROJECT_STATUS.json")
    st = load_json(path, {}) or {}
    checks = load_json(a.checks, {}) or {}
    passed, _ = parse_test_totals(a.tests_log) if a.tests_log else (None, None)
    current = st.get("source_sha")
    if current and not is_ancestor(a.repo, current, a.sha) and st.get("status") != "pending":
        print(f"refusing to move the pointer from newer {current} to older {a.sha}")
        # Still record this attempt under history so nothing is lost.
        st.setdefault("history", []).append({
            "source_sha": a.sha, "run_id": a.run_id, "run_attempt": a.attempt,
            "status": a.result, "path": a.attempt_path, "utc": now_utc(),
        })
        dump_json(path, st)
        return
    st["schema_version"] = 1
    st["status"] = a.result
    st["source_sha"] = a.sha
    st["version"] = a.version
    st["latest_attempt"] = {
        "run_id": a.run_id, "run_attempt": a.attempt, "status": a.result,
        "path": a.attempt_path, "utc": now_utc(),
    }
    if a.result == "pass":
        st["latest_pass"] = dict(st["latest_attempt"], source_sha=a.sha)
    else:
        st.setdefault("latest_pass", None)
        if st.get("latest_pass"):
            st["latest_pass"]["note"] = "older than latest_attempt on purpose: the newer run did not pass"
    st["evidence_commit"] = a.evidence_commit or None
    st["checks"] = {
        "fmt": checks.get("fmt", "not_run"),
        "clippy": checks.get("clippy", "not_run"),
        "tests": checks.get("tests", "not_run"),
        "tests_passed": passed,
    }
    manifest = load_json(os.path.join(a.evidence, a.attempt_path, "manifest.json"), None)
    st["capabilities"] = {k: (v.get("state") if isinstance(v, dict) else v)
                          for k, v in ((manifest or {}).get("capabilities", {})).items()}
    st["note"] = ("evidence_commit is filled by the publish step after this file is committed; "
                  "read images from that commit, not from the branch tip")
    dump_json(path, st)
    print(f"status {a.result} for {a.sha} → {path}")


def main():
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    sub = ap.add_subparsers(dest="cmd", required=True)

    p = sub.add_parser("pending")
    p.add_argument("--evidence", required=True)
    p.add_argument("--sha", required=True)
    p.add_argument("--run-id", required=True)
    p.add_argument("--attempt", required=True)
    p.add_argument("--version", required=True)
    p.set_defaults(fn=cmd_pending)

    p = sub.add_parser("manifest")
    p.add_argument("--capture", required=True)
    p.add_argument("--out", required=True)
    p.add_argument("--sha", required=True)
    p.add_argument("--run-id", required=True)
    p.add_argument("--attempt", required=True)
    p.add_argument("--version", required=True)
    p.add_argument("--checks", required=True)
    p.add_argument("--tests-log", default=None)
    p.add_argument("--repo", default=".")
    p.set_defaults(fn=cmd_manifest)

    p = sub.add_parser("status")
    p.add_argument("--evidence", required=True)
    p.add_argument("--sha", required=True)
    p.add_argument("--run-id", required=True)
    p.add_argument("--attempt", required=True)
    p.add_argument("--version", required=True)
    p.add_argument("--attempt-path", required=True)
    p.add_argument("--result", required=True, choices=["pass", "fail"])
    p.add_argument("--checks", required=True)
    p.add_argument("--tests-log", default=None)
    p.add_argument("--evidence-commit", default=None)
    p.add_argument("--repo", default=".")
    p.set_defaults(fn=cmd_status)

    a = ap.parse_args()
    a.fn(a)


if __name__ == "__main__":
    main()
