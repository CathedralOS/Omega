#!/usr/bin/env python3
"""Exercise recorded Build names through the ordinary HTTPS Git/update route."""
import argparse
import http.server
import json
import os
from pathlib import Path
import shutil
import ssl
import subprocess
import threading
import concurrent.futures
import sys
import urllib.parse


def run(args, *, cwd=None, env=None, output=None):
    result = subprocess.run(args, cwd=cwd, env=env, capture_output=True, text=True, timeout=180)
    if output:
        Path(output).write_text(result.stdout + result.stderr)
    if result.returncode:
        raise RuntimeError(f"{args[0]} exited {result.returncode}: {result.stderr[-1200:]}")
    return result.stdout.strip()


class GitHandler(http.server.BaseHTTPRequestHandler):
    def log_message(self, *args):
        pass

    def do_GET(self):
        self.respond()

    def do_POST(self):
        self.respond()

    def respond(self):
        address = urllib.parse.urlsplit(self.path)
        environment = dict(os.environ, GIT_PROJECT_ROOT=str(self.server.repositories),
                           GIT_HTTP_EXPORT_ALL="1", REQUEST_METHOD=self.command,
                           PATH_INFO=address.path, QUERY_STRING=address.query,
                           CONTENT_TYPE=self.headers.get("Content-Type", ""),
                           CONTENT_LENGTH=self.headers.get("Content-Length", "0"),
                           REMOTE_ADDR="127.0.0.1", SERVER_PROTOCOL="HTTP/1.1")
        if self.headers.get("Git-Protocol"):
            environment["HTTP_GIT_PROTOCOL"] = self.headers["Git-Protocol"]
        body = self.rfile.read(int(environment["CONTENT_LENGTH"]))
        result = subprocess.run(["git", "http-backend"], input=body,
                                capture_output=True, env=environment)
        headers, payload = result.stdout.split(b"\r\n\r\n", 1)
        parsed = [line.decode().split(":", 1) for line in headers.split(b"\r\n")]
        status = next((int(v.strip().split()[0]) for k, v in parsed if k.lower() == "status"), 200)
        self.send_response(status)
        for name, value in parsed:
            if name.lower() != "status":
                self.send_header(name, value.strip())
        self.send_header("Content-Length", str(len(payload)))
        self.end_headers()
        self.wfile.write(payload)


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--cli", type=Path)
    parser.add_argument("--root", type=Path, required=True)
    parser.add_argument("--place", default="local")
    parser.add_argument("--selection", choices=["root", "named"], default="named")
    parser.add_argument("--literal", action="store_true")
    parser.add_argument("--transport-only", action="store_true")
    parser.add_argument("--all", action="store_true")
    parser.add_argument("--jobs", type=int, choices=[1, 2], default=2)
    parser.add_argument("--invalid", action="store_true")
    parser.add_argument("--conditional", action="store_true")
    parser.add_argument("--request-missing", action="store_true")
    args = parser.parse_args()
    repository = Path(__file__).resolve().parents[4]
    if not args.transport_only:
        if args.cli is None:
            parser.error("--cli is required for compiler checks")
        sys.path.insert(0, str(repository / "tools"))
        from compiler_profile import require_optimized
        require_optimized(args.cli)
        args.cli = args.cli.resolve()
    if args.invalid and args.selection != "named":
        parser.error("the wrong-name twin uses named workspace selection")
    if args.request_missing and not args.conditional:
        parser.error("--request-missing requires --conditional")
    if args.all and (args.literal or args.conditional or args.transport_only):
        parser.error("literal, conditional and transport controls run separately")
    if args.all:
        places = ["local", "field", "parameter", "call_result", "match_join", "const_name", "other_state", "other_machine", "target_body"]
        args.root.mkdir(parents=True, exist_ok=False)
        def one(place):
            command = [sys.executable, str(Path(__file__).resolve()), "--cli", str(args.cli), "--root", str(args.root / place), "--place", place, "--selection", args.selection]
            if args.invalid:
                command.append("--invalid")
            with (args.root / (place + ".log")).open("w") as log:
                result = subprocess.run(command, stdout=log, stderr=subprocess.STDOUT, timeout=360)
            return place, result.returncode
        with concurrent.futures.ThreadPoolExecutor(max_workers=args.jobs) as pool:
            outcomes = list(pool.map(one, places))
        (args.root / "outcomes.json").write_text(json.dumps(dict(outcomes), indent=2))
        for place, result in outcomes:
            print(place, "exit", result, flush=True)
        if any(code for _, code in outcomes):
            raise SystemExit(1)
        return
    fact = repository / "samples/cli/facts/recorded_build_role" / args.place
    root = args.root.resolve()
    root.mkdir(parents=True, exist_ok=False)
    remote = root / "remote"
    remote.mkdir()
    member = remote / "library" if args.selection == "named" else remote
    shutil.copytree(fact / ("invalid/library" if args.invalid else "library"), member, dirs_exist_ok=True)
    if args.literal:
        (member / "build.omg").write_text('machine build(builder: &mut Build) { Build::package(builder, "my_library"); }\n')
    if args.conditional:
        enabled = "true" if args.request_missing else "false"
        declaration = 'machine build(builder: &mut Build) { Build::package(builder, "my_library"); conditional_dependency(builder, ' + enabled + '); }\n'
        declaration += '''machine conditional_dependency(builder: &mut Build, enabled: bool) {
    transition enabled {
        true -> register_dependency(builder)
        false -> no_dependency()
    }
}
machine register_dependency(builder: &mut Build) {
    let unreachable_source_registration: DependencyRegistration = Build::dependency(builder, dependency::from_source(Source::Path { location: "unreachable_source" }));
}
machine no_dependency() {}
'''
        (member / "build.omg").write_text(declaration)
    if args.selection == "named":
        (remote / "build.omg").write_text('machine build(builder: &mut Build) { Build::member(builder, "library"); }\n')
    run(["git", "init", "-q", "-b", "witness"], cwd=remote)
    run(["git", "add", "."], cwd=remote)
    commit_args = ["git", "-c", "user.name=OmegaFixture", "-c", "user.email=fixture@example.invalid", "commit", "-q", "-m"]
    run([*commit_args, "original"], cwd=remote)
    original = run(["git", "rev-parse", "HEAD"], cwd=remote)
    repositories = root / "repositories"
    repositories.mkdir()
    bare = repositories / "roles.git"
    run(["git", "clone", "--bare", "-q", str(remote), str(bare)])
    certificate, key = root / "certificate.pem", root / "key.pem"
    run(["openssl", "req", "-x509", "-newkey", "rsa:2048", "-nodes", "-days", "1",
         "-subj", "/CN=localhost", "-addext", "subjectAltName=IP:127.0.0.1,DNS:localhost",
         "-keyout", str(key), "-out", str(certificate)])
    server = http.server.ThreadingHTTPServer(("127.0.0.1", 0), GitHandler)
    server.repositories = repositories
    context = ssl.SSLContext(ssl.PROTOCOL_TLS_SERVER)
    context.load_cert_chain(certificate, key)
    server.socket = context.wrap_socket(server.socket, server_side=True)
    threading.Thread(target=server.serve_forever, daemon=True).start()
    url = f"https://127.0.0.1:{server.server_port}/roles.git"
    environment = dict(os.environ, GIT_SSL_CAINFO=str(certificate))
    for name in ["XDG_CACHE_HOME", "XDG_DATA_HOME", "XDG_CONFIG_HOME"]:
        path = root / name.lower()
        path.mkdir(mode=0o700)
        environment[name] = str(path)
    record = dict(place=args.place, selection=args.selection, literal=args.literal,
                  original_commit=original, url=url, invalid=args.invalid,
                  conditional=args.conditional, request_missing=args.request_missing)
    try:
        run(["git", "ls-remote", url, "refs/heads/witness"], env=environment,
            output=root / "transport.log")
        if args.transport_only:
            print("HTTPS Git fixture transport verified", flush=True)
            return
        application = root / "application"
        application.mkdir()
        shutil.copy2(fact / "main.omg", application / "main.omg")
        source = (fact / "build.omg").read_text()
        selection = 'PackageSelection::Named { package: "my_library" }' if args.selection == "named" else "PackageSelection::Root {}"
        source = source.replace('Source::Path { location: "library" }',
                                f'Source::Git {{ repository: "{url}", revision: "witness", selection: {selection} }}')
        (application / "build.omg").write_text(source)
        cli = str(args.cli.resolve())
        command = [cli, "packages", "update", "--project", str(application), "--target", "linux_x86_64"]
        if args.invalid or args.request_missing:
            result = subprocess.run(command, env=environment, capture_output=True, text=True, timeout=180)
            text = result.stdout + result.stderr
            (root / "first-update.log").write_text(text)
            required = "undeclared workspace member" if args.request_missing else "workspace has no declared package"
            if not result.returncode or required not in text or (application / "omega.lock").exists():
                raise RuntimeError("negative control did not reject for its exact missing obligation")
            print("missing declaration/source rejected without publishing a lock", flush=True)
            return
        run(command, env=environment, output=root / "first-update.log")
        print("initial update completed", flush=True)
        run([cli, "run", "--both", str(application / "main.omg")],
            env=environment, output=root / "native-terminal.log")
        native = (root / "native-terminal.log").read_text()
        if "native exit 0" not in native or "interp exit: 0" not in native:
            raise RuntimeError("native/Terminal exit-zero oracle did not hold")
        print("native/Terminal execution completed", flush=True)
        value = next(member.glob("values.omg"))
        value.write_text(value.read_text() + "\n// Changed source at the next immutable Git revision.\n")
        run(["git", "add", "."], cwd=remote)
        run([*commit_args, "advance"], cwd=remote)
        run(["git", "push", "-q", str(bare), "HEAD:refs/heads/witness"], cwd=remote)
        record["candidate_commit"] = run(["git", "rev-parse", "HEAD"], cwd=remote)
        report = run(command, env=environment, output=root / "second-update.log")
        if "Source diff unavailable: my_library" in report or "Source diff: my_library [" not in report or "]: 1 changed entries." not in report:
            raise RuntimeError("historical my_library source was not recovered; see second-update.log")
        print("historical source comparison completed", flush=True)
    finally:
        server.shutdown()
        server.server_close()
        (root / "record.json").write_text(json.dumps(record, indent=2))


if __name__ == "__main__":
    main()
