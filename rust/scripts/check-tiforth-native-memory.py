#!/usr/bin/env python3
"""Check the narrow native bridge/allocator identity, not admission or all hosts.

Feed fresh `cargo metadata --locked --format-version 1` from the same workspace.
No Cargo command, network request, or source write is performed by this script.
"""

import argparse
import json
from pathlib import Path
import sys

try:
    import tomllib
except ImportError:  # Python 3.9/3.10 may use an existing tomli installation.
    import tomli as tomllib


TIFORTH_REV = "9628339f697252d29eea885f2eac693f2673935d"
TIFORTH_GIT = "https://github.com/zanmato1984/tiforth"
TIFORTH_DECLARATION = f"git+{TIFORTH_GIT}?rev={TIFORTH_REV}"
TIFORTH_SOURCE = f"{TIFORTH_DECLARATION}#{TIFORTH_REV}"
REGISTRY = "registry+https://github.com/rust-lang/crates.io-index"
LOCKED = {
    "tiforth-memory": ("0.1.0", TIFORTH_SOURCE, None),
    "tikv-jemallocator": (
        "0.7.0", REGISTRY,
        "249f09e49ab1609436f34c776e84231bead18d6a955f119f939bdc1d847561bd",
    ),
    "tikv-jemalloc-sys": (
        "0.7.1+5.3.1-0-g81034ce1f1373e37dc865038e1bc8eeecf559ce8", REGISTRY,
        "1a2825c78386b4ae0314074867860ba9577875de945f05992c38815cbec327f0",
    ),
}
SERVER_JEMALLOC = [
    "dep:tikv-jemallocator", "dep:tikv-jemalloc-sys",
    "tidb-build-tags/jemalloc", "tikv-jemallocator/profiling",
]
BRIDGE = "tidb-tiforth-memory"
SERVER = "tidb-server"


class IdentityError(ValueError):
    """A required native dependency identity is absent or changed."""


def require(condition, message):
    if not condition:
        raise IdentityError(message)


def read_toml(path):
    with path.open("rb") as stream:
        return tomllib.load(stream)


def one(items, predicate, label):
    matches = [item for item in items if predicate(item)]
    require(len(matches) == 1, f"expected exactly one {label}, found {len(matches)}")
    return matches[0]


def check_source(workspace):
    """Validate source declarations and exact relevant lock entries only."""
    workspace = Path(workspace).resolve()
    manifest = read_toml(workspace / "Cargo.toml")
    require(f"crates/{BRIDGE}" in manifest["workspace"]["members"],
            "native bridge is not an explicit workspace member")
    bridge = read_toml(workspace / "crates" / BRIDGE / "Cargo.toml")
    server = read_toml(workspace / "crates" / SERVER / "Cargo.toml")
    require(bridge["package"]["name"] == BRIDGE, "wrong bridge package name")
    require(server["package"]["name"] == SERVER, "wrong server package name")
    deps = bridge["dependencies"]
    require(deps["tiforth-memory"] == {"git": TIFORTH_GIT, "rev": TIFORTH_REV},
            "bridge must declare the real pinned Git tiforth-memory, not a path copy")
    require(deps["tikv-jemallocator"] == {"version": "0.7", "features": ["profiling"]},
            "bridge must select Jemalloc 0.7 with profiling")
    require(deps["tikv-jemalloc-sys"] == "0.7", "bridge must select Jemalloc sys 0.7")
    for name in ("tikv-jemallocator", "tikv-jemalloc-sys"):
        require(server["dependencies"][name] == {"version": "0.7", "optional": True},
                f"server {name} optional dependency changed")
    require(server["features"]["default"] == ["jemalloc"],
            "server default must remain exactly jemalloc")
    require(server["features"]["jemalloc"] == SERVER_JEMALLOC,
            "server jemalloc feature declaration changed (including profiling)")
    lock = read_toml(workspace / "Cargo.lock")
    for name, (version, source, checksum) in LOCKED.items():
        entry = one(lock["package"], lambda p: p["name"] == name, f"locked {name}")
        require(entry["version"] == version and entry.get("source") == source,
                f"locked {name} version/source differs from native identity")
        if checksum is not None:
            require(entry.get("checksum") == checksum, f"locked {name} checksum changed")
    return {"workspace": workspace, "bridge": bridge, "server": server}


def check_metadata(metadata, source):
    """Check actual resolved native edges without closing the whole workspace."""
    workspace = source["workspace"]
    require(isinstance(metadata, dict), "metadata document must be a JSON object")
    require(metadata.get("version") == 1, "expected cargo metadata format version 1")
    require(Path(metadata["workspace_root"]).resolve() == workspace,
            "metadata belongs to a different source workspace")
    packages = metadata["packages"]
    by_id = {p["id"]: p for p in packages}
    require(len(by_id) == len(packages), "metadata contains duplicate package IDs")
    resolve = metadata.get("resolve")
    require(isinstance(resolve, dict), "metadata has no resolved graph; do not use --no-deps")
    nodes = resolve["nodes"]
    by_node = {n["id"]: n for n in nodes}
    require(len(by_node) == len(nodes), "metadata contains duplicate resolution nodes")

    def node(package):
        require(package["id"] in by_node, f"missing resolution node for {package['name']}")
        return by_node[package["id"]]

    def local_package(name):
        package = one(packages, lambda p: p["name"] == name, name)
        require(package.get("source") is None, f"{name} must be a source workspace package")
        require(package["id"] in metadata["workspace_members"], f"{name} is not a workspace member")
        expected = workspace / "crates" / name / "Cargo.toml"
        require(Path(package["manifest_path"]).resolve() == expected,
                f"{name} metadata does not belong to the checked source manifest")
        return package

    def declaration(owner, name, optional, declared_source, features):
        dep = one(owner["dependencies"],
                  lambda d: d["name"] == name and d.get("kind") is None,
                  f"{owner['name']} normal declaration of {name}")
        require(dep.get("source") == declared_source and dep.get("optional") is optional,
                f"{owner['name']} {name} dependency source/optional identity changed")
        require(not dep.get("path") and dep.get("rename") is None and dep.get("target") is None,
                f"{owner['name']} {name} must not be renamed, path-replaced, or target-gated")
        require(dep.get("uses_default_features") is True and dep.get("features") == features,
                f"{owner['name']} {name} declared features changed")
        if name.startswith("tikv-jemalloc"):
            require(dep["req"] == "^0.7", f"{owner['name']} {name} version requirement changed")

    def selected(owner, name):
        edge = one(node(owner)["deps"], lambda d: d["name"].replace("_", "-") == name,
                   f"{owner['name']} resolved edge to {name}")
        require(any(k.get("kind") is None and k.get("target") is None
                    for k in edge["dep_kinds"]), f"{name} is not an unconditional normal dependency")
        require(edge["pkg"] in by_id, f"missing selected package for {name}")
        package = by_id[edge["pkg"]]
        version, origin, _checksum = LOCKED[name]
        require(package["name"] == name and package["version"] == version
                and package.get("source") == origin,
                f"{owner['name']} resolved {name} is not the exact locked native identity")
        return package

    bridge, server = local_package(BRIDGE), local_package(SERVER)
    declaration(bridge, "tiforth-memory", False, TIFORTH_DECLARATION, [])
    declaration(bridge, "tikv-jemallocator", False, REGISTRY, ["profiling"])
    declaration(bridge, "tikv-jemalloc-sys", False, REGISTRY, [])
    tiforth = selected(bridge, "tiforth-memory")
    allocator = selected(bridge, "tikv-jemallocator")
    sys_package = selected(bridge, "tikv-jemalloc-sys")
    require(selected(allocator, "tikv-jemalloc-sys")["id"] == sys_package["id"],
            "bridge allocator and direct sys dependency resolve differently")
    for package in (allocator, sys_package):
        require("profiling" in node(package)["features"],
                f"{package['name']} resolved profiling is not active")
    for name, expected in (("tikv-jemallocator", allocator), ("tikv-jemalloc-sys", sys_package)):
        declaration(server, name, True, REGISTRY, [])
        require(selected(server, name)["id"] == expected["id"],
                f"server and bridge select different {name} packages")
    require(server["features"].get("default") == ["jemalloc"]
            and server["features"].get("jemalloc") == SERVER_JEMALLOC,
            "server metadata disagrees with unchanged default Jemalloc declarations")
    require({"default", "jemalloc"}.issubset(node(server)["features"]),
            "server default Jemalloc feature is not active in metadata")
    return {"tiforth_source": tiforth["source"],
            "allocator_version": allocator["version"], "sys_version": sys_package["version"],
            "profiling": True, "server_default": ["jemalloc"]}


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--workspace", type=Path, required=True, help="Rust workspace directory")
    mode = parser.add_mutually_exclusive_group(required=True)
    mode.add_argument("--metadata", type=Path, help="fresh locked cargo metadata format-1 JSON")
    mode.add_argument("--source-only", action="store_true", help="declarations/lock only, NOT resolved metadata")
    args = parser.parse_args(argv)
    try:
        source = check_source(args.workspace)
        result = {"scope": "native dependency identity only; not memory admission",
                  "metadata_checked": args.metadata is not None}
        if args.metadata is not None:
            result.update(check_metadata(json.loads(args.metadata.read_text()), source))
        else:
            result["mode"] = "source declarations/lock only; resolution NOT checked"
        print(json.dumps(result, sort_keys=True))
        return 0
    except (IdentityError, OSError, ValueError, KeyError, TypeError) as error:
        print(f"native-memory identity rejected: {error}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    sys.exit(main())
