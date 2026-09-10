#!/usr/bin/env python3
"""Self-test of tools/evidence.py's coverage and capability rules.

Run: python3 tools/test_evidence.py  (stdlib unittest; no game build needed).

Builds a synthetic capture directory that mirrors what `overframe --capture`
writes (index, frame-data report, contact report, sheets, contact JSON),
then checks that removing or omitting any piece of contact evidence never
leaves the coverage capability `verified`, and that an unexplained gap
fails the manifest.
"""
import json
import os
import shutil
import struct
import subprocess
import sys
import tempfile
import unittest
import zlib

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, HERE)
import evidence  # noqa: E402

CASES = [("jab", "ground", "hitbox"), ("special_n", "ground", "projectile"), ("throw_f", "from_grab", "throw_no_hitbox")]


def png_bytes(w, h):
    """A minimal valid PNG (grey), so image_dims() and sizes behave."""
    raw = b"".join(b"\x00" + bytes([128, 128, 128, 255]) * w for _ in range(h))

    def chunk(tag, data):
        c = struct.pack(">I", len(data)) + tag + data
        return c + struct.pack(">I", zlib.crc32(tag + data) & 0xFFFFFFFF)

    return (b"\x89PNG\r\n\x1a\n" + chunk(b"IHDR", struct.pack(">IIBBBBB", w, h, 8, 6, 0, 0, 0))
            + chunk(b"IDAT", zlib.compress(raw)) + chunk(b"IEND", b""))


def make_capture(root, cases=CASES, drop=(), skipped=None, report_status=None, isolation=True):
    """Write a synthetic capture dir. `drop` = relative paths to omit."""
    files = []
    expected = []
    report = []
    contact_report = []
    for action, variant, kind in cases:
        primary = f"characters/kestrel/contact-{action}.png"
        wide = f"characters/kestrel/contact-{action}-wide.png"
        cjson = f"runtime/contact/kestrel/{action}-{variant}.json"
        expected.append({"character_id": "kestrel", "action_id": action, "variant_id": variant,
                         "primary": primary, "wide": wide, "evidence_kind": kind})
        report.append({"character_id": "kestrel", "action_id": action, "variant_id": variant,
                       "status": (report_status or {}).get(action, "exported"), "rows": 5, "recipe": "t0"})
        contact_report.append({"character_id": "kestrel", "action_id": action, "variant_id": variant,
                               "path": cjson, "status": "written" if cjson not in drop else "error: synthetic"})
        for rel, (w, h) in ((primary, (1536, 512)), (wide, (2048, 1024))):
            if rel in drop:
                continue
            p = os.path.join(root, rel)
            os.makedirs(os.path.dirname(p), exist_ok=True)
            with open(p, "wb") as f:
                f.write(png_bytes(w, h))
            files.append({"path": rel, "kind": "game3d", "width": w, "height": h, "camera": {}})
        if cjson not in drop:
            p = os.path.join(root, cjson)
            os.makedirs(os.path.dirname(p), exist_ok=True)
            with open(p, "w", encoding="utf-8") as f:
                json.dump({"facings": []}, f)
    os.makedirs(os.path.join(root, "runtime"), exist_ok=True)
    with open(os.path.join(root, "runtime", "frame-data-report.json"), "w", encoding="utf-8") as f:
        json.dump(report, f)
    with open(os.path.join(root, "runtime", "contact-report.json"), "w", encoding="utf-8") as f:
        json.dump(contact_report, f)
    with open(os.path.join(root, "runtime", "frame-data.csv"), "w", encoding="utf-8") as f:
        f.write(evidence_header() + "\n")
    index = {
        "schema_version": 1, "tool": "test", "source_sha": "0" * 40, "suite_id": "t",
        "renderer": "test", "window": [1920, 1080], "outline_widths": [], "triangles": {},
        "files": files, "skipped": [list(x) for x in (skipped or [])],
        "frame_data_cases": report, "characters": ["kestrel"], "contact_expected": expected,
        "isolation_check": {"identical": True} if isolation else {"identical": False, "primary_differing_pixels": 3},
    }
    with open(os.path.join(root, "capture-index.json"), "w", encoding="utf-8") as f:
        json.dump(index, f, ensure_ascii=False)


def evidence_header():
    return "source_sha,character_id,action_id,variant_id,sample_phase,tick_index,state_frame,hitbox_id,hitbox_active,center_x,center_y,radius,hitlag_remaining,contact_marker"


def run_manifest(capture, out, checks=None, encoding="cp1252"):
    """Run the CLI as CI does — a child process — under a narrow console
    encoding (Windows' cp1252 by default here, whatever the parent has), so
    an accidental non-encodable character in a log line fails this test
    instead of a publish."""
    checks = checks or {"fmt": "pass", "clippy": "pass", "tests": "pass", "build": "pass", "capture": "pass"}
    cpath = os.path.join(out, "checks.json")
    os.makedirs(out, exist_ok=True)
    with open(cpath, "w", encoding="utf-8") as f:
        json.dump(checks, f)
    env = dict(os.environ)
    env["PYTHONIOENCODING"] = encoding
    env.pop("PYTHONUTF8", None)
    r = subprocess.run(
        [sys.executable, "-X", "utf8=0", os.path.join(HERE, "evidence.py"), "manifest", "--capture", capture, "--out",
         os.path.join(out, "attempt"), "--sha", "0" * 40, "--run-id", "1", "--attempt", "1",
         "--version", "test", "--checks", cpath, "--repo", os.path.join(HERE, "..")],
        capture_output=True, text=True, env=env, encoding=encoding, errors="replace",
    )
    manifest = evidence.load_json(os.path.join(out, "attempt", "manifest.json"), {})
    return r.returncode, manifest, r.stdout + r.stderr


class Coverage(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.mkdtemp()
        self.cap = os.path.join(self.tmp, "cap")
        self.out = os.path.join(self.tmp, "out")

    def tearDown(self):
        shutil.rmtree(self.tmp, ignore_errors=True)

    def cov_state(self, manifest):
        return manifest["capabilities"]["contact_coverage_kestrel"]["state"]

    def test_complete_run_is_verified(self):
        make_capture(self.cap)
        code, m, log = run_manifest(self.cap, self.out)
        self.assertEqual(code, 0, log)
        self.assertEqual(self.cov_state(m), "verified")
        self.assertTrue(m["coverage"]["characters"]["kestrel"]["complete"])
        self.assertEqual(m["coverage"]["characters"]["kestrel"]["expected_files"], 9)
        self.assertEqual(m["capabilities"]["capture_isolation"]["state"], "verified")
        self.assertEqual(m["capabilities"]["art_review_kestrel"]["state"], "not_verified")
        for k, v in m["capabilities"].items():
            if isinstance(v, dict):
                self.assertIn(v["state"], evidence.STATES, k)

    def test_missing_sheet_without_reason_fails_and_is_never_verified(self):
        make_capture(self.cap, drop=("characters/kestrel/contact-jab-wide.png",))
        code, m, log = run_manifest(self.cap, self.out)
        self.assertEqual(code, 3, log)
        self.assertNotEqual(self.cov_state(m), "verified")
        self.assertTrue(any("without a reason" in p for p in m["problems"]))

    def test_missing_contact_json_is_never_verified(self):
        make_capture(self.cap, drop=("runtime/contact/kestrel/special_n-ground.json",))
        code, m, log = run_manifest(self.cap, self.out)
        self.assertNotEqual(self.cov_state(m), "verified")
        missing = m["coverage"]["characters"]["kestrel"]["missing"]
        self.assertTrue(any(x["path"] and x["path"].endswith("special_n-ground.json") for x in missing))
        # The contact-report explains it, so it is not an unexplained problem.
        self.assertEqual(code, 0, log)

    def test_explained_skip_is_visible_but_not_verified(self):
        make_capture(self.cap, drop=("characters/kestrel/contact-throw_f.png",),
                     skipped=[("characters/kestrel/contact-throw_f.png", "no release tick in export")])
        code, m, log = run_manifest(self.cap, self.out)
        self.assertEqual(code, 0, log)
        self.assertEqual(self.cov_state(m), "implemented_unverified")

    def test_runtime_case_without_declared_sheets_is_a_problem(self):
        make_capture(self.cap)
        idx = evidence.load_json(os.path.join(self.cap, "capture-index.json"))
        idx["contact_expected"] = [e for e in idx["contact_expected"] if e["action_id"] != "jab"]
        with open(os.path.join(self.cap, "capture-index.json"), "w", encoding="utf-8") as f:
            json.dump(idx, f, ensure_ascii=False)
        code, m, log = run_manifest(self.cap, self.out)
        self.assertEqual(code, 3, log)
        self.assertNotEqual(self.cov_state(m), "verified")

    def test_failed_tests_downgrade_every_verified_capability(self):
        make_capture(self.cap)
        code, m, log = run_manifest(self.cap, self.out, checks={"fmt": "pass", "clippy": "pass", "tests": "fail",
                                                                 "build": "pass", "capture": "pass"})
        for k, v in m["capabilities"].items():
            if isinstance(v, dict):
                self.assertNotEqual(v["state"], "verified", k)

    def test_isolation_mismatch_is_not_verified(self):
        make_capture(self.cap, isolation=False)
        code, m, log = run_manifest(self.cap, self.out)
        self.assertEqual(m["capabilities"]["capture_isolation"]["state"], "implemented_unverified")

    def test_console_encoding_never_breaks_a_publish(self):
        # A reason with characters cp1252 cannot encode (arrow, CJK) must
        # still be reported and the manifest still written, under cp1252
        # and under plain ASCII consoles.
        for enc in ("cp1252", "ascii"):
            shutil.rmtree(self.out, ignore_errors=True)
            shutil.rmtree(self.cap, ignore_errors=True)
            make_capture(self.cap, drop=("characters/kestrel/contact-throw_f.png",),
                         skipped=[("characters/kestrel/contact-throw_f.png", "motivo \u2192 \u65e5\u672c \u00e9")])
            code, m, log = run_manifest(self.cap, self.out, encoding=enc)
            self.assertEqual(code, 0, log)
            self.assertNotIn("UnicodeEncodeError", log)
            self.assertNotIn("Traceback", log)
            reasons = [x["reason"] for x in m["coverage"]["characters"]["kestrel"]["missing"]]
            self.assertTrue(any("\u2192" in r for r in reasons), "reason kept intact in the JSON")
            # And an unexplained gap (which prints the path) also survives.
            shutil.rmtree(self.out, ignore_errors=True)
            shutil.rmtree(self.cap, ignore_errors=True)
            make_capture(self.cap, drop=("characters/kestrel/contact-jab-wide.png",))
            code, m, log = run_manifest(self.cap, self.out, encoding=enc)
            self.assertEqual(code, 3, log)
            self.assertNotIn("Traceback", log)
            self.assertIn("problem:", log)

    def test_spec_hashes_list_every_spec_even_when_missing(self):
        make_capture(self.cap)
        code, m, log = run_manifest(self.cap, self.out)
        for rel in evidence.SPEC_FILES:
            self.assertIn(rel, m["spec_sha256"])
        self.assertIn("docs/art/procedural/anim/kestrel.json", m["spec_sha256"])


if __name__ == "__main__":
    unittest.main(verbosity=2)
