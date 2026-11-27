#!/usr/bin/env bash
# SPDX-License-Identifier: GPL-3.0-only
# Copyright (C) 2026 ftxr-modem Contributors
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT_DIR"

echo "[1/6] Checking canonical GPL-3.0-only LICENSE and crate-local LICENSE/NOTICE parity..."
EXPECTED_GPL3_SHA256="3972dc9744f6499f0f9b2dbf76696f2ae7ad8af9b23dde66d6af86c9dfb36986"
if command -v sha256sum >/dev/null 2>&1; then
  ACTUAL_LICENSE_SHA256="$(sha256sum LICENSE | awk '{print $1}')"
elif command -v shasum >/dev/null 2>&1; then
  ACTUAL_LICENSE_SHA256="$(shasum -a 256 LICENSE | awk '{print $1}')"
else
  echo "error: neither sha256sum nor shasum found" >&2
  exit 1
fi
if [[ "$ACTUAL_LICENSE_SHA256" != "$EXPECTED_GPL3_SHA256" ]]; then
  echo "error: LICENSE SHA-256 ($ACTUAL_LICENSE_SHA256) != canonical FSF gpl-3.0.txt ($EXPECTED_GPL3_SHA256)" >&2
  exit 1
fi

for crate in ftxr-core ftxr-modem; do
  cmp -s LICENSE "crates/${crate}/LICENSE" || {
    echo "error: crates/${crate}/LICENSE differs from root LICENSE" >&2
    exit 1
  }
  cmp -s NOTICE "crates/${crate}/NOTICE" || {
    echo "error: crates/${crate}/NOTICE differs from root NOTICE" >&2
    exit 1
  }
done

echo "[2/6] Verifying SPDX-License-Identifier: GPL-3.0-only source headers..."
missing_spdx=0
while IFS= read -r -d '' rs_file; do
  if ! head -n 5 "$rs_file" | grep -q "SPDX-License-Identifier: GPL-3.0-only"; then
    echo "error: missing GPL-3.0-only SPDX header in $rs_file" >&2
    missing_spdx=1
  fi
done < <(find crates -type f -name '*.rs' -print0 | sort -z)
if [[ "$missing_spdx" -ne 0 ]]; then
  exit 1
fi

echo "[3/6] Scanning crates/ for excluded non-GPL / Numerical Recipes / native C symbols..."
EXCLUSION_REGEX='bessi0|Numerical Recipes|3\.5156229|0\.39894228|wsjtx_config|fftw3|libgfortran|ftxr-oracle|unsafe[[:space:]]*\{|unsafe[[:space:]]+fn|unsafe[[:space:]]+impl'
if command -v rg >/dev/null 2>&1; then
  if rg -n "$EXCLUSION_REGEX" crates/; then
    echo "error: excluded symbol or unsafe code found in crates/" >&2
    exit 1
  fi
else
  if grep -rnE "$EXCLUSION_REGEX" crates/; then
    echo "error: excluded symbol or unsafe code found in crates/" >&2
    exit 1
  fi
fi

echo "[4/6] Checking WSJT-X trademark and attribution hygiene..."
README_TITLE="$(head -n 1 README.md)"
if echo "$README_TITLE" | grep -qiE 'wsjt|map65|qmap'; then
  echo "error: README.md title must not use WSJT-X trademarks: $README_TITLE" >&2
  exit 1
fi
grep -q "neither affiliated with nor endorsed by the WSJT Development Team" NOTICE || {
  echo "error: NOTICE missing WSJT Development Team non-endorsement disclaimer" >&2
  exit 1
}

echo "[5/6] Auditing cargo metadata licenses and pure-Rust dependency tree..."
cargo metadata --format-version 1 --locked | python3 -c '
import json, re, sys

meta = json.load(sys.stdin)
pkgs = {p["id"]: p for p in meta["packages"]}
nodes = {n["id"]: n for n in meta["resolve"]["nodes"]}

ws_pkgs = [p for p in meta["packages"] if p["id"] in meta["workspace_members"]]
assert len(ws_pkgs) == 2, "unexpected workspace members: %r" % ([p["name"] for p in ws_pkgs],)
versions = set()
for p in ws_pkgs:
    assert p.get("license") == "GPL-3.0-only", "%s license=%r" % (p["name"], p.get("license"))
    versions.add(p["version"])
assert len(versions) == 1, "workspace crate version mismatch: %r" % (versions,)

allowed_atoms = {
    "GPL-3.0-only",
    "GPL-3.0-or-later",
    "MIT",
    "Apache-2.0",
    "Apache-2.0 WITH LLVM-exception",
    "BSD-2-Clause",
    "BSD-3-Clause",
    "ISC",
    "Zlib",
    "Unicode-3.0",
    "BSL-1.0",
    "CC0-1.0",
    "Unlicense",
}

def eval_spdx(expr):
    tokens = re.findall(r"\(|\)|\bAND\b|\bOR\b|[^\s()]+(?:\s+WITH\s+[^\s()]+)?", expr)
    pos = [0]

    def parse_or():
        val = parse_and()
        while pos[0] < len(tokens) and tokens[pos[0]] == "OR":
            pos[0] += 1
            rhs = parse_and()
            val = val or rhs
        return val

    def parse_and():
        val = parse_atom()
        while pos[0] < len(tokens) and tokens[pos[0]] == "AND":
            pos[0] += 1
            rhs = parse_atom()
            val = val and rhs
        return val

    def parse_atom():
        tok = tokens[pos[0]]
        if tok == "(":
            pos[0] += 1
            val = parse_or()
            assert tokens[pos[0]] == ")", "unclosed paren in %r" % (expr,)
            pos[0] += 1
            return val
        pos[0] += 1
        return tok in allowed_atoms

    ok = parse_or()
    assert pos[0] == len(tokens), "unexpected trailing SPDX tokens in %r" % (expr,)
    return ok

banned_names = {"cc", "cmake", "bindgen", "pkg-config", "fftw-sys", "gfortran", "ftxr-oracle"}
allowed_sys = {"linux-raw-sys", "windows-sys"}

seen = set()
stack = [p["id"] for p in ws_pkgs]
while stack:
    pid = stack.pop()
    if pid in seen:
        continue
    seen.add(pid)
    for dep in nodes[pid]["dependencies"]:
        stack.append(dep)

for pid in sorted(seen):
    p = pkgs[pid]
    name = p["name"]
    assert name not in banned_names, "banned crate in dependency graph: %s" % (name,)
    if name.endswith("-sys"):
        assert name in allowed_sys, "unapproved -sys crate in dependency graph: %s" % (name,)
    lic = p.get("license")
    assert lic, "missing SPDX license on %s %s" % (name, p["version"])
    assert eval_spdx(lic), "disallowed license %r on %s %s" % (lic, name, p["version"])

print("Verified %d packages across workspace dependency graph (100%% GPL-3.0-only compatible, pure Rust)." % (len(seen),))
'

echo "[6/6] Checking fixture budget (< 1.5 MiB) and cargo package archive contents..."
FIXTURE_BYTES="$(python3 -c '
import os
total = 0
for dirpath, _, filenames in os.walk("tests/fixtures"):
    for fn in filenames:
        total += os.path.getsize(os.path.join(dirpath, fn))
print(total)
')"
if (( FIXTURE_BYTES >= 1572864 )); then
  echo "error: tests/fixtures exceeds 1.5 MiB ($FIXTURE_BYTES bytes)" >&2
  exit 1
fi
echo "Fixture footprint: ${FIXTURE_BYTES} bytes (< 1,572,864 limit)"

for crate in ftxr-core ftxr-modem; do
  pkg_list="$(cargo package --list --locked --allow-dirty -p "$crate")"
  for req in Cargo.toml LICENSE NOTICE README.md; do
    echo "$pkg_list" | grep -qx "$req" || {
      echo "error: cargo package for $crate missing $req" >&2
      exit 1
    }
  done
  if echo "$pkg_list" | grep -qE '\.wav$'; then
    echo "error: cargo package for $crate unexpectedly includes .wav fixtures" >&2
    exit 1
  fi
done

echo "ALL COMPLIANCE CHECKS PASSED."
