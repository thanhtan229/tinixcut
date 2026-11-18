"""Smoke-check the native CLI operation registry and persistent JSONL session."""
import json
import os
import queue
import subprocess
import tempfile
import threading
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
BIN = Path(os.environ["TINIXCUT_TEST_BINARY"]).resolve() if os.environ.get("TINIXCUT_TEST_BINARY") else ROOT / "src-tauri/target/debug/tinixcut.exe"
if not BIN.exists() and not os.environ.get("TINIXCUT_TEST_BINARY"):
    BIN = ROOT / "target/debug/tinixcut.exe"
if not BIN.exists():
    raise SystemExit(f"Build the CLI first; executable not found: {BIN}")


class Session:
    def __init__(self):
        self.process = subprocess.Popen(
            [str(BIN), "api-session"], stdin=subprocess.PIPE,
            stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True,
            creationflags=getattr(subprocess, "CREATE_NO_WINDOW", 0),
        )
        self.lines = queue.Queue()
        self.sequence = 0

        def read_output():
            for line in self.process.stdout:
                self.lines.put(line)
            self.lines.put(None)

        threading.Thread(target=read_output, daemon=True).start()

    def request(self, command, arguments):
        self.sequence += 1
        request_id = self.sequence
        request = {"id": request_id, "command": command, "arguments": arguments}
        self.process.stdin.write(json.dumps(request) + "\n")
        self.process.stdin.flush()
        line = self.lines.get(timeout=180)
        assert line is not None, f"CLI exited early: {self.process.poll()}"
        response = json.loads(line)
        assert response.get("id") == request_id, response
        assert "result" in response or "error" in response, response
        return response

    def close(self):
        self.process.stdin.close()
        self.process.wait(timeout=30)
        assert self.process.returncode == 0, self.process.stderr.read()


session = Session()
try:
    schemas = json.loads(subprocess.check_output([str(BIN), "commands"], text=True))
    names = {schema["name"] for schema in schemas}
    assert len(schemas) == 36, f"Expected 36 registered operations, found {len(schemas)}"
    assert len(names) == len(schemas), "Operation registry contains duplicate names"

    # Every operation accepts a request and returns a well-formed session response.
    for schema in schemas:
        if schema["name"] == "capabilities":
            continue
        session.request(schema["name"], {})
    capabilities = session.request("capabilities", {})
    assert "result" in capabilities and not capabilities["result"].get("isError", False)

    with tempfile.TemporaryDirectory(prefix="tinixcut-cli-qualification-") as temp:
        project_path = str(Path(temp) / "project.tcproj")
        created = session.request(
            "project_create", {"projectPath": project_path, "name": "CLI qualification"}
        )["result"]
        assert not created.get("isError", False), created
        inspected = session.request("inspect", {"projectPath": project_path})["result"]
        assert not inspected.get("isError", False), inspected
        saved = session.request("save", {"projectPath": project_path})["result"]
        assert not saved.get("isError", False), saved
        assert Path(project_path).is_file()

    unknown = session.request("not_a_command", {})
    assert "error" in unknown, unknown
    cancelled = session.request("cancel", {"id": "absent"})["result"]
    assert cancelled["cancelled"] is False, cancelled

    one_shot = subprocess.run(
        [str(BIN), "call", "capabilities"], capture_output=True,
        text=True, timeout=15,
    )
    assert one_shot.returncode == 0
    assert not json.loads(one_shot.stdout)["result"].get("isError", False)
    print("PASS: 36 CLI schemas; JSONL dispatch, project create/inspect/save, errors, cancellation and one-shot call")
finally:
    session.close()
