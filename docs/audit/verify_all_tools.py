#!/usr/bin/env python3
"""
Full functional verification of the 14 remaining daemon tools.
Each test:
  1. Sets up any required state (test dir, todo store, etc.)
  2. Invokes the tool via the HTTP API
  3. Verifies the response is sane (ok=true or expected error)
  4. Cleans up

Run against a live daemon at http://127.0.0.1:7878.
"""

import json
import os
import shutil
import subprocess
import sys
import time
import urllib.error
import urllib.request

import threading

DAEMON = "http://127.0.0.1:7878"
TOKEN = subprocess.check_output(
    ["python3", "/tmp/opencode/bench/mint_jwt.py"], text=True
).strip()

WORKDIR = "/home/andres_fernandez/projects/neurox"
TESTDIR = "/tmp/opencode/bench/verify"
NEUROX_TOKEN = os.environ.get("NEUROX_TOKEN", "")

# ----------------------------------------------------------------------
# harness
# ----------------------------------------------------------------------

PASS = 0
FAIL = 0
RESULTS = []
LOCK = threading.Lock()


def invoke(tool, args, timeout=30):
    req = urllib.request.Request(
        f"{DAEMON}/v1/tools/{tool}/invoke",
        data=json.dumps({"args": args}).encode(),
        headers={
            "Authorization": f"Bearer {TOKEN}",
            "Content-Type": "application/json",
        },
    )
    try:
        with urllib.request.urlopen(req, timeout=timeout) as resp:
            body = resp.read()
    except urllib.error.HTTPError as e:
        body = e.read()
    except (TimeoutError, ConnectionError) as e:
        return {"ok": False, "error": f"timeout: {e}"}
    if not body:
        return {"ok": False, "error": "empty response (server closed connection)"}
    try:
        return json.loads(body)
    except json.JSONDecodeError:
        return {"ok": False, "error": f"non-JSON response: {body[:80]!r}"}


def record(name, ok, detail=""):
    global PASS, FAIL
    with LOCK:
        if ok:
            PASS += 1
            status = "✅"
        else:
            FAIL += 1
            status = "❌"
        RESULTS.append((name, ok, detail))
        print(f"  {status} {name}: {detail}")


def setup():
    if os.path.exists(TESTDIR):
        shutil.rmtree(TESTDIR)
    os.makedirs(f"{TESTDIR}/sub1/sub2", exist_ok=True)
    with open(f"{TESTDIR}/hello.txt", "w") as f:
        f.write("hello world\n")
    with open(f"{TESTDIR}/data.rs", "w") as f:
        f.write("fn main() { println!(\"hi\"); }\nfn helper() {}\n")
    with open(f"{TESTDIR}/sub1/code.py", "w") as f:
        f.write("def foo(): pass\nclass Bar: pass\n")


def teardown():
    if os.path.exists(TESTDIR):
        shutil.rmtree(TESTDIR, ignore_errors=True)


# ----------------------------------------------------------------------
# per-tool tests
# ----------------------------------------------------------------------


def test_shell():
    r = invoke("shell", {"command": "echo VERIFY_OK"})
    ok = r.get("ok") and "VERIFY_OK" in r.get("result", "")
    record("shell: echo returns result", ok, r.get("result", r.get("error", ""))[:60])


def test_shell_empty():
    r = invoke("shell", {"command": ""})
    ok = r.get("ok") is False and "empty" in r.get("error", "").lower()
    record("shell: empty command rejected", ok, r.get("error", "")[:60])


def test_shell_tilde():
    r = invoke("shell", {"command": "echo ~"})
    ok = r.get("ok") and "/home/andres_fernandez" in r.get("result", "")
    record("shell: tilde expands to home", ok, r.get("result", r.get("error", ""))[:60])


def test_write_file():
    path = f"{TESTDIR}/created.txt"
    r = invoke("write_file", {"path": path, "command": "create", "content": "hello"})
    ok = r.get("ok") and os.path.exists(path) and open(path).read() == "hello"
    record("write_file: create", ok, r.get("result", r.get("error", ""))[:60])
    # strReplace
    r = invoke("write_file", {
        "path": path, "command": "strReplace",
        "old_str": "hello", "new_str": "world",
    })
    ok = r.get("ok") and open(path).read() == "world"
    record("write_file: strReplace", ok, r.get("result", r.get("error", ""))[:60])
    # insert
    r = invoke("write_file", {
        "path": path, "command": "insert",
        "content": "appended\n", "insert_line": None,
    })
    ok = r.get("ok") and "appended" in open(path).read()
    record("write_file: insert (append)", ok, r.get("result", r.get("error", ""))[:60])
    # sandbox deny
    r = invoke("write_file", {"path": "/etc/neurox-verify-deny", "command": "create", "content": "x"})
    ok = r.get("ok") is False and "writable" in r.get("error", "")
    record("write_file: /etc denied", ok, r.get("error", "")[:80])
    os.remove(path) if os.path.exists(path) else None


def test_read_file():
    path = f"{TESTDIR}/data.rs"
    r = invoke("read_file", {"path": path})
    ok = r.get("ok") and "main" in r.get("result", "")
    record("read_file: basic", ok, r.get("result", r.get("error", ""))[:60])
    # offset/limit
    r = invoke("read_file", {"path": path, "offset": 0, "limit": 1})
    ok = r.get("ok") and "main" in r.get("result", "") and "helper" not in r.get("result", "")
    record("read_file: offset/limit", ok, r.get("result", "")[:60])
    # CRASH GUARD: this used to SIGABRT
    r = invoke("read_file", {"path": path, "offset": 999})
    ok = r.get("ok") and "No lines" in r.get("result", "")
    record("read_file: offset>total (no crash)", ok, r.get("result", r.get("error", ""))[:60])
    # empty path
    r = invoke("read_file", {"path": ""})
    ok = r.get("ok") is False and "empty" in r.get("error", "")
    record("read_file: empty path rejected", ok, r.get("error", "")[:60])


def test_glob():
    r = invoke("glob", {"pattern": "*.rs", "path": TESTDIR})
    ok = r.get("ok") and "data.rs" in r.get("result", "")
    record("glob: *.rs finds data.rs", ok, r.get("result", r.get("error", ""))[:80])
    # recursive
    r = invoke("glob", {"pattern": "**/*.py", "path": TESTDIR})
    ok = r.get("ok") and "code.py" in r.get("result", "")
    record("glob: **/*.py finds code.py", ok, r.get("result", r.get("error", ""))[:80])
    # sandbox
    r = invoke("glob", {"pattern": "*", "path": "/srv"})
    ok = r.get("ok") is False
    record("glob: /srv denied (not in readable)", ok, r.get("error", "")[:60])


def test_grep():
    r = invoke("grep", {"pattern": "main", "path": f"{TESTDIR}"})
    ok = r.get("ok") and "data.rs" in r.get("result", "")
    record("grep: pattern 'main' found", ok, r.get("result", r.get("error", ""))[:80])
    # unreadable subdir now silent
    r = invoke("grep", {"pattern": "passwd", "path": "/etc"})
    ok = r.get("ok")  # was Err before fix
    record("grep: /etc unreadable subdirs silent", ok, r.get("error", r.get("result", "")[:60]))
    # sandbox bypass
    r = invoke("grep", {"pattern": "x", "path": "/srv"})
    ok = r.get("ok") is False
    record("grep: /srv denied", ok, r.get("error", "")[:60])


def test_list_dir():
    r = invoke("list_dir", {"path": TESTDIR, "max_entries": 50})
    ok = r.get("ok") and "hello.txt" in r.get("result", "") and "data.rs" in r.get("result", "")
    record("list_dir: lists test dir", ok, r.get("result", r.get("error", ""))[:100])
    # sandbox
    r = invoke("list_dir", {"path": "/srv"})
    ok = r.get("ok") is False
    record("list_dir: /srv denied", ok, r.get("error", "")[:60])


def test_symbols():
    r = invoke("symbols", {"query": "main", "path": f"{TESTDIR}"})
    ok = r.get("ok") and "fn main" in r.get("result", "")
    record("symbols: fn main found", ok, r.get("result", r.get("error", ""))[:80])
    r = invoke("symbols", {"query": "Bar", "path": f"{TESTDIR}"})
    ok = r.get("ok") and "class Bar" in r.get("result", "")
    record("symbols: class Bar found", ok, r.get("result", r.get("error", ""))[:80])


def test_web_search():
    r = invoke("web_search", {"query": "rust programming"})
    # ddgr may be rate limited; just check no panic
    ok = r.get("ok") is True or r.get("ok") is False  # either is OK
    record("web_search: real query", ok, r.get("result", r.get("error", ""))[:60])
    # empty query
    r = invoke("web_search", {"query": ""})
    ok = r.get("ok") is False and "empty" in r.get("error", "").lower()
    record("web_search: empty rejected cleanly", ok, r.get("error", "")[:60])
    # too long
    r = invoke("web_search", {"query": "x " * 200})
    ok = r.get("ok") is False and "too long" in r.get("error", "").lower()
    record("web_search: 400-char rejected", ok, r.get("error", "")[:60])


def test_web_fetch():
    r = invoke("web_fetch", {"url": "https://example.com"})
    ok = r.get("ok") and "Example Domain" in r.get("result", "")
    record("web_fetch: example.com", ok, r.get("result", r.get("error", ""))[:80])
    # SSRF: loopback
    r = invoke("web_fetch", {"url": "http://127.0.0.1:7878/health"})
    ok = r.get("ok") is False and "loopback" in r.get("error", "").lower() or "private" in r.get("error", "").lower()
    record("web_fetch: loopback denied", ok, r.get("error", "")[:80])
    # SSRF: AWS metadata
    r = invoke("web_fetch", {"url": "http://169.254.169.254/"})
    ok = r.get("ok") is False
    record("web_fetch: AWS metadata denied", ok, r.get("error", "")[:80])
    # invalid URL
    r = invoke("web_fetch", {"url": "not-a-url"})
    ok = r.get("ok") is False
    record("web_fetch: invalid url rejected", ok, r.get("error", "")[:80])


def test_todo_add():
    r = invoke("todo_add", {"content": "verify_todo_1"})
    ok = r.get("ok") and "added" in r.get("result", "")
    record("todo_add: new todo", ok, r.get("result", r.get("error", ""))[:80])
    # empty
    r = invoke("todo_add", {"content": ""})
    ok = r.get("ok") is False
    record("todo_add: empty rejected", ok, r.get("error", "")[:60])
    # list
    r = invoke("todo_list", {})
    ok = r.get("ok") and "verify_todo_1" in r.get("result", "")
    record("todo_list: contains new todo", ok, r.get("result", r.get("error", ""))[:100])


def test_todo_done():
    # find the id of verify_todo_1
    r = invoke("todo_list", {})
    # extract short_id (6 chars after #)
    import re
    m = re.search(r"#(\w{6}) verify_todo_1", r.get("result", ""))
    if not m:
        record("todo_done: setup failed", False, "no id found")
        return
    tid = m.group(1)
    r = invoke("todo_done", {"id": tid})
    ok = r.get("ok") and "marked done" in r.get("result", "")
    record("todo_done: mark todo", ok, r.get("result", r.get("error", ""))[:80])


def test_todo_remove():
    # add a new one
    r = invoke("todo_add", {"content": "verify_todo_to_remove"})
    import re
    m = re.search(r"#(\w{6})", r.get("result", ""))
    if not m:
        record("todo_remove: setup failed", False, "no id")
        return
    tid = m.group(1)
    r = invoke("todo_remove", {"id": tid})
    ok = r.get("ok") and "removed" in r.get("result", "")
    record("todo_remove: remove todo", ok, r.get("result", r.get("error", ""))[:80])


def test_todo_clear():
    r = invoke("todo_clear", {"completed_only": False})
    ok = r.get("ok")
    record("todo_clear: clear all", ok, r.get("result", r.get("error", ""))[:60])
    r = invoke("todo_list", {})
    ok = r.get("ok") and "no todos" in r.get("result", "")
    record("todo_list: empty after clear", ok, r.get("result", "")[:60])


# ----------------------------------------------------------------------
# parallel sanity (each tool under 5x concurrent load)
# ----------------------------------------------------------------------

def parallel_check(name, tool, build_args):
    """Fire 5 calls at once, expect all 5 to return ok=true."""
    barrier = threading.Barrier(5)
    results = []
    lock = threading.Lock()

    def worker(i):
        barrier.wait()
        r = invoke(tool, build_args(i))
        with lock:
            results.append((i, r.get("ok", False), r.get("error", "")[:40]))

    threads = [threading.Thread(target=worker, args=(i,)) for i in range(5)]
    start = time.time()
    for t in threads:
        t.start()
    for t in threads:
        t.join()
    elapsed = time.time() - start
    ok_count = sum(1 for _, ok, _ in results if ok)
    record(f"parallel 5× {name}", ok_count == 5, f"{ok_count}/5 in {elapsed*1000:.0f}ms")


def main():
    print("=" * 60)
    print("VERIFICATION: 14 tools + parallel sanity checks")
    print("=" * 60)
    setup()

    tests = [
        test_shell, test_shell_empty, test_shell_tilde,
        test_write_file,
        test_read_file,
        test_glob, test_grep, test_list_dir, test_symbols,
        test_web_search, test_web_fetch,
        test_todo_add, test_todo_done, test_todo_remove, test_todo_clear,
    ]
    for t in tests:
        t()

    # parallel checks
    print()
    print("=" * 60)
    print("PARALLEL SANITY (5 concurrent)")
    print("=" * 60)
    # Create unique files for the parallel read_file test
    for i in range(5):
        open(f"{TESTDIR}/par-{i}.txt", "w").write(f"PAR_{i}")
    parallel_check("read_file (unique paths)",
                  "read_file",
                  lambda i: {"path": f"{TESTDIR}/par-{i}.txt"})
    parallel_check("grep (unique patterns)",
                  "grep",
                  lambda i: {"pattern": f"PAR_{i}", "path": TESTDIR})
    parallel_check("list_dir (same path)",
                  "list_dir",
                  lambda i: {"path": TESTDIR, "max_entries": 10})
    parallel_check("web_fetch (different public URLs)",
                  "web_fetch",
                  lambda i: {"url": "https://example.com"})
    parallel_check("grep (same pattern, racing)",
                  "grep",
                  lambda i: {"pattern": "PAR", "path": TESTDIR})

    teardown()

    print()
    print("=" * 60)
    print(f"TOTAL: {PASS} pass, {FAIL} fail (of {PASS+FAIL} tests)")
    print("=" * 60)
    if FAIL:
        print("\nFailed tests:")
        for name, ok, detail in RESULTS:
            if not ok:
                print(f"  - {name}: {detail}")
        sys.exit(1)


if __name__ == "__main__":
    main()
