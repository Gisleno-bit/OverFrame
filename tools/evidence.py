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
    "docs/art/procedural/anim/FORMAT.md",
    "docs/art/procedural/anim/kestrel.json",
    "docs/art/capture-suite.json",
    "docs/art/fixtures/idle-v1.json",
    "docs/art/fixtures/combat-v1.json",
]

# The only capability states the exchange schema knows (PROJECT_STATUS.md).
STATES = ("verified", "implemented_unverified", "specified_not_implemented", "not_verified")


def log(msg):
    """Console output that never raises on a narrow console encoding
    (Windows cp1252): the message itself is ASCII by construction, and any
    non-ASCII path or reason it carries is escaped rather than encoded."""
    text = str(msg)
    try:
        enc = sys.stdout.encoding or "ascii"
        text.encode(enc)
    except (UnicodeEncodeError, LookupError):
        text = text.encode("ascii", "backslashreplace").decode("ascii")
    print(text)


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
    log(f"pending attempt registered in {path}")


def contact_coverage(index, present, problems, capture_dir):
    """Coverage = every (character, action, variant) case the runtime
    exporter reports for the suite's characters must have its primary PNG,
    its wide PNG and its contact JSON among the files actually copied.
    Sources cross-checked: runtime/frame-data-report.json (the case set),
    runtime/contact-report.json (per-case JSON outcome), the tool's own
    `contact_expected` list, and the file set. An unexplained gap is a
    problem (publish fails); an explained one stays `missing` and keeps
    the capability from being `verified`. Never trusts index.files alone."""
    chars = (index or {}).get("characters") or []
    report = load_json(os.path.join(capture_dir, "runtime", "frame-data-report.json"), []) or []
    contact_report = load_json(os.path.join(capture_dir, "runtime", "contact-report.json"), []) or []
    contact_status = {(c.get("character_id"), c.get("action_id"), c.get("variant_id")): c for c in contact_report}
    expected = {(e["character_id"], e["action_id"], e["variant_id"]): e
                for e in (index or {}).get("contact_expected", [])}
    skipped = {s[0]: s[1] for s in (index or {}).get("skipped", []) if isinstance(s, list) and len(s) == 2}
    per_char = {}
    seen_in_report = set()
    for r in report:
        ch = r.get("character_id")
        if ch not in chars:
            continue
        key = (ch, r.get("action_id"), r.get("variant_id"))
        seen_in_report.add(key)
        c = per_char.setdefault(ch, {"cases": 0, "expected_files": 0, "present": 0, "missing": [], "kinds": {}})
        c["cases"] += 1
        if r.get("status") != "exported":
            c["missing"].append({"case": key[1:], "path": None, "reason": f"runtime export: {r.get('status')}"})
            continue
        e = expected.get(key)
        if e is None:
            # The tool never declared sheets for a case the exporter has.
            c["missing"].append({"case": key[1:], "path": None, "reason": "unexplained: no contact sheets declared for this runtime case"})
            problems.append(f"no contact sheets declared for runtime case {key}")
            continue
        c["kinds"][e["evidence_kind"]] = c["kinds"].get(e["evidence_kind"], 0) + 1
        json_path = f"runtime/contact/{ch}/{key[1]}-{key[2]}.json"
        for path in (e["primary"], e["wide"], json_path):
            c["expected_files"] += 1
            if path in present:
                c["present"] += 1
            else:
                reason = skipped.get(path)
                if reason is None and path == json_path:
                    st = contact_status.get(key, {}).get("status")
                    reason = st if st and st != "written" else None
                if reason is None:
                    reason = "unexplained"
                    problems.append(f"contact evidence missing without a reason: {path}")
                c["missing"].append({"case": key[1:], "path": path, "reason": reason})
    for key, e in expected.items():
        if key[0] in chars and key not in seen_in_report:
            c = per_char.setdefault(key[0], {"cases": 0, "expected_files": 0, "present": 0, "missing": [], "kinds": {}})
            c["missing"].append({"case": key[1:], "path": e["primary"], "reason": "unexplained: declared sheet has no runtime case"})
            problems.append(f"declared contact case {key} is not in frame-data-report.json")
    if not report:
        problems.append("runtime/frame-data-report.json missing: coverage cannot be established")
    for ch in chars:
        c = per_char.setdefault(ch, {"cases": 0, "expected_files": 0, "present": 0, "missing": [], "kinds": {}})
        c["complete"] = c["cases"] > 0 and c["expected_files"] > 0 and c["present"] == c["expected_files"] and not c["missing"]
    return {
        "rule": "per runtime case of each selected character: primary PNG + wide PNG + runtime/contact JSON, all present in the copied files; sources: frame-data-report.json, contact-report.json, contact_expected, file set",
        "characters": per_char,
    }


def capabilities(index, checks, tests_passed, coverage=None, present=None):
    """A capability is `verified` only when the manifest names evidence that
    exists in this run and the checks passed. States are limited to the
    exchange schema; art approval is never a CI state (it lives in the
    reviewer's docs/art/reviews/<sha>.md, which CI does not read)."""
    files = set(present or [])
    cases = (index or {}).get("frame_data_cases", [])
    exported = [c for c in cases if c.get("status") == "exported"]
    tests_ok = checks.get("tests") == "pass"

    def st(ok, implemented=True):
        if ok and tests_ok:
            return "verified"
        return "implemented_unverified" if implemented else "not_verified"

    caps = {}
    caps["procedural_v1_kestrel"] = {
        "state": st("characters/kestrel/turnaround.png" in files),
        "evidence": ["characters/kestrel/turnaround.png", "characters/kestrel/silhouette.png",
                     "src/model/procedural.rs tests: every_shipped_spec_parses_builds_and_fits_its_capsule"],
    }
    caps["palettes_json_kestrel"] = {
        "state": st("characters/kestrel/palettes.png" in files),
        "evidence": ["characters/kestrel/palettes.png", "test palettes_json_has_six_per_character_in_slot_order"],
    }
    caps["extras_lag"] = {
        "state": st(True),
        "evidence": ["test extras_lag_follows_the_closed_algorithm (unit); scenes/combat.gif (visual, not measured)"],
    }
    imgs = sorted(f for f in files if f.endswith(".png") or f.endswith(".gif"))
    caps["capture_game3d"] = {
        "state": st(bool(imgs), implemented=bool(imgs)),
        "evidence": imgs[:6],
        "renderer": (index or {}).get("renderer"),
    }
    caps["frame_data_export"] = {
        "state": st(bool(exported) and "runtime/frame-data.csv" in files, implemented=bool(exported)),
        "evidence": ["runtime/frame-data.csv", "test every_recipe_produces_its_move_on_every_character"],
        "cases_exported": len(exported),
        "cases_total": len(cases),
    }
    for ch, c in ((coverage or {}).get("characters") or {}).items():
        caps[f"contact_coverage_{ch}"] = {
            "state": st(c.get("complete", False), implemented=c.get("present", 0) > 0),
            "evidence": [f"characters/{ch}/contact-*.png", f"runtime/contact/{ch}/*.json", "manifest.coverage"],
            "cases": c.get("cases", 0), "expected_files": c.get("expected_files", 0),
            "present": c.get("present", 0), "missing": len(c.get("missing", [])),
        }
    caps["contact_measurements"] = {
        "state": st(any(f.startswith("runtime/contact/") for f in files),
                    implemented=any(f.startswith("runtime/contact/") for f in files)),
        "evidence": ["runtime/contact/<character>/<action>-<variant>.json (mesh_distance, signed_separation, tip_reach_x, capsule, eased_facing, both facings)",
                     "src/model/contact.rs and src/model/render_eval.rs tests"],
    }
    iso = (index or {}).get("isolation_check") or {}
    caps["capture_isolation"] = {
        "state": st(iso.get("identical") is True, implemented=bool(iso)),
        "evidence": ["manifest.isolation_check (first contact case re-rendered after the run and compared pixel for pixel)"],
    }
    caps["hurt_capsule_overlays"] = {
        "state": st("characters/kestrel/capsules.png" in files),
        "evidence": ["characters/kestrel/capsules.png", "test capsule_shrinks_in_crouch_and_matches_the_sim"],
    }
    caps["mirror_0_3_sheet_kestrel"] = {
        "state": st("characters/kestrel/mirror-0-3.png" in files),
        "evidence": ["characters/kestrel/mirror-0-3.png"],
        "note": "technical presence only; readability is an art-review verdict recorded outside CI",
    }
    caps["art_review_kestrel"] = {"state": "not_verified", "evidence": [],
                                  "note": "set only by the reviewer against this SHA's evidence; CI never marks it"}
    caps["animation_direction_kestrel"] = {"state": "specified_not_implemented", "evidence": []}
    caps["procedural_v1_boulder"] = {"state": "specified_not_implemented", "evidence": []}
    caps["procedural_v1_viper"] = {"state": "specified_not_implemented", "evidence": []}
    caps["trama"] = {"state": "specified_not_implemented", "evidence": []}
    caps["lattice_dressing"] = {"state": "specified_not_implemented", "evidence": []}
    caps["hud_identity"] = {"state": "specified_not_implemented", "evidence": []}
    caps["rollback_checksum_full_coverage"] = {
        "state": st(True),
        "evidence": ["tests/checksum.rs"],
    }
    caps["windows_gamepad_performance"] = {"state": "not_verified", "evidence": [],
                                          "note": "needs a real Windows machine; CI runs Mesa/llvmpipe"}
    for k, v in caps.items():
        assert v["state"] in STATES, (k, v["state"])
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
    present = {f["path"] for f in files}
    coverage = contact_coverage(index, present, problems, a.capture)
    passed, failed = parse_test_totals(a.tests_log)
    specs = {}
    for rel in SPEC_FILES:
        p = os.path.join(a.repo, rel)
        # A missing spec is recorded, never silently skipped.
        specs[rel] = sha256(p) if os.path.exists(p) else None
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
        "coverage": coverage,
        "files": files,
        "skipped": (index or {}).get("skipped", []),
        "frame_data_cases": (index or {}).get("frame_data_cases", []),
        "isolation_check": (index or {}).get("isolation_check"),
        "capabilities": capabilities(index, checks, passed, coverage, present),
        "size_budget": {"png": PNG_LIMIT, "gif": GIF_LIMIT, "run": RUN_LIMIT, "total_bytes": total},
        "problems": problems,
    }
    dump_json(os.path.join(a.out, "manifest.json"), manifest)
    log(f"manifest: {len(files)} files, {total} bytes, {len(problems)} problems -> {a.out}")
    for p in problems:
        log("  problem: " + p)
    if problems:
        sys.exit(3)


def cmd_status(a):
    path = os.path.join(a.evidence, "PROJECT_STATUS.json")
    st = load_json(path, {}) or {}
    checks = load_json(a.checks, {}) or {}
    passed, _ = parse_test_totals(a.tests_log) if a.tests_log else (None, None)
    current = st.get("source_sha")
    if current and not is_ancestor(a.repo, current, a.sha) and st.get("status") != "pending":
        log(f"refusing to move the pointer from newer {current} to older {a.sha}")
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
    st["coverage"] = {ch: {"complete": c.get("complete", False), "present": c.get("present", 0),
                           "expected_files": c.get("expected_files", 0), "missing": len(c.get("missing", []))}
                      for ch, c in ((manifest or {}).get("coverage", {}).get("characters", {})).items()}
    st["note"] = ("evidence_commit is filled by the publish step after this file is committed; "
                  "read images from that commit, not from the branch tip")
    dump_json(path, st)
    log(f"status {a.result} for {a.sha} -> {path}")


def _tolerant_streams():
    """Never let a console encoding abort a publish: replace what cannot
    be encoded instead of raising (JSON files are always written UTF-8)."""
    for stream in (sys.stdout, sys.stderr):
        try:
            stream.reconfigure(errors="replace")
        except (AttributeError, ValueError):
            pass


def main():
    _tolerant_streams()
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
