#!/usr/bin/env python3
"""Production-guard rejection tests and real workflow-block shell probes.

Metadata below is deliberately synthetic, not a captured Cargo execution.
Source declarations/lock are copied from the checked workspace. No real Cargo
is invoked: shell probes place an executable mock first on PATH.
"""

import copy
import importlib.util
import json
import os
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile
import unittest

sys.dont_write_bytecode = True
SCRIPTS = Path(__file__).resolve().parent
GUARD_PATH = SCRIPTS / "check-tiforth-native-memory.py"
WORKFLOW = SCRIPTS.parents[1] / ".github/workflows/tiforth-native-memory.yml"
SOURCE = Path(os.environ.get("TIFORTH_NATIVE_MEMORY_WORKSPACE", str(SCRIPTS.parent))).resolve()
SPEC = importlib.util.spec_from_file_location("native_memory_guard", GUARD_PATH)
guard = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(guard)


def run_blocks():
    """Extract literal run bodies for execution, not general YAML validation."""
    result = {}
    # Bound bodies by the next step. This workflow uses literal, indented runs.
    for step in WORKFLOW.read_text().split("      - name: ")[1:]:
        lines = step.splitlines()
        name = lines[0]
        if "        run: |" in lines:
            start = lines.index("        run: |") + 1
            body = []
            for line in lines[start:]:
                if line and not line.startswith("          "):
                    break
                body.append(line[10:] if line else "")
            result[name] = "\n".join(body) + "\n"
    return result


def synthetic_metadata(workspace):
    def package(name, version="0.0.0", source=None):
        identity = f"{source or 'path+file://' + str(workspace / 'crates' / name)}#{name}@{version}"
        return {"name": name, "version": version, "source": source, "id": identity,
                "manifest_path": str(workspace / "crates" / name / "Cargo.toml"),
                "dependencies": [], "features": {}}

    packages = [package(guard.BRIDGE), package(guard.SERVER)]
    packages += [package(name, version, origin)
                 for name, (version, origin, _checksum) in guard.LOCKED.items()]
    by_name = {p["name"]: p for p in packages}
    nodes = [{"id": p["id"], "deps": [], "features": []} for p in packages]
    by_node = {p["name"]: n for p, n in zip(packages, nodes)}

    def dependency(owner, name, optional=False, features=None):
        origin = guard.TIFORTH_DECLARATION if name == "tiforth-memory" else guard.REGISTRY
        by_name[owner]["dependencies"].append({
            "name": name, "kind": None, "source": origin, "req": "^0.7",
            "optional": optional, "features": features or [], "uses_default_features": True,
            "rename": None, "target": None,
        })
        by_node[owner]["deps"].append({
            "name": name.replace("-", "_"), "pkg": by_name[name]["id"],
            "dep_kinds": [{"kind": None, "target": None}],
        })

    dependency(guard.BRIDGE, "tiforth-memory")
    dependency(guard.BRIDGE, "tikv-jemallocator", features=["profiling"])
    dependency(guard.BRIDGE, "tikv-jemalloc-sys")
    dependency("tikv-jemallocator", "tikv-jemalloc-sys")
    dependency(guard.SERVER, "tikv-jemallocator", optional=True)
    dependency(guard.SERVER, "tikv-jemalloc-sys", optional=True)
    by_name[guard.SERVER]["features"] = {"default": ["jemalloc"], "jemalloc": guard.SERVER_JEMALLOC[:]}
    by_node[guard.SERVER]["features"] = ["default", "jemalloc"]
    for name in ("tikv-jemallocator", "tikv-jemalloc-sys"):
        by_node[name]["features"] = ["profiling"]
    return {"version": 1, "workspace_root": str(workspace), "packages": packages,
            "workspace_members": [by_name[n]["id"] for n in (guard.BRIDGE, guard.SERVER)],
            "resolve": {"nodes": nodes}}


class GuardTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory(prefix="native-memory-identity-")
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)
        self.workspace = self.root / "rust"
        for rel in ("Cargo.toml", "Cargo.lock", f"crates/{guard.BRIDGE}/Cargo.toml",
                    f"crates/{guard.SERVER}/Cargo.toml"):
            target = self.workspace / rel
            target.parent.mkdir(parents=True, exist_ok=True)
            shutil.copyfile(SOURCE / rel, target)
        self.source = guard.check_source(self.workspace)
        self.metadata = synthetic_metadata(self.workspace)

    def package(self, name):
        return next(p for p in self.metadata["packages"] if p["name"] == name)

    def node(self, name):
        identity = self.package(name)["id"]
        return next(n for n in self.metadata["resolve"]["nodes"] if n["id"] == identity)

    def rejects(self, fragment):
        with self.assertRaisesRegex(guard.IdentityError, fragment):
            guard.check_metadata(self.metadata, self.source)

    def edit(self, rel, old, new):
        path = self.workspace / rel
        contents = path.read_text()
        self.assertIn(old, contents)
        path.write_text(contents.replace(old, new, 1))

    def test_coherent_synthetic_metadata_and_real_source(self):
        result = guard.check_metadata(self.metadata, self.source)
        self.assertTrue(result["profiling"])
        self.assertEqual(result["server_default"], ["jemalloc"])

    def test_unrelated_workspace_graph_is_not_assumed_closed(self):
        self.metadata["packages"].append({"id": "unrelated@1", "name": "tikv-jemallocator",
                                          "version": "0.6.0", "source": guard.REGISTRY})
        guard.check_metadata(self.metadata, self.source)

    def test_missing_tiforth(self):
        self.metadata["packages"].remove(self.package("tiforth-memory"))
        self.rejects("missing selected package")

    def test_wrong_git_revision(self):
        self.package("tiforth-memory")["source"] = guard.TIFORTH_SOURCE.replace(guard.TIFORTH_REV, "0" * 40)
        self.rejects("locked native identity")

    def test_local_path_copy_cannot_masquerade_as_tiforth(self):
        self.package("tiforth-memory")["source"] = None
        self.rejects("locked native identity")

    def test_missing_git_revision_in_declaration(self):
        self.package(guard.BRIDGE)["dependencies"][0]["source"] = "git+" + guard.TIFORTH_GIT
        self.rejects("dependency source")

    def test_resolved_edge_cannot_hide_behind_correct_unused_package(self):
        fake = copy.deepcopy(self.package("tiforth-memory"))
        fake.update(id="path+file:///copy#tiforth-memory@0.1.0", source=None)
        self.metadata["packages"].append(fake)
        self.node(guard.BRIDGE)["deps"][0]["pkg"] = fake["id"]
        self.rejects("locked native identity")

    def test_wrong_allocator_or_sys_version(self):
        for name in ("tikv-jemallocator", "tikv-jemalloc-sys"):
            with self.subTest(package=name):
                original = self.package(name)["version"]
                self.package(name)["version"] = "0.6.0"
                self.rejects("locked native identity")
                self.package(name)["version"] = original

    def test_missing_activated_profiling(self):
        for name in ("tikv-jemallocator", "tikv-jemalloc-sys"):
            with self.subTest(package=name):
                self.node(name)["features"] = []
                self.rejects("profiling is not active")
                self.node(name)["features"] = ["profiling"]

    def test_missing_declared_profiling(self):
        self.package(guard.BRIDGE)["dependencies"][1]["features"] = []
        self.rejects("declared features")

    def test_server_default_not_activated(self):
        self.node(guard.SERVER)["features"] = ["jemalloc"]
        self.rejects("default Jemalloc feature is not active")

    def test_server_metadata_default_changed(self):
        self.package(guard.SERVER)["features"]["default"] = []
        self.rejects("default Jemalloc declarations")

    def test_wrong_workspace_or_bridge_manifest(self):
        original = self.metadata["workspace_root"]
        self.metadata["workspace_root"] = str(self.root)
        self.rejects("different source workspace")
        self.metadata["workspace_root"] = original
        self.package(guard.BRIDGE)["manifest_path"] = str(self.root / "fake/Cargo.toml")
        self.rejects("checked source manifest")

    def test_missing_resolved_graph_or_node(self):
        original = self.metadata["resolve"]
        self.metadata["resolve"] = None
        self.rejects("no resolved graph")
        self.metadata["resolve"] = original
        original["nodes"].remove(self.node(guard.SERVER))
        self.rejects("missing resolution node")

    def test_target_gated_or_renamed_dependency(self):
        dep = self.package(guard.BRIDGE)["dependencies"][0]
        for key, value in (("target", "cfg(windows)"), ("rename", "fake"), ("path", "/copy")):
            with self.subTest(key=key):
                dep[key] = value
                self.rejects("path-replaced, or target-gated")
                dep[key] = None

    def test_source_wrong_rev_path_profile_default_and_lock_rejected(self):
        cases = [
            (f"crates/{guard.BRIDGE}/Cargo.toml", guard.TIFORTH_REV, "0" * 40, "real pinned Git"),
            (f"crates/{guard.BRIDGE}/Cargo.toml", f'git = "{guard.TIFORTH_GIT}"', 'path = "../copy"', "real pinned Git"),
            (f"crates/{guard.BRIDGE}/Cargo.toml", 'features = ["profiling"]', 'features = []', "with profiling"),
            (f"crates/{guard.SERVER}/Cargo.toml", 'default = ["jemalloc"]', 'default = []', "default must remain"),
            ("Cargo.lock", 'name = "tikv-jemallocator"\nversion = "0.7.0"',
             'name = "tikv-jemallocator"\nversion = "0.6.0"', "locked tikv-jemallocator"),
            ("Cargo.lock", guard.LOCKED["tikv-jemallocator"][2], "0" * 64, "checksum changed"),
        ]
        for rel, old, new, message in cases:
            with self.subTest(corruption=message):
                original = (self.workspace / rel).read_bytes()
                self.edit(rel, old, new)
                with self.assertRaisesRegex(guard.IdentityError, message):
                    guard.check_source(self.workspace)
                (self.workspace / rel).write_bytes(original)

    def test_cli_source_only_is_explicit_and_bad_json_fails(self):
        command = [sys.executable, str(GUARD_PATH), "--workspace", str(self.workspace)]
        result = subprocess.run(command + ["--source-only"], capture_output=True, text=True)
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertFalse(json.loads(result.stdout)["metadata_checked"])
        bad = self.root / "bad.json"
        bad.write_text("{not JSON")
        result = subprocess.run(command + ["--metadata", str(bad)], capture_output=True, text=True)
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("identity rejected", result.stderr)
        self.assertNotIn("Traceback", result.stderr)
        bad.write_text("[]")
        result = subprocess.run(command + ["--metadata", str(bad)], capture_output=True, text=True)
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("must be a JSON object", result.stderr)
        self.assertNotIn("Traceback", result.stderr)

    def shell(self, body, mock):
        binary = self.root / "bin"
        binary.mkdir(exist_ok=True)
        cargo = binary / "cargo"
        cargo.write_text('#!/bin/bash\necho "$*" >> "$MOCK_CALLS"\n' + mock + "\n")
        cargo.chmod(0o755)
        scripts = self.workspace / "scripts"
        scripts.mkdir(exist_ok=True)
        shutil.copyfile(GUARD_PATH, scripts / GUARD_PATH.name)
        logs = self.root / "logs"
        logs.mkdir(exist_ok=True)
        env = dict(os.environ, PATH=str(binary) + os.pathsep + os.environ["PATH"],
                   NATIVE_MEMORY_LOGS=str(logs), MOCK_CALLS=str(self.root / "calls"),
                   PYTHONDONTWRITEBYTECODE="1")
        return subprocess.run(["bash", "--noprofile", "--norc", "-e", "-o", "pipefail", "-c", body],
                              cwd=self.workspace, env=env, capture_output=True, text=True)

    def test_actual_workflow_cargo_failures_survive_successful_tee(self):
        blocks = run_blocks()
        names = ["Check locked native dependency identities", "Check bridge all targets and features",
                 "Check scoped formatting", "Check strict bridge Clippy",
                 "Run complete bridge tests without retries", "Run public native owned root example",
                 "Run unchanged native Chunk suite without retries", "Build required default Jemalloc server"]
        for name in names:
            with self.subTest(step=name):
                result = self.shell(blocks[name], 'echo "mock Cargo failed" >&2\nexit 23')
                self.assertEqual(result.returncode, 23, result.stdout + result.stderr)
        self.assertIn("mock Cargo failed", (self.root / "logs/server-build.log").read_text())

    def test_actual_workflow_doc_test_failure_is_not_masked(self):
        result = self.shell(run_blocks()["Run complete bridge tests without retries"],
                            'case " $* " in *" test "*) exit 24;; *) exit 0;; esac')
        self.assertEqual(result.returncode, 24, result.stdout + result.stderr)

    def test_actual_workflow_real_guard_rejection_fails_after_metadata_success(self):
        self.package("tiforth-memory")["source"] = None
        metadata_path = self.root / "synthetic.json"
        metadata_path.write_text(json.dumps(self.metadata))
        # Paths come from TemporaryDirectory, not user input or a shell command.
        result = self.shell(run_blocks()["Check locked native dependency identities"],
                            'cat "' + str(metadata_path) + '"\nexit 0')
        self.assertNotEqual(result.returncode, 0, result.stdout + result.stderr)
        self.assertIn("identity rejected", (self.root / "logs/identity.log").read_text())


if __name__ == "__main__":
    unittest.main()
