#!/usr/bin/env node
// test-launcher.mjs: tests of the compiler launcher (templates/compiler/rbxtsc.mjs) against a fake release served
// from a local HTTP server. Node 18 or newer, no npm dependencies; `node tools/test-launcher.mjs` (also run by
// `cargo test -p rbxts_cli --test launcher`). The "binary" is a copy of the running node executable, so a test
// project's rbxtsc runs a script (fake-rbxtsc.mjs) that echoes, exits with a code, or waits to be stopped.
// Building with Rust is not tested here (the end-to-end check in templates/compiler/README.md covers it).

import { spawn, spawnSync } from "node:child_process";
import { createHash } from "node:crypto";
import fs from "node:fs";
import http from "node:http";
import os from "node:os";
import path from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";
import { makeZip, presetProblems } from "./release-binary.mjs";

const REPO = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const SKELETON = path.join(REPO, "templates", "compiler");
const SKELETON_FILES = [".cargo/config.toml", "Cargo.toml", "Cargo.lock", "rust-toolchain.toml", "src/main.rs", "rbxtsc.mjs", "preset.json"];
const launcher = await import(pathToFileURL(path.join(SKELETON, "rbxtsc.mjs")).href);
const WIN = process.platform === "win32";
const TRIPLE = launcher.hostTriple();
const EXE = WIN ? "rbxtsc.exe" : "rbxtsc";

const ROOT = fs.mkdtempSync(path.join(os.tmpdir(), "rbxtsc-launcher-test-"));
const FAKE = path.join(ROOT, "fake-rbxtsc.mjs");
fs.writeFileSync(
	FAKE,
	`const [mode, value] = process.argv.slice(2);
if (mode === "--echo") console.log("args: " + JSON.stringify(process.argv.slice(3)));
if (mode === "--exit") process.exit(Number(value));
if (mode === "--wait") {
	require("fs").writeFileSync(value, String(process.pid));
	console.log("ready");
	setInterval(() => {}, 1000);
}
`.replace('require("fs")', "(await import('node:fs')).default"),
);
const NODE = fs.readFileSync(process.execPath);

let failures = 0;
let passed = 0;
async function test(name, body) {
	try {
		await body();
		passed++;
		console.log(`ok   ${name}`);
	} catch (e) {
		failures++;
		console.log(`FAIL ${name}\n     ${String(e.stack ?? e).split("\n").join("\n     ")}`);
	}
}
function assert(condition, message, detail = "") {
	if (!condition) throw new Error(`${message}${detail ? `\n${detail}` : ""}`);
}

/** A fresh copy of the skeleton, with `preset` as its preset.json (the skeleton's own when omitted). */
function project(name, preset) {
	const dir = path.join(ROOT, name, "compiler");
	for (const file of SKELETON_FILES) {
		fs.mkdirSync(path.dirname(path.join(dir, file)), { recursive: true });
		fs.copyFileSync(path.join(SKELETON, file), path.join(dir, file));
	}
	if (preset) fs.writeFileSync(path.join(dir, "preset.json"), JSON.stringify(preset(launcher.configHash(dir)), null, "\t"));
	return dir;
}

/** Puts the fake binary in place, stamped with `stamp` (the config hash by default). */
function installFake(dir, stamp = launcher.configHash(dir)) {
	const bin = path.join(dir, "bin", TRIPLE);
	fs.mkdirSync(bin, { recursive: true });
	fs.writeFileSync(path.join(bin, EXE), NODE);
	if (!WIN) fs.chmodSync(path.join(bin, EXE), 0o755);
	fs.writeFileSync(path.join(bin, ".stamp"), `${stamp}\n`);
}

/**
 * The environment of a launcher that builds with a fake cargo and rustup (copies of the node executable that a
 * preloaded script turns into them) and uses `tmp` as its temp folder. The fake `cargo build` makes its target folder
 * and a binary there; with FAKE_CARGO_WAIT it writes its process id to that file instead and waits until the file
 * FAKE_CARGO_STOP appears (then exits 1) or it is ended.
 */
function fakeCargoEnv(tmp) {
	const bin = path.join(ROOT, "fake-rust");
	const preload = path.join(ROOT, "fake-rust.cjs");
	if (!fs.existsSync(bin)) {
		fs.mkdirSync(bin);
		for (const name of ["cargo", "rustup"]) {
			const file = path.join(bin, WIN ? `${name}.exe` : name);
			fs.copyFileSync(process.execPath, file);
			if (!WIN) fs.chmodSync(file, 0o755);
		}
		fs.writeFileSync(
			preload,
			`const fs = require("fs"), path = require("path");
const tool = path.basename(process.execPath).toLowerCase().replace(/\\.exe$/, "");
if (tool === "cargo" || tool === "rustup") {
	// node took the first argument for its script: \`cargo build …\` runs ./build
	const command = path.basename(process.argv[1] ?? "");
	if (tool === "cargo" && command === "metadata") fs.writeSync(1, JSON.stringify({ packages: [] }));
	if (tool === "cargo" && command === "build") {
		const target = process.argv[process.argv.indexOf("--target-dir") + 1];
		fs.mkdirSync(path.join(target, "release"), { recursive: true });
		fs.writeFileSync(path.join(target, "release", "partial.rlib"), "x");
		if (process.env.FAKE_CARGO_WAIT) {
			fs.writeFileSync(process.env.FAKE_CARGO_WAIT, String(process.pid));
			const cell = new Int32Array(new SharedArrayBuffer(4));
			while (!(process.env.FAKE_CARGO_STOP && fs.existsSync(process.env.FAKE_CARGO_STOP))) Atomics.wait(cell, 0, 0, 50);
			process.exit(1);
		}
		fs.writeFileSync(path.join(target, "release", ${JSON.stringify(EXE)}), "fake binary");
	}
	process.exit(0);
}
`,
		);
	}
	const pathKey = Object.keys(process.env).find((k) => k.toUpperCase() === "PATH") ?? "PATH";
	return {
		[pathKey]: `${bin}${path.delimiter}${process.env[pathKey]}`,
		NODE_OPTIONS: `--require ${JSON.stringify(preload)}`,
		TEMP: tmp,
		TMP: tmp,
		TMPDIR: tmp,
	};
}

/** Runs the launcher (asynchronously: the fake release is served from this process). */
function runLauncher(dir, args, { env = {}, input = "" } = {}) {
	const child = spawn(process.execPath, [path.join(dir, "rbxtsc.mjs"), ...args], {
		cwd: path.dirname(dir),
		// RBXTSC_TEST_HTTP: the fake release is served over plain http
		env: { ...process.env, RBXTSC_INTERACTIVE: "0", RBXTSC_BUILD: "", RBXTSC_TEST_HTTP: "1", ...env },
	});
	let stdout = "";
	let stderr = "";
	child.stdout.on("data", (d) => (stdout += d));
	child.stderr.on("data", (d) => (stderr += d));
	child.stdin.end(input);
	const timer = setTimeout(() => child.kill(), 60_000);
	return new Promise((resolve) =>
		child.on("close", (code) => {
			clearTimeout(timer);
			resolve({ code, stdout, stderr, all: `${stdout}${stderr}` });
		}),
	);
}

function alive(pid) {
	try {
		process.kill(pid, 0);
		return true;
	} catch (e) {
		return e.code === "EPERM";
	}
}

// the fake release: /dl/<name> redirects to /files/<name>; /loop redirects to itself; /drop/<name> sends the headers
// and a few bytes of the file, then drops the connection
const served = new Map();
let requests = 0;
const server = http.createServer((req, res) => {
	requests++;
	const [, kind, name] = req.url.split("/");
	if (kind === "loop") {
		res.writeHead(302, { location: "/loop" });
		res.end();
	} else if (kind === "drop" && served.has(name)) {
		res.writeHead(200, { "content-type": "application/zip", "content-length": served.get(name).length });
		res.write(served.get(name).subarray(0, 3), () => req.socket.destroy());
	} else if (kind === "dl") {
		res.writeHead(302, { location: `/files/${name}` });
		res.end();
	} else if (kind === "files" && served.has(name)) {
		res.writeHead(200, { "content-type": "application/zip" });
		res.end(served.get(name));
	} else {
		res.writeHead(404);
		res.end();
	}
});
await new Promise((resolve) => server.listen(0, "127.0.0.1", resolve));
const BASE = `http://127.0.0.1:${server.address().port}`;
const releaseZip = makeZip([
	{ name: EXE, data: NODE, mode: 0o755, store: true },
	{ name: "LICENSE", data: Buffer.from("MIT\n"), mode: 0o644 },
	{ name: "README.md", data: Buffer.from("# rbxtsc\n"), mode: 0o644 },
]);
served.set("release.zip", releaseZip);
const releaseSha = createHash("sha256").update(releaseZip).digest("hex");
const withRelease = (sha256 = releaseSha) => (hash) => ({
	bin: "rbxtsc",
	version: "0.0.0-test",
	config: hash,
	binaries: { [TRIPLE]: { url: `${BASE}/dl/release.zip`, sha256 } },
});

try {
	await test("config hash: CRLF and LF checkouts hash the same, any content change does not", async () => {
		const lf = project("hash-lf");
		const crlf = project("hash-crlf");
		for (const file of [".cargo/config.toml", "Cargo.toml", "Cargo.lock", "rust-toolchain.toml", "src/main.rs"]) {
			const p = path.join(crlf, file);
			fs.writeFileSync(p, fs.readFileSync(p, "utf8").replace(/\r?\n/g, "\r\n"));
		}
		assert(fs.readFileSync(path.join(crlf, "Cargo.toml"), "utf8").includes("\r\n"), "the CRLF copy has no CRLF");
		const hash = launcher.configHash(lf);
		assert(hash === launcher.configHash(crlf), "CRLF changed the hash");
		assert(hash === launcher.configHash(SKELETON), "the copy hashes differently from the skeleton");
		fs.appendFileSync(path.join(crlf, "src", "main.rs"), "// a change\n");
		assert(launcher.configHash(crlf) !== hash, "a change in src/ did not change the hash");
		fs.appendFileSync(path.join(lf, ".cargo", "config.toml"), "# a change\n");
		assert(launcher.configHash(lf) !== hash, "a change in .cargo/config.toml did not change the hash");
		fs.copyFileSync(path.join(SKELETON, ".cargo", "config.toml"), path.join(lf, ".cargo", "config.toml"));
		// files outside the config do not count
		fs.writeFileSync(path.join(lf, "README.md"), "notes\n");
		fs.writeFileSync(path.join(lf, "preset.json"), "{}");
		assert(launcher.configHash(lf) === hash, "README.md or preset.json changed the hash");
	});

	await test("config hash: a known answer (the format is fixed: path, NUL, length, NUL, bytes; sorted by path)", async () => {
		const dir = path.join(ROOT, "known");
		fs.mkdirSync(path.join(dir, "src", "b"), { recursive: true });
		fs.mkdirSync(path.join(dir, ".cargo"));
		fs.writeFileSync(path.join(dir, ".cargo", "config.toml"), "[build]\n");
		fs.writeFileSync(path.join(dir, ".cargo", "credentials.toml"), "not hashed\n");
		fs.writeFileSync(path.join(dir, "Cargo.toml"), "[package]\r\nname = \"x\"\r\n");
		fs.writeFileSync(path.join(dir, "src", "main.rs"), "fn main() {}\n");
		fs.writeFileSync(path.join(dir, "src", "b", "a.rs"), "\r\n");
		fs.writeFileSync(path.join(dir, "src", "B.rs"), "");
		assert(launcher.configFiles(dir).join() === ".cargo/config.toml,Cargo.toml,src/B.rs,src/b/a.rs,src/main.rs", "file order", launcher.configFiles(dir).join());
		const expected = createHash("sha256")
			.update(".cargo/config.toml\x008\x00[build]\n")
			.update('Cargo.toml\x0021\x00[package]\nname = "x"\n')
			.update("src/B.rs\x000\x00")
			.update("src/b/a.rs\x001\x00\n")
			.update("src/main.rs\x0013\x00fn main() {}\n")
			.digest("hex");
		assert(launcher.configHash(dir) === expected, "hash", launcher.configHash(dir));
	});

	await test("--where: binary path and stamp state", async () => {
		const dir = project("where", withRelease());
		let r = await runLauncher(dir, ["--where"]);
		assert(r.code === 0 && /\(missing\)/.test(r.stdout) && /stamp: no binary/.test(r.stdout), "missing binary", r.all);
		assert(r.stdout.includes(path.join(dir, "bin", TRIPLE, EXE)), "binary path", r.all);
		installFake(dir);
		r = await runLauncher(dir, ["--where"]);
		assert(/\(present\)/.test(r.stdout) && /stamp: matches the config/.test(r.stdout), "stamp hit", r.all);
		assert(/preset: rbxtsc 0\.0\.0-test, matches; prebuilt for/.test(r.stdout), "preset line", r.all);
		installFake(dir, "0123456789abcdef");
		r = await runLauncher(dir, ["--where"]);
		assert(/stamp: stale \(made from config 0123456789ab\)/.test(r.stdout), "stale stamp", r.all);
	});

	await test("stamp hit: runs the binary with the arguments, stdio inherited, no download", async () => {
		const dir = project("hit", withRelease());
		installFake(dir);
		const before = requests;
		const r = await runLauncher(dir, [FAKE, "--echo", "build", "-p", "tsconfig.json", "--a b"]);
		assert(r.code === 0, "exit code", r.all);
		assert(r.stdout === 'args: ["build","-p","tsconfig.json","--a b"]\n', "arguments", r.all);
		assert(r.stderr === "", "launcher output on a stamp hit", r.all);
		assert(requests === before, "downloaded on a stamp hit");
	});

	await test("exit code passthrough", async () => {
		const dir = project("exit", withRelease());
		installFake(dir);
		for (const code of [0, 1, 7, 255]) {
			const r = await runLauncher(dir, [FAKE, "--exit", String(code)]);
			assert(r.code === code, `exit ${code}`, r.all);
		}
	});

	await test("preset download: follows the redirect, checks the sha256, unzips the exe, stamps it, then runs it", async () => {
		const dir = project("download", withRelease());
		const before = requests;
		const r = await runLauncher(dir, [FAKE, "--echo", "x"]);
		assert(r.code === 0, "exit code", r.all);
		const line = `rbxtsc.mjs: downloaded rbxtsc 0.0.0-test (${TRIPLE}) from ${BASE}/dl/release.zip, sha256 ${releaseSha}\n`;
		assert(r.stderr === line, "the one download line", r.all);
		assert(r.stdout === 'args: ["x"]\n', "ran after the download", r.all);
		assert(requests === before + 2, "redirect then file", `${requests - before} requests`);
		assert(Buffer.compare(fs.readFileSync(path.join(dir, "bin", TRIPLE, EXE)), NODE) === 0, "the unzipped exe");
		assert(fs.readFileSync(path.join(dir, "bin", TRIPLE, ".stamp"), "utf8") === `${launcher.configHash(dir)}\n`, "stamp");
		const again = await runLauncher(dir, [FAKE, "--exit", "3"]);
		assert(again.code === 3 && again.stderr === "" && requests === before + 2, "second run is a stamp hit", again.all);
		console.log(`     sample: ${line.trim()}`);
	});

	await test("an http: preset URL is refused outside the tests (and anything but https:)", async () => {
		for (const url of [`${BASE}/dl/release.zip`, "ftp://example.com/release.zip", "not a url"]) {
			const dir = project("http", (hash) => ({ ...withRelease()(hash), binaries: { [TRIPLE]: { url, sha256: releaseSha } } }));
			const before = requests;
			const r = await runLauncher(dir, [FAKE, "--echo"], { env: { RBXTSC_TEST_HTTP: "" } });
			assert(r.code === 1, `${url}: exit code`, r.all);
			assert(r.stderr.startsWith(`rbxtsc.mjs: not downloading ${url}: preset.json's binaries must be https: URLs.
`), `${url}: message`, r.all);
			assert(requests === before && !fs.existsSync(path.join(dir, "bin")), `${url}: downloaded or wrote something`, r.all);
			fs.rmSync(path.dirname(dir), { recursive: true });
		}
	});

	await test("sha256 mismatch: aborts, installs nothing", async () => {
		const dir = project("badsha", withRelease("0".repeat(64)));
		const r = await runLauncher(dir, [FAKE, "--echo"]);
		assert(r.code === 1, "exit code", r.all);
		assert(r.stderr.includes(`sha256 mismatch for ${BASE}/dl/release.zip: expected ${"0".repeat(64)}, got ${releaseSha}. Nothing was installed.`), "message", r.all);
		assert(!fs.existsSync(path.join(dir, "bin", TRIPLE, EXE)), "the exe was written");
		console.log(`     sample: ${r.stderr.trim().split("\n")[0]}`);
	});

	await test("a failed download (a redirect loop, a connection dropped in the body) exits 1 cleanly with the build hint", async () => {
		for (const [label, route] of [["redirect loop", "loop"], ["dropped body", "drop/release.zip"]]) {
			const dir = project(`fail-${route.split("/")[0]}`, (hash) => ({ ...withRelease()(hash), binaries: { [TRIPLE]: { url: `${BASE}/${route}`, sha256: releaseSha } } }));
			const start = Date.now();
			const r = await runLauncher(dir, [FAKE, "--echo"]);
			const seconds = (Date.now() - start) / 1000;
			// not Node 23's libuv assertion on Windows (0xC0000409 = 3221226505) after process.exit()
			assert(r.code === 1, `${label}: exit code ${r.code}`, r.all);
			assert(r.stderr.startsWith(`rbxtsc.mjs: could not download ${BASE}/${route}: `), `${label}: message`, r.all);
			assert(r.stderr.includes("(or build it with Rust: node compiler/rbxtsc.mjs --build)"), `${label}: build hint`, r.all);
			assert(!fs.existsSync(path.join(dir, "bin")), `${label}: something was written`, r.all);
			assert(seconds < 10, `${label}: took ${seconds} s to end`, r.all);
			console.log(`     ${label} (${seconds.toFixed(1)} s): ${r.stderr.trim().split("\n")[0]}`);
		}
	});

	await test("config changed, not a terminal (CI): explains, prints the build command, exits 1", async () => {
		const dir = project("changed-ci", withRelease());
		fs.appendFileSync(path.join(dir, "Cargo.toml"), "\n# a change\n");
		installFake(dir, "stale");
		const r = await runLauncher(dir, [FAKE, "--echo"]);
		assert(r.code === 1, "exit code", r.all);
		assert(/changed: it hashes to [0-9a-f]{12}, no longer/.test(r.stderr), "says the config changed", r.all);
		assert(r.stderr.includes("run: node compiler/rbxtsc.mjs --build"), "the exact command", r.all);
		assert(r.stdout === "", "ran the binary", r.all);
		console.log(`     sample:\n       ${r.stderr.trim().split("\n").join("\n       ")}`);
	});

	await test("config changed, terminal: asks, and a no exits 1 without building", async () => {
		const dir = project("changed-tty", withRelease());
		fs.writeFileSync(path.join(dir, "src", "main.rs"), "fn main() {}\n");
		const r = await runLauncher(dir, [FAKE], { env: { RBXTSC_INTERACTIVE: "1" }, input: "n\n" });
		assert(r.code === 1, "exit code", r.all);
		assert(r.stderr.includes("Build it with Rust now? This takes about "), "the prompt", r.all);
		assert(/ min\. \[y\/N\] /.test(r.stderr), "the prompt's [y/N]", r.all);
		assert(r.stderr.includes("not building. To build it later: node compiler/rbxtsc.mjs --build"), "the no", r.all);
		console.log(`     sample:\n       ${r.stderr.trim().split("\n").join("\n       ")}`);
	});

	await test("no prebuilt binary for this platform: says so", async () => {
		const dir = project("no-triple", (hash) => ({ bin: "rbxtsc", version: "0.0.0-test", config: hash, binaries: { "other-triple": { url: "x", sha256: "y" } } }));
		const r = await runLauncher(dir, []);
		assert(r.code === 1 && r.stderr.includes(`There is no prebuilt rbxtsc 0.0.0-test for ${TRIPLE} (preset.json has: other-triple).`), "message", r.all);
	});

	await test("build asked without Rust: points at rustup, exits 1", async () => {
		const dir = project("norust", withRelease());
		fs.appendFileSync(path.join(dir, "Cargo.toml"), "\n# a change\n");
		const empty = path.join(ROOT, "empty-path");
		fs.mkdirSync(empty, { recursive: true });
		const env = Object.fromEntries(Object.entries(process.env).filter(([k]) => k.toUpperCase() !== "PATH"));
		const r = spawnSync(process.execPath, [path.join(dir, "rbxtsc.mjs"), "--build"], { cwd: path.dirname(dir), env: { ...env, PATH: empty }, encoding: "utf8", timeout: 60_000 });
		assert(r.status === 1, "exit code", r.stderr);
		assert(r.stderr.includes("https://rustup.rs"), "rustup link", r.stderr);
	});

	await test("duplicate rbxts-rs copies are found in cargo metadata", async () => {
		const meta = {
			packages: [
				{ name: "rbxts_cli", source: "git+https://github.com/rbxts-rs/rbxts-rs?tag=v0.1.0#0fa1528d" },
				{ name: "rbxts_plugin", source: "git+https://github.com/rbxts-rs/rbxts-rs?tag=v0.1.0#0fa1528d" },
				{ name: "rbxts_plugin", source: "git+https://github.com/rbxts-rs/rbxts-rs?rev=1234567#1234567" },
				{ name: "serde", source: "registry+https://github.com/rust-lang/crates.io-index" },
			],
		};
		const sources = launcher.rbxtsSources(meta);
		assert(new Set(sources.get("rbxts_plugin")).size === 2 && new Set(sources.get("rbxts_cli")).size === 1, "sources", JSON.stringify([...sources]));
	});

	await test("zip reader: deflated entries, an exe at any depth, exactly one exe", async () => {
		const exe = Buffer.from("not really an exe ".repeat(1000));
		const zip = makeZip([
			{ name: "LICENSE", data: Buffer.from("MIT\n") },
			{ name: `dir/${EXE}`, data: exe, mode: 0o755 },
		]);
		assert(Buffer.compare(launcher.extractExe(zip, EXE), exe) === 0, "deflated exe");
		const two = makeZip([
			{ name: EXE, data: exe },
			{ name: `x/${EXE}`, data: exe },
		]);
		let error = "";
		try {
			launcher.extractExe(two, EXE);
		} catch (e) {
			error = e.message;
		}
		assert(error === `the archive has 2 files named ${EXE}, not one`, "two exes", error);
	});

	await test("--build: deletes the target folders of builds that are gone, keeps a running build's", async () => {
		const dir = project("stale", withRelease());
		fs.appendFileSync(path.join(dir, "Cargo.toml"), "\n# a change\n");
		const tmp = path.join(ROOT, "tmp-stale");
		fs.mkdirSync(tmp, { recursive: true });
		const dead = spawnSync(process.execPath, ["-e", ""]).pid;
		const folders = {
			dead: `rbxtsc-build-0123456789ab-${dead}`,
			legacy: "rbxtsc-build-0123456789ab",
			running: `rbxtsc-build-0123456789ab-${process.pid}`,
			other: "rbxtsc-build-notes",
		};
		for (const name of Object.values(folders)) fs.mkdirSync(path.join(tmp, name, "release"), { recursive: true });
		const r = await runLauncher(dir, ["--build"], { env: fakeCargoEnv(tmp) });
		assert(r.code === 0, "exit code", r.all);
		assert(r.stderr.includes(`deleted 2 target folder(s) interrupted builds left in ${tmp}`), "the note", r.all);
		const left = fs.readdirSync(tmp).sort();
		assert(left.join() === [folders.running, folders.other].sort().join(), "the folders left", left.join());
		assert(/--target-dir \S*rbxtsc-build-[0-9a-f]{12}-\d+/.test(r.stderr), "the build's own folder names its launcher", r.all);
		assert(fs.readFileSync(path.join(dir, "bin", TRIPLE, ".stamp"), "utf8") === `${launcher.configHash(dir)}\n`, "built and stamped", r.all);
	});

	await test("Ctrl+C during --build: waits for cargo, deletes the target folder, ends as interrupted", async () => {
		for (const cargoEnds of [true, false]) {
			const dir = project(`interrupt-${cargoEnds}`, withRelease());
			fs.appendFileSync(path.join(dir, "Cargo.toml"), "\n# a change\n");
			const tmp = path.join(ROOT, `tmp-interrupt-${cargoEnds}`);
			fs.mkdirSync(tmp, { recursive: true });
			const pidFile = path.join(ROOT, `cargo-${cargoEnds}.pid`);
			const stopFile = path.join(ROOT, `cargo-${cargoEnds}.stop`);
			// the launcher runs in a driver that raises SIGINT in itself, as a console Ctrl+C does (Windows has no
			// signals to send to another process). The fake cargo ends when the stop file appears (it got the Ctrl+C
			// too) or, without one, has to be ended by the launcher.
			const driver = path.join(ROOT, `driver-build-${cargoEnds}.mjs`);
			fs.writeFileSync(
				driver,
				`import { main } from ${JSON.stringify(pathToFileURL(path.join(dir, "rbxtsc.mjs")).href)};
process.stdin.once("data", () => process.emit("SIGINT", "SIGINT"));
main(["--build"]).then((r) => { console.log("result " + JSON.stringify(r)); process.exit(0); });
`,
			);
			const env = { ...process.env, RBXTSC_INTERACTIVE: "0", ...fakeCargoEnv(tmp), FAKE_CARGO_WAIT: pidFile, FAKE_CARGO_STOP: stopFile };
			const child = spawn(process.execPath, [driver], { cwd: path.dirname(dir), env, stdio: ["pipe", "pipe", "pipe"] });
			let out = "";
			child.stdout.on("data", (d) => (out += d));
			child.stderr.on("data", (d) => (out += d));
			const exited = new Promise((resolve) => child.on("exit", resolve));
			const deadline = Date.now() + 20_000;
			while (!fs.existsSync(pidFile) && Date.now() < deadline) await new Promise((r) => setTimeout(r, 50));
			assert(fs.existsSync(pidFile), "cargo did not start", out);
			const cargoPid = Number(fs.readFileSync(pidFile, "utf8"));
			const building = fs.readdirSync(tmp).filter((n) => n.startsWith("rbxtsc-build-"));
			assert(building.length === 1, "the build's target folder", building.join());
			child.stdin.write("go\n");
			if (cargoEnds) fs.writeFileSync(stopFile, "");
			const result = await Promise.race([exited, new Promise((r) => setTimeout(() => r("timeout"), 20_000))]);
			if (result === "timeout") child.kill();
			assert(result !== "timeout", "the launcher did not end", out);
			assert(out.includes('result {"signal":"SIGINT"}'), "the launcher's result", out);
			assert(out.includes("the build was stopped (SIGINT); deleting its target folder."), "the note", out);
			assert(!alive(cargoPid), "cargo was left running", out);
			assert(fs.readdirSync(tmp).length === 0, "the target folder was left", fs.readdirSync(tmp).join());
			assert(!fs.existsSync(path.join(dir, "bin", TRIPLE, EXE)), "a binary was installed");
		}
	});

	await test("release tool: the preset must name the skeleton's hash and the pinned tag", async () => {
		const hash = launcher.configHash(SKELETON);
		const preset = JSON.parse(fs.readFileSync(path.join(SKELETON, "preset.json"), "utf8"));
		const tag = /^rbxts_cli\s*=\s*\{[^}]*\btag\s*=\s*"v([^"]+)"/m.exec(fs.readFileSync(path.join(SKELETON, "Cargo.toml"), "utf8"))?.[1];
		assert(presetProblems(preset, hash, tag).length === 0, "the committed skeleton's preset", presetProblems(preset, hash, tag).join("\n"));
		const problems = presetProblems({ ...preset, config: "0".repeat(64), version: "9.9.9" }, hash, tag);
		assert(problems.length === 2, "both problems", problems.join("\n"));
		assert(problems[0] === `"config" is "${"0".repeat(64)}": set it to "${hash}" (\`node templates/compiler/rbxtsc.mjs --where\` prints it)`, "config", problems[0]);
		assert(problems[1] === `"version" is "9.9.9": set it to "${tag}" (the tag rbxts_cli pins in Cargo.toml)`, "version", problems[1]);
	});

	await test("signal forwarding: the launcher's SIGINT/SIGTERM stops the binary and the launcher waits for it", async () => {
		const dir = project("signal", withRelease());
		installFake(dir);
		const pidFile = path.join(ROOT, "signal.pid");
		// Windows has no signals to send to another process (Node's kill() there terminates it outright), so the
		// launcher runs in a driver process that raises SIGINT in itself, as a console Ctrl+C would. Elsewhere the
		// launcher gets a real SIGTERM.
		const driver = path.join(ROOT, "driver.mjs");
		fs.writeFileSync(
			driver,
			`import { main } from ${JSON.stringify(pathToFileURL(path.join(dir, "rbxtsc.mjs")).href)};
process.stdin.once("data", () => process.emit("SIGINT", "SIGINT"));
main(process.argv.slice(2)).then((r) => { console.log("result " + JSON.stringify(r)); process.exit(typeof r === "number" ? r : 99); });
`,
		);
		const args = WIN ? [driver, FAKE, "--wait", pidFile] : [path.join(dir, "rbxtsc.mjs"), FAKE, "--wait", pidFile];
		const child = spawn(process.execPath, args, { cwd: path.dirname(dir), stdio: ["pipe", "pipe", "pipe"] });
		let out = "";
		child.stdout.on("data", (d) => (out += d));
		child.stderr.on("data", (d) => (out += d));
		const exited = new Promise((resolve) => child.on("exit", (code, signal) => resolve({ code, signal })));
		const deadline = Date.now() + 20_000;
		while (!out.includes("ready") && Date.now() < deadline) await new Promise((r) => setTimeout(r, 50));
		assert(out.includes("ready"), "the binary did not start", out);
		const pid = Number(fs.readFileSync(pidFile, "utf8"));
		assert(alive(pid), "binary not running");
		if (WIN) child.stdin.write("go\n");
		else child.kill("SIGTERM");
		const timeout = new Promise((resolve) => setTimeout(() => resolve("timeout"), 15_000));
		const result = await Promise.race([exited, timeout]);
		if (result === "timeout") child.kill("SIGKILL");
		assert(result !== "timeout", "the launcher did not exit", out);
		await new Promise((r) => setTimeout(r, 200));
		assert(!alive(pid), "the binary was left running (orphan)", out);
		if (WIN) assert(/result \{"signal":"SIGTERM"\}|result \{"signal":"SIGINT"\}|result 1\b/.test(out), "launcher result", `${out} ${JSON.stringify(result)}`);
		else assert(result.signal === "SIGTERM" || result.code === 143, "launcher ended by the signal", JSON.stringify(result));
		console.log(`     ${WIN ? "driver" : "launcher"} result: ${WIN ? out.trim().split("\n").pop() : JSON.stringify(result)}`);
	});
} finally {
	server.close();
	fs.rmSync(ROOT, { recursive: true, force: true });
}

console.log(`\n${passed} passed, ${failures} failed`);
process.exit(failures === 0 ? 0 : 1);
