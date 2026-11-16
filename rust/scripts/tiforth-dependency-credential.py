#!/usr/bin/env python3
"""Ephemeral Git credential protocol for the one private locked dependency.

Stdout belongs only to the requesting Git process, never a diagnostic stream.
No token is read unless action, protocol, host and path have all been accepted.
No files, network requests, Git configuration or credential stores are used.
"""

import os
import sys


def main(argv=None):
    argv = sys.argv[1:] if argv is None else argv
    if argv != ["get"]:
        return 1
    fields = {}
    # Git helpers receive newline-delimited fields, terminated by EOF or blank.
    # Bound malformed requests without ever echoing request fields.
    for _ in range(32):
        line = sys.stdin.buffer.readline(4097)
        if line in (b"", b"\n"):
            break
        if len(line) > 4096 or not line.endswith(b"\n") or b"\r" in line or b"\0" in line:
            return 1
        try:
            key, value = line[:-1].decode("utf-8").split("=", 1)
        except (UnicodeError, ValueError):
            return 1
        if key in fields or key not in {"protocol", "host", "path", "username"}:
            return 1
        fields[key] = value
    else:
        return 1
    if (fields.get("protocol") != "https" or fields.get("host") != "github.com"
            or fields.get("path") not in {"zanmato1984/tiforth", "zanmato1984/tiforth.git"}):
        return 1
    token = os.environ.get("TIFORTH_DEPENDENCY_TOKEN", "")
    if not token or any(c in token for c in "\r\n\0"):
        return 1
    sys.stdout.write("username=x-access-token\npassword=" + token + "\n\n")
    return 0


if __name__ == "__main__":
    sys.exit(main())
