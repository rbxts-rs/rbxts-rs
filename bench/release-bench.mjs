#!/usr/bin/env node
// release-bench.mjs: the numbers recorded for a release in bench/RESULTS.md ("Per release"): clean builds and watch
// rebuilds of a few projects, each build of the release binary next to the stock roblox-ts pipeline, interleaved.
// Windows only (bench/measure.cs reads the peak working set and commit from the process handle).
//
//   node bench/release-bench.mjs <config.json> [--rounds N] [--edits N] [--only clean|watch] [--json FILE]
//
// --rounds: clean-build rounds (default 5). --edits: watch edits per file (default 3). --json: also write every
// measurement to FILE.
//
// The config names the projects and the commands; it stays out of the repo when a project is private:
//   { "projects": [ {
//       "name": "the real project",            // as printed in the tables
//       "dir": "E:/scratch/real",               // a scratch copy: builds delete its `clean` folders and watch runs
//                                               // edit two of its files (restored afterwards)
//       "clean": ["out", "include", "flamework.build"],   // removed before each build (default out, include);
//                                               // `*buildinfo` files in dir are always removed
//       "builds": [ { "label": "stock", "exe": "node", "args": ["node_modules/roblox-ts/out/CLI/cli.js", "build"] },
//                   { "label": "rbxts-rs", "exe": "E:/rel/rbxtsc.exe", "args": ["build"] } ],
//       "watch":  [ ...the same shape, with -w in args ] } ] }
// A build's "dir" overrides the project's (two copies of one project, for instance with other compiler-types).
//
// Clean builds: per round, every build of every project once (out folders cleaned first), measured by measure.exe:
// wall, CPU (user + kernel), peak working set, peak commit, and the files in out/. Medians printed.
// Watch: starts the watcher, waits for its first "Found N error(s)" line (the initial build), then edits a leaf (a src
// file no src file imports, of median size among those) and the hub (the src file most src files import) N times each,
// appending an exported value, and times each until the next status line (watch debounce included). File names are
// never printed: a hub is shown by its importer count.
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import cp from "node:child_process";
import crypto from "node:crypto";

const here = path.dirname(new URL(import.meta.url).pathname.replace(/^\/([A-Za-z]:)/, "$1"));
const argv = process.argv.slice(2);
const opt = (name, fallback) => {
	const i = argv.indexOf(name);
	return i >= 0 ? argv.splice(i, 2)[1] : fallback;
};
const rounds = Number(opt("--rounds", 5));
const edits = Number(opt("--edits", 3));
const only = opt("--only", null);
const jsonOut = opt("--json", null);
if (argv.length !== 1 || process.platform !== "win32") {
	console.error("usage (Windows): node bench/release-bench.mjs <config.json> [--rounds N] [--edits N] [--only clean|watch] [--json FILE]");
	process.exit(2);
}
const config = JSON.parse(fs.readFileSync(argv[0], "utf8"));

// measure.exe, compiled once per measure.cs content with the .NET Framework compiler every Windows has
function measureExe() {
	const source = fs.readFileSync(path.join(here, "measure.cs"));
	const hash = crypto.createHash("sha256").update(source).digest("hex").slice(0, 12);
	const exe = path.join(os.tmpdir(), `rbxts-bench-measure-${hash}.exe`);
	if (!fs.existsSync(exe)) {
		const csc = path.join(process.env.WINDIR ?? "C:/Windows", "Microsoft.NET/Framework64/v4.0.30319/csc.exe");
		cp.execFileSync(csc, ["-nologo", "-optimize", `-out:${exe}`, path.join(here, "measure.cs")], { stdio: "inherit" });
	}
	return exe;
}

const median = (xs) => {
	const s = [...xs].sort((a, b) => a - b);
	return s.length % 2 ? s[(s.length - 1) / 2] : (s[s.length / 2 - 1] + s[s.length / 2]) / 2;
};
const range = (xs) => `${Math.min(...xs)}-${Math.max(...xs)}`;
const seconds = (ms) => (ms >= 10000 ? `${(ms / 1000).toFixed(1)} s` : ms >= 1000 ? `${(ms / 1000).toFixed(2)} s` : `${Math.round(ms)} ms`);
const countFiles = (d) => {
	let n = 0;
	try {
		for (const e of fs.readdirSync(d, { withFileTypes: true })) n += e.isDirectory() ? countFiles(path.join(d, e.name)) : 1;
	} catch {}
	return n;
};
function clean(project, dir) {
	for (const f of project.clean ?? ["out", "include"]) fs.rmSync(path.join(dir, f), { recursive: true, force: true, maxRetries: 5 });
	for (const f of fs.readdirSync(dir)) if (f.endsWith("buildinfo")) fs.rmSync(path.join(dir, f));
}
const records = [];

function cleanBuilds() {
	const measure = measureExe();
	const logs = fs.mkdtempSync(path.join(os.tmpdir(), "rbxts-bench-logs-"));
	const rows = new Map();
	for (let r = 1; r <= rounds; r++) {
		for (const project of config.projects) {
			for (const build of project.builds ?? []) {
				const dir = build.dir ?? project.dir;
				clean(project, dir);
				const log = path.join(logs, `${records.length}.log`);
				const res = cp.spawnSync(measure, [dir, log, build.exe, ...build.args], { encoding: "utf8", timeout: 900000 });
				const m = JSON.parse(res.stdout.trim());
				m.outFiles = countFiles(path.join(dir, "out"));
				records.push({ kind: "clean", project: project.name, build: build.label, round: r, ...m });
				const key = `${project.name}\u0000${build.label}`;
				if (!rows.has(key)) rows.set(key, { project: project.name, build: build.label, runs: [] });
				rows.get(key).runs.push(m);
				console.error(`round ${r}: ${project.name}, ${build.label}: exit ${m.exit}, ${m.wallMs} ms`);
				if (m.exit !== 0) console.error(`  failed; its output: ${log}`);
			}
		}
	}
	console.log(`\nClean builds, medians of ${rounds} (wall range in ms):\n`);
	console.log("| project | build | wall | CPU | peak WS | peak commit | out files | exit |");
	console.log("| --- | --- | --- | --- | --- | --- | --- | --- |");
	for (const { project, build, runs } of rows.values()) {
		const walls = runs.map((m) => m.wallMs);
		const outs = [...new Set(runs.map((m) => m.outFiles))].join("/");
		const exits = [...new Set(runs.map((m) => m.exit))].join("/");
		const cpu = seconds(median(runs.map((m) => m.cpuMs)));
		const ws = median(runs.map((m) => m.peakWsMiB)).toFixed(0);
		const commit = median(runs.map((m) => m.peakCommitMiB)).toFixed(0);
		console.log(`| ${project} | ${build} | ${seconds(median(walls))} (${range(walls)}) | ${cpu} | ${ws} MiB | ${commit} MiB | ${outs} | ${exits} |`);
	}
}

// leaf and hub by a regex scan of relative and src-rooted (client/, server/, shared/) imports
function pickFiles(dir) {
	const src = path.join(dir, "src");
	const files = [];
	(function walk(d) {
		for (const e of fs.readdirSync(d, { withFileTypes: true })) {
			const p = path.join(d, e.name);
			if (e.isDirectory()) walk(p);
			else if (/\.tsx?$/.test(e.name) && !e.name.endsWith(".d.ts")) files.push(p);
		}
	})(src);
	const key = (p) => p.replace(/\.tsx?$/, "").replace(/[\\/]index$/, "").toLowerCase();
	const byKey = new Map(files.map((f) => [key(f), f]));
	const importers = new Map(files.map((f) => [f, 0]));
	for (const f of files) {
		const seen = new Set();
		for (const m of fs.readFileSync(f, "utf8").matchAll(/(?:from|import)\s*\(?\s*["']([^"']+)["']/g)) {
			const spec = m[1];
			const target = spec.startsWith(".") ? path.resolve(path.dirname(f), spec) : /^(client|server|shared)\//.test(spec) ? path.join(src, spec) : null;
			const hit = target && byKey.get(key(target));
			if (hit && hit !== f && !seen.has(hit)) {
				seen.add(hit);
				importers.set(hit, importers.get(hit) + 1);
			}
		}
	}
	const ranked = [...importers].sort((a, b) => b[1] - a[1]);
	const leaves = ranked.filter(([, n]) => n === 0).map(([f]) => [f, fs.statSync(f).size]).sort((a, b) => a[1] - b[1]);
	return { files: files.length, hub: ranked[0][0], hubImporters: ranked[0][1], leaf: leaves[Math.floor(leaves.length / 2)][0] };
}

function watchRun(project, build) {
	const dir = build.dir ?? project.dir;
	clean(project, dir);
	const pick = pickFiles(dir);
	const originals = new Map([pick.leaf, pick.hub].map((f) => [f, fs.readFileSync(f, "utf8")]));
	const restore = () => {
		for (const [f, s] of originals) fs.writeFileSync(f, s);
	};
	return new Promise((resolve) => {
		const w = cp.spawn(build.exe, build.args, { cwd: dir });
		let buf = "";
		const waiters = [];
		const feed = (d) => {
			buf += d;
			let i;
			while ((i = buf.indexOf("\n")) >= 0) {
				if (/Found \d+ errors?/.test(buf.slice(0, i))) waiters.shift()?.(buf.slice(0, i));
				buf = buf.slice(i + 1);
			}
		};
		w.stdout.on("data", feed);
		w.stderr.on("data", feed);
		const next = () => new Promise((r) => waiters.push(r));
		const sleep = (ms) => new Promise((r) => setTimeout(r, ms));
		const result = { project: project.name, build: build.label, hubImporters: pick.hubImporters, srcFiles: pick.files, leaf: [], hub: [] };
		const timer = setTimeout(() => {
			restore();
			w.kill();
			result.timedOut = true;
			resolve(result);
		}, 900000);
		(async () => {
			const t0 = Date.now();
			await next();
			result.initial = Date.now() - t0;
			for (const [kind, f] of [["leaf", pick.leaf], ["hub", pick.hub]]) {
				for (let i = 1; i <= edits; i++) {
					await sleep(2000);
					const p = next();
					const s = Date.now();
					fs.writeFileSync(f, originals.get(f) + `\nexport const __releaseBenchProbe${i} = ${i};\n`);
					await p;
					result[kind].push(Date.now() - s);
				}
			}
			restore();
			await sleep(500);
			clearTimeout(timer);
			w.kill();
			resolve(result);
		})();
	});
}

async function watchBuilds() {
	const results = [];
	for (const project of config.projects)
		for (const build of project.watch ?? []) {
			const r = await watchRun(project, build);
			const times = `initial ${r.initial} ms, leaf ${r.leaf.join("/")} ms, hub ${r.hub.join("/")} ms`;
			console.error(`watch: ${project.name}, ${build.label}: ${times}${r.timedOut ? " (TIMED OUT)" : ""}`);
			records.push({ kind: "watch", ...r });
			results.push(r);
		}
	console.log(`\nWatch rebuilds (${edits} edits per file; file write to the status line):\n`);
	console.log("| project | build | initial | leaf edit (median, range ms) | hub edit (median, range ms) |");
	console.log("| --- | --- | --- | --- | --- |");
	for (const r of results)
		console.log(`| ${r.project} (hub: ${r.hubImporters} importers) | ${r.build} | ${seconds(r.initial)} | ${seconds(median(r.leaf))} (${range(r.leaf)}) | ${seconds(median(r.hub))} (${range(r.hub)}) |`);
}

if (only !== "watch") cleanBuilds();
if (only !== "clean") await watchBuilds();
if (jsonOut) fs.writeFileSync(jsonOut, JSON.stringify(records, null, "\t") + "\n");
