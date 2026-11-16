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
import shlex
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


class WorkflowEnvironmentTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory(prefix="native-memory-runtime-")
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)
        self.runner_temp = self.root / "runner temp"
        self.runner_temp.mkdir()
        self.github_env = self.root / "github env"
        self.github_env.touch()
        binary = self.root / "bin"
        binary.mkdir()
        # Synthetic checkout identity: this shell probe needs no Git repository.
        git = binary / "git"
        git.write_text('#!/bin/bash\ntest "$*" = "rev-parse HEAD" || exit 97\n'
                       'printf "%s\\n" "' + "1" * 40 + '"\n')
        git.chmod(0o755)
        self.env = dict(os.environ, RUNNER_TEMP=str(self.runner_temp),
                        GITHUB_ENV=str(self.github_env),
                        PATH=str(binary) + os.pathsep + os.environ["PATH"],
                        PYTHONDONTWRITEBYTECODE="1")
        self.expected = {
            "CARGO_HOME": str(self.runner_temp / "tiforth-native-memory-cargo"),
            "CARGO_TARGET_DIR": str(self.runner_temp / "tiforth-native-memory-target"),
            "NATIVE_MEMORY_LOGS": str(self.runner_temp / "tiforth-native-memory-logs"),
        }
        for key in self.expected:
            self.env.pop(key, None)

    def shell(self, body, env):
        return subprocess.run(["bash", "--noprofile", "--norc", "-e", "-o", "pipefail", "-c", body],
                              cwd=self.root, env=env, capture_output=True, text=True)

    def test_actual_setup_exports_immediately_and_persists_for_next_step(self):
        # A child process sees exports, not merely unexported shell variables.
        probe = "import json,os;print(json.dumps({k:os.environ[k] for k in " + repr(list(self.expected)) + "}))"
        command = shlex.quote(sys.executable) + " -B -c " + shlex.quote(probe)
        result = self.shell(run_blocks()["Prepare diagnostics"] + command, self.env)
        self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
        self.assertEqual(json.loads(result.stdout.splitlines()[-1]), self.expected)
        lines = self.github_env.read_text().splitlines()
        self.assertEqual(len(lines), len(self.expected))
        persisted = dict(line.split("=", 1) for line in lines)
        self.assertEqual(persisted, self.expected)
        # The runner loads GITHUB_ENV records, without shell expansion, next step.
        result = self.shell(command, dict(self.env, **persisted))
        self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
        self.assertEqual(json.loads(result.stdout), self.expected)
        for path in self.expected.values():
            self.assertTrue(Path(path).is_dir())
            self.assertEqual(Path(path).parent, self.runner_temp)
        logs = Path(self.expected["NATIVE_MEMORY_LOGS"])
        self.assertIn("Filesystem", (logs / "disk-before.log").read_text())
        self.assertEqual((logs / "source-head.log").read_text(), "1" * 40 + "\n")
        self.assertEqual(sorted(p.name for p in self.runner_temp.iterdir()),
                         sorted(Path(p).name for p in self.expected.values()))

    def test_actual_setup_missing_runner_temp_fails(self):
        self.env.pop("RUNNER_TEMP")
        result = self.shell(run_blocks()["Prepare diagnostics"], self.env)
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("RUNNER_TEMP", result.stderr)
        self.assertEqual(self.github_env.read_text(), "")
        self.assertEqual(list(self.runner_temp.iterdir()), [])

    def test_actual_setup_environment_persistence_failure_is_not_masked(self):
        self.env["GITHUB_ENV"] = str(self.root)  # A directory cannot receive records.
        result = self.shell(run_blocks()["Prepare diagnostics"], self.env)
        self.assertNotEqual(result.returncode, 0, result.stdout + result.stderr)
        self.assertEqual(list(self.runner_temp.iterdir()), [])


class WorkflowContextTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory(prefix="native-memory-context-")
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)
        self.output = self.root / "output"
        self.output.touch()
        self.host_sha = "a" * 40
        self.env = {"PATH": os.environ["PATH"], "GITHUB_OUTPUT": str(self.output),
                    "CALLER_REPOSITORY": "zanmato1984/tidb", "CALLER_EVENT": "push",
                    "CALLER_REF": "refs/heads/task/native",
                    "CALLER_WORKFLOW_REF": "zanmato1984/tidb/.github/workflows/tiforth-native-memory.yml@refs/heads/task/native",
                    "EVENT_HEAD_REPOSITORY": "zanmato1984/tidb",
                    "EVENT_SOURCE_SHA": self.host_sha, "REQUESTED_HOST_SOURCE_SHA": ""}

    def shell(self, name, env=None, body=None):
        return subprocess.run(["bash", "--noprofile", "--norc", "-c", body or run_blocks()[name]],
                              cwd=self.root, env=env or self.env, capture_output=True, text=True)

    def test_actual_own_host_events_select_only_event_source(self):
        for event in ("push", "pull_request", "workflow_dispatch"):
            with self.subTest(event=event):
                self.output.write_text("")
                result = self.shell("Validate caller context and exact host source",
                                    dict(self.env, CALLER_EVENT=event))
                self.assertEqual(result.returncode, 0, result.stderr)
                self.assertEqual(self.output.read_text(), "source_sha=" + self.host_sha + "\n")

    def test_actual_private_caller_selects_explicit_host_not_caller_sha(self):
        env = dict(self.env, CALLER_REPOSITORY="zanmato1984/tiforth", CALLER_EVENT="workflow_dispatch",
                   EVENT_HEAD_REPOSITORY="zanmato1984/tiforth", EVENT_SOURCE_SHA="b" * 40,
                   REQUESTED_HOST_SOURCE_SHA=self.host_sha,
                   CALLER_WORKFLOW_REF="zanmato1984/tiforth/.github/workflows/host-native-memory.yml@refs/heads/task/native")
        result = self.shell("Validate caller context and exact host source", env)
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(self.output.read_text(), "source_sha=" + self.host_sha + "\n")
        for changes in ({"CALLER_EVENT": "push"}, {"CALLER_REF": "refs/tags/tag"},
                        {"EVENT_HEAD_REPOSITORY": "foreign/tiforth"},
                        {"CALLER_WORKFLOW_REF": "zanmato1984/tiforth/.github/workflows/other.yml@refs/heads/task/native"},
                        {"REQUESTED_HOST_SOURCE_SHA": ""}, {"REQUESTED_HOST_SOURCE_SHA": "0" * 40}):
            with self.subTest(changes=changes):
                self.output.write_text("")
                result = self.shell("Validate caller context and exact host source", dict(env, **changes))
                self.assertNotEqual(result.returncode, 0)
                self.assertEqual(self.output.read_text(), "")

    def test_actual_private_own_repository_pull_request_launch_context(self):
        env = dict(self.env, CALLER_REPOSITORY="zanmato1984/tiforth", CALLER_EVENT="pull_request",
                   CALLER_EVENT_ACTION="opened", CALLER_REF="refs/pull/37/merge",
                   EVENT_HEAD_REPOSITORY="zanmato1984/tiforth", EVENT_SOURCE_SHA="b" * 40,
                   REQUESTED_HOST_SOURCE_SHA=self.host_sha,
                   CALLER_WORKFLOW_REF="zanmato1984/tiforth/.github/workflows/host-native-memory.yml@refs/pull/37/merge")
        for action in ("opened", "synchronize", "reopened"):
            self.output.write_text("")
            result = self.shell("Validate caller context and exact host source", dict(env, CALLER_EVENT_ACTION=action))
            self.assertEqual(result.returncode, 0, result.stderr)
            self.assertEqual(self.output.read_text(), "source_sha=" + self.host_sha + "\n")
        for changes in ({"EVENT_HEAD_REPOSITORY": "foreign/tiforth"}, {"EVENT_HEAD_REPOSITORY": ""},
                        {"CALLER_REPOSITORY": "foreign/tiforth"}, {"CALLER_EVENT": "pull_request_target"},
                        {"CALLER_EVENT_ACTION": "closed"}, {"CALLER_EVENT_ACTION": "labeled"},
                        {"CALLER_REF": "refs/heads/main"}, {"CALLER_REF": "refs/pull/x/merge"},
                        {"CALLER_REF": "refs/pull/38/merge"}, {"REQUESTED_HOST_SOURCE_SHA": "0" * 40},
                        {"REQUESTED_HOST_SOURCE_SHA": "a" * 39},
                        {"CALLER_WORKFLOW_REF": "zanmato1984/tiforth/.github/workflows/other.yml@refs/pull/37/merge"}):
            with self.subTest(changes=changes):
                self.output.write_text("")
                result = self.shell("Validate caller context and exact host source", dict(env, **changes))
                self.assertNotEqual(result.returncode, 0)
                self.assertEqual(self.output.read_text(), "")

    def test_actual_context_invalid_and_mismatched_sources_reject_before_checkout(self):
        for changes in ({"CALLER_REPOSITORY": "attacker/tidb"}, {"CALLER_EVENT": "pull_request_target"},
                        {"EVENT_HEAD_REPOSITORY": "attacker/tidb"},
                        {"CALLER_REF": "refs/heads/other"},
                        {"CALLER_WORKFLOW_REF": "zanmato1984/tidb/other.yml@refs/heads/task/native"},
                        {"REQUESTED_HOST_SOURCE_SHA": "b" * 40},
                        {"EVENT_SOURCE_SHA": "0" * 40}, {"EVENT_SOURCE_SHA": "a" * 39},
                        {"EVENT_SOURCE_SHA": "A" * 40}, {"EVENT_SOURCE_SHA": "$(false)"}):
            with self.subTest(changes=changes):
                self.output.write_text("")
                result = self.shell("Validate caller context and exact host source", dict(self.env, **changes))
                self.assertNotEqual(result.returncode, 0)
                self.assertEqual(self.output.read_text(), "")

    def test_actual_checkout_rejects_wrong_caller_source_and_git_failure(self):
        binary = self.root / "bin"
        binary.mkdir()
        git = binary / "git"
        env = dict(self.env, PATH=str(binary) + os.pathsep + self.env["PATH"],
                   EXPECTED_HOST_SOURCE_SHA=self.host_sha)
        for content, expected in (("printf '%s\\n' '" + self.host_sha + "'", 0),
                                  ("printf '%s\\n' '" + "b" * 40 + "'", 1), ("exit 23", 23)):
            git.write_text("#!/bin/bash\n" + content + "\n")
            git.chmod(0o755)
            result = self.shell("Verify exact host checkout", env)
            self.assertEqual(result.returncode, expected, result.stderr)

    def test_failure_mask_mutant_is_detectable(self):
        body = run_blocks()["Check bridge all targets and features"]
        binary = self.root / "bin"
        binary.mkdir()
        cargo = binary / "cargo"
        cargo.write_text("#!/bin/bash\nexit 23\n")
        cargo.chmod(0o755)
        env = dict(self.env, PATH=str(binary) + os.pathsep + self.env["PATH"], NATIVE_MEMORY_LOGS=str(self.root))
        self.assertEqual(self.shell("", env, body).returncode, 23)
        mutant = body.replace("set -euo pipefail", "set -eu\nset +o pipefail")
        self.assertEqual(self.shell("", env, mutant).returncode, 0)


class CredentialProtocolTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory(prefix="native memory ;$` ")
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)
        self.helper = SCRIPTS / "tiforth-dependency-credential.py"
        self.token = "FAKE_NATIVE_CREDENTIAL_SENTINEL_9086"
        self.env = {"PATH": os.environ["PATH"], "HOME": str(self.root),
                    # Keep the already-installed TOML parser available under
                    # isolated HOME on Python 3.9/3.10; install nothing here.
                    "PYTHONPATH": str(Path(guard.tomllib.__file__).resolve().parent.parent),
                    "XDG_CONFIG_HOME": str(self.root / "config"), "GIT_CONFIG_NOSYSTEM": "1",
                    "GIT_CONFIG_GLOBAL": str(self.root / "global-config"), "GIT_TERMINAL_PROMPT": "0",
                    "PYTHONDONTWRITEBYTECODE": "1", "TIFORTH_DEPENDENCY_TOKEN": self.token,
                    "GIT_CONFIG_COUNT": "3", "GIT_CONFIG_KEY_0": "credential.helper", "GIT_CONFIG_VALUE_0": "",
                    "GIT_CONFIG_KEY_1": "credential.useHttpPath", "GIT_CONFIG_VALUE_1": "true",
                    "GIT_CONFIG_KEY_2": "credential.helper",
                    "GIT_CONFIG_VALUE_2": "!" + shlex.quote(sys.executable) + " -B " + shlex.quote(str(self.helper))}

    def request(self, protocol="https", host="github.com", path="zanmato1984/tiforth"):
        return f"protocol={protocol}\nhost={host}\npath={path}\n\n"

    def invoke(self, request=None, action="get", env=None, git=False):
        command = ["git", "credential", "fill"] if git else [sys.executable, "-B", str(self.helper), action]
        return subprocess.run(command, input=request or self.request(), cwd=self.root,
                              env=self.env if env is None else env, capture_output=True, text=True)

    def assert_secret(self, result, token=None):
        token = self.token if token is None else token
        self.assertEqual(result.returncode, 0, "credential protocol failed; streams withheld")
        self.assertTrue("password=" + token + "\n" in result.stdout, "credential mismatch; streams withheld")
        self.assertTrue("username=x-access-token\n" in result.stdout)
        self.assertFalse(token in result.stderr, "credential leaked to stderr")

    def assert_rejected(self, result):
        self.assertNotEqual(result.returncode, 0, "unexpected credential acceptance")
        self.assertFalse(self.token in result.stdout + result.stderr, "credential leaked during rejection")
        self.assertFalse("password=" in result.stdout, "unexpected credential output")

    def test_actual_helper_and_git_fill_accept_exact_target_variants(self):
        for git in (False, True):
            for path in ("zanmato1984/tiforth", "zanmato1984/tiforth.git"):
                with self.subTest(git=git, path=path):
                    self.assert_secret(self.invoke(self.request(path=path), git=git))
        self.assertFalse((self.root / "global-config").exists())

    def test_actual_helper_and_git_fill_reject_other_targets(self):
        for protocol, host, path in (("http", "github.com", "zanmato1984/tiforth"),
                                    ("ssh", "github.com", "zanmato1984/tiforth"),
                                    ("https", "github.com.evil", "zanmato1984/tiforth"),
                                    ("https", "github.com:443", "zanmato1984/tiforth"),
                                    ("https", "github.com", "zanmato1984/tiforth-extra"),
                                    ("https", "github.com", "other/tiforth"),
                                    ("https", "github.com", "zanmato1984/tiforth.git/extra"),
                                    ("https", "github.com", "zanmato1984/tiforth;$(false)")):
            for git in (False, True):
                with self.subTest(git=git, protocol=protocol, host=host, path=path):
                    self.assert_rejected(self.invoke(self.request(protocol, host, path), git=git))

    def test_actual_helper_rejects_non_get_missing_fields_duplicate_and_malformed(self):
        for action in ("store", "erase", "", "get;false"):
            self.assert_rejected(self.invoke(action=action))
        for request in ("protocol=https\nhost=github.com\n\n", "host=github.com\npath=zanmato1984/tiforth\n\n",
                        "protocol=https\npath=zanmato1984/tiforth\n\n",
                        self.request().replace("\n\n", "\nhost=github.com\n\n"),
                        self.request().replace("https\n", "https\r\n"), "invalid\n\n",
                        self.request().rstrip("\n"), self.request().replace("\n\n", "\nunknown=value\n\n")):
            self.assert_rejected(self.invoke(request))

    def test_actual_helper_and_git_fill_missing_empty_and_multiline_token_reject(self):
        for token in (None, "", "bad\nvalue", "bad\rvalue"):
            env = dict(self.env)
            if token is None:
                env.pop("TIFORTH_DEPENDENCY_TOKEN")
            else:
                env["TIFORTH_DEPENDENCY_TOKEN"] = token
            for git in (False, True):
                with self.subTest(git=git, token_kind="missing" if token is None else "malformed"):
                    self.assert_rejected(self.invoke(env=env, git=git))

    def test_git_fill_missing_helper_control_fails_without_network(self):
        env = dict(self.env, GIT_CONFIG_COUNT="2")
        self.assert_rejected(self.invoke(env=env, git=True))

    def test_protocol_keeps_shell_metacharacters_and_spaces_literal(self):
        for token in (self.token + " $(touch SHOULD_NOT_EXIST);`false`'\\\" ", self.token + " with spaces\tend"):
            for git in (False, True):
                result = self.invoke(env=dict(self.env, TIFORTH_DEPENDENCY_TOKEN=token), git=git)
                self.assert_secret(result, token)
        self.assertFalse((self.root / "SHOULD_NOT_EXIST").exists())

    def test_actual_metadata_step_uses_only_ephemeral_config_and_no_secret_diagnostics(self):
        workspace = self.root / "rust"
        for rel in ("Cargo.toml", "Cargo.lock", f"crates/{guard.BRIDGE}/Cargo.toml", f"crates/{guard.SERVER}/Cargo.toml"):
            target = workspace / rel
            target.parent.mkdir(parents=True, exist_ok=True)
            shutil.copyfile(SOURCE / rel, target)
        scripts = workspace / "scripts"
        scripts.mkdir()
        shutil.copyfile(GUARD_PATH, scripts / GUARD_PATH.name)
        shutil.copyfile(self.helper, scripts / self.helper.name)
        binary = self.root / "bin"
        binary.mkdir()
        cargo = binary / "cargo"
        # The mock emits synthetic metadata only after real Git credential IPC.
        probe = '''import json,os,subprocess,sys
assert os.environ["CARGO_NET_GIT_FETCH_WITH_CLI"] == "true"
assert os.environ["GIT_CONFIG_COUNT"] == "3"
assert os.environ["GIT_CONFIG_VALUE_0"] == ""
assert os.environ["GIT_CONFIG_VALUE_1"] == "true"
r = subprocess.run(["git", "credential", "fill"], input="protocol=https\\nhost=github.com\\npath=zanmato1984/tiforth\\n\\n", capture_output=True, text=True)
token = os.environ["TIFORTH_DEPENDENCY_TOKEN"]
assert r.returncode == 0, "Git credential IPC failed; streams withheld"
assert "password=" + token + "\\n" in r.stdout, "Git credential mismatch; streams withheld"
assert token not in r.stderr, "credential leaked"
print(json.dumps(METADATA))
'''.replace("METADATA", repr(synthetic_metadata(workspace)))
        cargo.write_text("#!" + sys.executable + "\n" + probe)
        cargo.chmod(0o755)
        logs = self.root / "logs"
        logs.mkdir()
        github_env = self.root / "github-env"
        github_env.touch()
        env = dict(self.env, PATH=str(binary) + os.pathsep + self.env["PATH"],
                   NATIVE_MEMORY_LOGS=str(logs), GITHUB_ENV=str(github_env))
        # Only the actual workflow block sets credential configuration.
        for key in list(env):
            if key.startswith("GIT_CONFIG_KEY_") or key.startswith("GIT_CONFIG_VALUE_") or key == "GIT_CONFIG_COUNT":
                env.pop(key)
        result = subprocess.run(["bash", "--noprofile", "--norc", "-c", run_blocks()["Check locked native dependency identities"]],
                                cwd=workspace, env=env, capture_output=True, text=True)
        if result.returncode != 0:
            diagnostic = result.stdout + result.stderr + (logs / "metadata.stderr.log").read_text()
            self.fail("metadata shell probe failed: " + diagnostic.replace(self.token, "<redacted>"))
        self.assertFalse(self.token in result.stdout + result.stderr, "credential leaked to shell diagnostics")
        for path in logs.iterdir():
            self.assertFalse(self.token.encode() in path.read_bytes(), "credential leaked to artifact path")
        for path in self.root.rglob("*"):
            if path.is_file():
                self.assertFalse(self.token.encode() in path.read_bytes(), "credential leaked to sandbox file")
        self.assertEqual(github_env.read_text(), "")
        self.assertFalse((self.root / "global-config").exists())
        self.assertEqual(sorted(p.name for p in logs.iterdir()), ["identity.log", "metadata.json", "metadata.stderr.log"])


if __name__ == "__main__":
    unittest.main()
